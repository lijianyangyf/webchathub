use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::{mpsc, oneshot};

use crate::hub::HubCommand;
use crate::protocol::{ClientRequest, ServerEvent};

pub async fn start_ws_listener(
    addr: &str,
    hub_tx: mpsc::Sender<HubCommand>
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket listening on: {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let hub_tx = hub_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_ws_connection(stream, hub_tx).await {
                eprintln!("connection error: {:?}", e);
            }
        });
    }
}

async fn handle_ws_connection(
    stream: tokio::net::TcpStream,
    hub_tx: mpsc::Sender<HubCommand>
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 等待直到收到 Join
    let (room, name);
    loop {
        let msg = ws_receiver.next().await
            .ok_or_else(|| anyhow::anyhow!("Client disconnected before join"))??;
        let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
        match req {
            ClientRequest::Join { room: r, name: n } => {
                room = r;
                name = n;
                break;
            }
            ClientRequest::RoomList => {
                // 查房间列表
                let (resp_tx, mut resp_rx) = mpsc::channel(1);
                hub_tx.send(HubCommand::GetRoomList { resp: resp_tx }).await?;
                if let Some(room_list) = resp_rx.recv().await {
                    let event = ServerEvent::RoomList { rooms: room_list };
                    let msg = Message::Text(serde_json::to_string(&event)?);
                    ws_sender.send(msg).await?;
                }
                // 继续等待 Join
                continue;
            }
            _ => {
                // 其它操作一律忽略
                continue;
            }
        }
    }

    // 2. 发送 JoinRoom 给 hub，获取本房间 broadcast::Receiver
    let (resp_tx, mut resp_rx) = mpsc::channel(1);
    hub_tx.send(HubCommand::JoinRoom {
        room: room.clone(),
        name: name.clone(),
        resp: resp_tx,
    }).await?;

    let mut broadcast_rx = resp_rx.recv().await.ok_or_else(|| anyhow::anyhow!("Failed to get broadcast receiver"))?;

    // 主循环向推送任务发消息的 channel
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(8);

    // 3. 推送任务，负责所有 ws_sender 写
    let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();
    let push_task = tokio::spawn(async move {
        let mut ws_sender = ws_sender;
        tokio::select! {
            _ = async {
                loop {
                    tokio::select! {
                        Some(msg) = msg_rx.recv() => {
                            if ws_sender.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Ok(event) = broadcast_rx.recv() => {
                            let json = serde_json::to_string(&event).unwrap();
                            if ws_sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                        else => { break; }
                    }
                }
            } => {},
            _ = close_rx => {
                let _ = ws_sender.send(Message::Close(None)).await;
            }
        }
    });

    // 4. 主循环：只做消息逻辑，发给推送任务写
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if msg.is_text() {
            let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
            match req {
                ClientRequest::Message { room, text } => {
                    let event = ServerEvent::NewMessage {
                        room: room.clone(),
                        name: name.clone(),
                        text,
                        ts: chrono::Utc::now().timestamp() as u64,
                    };
                    hub_tx.send(HubCommand::SendMsg { room: room.clone(), event }).await?;
                }
                ClientRequest::Leave { room } => {
                    hub_tx.send(HubCommand::LeaveRoom { room: room.clone(), name: name.clone() }).await?;
                    let _ = close_tx.send(()); // 通知推送任务优雅关闭
                    break;
                }
                ClientRequest::Join { .. } => { continue; }
                ClientRequest::RoomList => { continue; }
            }
        }
    }

    let _ = push_task.await;
    Ok(())
}
