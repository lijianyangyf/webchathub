use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};

use crate::hub::HubCommand;
use crate::protocol::{ClientRequest, ServerEvent};

/// Start WebSocket listener and spawn a task per connection.
pub async fn start_ws_listener(addr: &str, hub_tx: mpsc::Sender<HubCommand>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket listening on: {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let hub_clone = hub_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_ws_connection(stream, hub_clone).await {
                eprintln!("connection error: {:?}", e);
            }
        });
    }
}

async fn handle_ws_connection(
    stream: tokio::net::TcpStream,
    hub_tx: mpsc::Sender<HubCommand>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // ---- 1. 等待 Join ----
    let (room, name) = loop {
        let msg = ws_receiver
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Client disconnected before join"))??;
        let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
        match req {
            ClientRequest::Join { room, name } => break (room, name),
            ClientRequest::RoomList => {
                let (resp_tx, mut resp_rx) = mpsc::channel(1);
                hub_tx
                    .send(HubCommand::GetRoomList { resp: resp_tx })
                    .await?;
                if let Some(rooms) = resp_rx.recv().await {
                    let ev = ServerEvent::RoomList { rooms };
                    ws_sender
                        .send(Message::Text(serde_json::to_string(&ev)?))
                        .await?;
                }
            }
            _ => continue,
        }
    };

    // ---- 2. 加入房间 ----
    let (resp_tx, mut resp_rx) = mpsc::channel(1);
    hub_tx
        .send(HubCommand::JoinRoom {
            room: room.clone(),
            name: name.clone(),
            resp: resp_tx,
        })
        .await?;
    let mut broadcast_rx = resp_rx
        .recv()
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to get broadcast receiver"))?;

    // ---- 3. 推送管道 & 历史补发 ----
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(32);

    // 获取历史并先行推送
    {
        let (hist_tx, mut hist_rx) = mpsc::channel(1);
        hub_tx
            .send(HubCommand::GetHistory {
                room: room.clone(),
                resp: hist_tx,
            })
            .await?;
        if let Some(history) = hist_rx.recv().await {
            for ev in history {
                let txt = serde_json::to_string(&ev)?;
                msg_tx.send(Message::Text(txt)).await?;
            }
        }
    }

    // 将 msg_rx + broadcast_rx 合并写入 ws_sender
    let (close_tx, close_rx) = oneshot::channel::<()>();
    let push_task = tokio::spawn(async move {
        tokio::select! {
            _ = async {
                loop {
                    tokio::select! {
                        Some(msg) = msg_rx.recv() => {
                            if ws_sender.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Ok(ev) = broadcast_rx.recv() => {
                            let txt = serde_json::to_string(&ev).unwrap();
                            if ws_sender.send(Message::Text(txt)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            } => {},
            _ = close_rx => {
                let _ = ws_sender.send(Message::Close(None)).await;
            }
        }
    });

    // ---- 4. 主循环 ----
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if !msg.is_text() {
            continue;
        }
        let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
        match req {
            ClientRequest::Message { room, text } => {
                let ev = ServerEvent::NewMessage {
                    room: room.clone(),
                    name: name.clone(),
                    text,
                    ts: chrono::Utc::now().timestamp_millis() as u64,
                };
                hub_tx
                    .send(HubCommand::SendMsg { room, event: ev })
                    .await?;
            }
            ClientRequest::Leave { room } => {
                hub_tx
                    .send(HubCommand::LeaveRoom {
                        room: room.clone(),
                        name: name.clone(),
                    })
                    .await?;
                let _ = close_tx.send(());
                break;
            }
            ClientRequest::Members { room } => {
                let (resp_tx, mut resp_rx) = mpsc::channel(1);
                hub_tx
                    .send(HubCommand::GetMembers {
                        room: room.clone(),
                        resp: resp_tx,
                    })
                    .await?;
                if let Some(list) = resp_rx.recv().await {
                    let ev = ServerEvent::MemberList { room, members: list };
                    msg_tx
                        .send(Message::Text(serde_json::to_string(&ev)?))
                        .await?;
                }
            }
            // 忽略不应出现的指令
            ClientRequest::Join { .. } | ClientRequest::RoomList => {}
        }
    }

    let _ = push_task.await;
    Ok(())
}
