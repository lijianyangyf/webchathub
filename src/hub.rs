use std::collections::HashMap;

use bytes::Bytes;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::protocol::ServerEvent;
use crate::room::{spawn_room_task, RoomCmd};

/// Commands accepted by [`ChatHub`].
pub enum HubCmd {
    Join {
        room: String,
        name: String,
        /// oneshot channel to return a broadcast receiver for this client
        resp: oneshot::Sender<broadcast::Receiver<Bytes>>, 
    },
    Send {
        room: String,
        event: ServerEvent,
    },
    Leave {
        room: String,
        name: String,
    },
    GetMembers {
        room: String,
        resp: oneshot::Sender<Vec<String>>,
    },
    GetHistory {
        room: String,
        resp: oneshot::Sender<Vec<Bytes>>,
    },
    GetRoomList {
        resp: oneshot::Sender<Vec<String>>,
    },
}

struct RoomHandle {
    tx: mpsc::Sender<RoomCmd>,
    _join: JoinHandle<()>, // kept to avoid detaching silently
}

/// Lightweight router hub
pub struct ChatHub {
    rooms: HashMap<String, RoomHandle>,
    rx: mpsc::Receiver<HubCmd>,
    cfg: Config,
}

impl ChatHub {
    pub fn new(rx: mpsc::Receiver<HubCmd>) -> Self {
        Self {
            rooms: HashMap::new(),
            rx,
            cfg: Config::from_env(),
        }
    }

    /// Spawn hub task; returns sender side.
    pub fn spawn() -> mpsc::Sender<HubCmd> {
        let (tx, rx) = mpsc::channel(256);
        let mut hub = ChatHub::new(rx);
        tokio::spawn(async move { hub.run().await });
        tx
    }

    async fn run(&mut self) {
        while let Some(cmd) = self.rx.recv().await {
            self.handle_cmd(cmd).await;
        }
    }

    async fn room_entry(&mut self, room: &str) -> &RoomHandle {
        if !self.rooms.contains_key(room) {
            let (tx, jh) = spawn_room_task(&self.cfg, room.to_string());
            self.rooms.insert(room.to_string(), RoomHandle { tx, _join: jh });
        }
        // unwrap safe now
        self.rooms.get(room).unwrap()
    }

    async fn handle_cmd(&mut self, cmd: HubCmd) {
        match cmd {
            HubCmd::Join { room, name, resp } => {
                let room_handle = self.room_entry(&room).await;
                let (rx_tx, rx_rx) = oneshot::channel();
                // forward
                let _ = room_handle
                    .tx
                    .send(RoomCmd::Join { name, resp: rx_tx })
                    .await;
                // wait for room to give us broadcast receiver then relay back
                if let Ok(bc_rx) = rx_rx.await {
                    let _ = resp.send(bc_rx);
                }
            }
            HubCmd::Send { room, event } => {
                if let Some(handle) = self.rooms.get(&room) {
                    let _ = handle.tx.send(RoomCmd::Send ( event )).await;
                }
            }
            HubCmd::Leave { room, name } => {
                if let Some(handle) = self.rooms.get(&room) {
                    let _ = handle.tx.send(RoomCmd::Leave { name }).await;
                }
            }
            HubCmd::GetMembers { room, resp } => {
                if let Some(handle) = self.rooms.get(&room) {
                    let (tx, rx) = oneshot::channel();
                    let _ = handle.tx.send(RoomCmd::GetMembers { resp: tx }).await;
                    let _ = resp.send(rx.await.unwrap_or_default());
                } else {
                    let _ = resp.send(Vec::new());
                }
            }
            HubCmd::GetHistory { room, resp } => {
                if let Some(handle) = self.rooms.get(&room) {
                    let (tx, rx) = oneshot::channel();
                    let _ = handle.tx.send(RoomCmd::GetHistory { resp: tx }).await;
                    let _ = resp.send(rx.await.unwrap_or_default());
                } else {
                    let _ = resp.send(Vec::new());
                }
            }
            HubCmd::GetRoomList { resp } => {
                self.rooms.retain(|_, h| !h.tx.is_closed());
                let list: Vec<String> = self.rooms.keys().cloned().collect();
                let _ = resp.send(list);
            }
        }
    }
}

