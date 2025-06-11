// src/hub.rs

use std::collections::HashMap;
use tokio::sync::{mpsc, broadcast};
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
}

/// 聊天室管理中心
pub struct ChatHub {
    /// 房间名 -> 广播 Sender
    rooms: HashMap<String, broadcast::Sender<ServerEvent>>,
    /// Hub 命令接收端
    cmd_rx: mpsc::Receiver<HubCommand>,
}

impl ChatHub {
    pub fn new(cmd_rx: mpsc::Receiver<HubCommand>) -> Self {
        Self {
            rooms: HashMap::new(),
            cmd_rx,
        }
    }

    /// 核心事件循环
    pub async fn run(&mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                HubCommand::JoinRoom { room, name, resp } => {
                    let sender = self.rooms
                        .entry(room.clone())
                        .or_insert_with(|| broadcast::channel(128).0)
                        .clone();
                    let receiver = sender.subscribe();
                    // 先返回 receiver
                    let _ = resp.send(receiver).await;
                    // 再广播 UserJoined
                    let _ = sender.send(ServerEvent::UserJoined {
                        room: room.clone(),
                        name: name.clone(),
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
                            name,
                        });
                    }
                }
                HubCommand::GetRoomList { resp } => {
                    // 获取房间名列表
                    let room_list = self.rooms.keys().cloned().collect();
                    let _ = resp.send(room_list).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ServerEvent;
    use tokio::sync::{mpsc, broadcast};

    #[tokio::test]
    async fn test_join_and_message() {
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut hub = ChatHub::new(cmd_rx);

        // 启动 hub task
        let hub_handle = tokio::spawn(async move {
            hub.run().await;
        });

        // 测试加入房间
        let (resp_tx, mut resp_rx) = mpsc::channel(1);
        cmd_tx.send(HubCommand::JoinRoom {
            room: "test".into(),
            name: "alice".into(),
            resp: resp_tx,
        }).await.unwrap();

        // 获取广播 receiver
        let mut receiver = resp_rx.recv().await.unwrap();

        // 发消息
        cmd_tx.send(HubCommand::SendMsg {
            room: "test".into(),
            event: ServerEvent::NewMessage {
                room: "test".into(),
                name: "alice".into(),
                text: "hi".into(),
                ts: 1,
            },
        }).await.unwrap();

        // 广播事件必须能收到 UserJoined 和 NewMessage
        let mut events = vec![];
        for _ in 0..2 {
            if let Ok(event) = receiver.recv().await {
                events.push(event);
            }
        }
        assert!(events.iter().any(|e| matches!(e, ServerEvent::UserJoined { .. })));
        assert!(events.iter().any(|e| matches!(e, ServerEvent::NewMessage { .. })));

        // 测试离开房间
        cmd_tx.send(HubCommand::LeaveRoom {
            room: "test".into(),
            name: "alice".into(),
        }).await.unwrap();

        // 能收到 UserLeft
        let user_left = receiver.recv().await.unwrap();
        assert!(matches!(user_left, ServerEvent::UserLeft { .. }));

        // 关闭 hub
        drop(cmd_tx);
        hub_handle.await.unwrap();
    }
}
