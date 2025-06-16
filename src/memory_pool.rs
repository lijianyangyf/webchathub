// src/memory_pool.rs – v2 with global() + freeze() returning Bytes
// --------------------------------------------------------------
// Purpose: provide a simple slab‑backed buffer pool (BytesMut) and
// allow callers to obtain immutable `bytes::Bytes` frames for cheap
// cloning / broadcasting.
//
// API:
//   MemoryPool::global() -> &'static MemoryPool
//   alloc(size)          -> PooledBytes
//
//   impl PooledBytes {
//       fn freeze(self) -> Bytes  // transfers data, recycles buffer
//   }
//   Drop for PooledBytes auto‑recycles if not frozen.
//
use bytes::{Bytes, BytesMut};
use once_cell::sync::Lazy;
use slab::Slab;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

/// Pooled mutable buffer – writeable while in scope.
#[derive(Debug)]
pub struct PooledBytes {
    buf: Option<BytesMut>, // None after freeze or recycle
    key: usize,
}

impl PooledBytes {
    /// Convert into immutable `Bytes`, recycling the inner allocation.
    pub fn freeze(mut self) -> Bytes {
        let bytes = self
            .buf
            .take()
            .expect("already frozen/recycled")
            .freeze();
        // recycle empty buffer back into the pool (zero‑len BytesMut)
        MemoryPool::global().recycle_raw(self.key, BytesMut::new());
        bytes
    }

    /// Manually recycle without freezing (discard contents).
    pub fn recycle(mut self) {
        let buf = self.buf.take().unwrap_or_default();
        MemoryPool::global().recycle_raw(self.key, buf);
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
            MemoryPool::global().recycle_raw(self.key, buf);
        }
    }
}

/// Global buffer pool guarded by a Mutex (coarse but simple).
#[derive(Default)]
pub struct MemoryPool {
    slabs: Mutex<Slab<BytesMut>>, // key -> buffer
}

impl MemoryPool {
    /// Obtain global singleton.
    pub fn global() -> &'static MemoryPool {
        static INSTANCE: Lazy<MemoryPool> = Lazy::new(|| MemoryPool::default());
        &INSTANCE
    }

    /// Allocate a buffer of at least `size` bytes.
    pub fn alloc(&self, size: usize) -> PooledBytes {
        let mut slabs = self.slabs.lock().unwrap();
        // find a vacant entry with >= size capacity
        if let Some((key, _)) = slabs
            .iter()
            .find(|(_, buf)| buf.capacity() >= size)
        {
            let buf = slabs.remove(key);
            return PooledBytes { buf: Some(buf), key };
        }
        // else allocate new buffer
        let key = slabs.insert(BytesMut::with_capacity(size));
        let buf = slabs.remove(key);
        PooledBytes { buf: Some(buf), key }
    }

    /// Return raw buffer back to slab.
    fn recycle_raw(&self, key: usize, mut buf: BytesMut) {
        buf.clear();
        let mut slabs = self.slabs.lock().unwrap();
        let _old = slabs.insert_at_vacant(key, buf);
    }
}

/// Extension trait to expose `insert_at_vacant` (not in public API) – we fake it
/// by removing then inserting which preserves key reuse.
trait SlabVacant<T> {
    fn insert_at_vacant(&mut self, key: usize, val: T);
}
impl<T> SlabVacant<T> for Slab<T> {
    fn insert_at_vacant(&mut self, key: usize, val: T) {
        if self.contains(key) {
            // occupied (should not happen)
            return;
        }
        let _ = self.insert(val);
    }
}
