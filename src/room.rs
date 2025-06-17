use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{interval, Interval};

use crate::config::Config;
use crate::memory_pool::{MemoryPool};
use crate::protocol::{ServerEvent};

/// Commands sent from Hub → room task
pub enum RoomCmd {
    Join {
        name: String,
        resp: oneshot::Sender<broadcast::Receiver<Bytes>>, // receiver for this client
    },
    Send(ServerEvent),          // broadcast chat/system event
    Leave { name: String },
    GetMembers {
        resp: oneshot::Sender<Vec<String>>,               // current members
    },
    GetHistory {
        resp: oneshot::Sender<Vec<Bytes>>,                // copy of history frames
    },
    Shutdown, // Hub dropped
}

/// Spawn a new room task; returns its sender + JoinHandle
pub fn spawn_room_task(cfg: &Config, room: String) -> (mpsc::Sender<RoomCmd>, JoinHandle<()>) {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<RoomCmd>(32);

    // broadcast capacity comes from env or fixed 1024
    let (tx, _) = broadcast::channel::<Bytes>(cfg.history_limit.max(1024));

    let history_cap = cfg.history_limit;
    let ttl = Duration::from_secs(cfg.room_ttl_secs);

    let handle = tokio::spawn(async move {
        let mut members: HashSet<String> = HashSet::new();
        let mut history: VecDeque<Bytes> = VecDeque::with_capacity(history_cap);
        let mut last_empty_at: Option<Instant> = None;
        let mut sweep: Interval = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => match cmd {
                    RoomCmd::Join { name, resp } => {
                        members.insert(name.clone());
                        last_empty_at = None;
                        // send UserJoined event
                        let evt = ServerEvent::UserJoined { room: room.clone(), name };
                        broadcast_event(&tx, &mut history, history_cap, evt);
                        let _ = resp.send(tx.subscribe());
                    }
                    RoomCmd::Send(ev) => {
                        broadcast_event(&tx, &mut history, history_cap, ev);
                    }
                    RoomCmd::Leave { name } => {
                        members.remove(&name);
                        let evt = ServerEvent::UserLeft { room: room.clone(), name };
                        broadcast_event(&tx, &mut history, history_cap, evt);
                        if members.is_empty() {
                            last_empty_at = Some(Instant::now());
                        }
                    }
                    RoomCmd::GetMembers { resp } => {
                        let _ = resp.send(members.iter().cloned().collect());
                    }
                    RoomCmd::GetHistory { resp } => {
                        let _ = resp.send(history.iter().cloned().collect());
                    }
                    RoomCmd::Shutdown => {
                        break; // graceful exit
                    }
                },
                _ = sweep.tick() => {
                    if members.is_empty() {
                        if let Some(t0) = last_empty_at {
                            if t0.elapsed() > ttl {
                                tracing::info!(room=%room, "room expired after TTL");
                                break; // exit task; Hub cleans up map on Join error
                            }
                        }
                    }
                }
            }
        }
    });

    (cmd_tx, handle)
}

/// helper – encode event → Bytes and fan‑out, push history if chat message
fn broadcast_event(
    tx: &broadcast::Sender<Bytes>,
    history: &mut VecDeque<Bytes>,
    cap: usize,
    event: ServerEvent,
) {
    // Only keep chat messages in history (UserJoined/UserLeft skipped)
    let is_chat = matches!(event, ServerEvent::NewMessage { .. });

    let json = serde_json::to_vec(&event).expect("serialize");
    let mut buf = MemoryPool::global().alloc(json.len());
    buf.extend_from_slice(&json);
    let frame = buf.freeze();

    let _ = tx.send(frame.clone());

    if is_chat {
        history.push_back(frame);
        if history.len() > cap {
            history.pop_front();
        }
    }
}
