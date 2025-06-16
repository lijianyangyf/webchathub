// src/hub.rs – borrow‑checker & BytesMut::Write fixes
// ---------------------------------------------------
// * Remove `self` from broadcast helper to avoid double mutable borrow.
// * Use `serde_json::to_vec` then `extend_from_slice`, so no need for Write.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Interval};

use crate::config::Config;
use crate::memory_pool::MemoryPool;
use crate::protocol::ServerEvent;

fn is_chat(ev: &ServerEvent) -> bool {
    matches!(ev, ServerEvent::NewMessage { .. })
}

pub enum HubCommand {
    JoinRoom {
        room: String,
        name: String,
        resp: mpsc::Sender<broadcast::Receiver<Bytes>>, // receiver of pooled frames
    },
    SendMsg {
        room: String,
        event: ServerEvent,
    },
    LeaveRoom {
        room: String,
        name: String,
    },
    GetRoomList {
        resp: mpsc::Sender<Vec<String>>, // list current rooms
    },
    GetMembers {
        room: String,
        resp: mpsc::Sender<Vec<String>>,
    },
    GetHistory {
        room: String,
        resp: mpsc::Sender<Vec<Bytes>>, // history frames
    },
}

struct Room {
    tx: broadcast::Sender<Bytes>,
    members: HashSet<String>,
    history: VecDeque<Bytes>,
    last_empty_at: Option<Instant>,
}

impl Room {
    fn new(cap: usize) -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            tx,
            members: HashSet::new(),
            history: VecDeque::with_capacity(cap),
            last_empty_at: None,
        }
    }
}

pub struct ChatHub {
    rooms: HashMap<String, Room>,
    cmd_rx: mpsc::Receiver<HubCommand>,
    history_limit: usize,
    room_ttl: Duration,
}

impl ChatHub {
    pub fn new(cmd_rx: mpsc::Receiver<HubCommand>) -> Self {
        let cfg = Config::from_env();
        Self {
            rooms: HashMap::new(),
            cmd_rx,
            history_limit: cfg.history_limit,
            room_ttl: Duration::from_secs(cfg.room_ttl_secs),
        }
    }

    pub async fn run(&mut self) {
        let mut sweep: Interval = interval(self.room_ttl.max(Duration::from_secs(1)));
        loop {
            tokio::select! {
                biased;
                Some(cmd) = self.cmd_rx.recv() => { self.handle_cmd(cmd).await; }
                _ = sweep.tick() => { self.gc_empty_rooms(); }
            }
        }
    }

    async fn handle_cmd(&mut self, cmd: HubCommand) {
        match cmd {
            HubCommand::JoinRoom { room, name, resp } => {
                let room_ref = self.rooms.entry(room.clone()).or_insert_with(|| Room::new(self.history_limit));
                room_ref.members.insert(name.clone());
                room_ref.last_empty_at = None;
                let _ = resp.send(room_ref.tx.subscribe()).await;
                let evt = ServerEvent::UserJoined { room, name };
                broadcast_event(room_ref, evt, self.history_limit);
            }
            HubCommand::SendMsg { room, event } => {
                if let Some(room_ref) = self.rooms.get_mut(&room) {
                    broadcast_event(room_ref, event, self.history_limit);
                }
            }
            HubCommand::LeaveRoom { room, name } => {
                if let Some(room_ref) = self.rooms.get_mut(&room) {
                    let evt = ServerEvent::UserLeft { room: room.clone(), name: name.clone() };
                    broadcast_event(room_ref, evt, self.history_limit);
                    room_ref.members.remove(&name);
                    if room_ref.members.is_empty() {
                        room_ref.last_empty_at = Some(Instant::now());
                    }
                }
            }
            HubCommand::GetRoomList { resp } => {
                let list: Vec<String> = self.rooms.keys().cloned().collect();
                let _ = resp.send(list).await;
            }
            HubCommand::GetMembers { room, resp } => {
                let members = self.rooms
                    .get(&room)
                    .map(|r| r.members.iter().cloned().collect())
                    .unwrap_or_default();
                let _ = resp.send(members).await;
            }
            HubCommand::GetHistory { room, resp } => {
                let msgs = self.rooms
                    .get(&room)
                    .map(|r| r.history.iter().cloned().collect())
                    .unwrap_or_default();
                let _ = resp.send(msgs).await;
            }
        }
    }

    fn gc_empty_rooms(&mut self) {
        let now = Instant::now();
        let ttl = self.room_ttl;
        self.rooms.retain(|name, room| {
            if room.members.is_empty() {
                if let Some(t) = room.last_empty_at {
                    if now.duration_since(t) > ttl {
                        tracing::info!(room = %name, "room expired after TTL");
                        return false;
                    }
                }
            }
            true
        });
    }
}

/// Serialize `ServerEvent` once, broadcast as `Bytes`, and record to history.
fn broadcast_event(room: &mut Room, event: ServerEvent, limit: usize) {
    // serialize to vec first
    let vec = serde_json::to_vec(&event).expect("serialize");
    let mut buf = MemoryPool::global().alloc(vec.len());
    buf.extend_from_slice(&vec);
    let frame: Bytes = buf.freeze();

    let _ = room.tx.send(frame.clone());

    if is_chat(&event) {
        room.history.push_back(frame);
        if room.history.len() > limit {
            room.history.pop_front();
        }
    }
}
