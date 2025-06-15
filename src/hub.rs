// src/hub.rs – 扩展 1‑A：支持查询房间成员列表
// --------------------------------------------------
// 仅在原始实现上做 **最小增量修改**：
// 1. 新增 `members: HashMap<room, HashSet<name>>` 用于维护在线用户。
// 2. 新增 `HubCommand::GetMembers` 处理分支。
// 3. 在 `JoinRoom/LeaveRoom` 时同步维护 `members` 表。
// 其余逻辑（广播 Sender、房间列表）保持原状，未引入破坏性变更。

use std::collections::{HashMap, HashSet};

use tokio::sync::{broadcast, mpsc};

use crate::protocol::ServerEvent;

/// 聊天室命令
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
    /// 新增：查询房间成员
    GetMembers {
        room: String,
        resp: mpsc::Sender<Vec<String>>,
    },
}

/// 聊天室管理中心
pub struct ChatHub {
    /// 房间名 -> 广播 Sender
    rooms: HashMap<String, broadcast::Sender<ServerEvent>>,
    /// 房间名 -> 在线成员集合
    members: HashMap<String, HashSet<String>>,
    /// Hub 命令接收端
    cmd_rx: mpsc::Receiver<HubCommand>,
}

impl ChatHub {
    pub fn new(cmd_rx: mpsc::Receiver<HubCommand>) -> Self {
        Self {
            rooms: HashMap::new(),
            members: HashMap::new(),
            cmd_rx,
        }
    }

    /// 核心事件循环
    pub async fn run(&mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                HubCommand::JoinRoom { room, name, resp } => {
                    // 创建房间广播 Sender（若不存在）
                    let sender = self
                        .rooms
                        .entry(room.clone())
                        .or_insert_with(|| broadcast::channel(128).0)
                        .clone();

                    // 维护成员表
                    self.members
                        .entry(room.clone())
                        .or_default()
                        .insert(name.clone());

                    // 先返回 receiver
                    let _ = resp.send(sender.subscribe()).await;
                    // 再广播 UserJoined
                    let _ = sender.send(ServerEvent::UserJoined {
                        room: room.clone(),
                        name,
                    });
                }

                HubCommand::SendMsg { room, event } => {
                    if let Some(sender) = self.rooms.get(&room) {
                        let _ = sender.send(event);
                    }
                }

                HubCommand::LeaveRoom { room, name } => {
                    if let Some(sender) = self.rooms.get(&room) {
                        let _ = sender.send(ServerEvent::UserLeft {
                            room: room.clone(),
                            name: name.clone(),
                        });
                    }

                    if let Some(set) = self.members.get_mut(&room) {
                        set.remove(&name);
                        // 如果房间已空，可在此处做 TTL 标记（留给 1‑C）
                        if set.is_empty() {
                            // 暂不立即删除，保持原行为
                        }
                    }
                }

                HubCommand::GetRoomList { resp } => {
                    let list: Vec<String> = self.rooms.keys().cloned().collect();
                    let _ = resp.send(list).await;
                }

                HubCommand::GetMembers { room, resp } => {
                    let members = self
                        .members
                        .get(&room)
                        .map(|set| set.iter().cloned().collect())
                        .unwrap_or_default();
                    let _ = resp.send(members).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ServerEvent;
    use tokio::sync::{broadcast, mpsc};

    #[tokio::test]
    async fn test_member_query_flow() {
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut hub = ChatHub::new(cmd_rx);

        let hub_handle = tokio::spawn(async move {
            hub.run().await;
        });

        // 加入房间
        let (resp_tx, mut resp_rx) = mpsc::channel(1);
        cmd_tx
            .send(HubCommand::JoinRoom {
                room: "rust".into(),
                name: "alice".into(),
                resp: resp_tx,
            })
            .await
            .unwrap();
        let _ = resp_rx.recv().await.unwrap();

        // 查询成员
        let (mem_tx, mut mem_rx) = mpsc::channel(1);
        cmd_tx
            .send(HubCommand::GetMembers {
                room: "rust".into(),
                resp: mem_tx,
            })
            .await
            .unwrap();
        let members = mem_rx.recv().await.unwrap();
        assert_eq!(members, vec!["alice".to_string()]);

        drop(cmd_tx);
        hub_handle.await.unwrap();
    }
}