// src/hub.rs – 完整实现（含 1‑A & 1‑B）
// -------------------------------------------
// * 1‑A: 成员列表查询 (已完成)
// * 1‑B: 历史消息环形缓冲
//   - 每房间 `VecDeque<ServerEvent>` 长度由 `Config::history_limit` 决定。
//   - `SendMsg` 时写入环形缓冲并裁剪。
//   - 新增 `GetHistory` 命令供 Listener 在 Join 之后批量拉取。
//   - **仅存储聊天消息** (ServerEvent::Message)，系统事件不进入历史。
//
// 对原有接口改动最小：
//   ChatHub::new(cmd_rx) 内部自行读取 Config::from_env() 拿到 history_limit，
//   上层调用方无需变更。

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use tokio::sync::{broadcast, mpsc};

use crate::config::Config;
use crate::protocol::ServerEvent;

/// Hub 内部保存的聊天记录，仅保存 `ServerEvent::Message` 变体
fn is_chat_message(ev: &ServerEvent) -> bool {
    matches!(ev, ServerEvent::NewMessage { .. })
}

/// Hub 外部命令
pub enum HubCommand {
    JoinRoom {
        room: String,
        name: String,
        resp: mpsc::Sender<broadcast::Receiver<ServerEvent>>,
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
        resp: mpsc::Sender<Vec<String>>,
    },
    /// 1‑A
    GetMembers {
        room: String,
        resp: mpsc::Sender<Vec<String>>,
    },
    /// 1‑B: 新增——批量拉取房间历史
    GetHistory {
        room: String,
        resp: mpsc::Sender<Vec<ServerEvent>>,
    },
}

/// 房间结构
struct Room {
    tx: broadcast::Sender<ServerEvent>,
    members: HashSet<String>,
    history: VecDeque<ServerEvent>,
    last_empty_at: Option<Instant>, // 留给 1‑C TTL 使用
}

impl Room {
    fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            tx,
            members: HashSet::new(),
            history: VecDeque::with_capacity(capacity),
            last_empty_at: None,
        }
    }
}

/// 聊天室管理中心
pub struct ChatHub {
    rooms: HashMap<String, Room>,
    cmd_rx: mpsc::Receiver<HubCommand>,
    history_limit: usize,
}

impl ChatHub {
    pub fn new(cmd_rx: mpsc::Receiver<HubCommand>) -> Self {
        let cfg = Config::from_env();
        Self {
            rooms: HashMap::new(),
            cmd_rx,
            history_limit: cfg.history_limit,
        }
    }

    pub async fn run(&mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                HubCommand::JoinRoom { room, name, resp } => {
                    let room_ref = self
                        .rooms
                        .entry(room.clone())
                        .or_insert_with(|| Room::new(self.history_limit));

                    room_ref.members.insert(name.clone());
                    room_ref.last_empty_at = None;

                    let _ = resp.send(room_ref.tx.subscribe()).await;
                    let _ = room_ref.tx.send(ServerEvent::UserJoined { room, name });
                }
                HubCommand::SendMsg { room, event } => {
                    if let Some(room_ref) = self.rooms.get_mut(&room) {
                        // 先广播
                        let _ = room_ref.tx.send(event.clone());
                        // 只记录聊天消息
                        if is_chat_message(&event) {
                            room_ref.history.push_back(event);
                            if room_ref.history.len() > self.history_limit {
                                room_ref.history.pop_front();
                            }
                        }
                    }
                }
                HubCommand::LeaveRoom { room, name } => {
                    if let Some(room_ref) = self.rooms.get_mut(&room) {
                        let _ = room_ref.tx.send(ServerEvent::UserLeft {
                            room: room.clone(),
                            name: name.clone(),
                        });
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
                    let members = self
                        .rooms
                        .get(&room)
                        .map(|r| r.members.iter().cloned().collect())
                        .unwrap_or_default();
                    let _ = resp.send(members).await;
                }
                HubCommand::GetHistory { room, resp } => {
                    let msgs = self
                        .rooms
                        .get(&room)
                        .map(|r| r.history.iter().cloned().collect())
                        .unwrap_or_default();
                    let _ = resp.send(msgs).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::{broadcast, mpsc};

    #[tokio::test]
    async fn test_history_cap() {
        let (tx, rx) = mpsc::channel(8);
        let mut hub = ChatHub::new(rx);
        let cap = hub.history_limit;

        // spawn hub
        let h = tokio::spawn(async move { hub.run().await });

        // join room
        let (resp_tx, mut resp_rx) = mpsc::channel(1);
        tx.send(HubCommand::JoinRoom {
            room: "rust".into(),
            name: "bob".into(),
            resp: resp_tx,
        })
        .await
        .unwrap();
        let _ = resp_rx.recv().await.unwrap();

        // send cap+1 messages
        for i in 0..=cap {
            tx.send(HubCommand::SendMsg {
                room: "rust".into(),
                event: ServerEvent::NewMessage {
                    room: "rust".into(),
                    name: "bob".into(),
                    text: format!("hello {i}"),
                    ts: 0,
                },
            })
            .await
            .unwrap();
        }

        let (his_tx, mut his_rx) = mpsc::channel(1);
        tx.send(HubCommand::GetHistory {
            room: "rust".into(),
            resp: his_tx,
        })
        .await
        .unwrap();
        let history = his_rx.recv().await.unwrap();
        assert_eq!(history.len(), cap); // oldest被裁掉

        drop(tx);
        h.await.unwrap();
    }
}
