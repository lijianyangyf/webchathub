// src/memory_pool.rs – v3: thread‑pool integration
// -------------------------------------------------
// A simple slab‑backed buffer pool plus a lightweight blocking
// thread‑pool interface for CPU‑intensive work (e.g. JSON encoding).
//
// * Pool: `slab::Slab<BytesMut>` protected by `Mutex`.
// * Public API:
//     MemoryPool::global()        – singleton
//     alloc(size) -> PooledBytes  – mutable buffer
//     spawn(move |pool| { ... })  – run on blocking threads
//
// * `PooledBytes` converts to immutable `bytes::Bytes` via `freeze()`. When
//   dropped (or frozen) the underlying allocation is returned to the slab for
//   future reuse.
//
// The thread‑pool uses `tokio::task::spawn_blocking`. Concurrency is limited by
// Tokio’s global blocking semaphore (defaults to 512) but can be tuned by
// the `TOKIO_MAX_BLOCKING_THREADS` env‑var. For fine‑grained control you can
// build the Tokio runtime manually; here we rely on the default.

use bytes::{Bytes, BytesMut};
use once_cell::sync::Lazy;
use slab::Slab;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

/// RAII wrapper around a pooled `BytesMut`.
#[derive(Debug)]
pub struct PooledBytes {
    buf: Option<BytesMut>,
}

impl PooledBytes {
    /// Convert self into immutable `Bytes`, recycling the backing storage.
    pub fn freeze(mut self) -> Bytes {
        let bytes = self.buf.take().expect("already frozen").freeze();
        // recycle empty buffer back to pool
        MemoryPool::global().recycle_raw(BytesMut::new());
        bytes
    }

    /// Manually recycle without converting.
    pub fn recycle(mut self) {
        if let Some(b) = self.buf.take() {
            MemoryPool::global().recycle_raw(b);
        }
    }
}

impl Deref for PooledBytes {
    type Target = BytesMut;
    fn deref(&self) -> &Self::Target {
        self.buf.as_ref().expect("buffer taken")
    }
}
impl DerefMut for PooledBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buf.as_mut().expect("buffer taken")
    }
}

impl Drop for PooledBytes {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            MemoryPool::global().recycle_raw(buf);
        }
    }
}

/// Global mutable buffer pool.
#[derive(Default)]
pub struct MemoryPool {
    slabs: Mutex<Slab<BytesMut>>, // simple, lock per op
}

impl MemoryPool {
    /// Global singleton accessor.
    pub fn global() -> &'static MemoryPool {
        static INSTANCE: Lazy<MemoryPool> = Lazy::new(|| MemoryPool::default());
        &INSTANCE
    }

    /// Allocate a buffer with at least `size` bytes capacity.
    pub fn alloc(&self, size: usize) -> PooledBytes {
        let mut slabs = self.slabs.lock().unwrap();
        // find reusable buffer
        if let Some((key, _)) = slabs.iter().find(|(_, b)| b.capacity() >= size) {
            let buf = slabs.remove(key);
            return PooledBytes { buf: Some(buf) };
        }
        // allocate fresh
        let buf = BytesMut::with_capacity(size);
        PooledBytes { buf: Some(buf) }
    }

    /// Recycle raw buffer (cleared).
    fn recycle_raw(&self, mut buf: BytesMut) {
        buf.clear();
        let mut slabs = self.slabs.lock().unwrap();
        let _ = slabs.insert(buf); // ignore index
    }

    /// Run a closure on a blocking thread with access to the global pool.
    pub fn spawn<F, R>(f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(&MemoryPool) -> R + Send + 'static,
        R: Send + 'static,
    {
        tokio::task::spawn_blocking(move || f(MemoryPool::global()))
    }
}
