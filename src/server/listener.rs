// src/server/listener.rs – fix oneshot types & Sized errors
use std::str;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::hub::HubCmd;
use crate::protocol::{ClientRequest, ServerEvent};

pub async fn start_ws_listener(addr: &str, hub_tx: mpsc::Sender<HubCmd>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket listening on: {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let hub_clone = hub_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_ws(stream, hub_clone).await {
                eprintln!("connection error: {:?}", e);
            }
        });
    }
}

async fn handle_ws(stream: tokio::net::TcpStream, hub: mpsc::Sender<HubCmd>) -> anyhow::Result<()> {
    let ws = accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    // -- wait for Join or RoomList
    let (room, name) = loop {
        let msg = ws_rx.next().await.ok_or_else(|| anyhow::anyhow!("eof"))??;
        let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
        match req {
            ClientRequest::Join { room, name } => break (room, name),
            ClientRequest::RoomList => {
                let (tx, rx) = oneshot::channel();
                hub.send(HubCmd::GetRoomList { resp: tx }).await?;
                let list = rx.await?;
                let ev = ServerEvent::RoomList { rooms: list };
                ws_tx.send(Message::Text(serde_json::to_string(&ev)?)).await?;
            }
            _ => {}
        }
    };

    // -- join room
    let (join_tx, join_rx) = oneshot::channel();
    hub.send(HubCmd::Join { room: room.clone(), name: name.clone(), resp: join_tx }).await?;
    let mut bcast_rx = join_rx.await?;

    // push channel -> websocket
    let (push_tx, mut push_rx) = mpsc::channel::<Message>(32);

    // history replay
    {
        let (htx, hrx) = oneshot::channel();
        hub.send(HubCmd::GetHistory { room: room.clone(), resp: htx }).await?;
        if let Ok(hist) = hrx.await {
            for frame in hist {
                if let Ok(txt) = str::from_utf8(&frame) {
                    push_tx.send(Message::Text(txt.to_owned())).await?;
                }
            }
        }
    }

    // background push task
    let (close_tx_raw, close_rx) = oneshot::channel::<()>();
        let mut close_tx = Some(close_tx_raw);
    let push_handle = tokio::spawn(async move {
        tokio::select! {
            _ = async {
                loop {
                    tokio::select! {
                        Some(m) = push_rx.recv() => {
                            if ws_tx.send(m).await.is_err() { break; }
                        }
                        Ok(frame) = bcast_rx.recv() => {
                            if let Ok(txt) = str::from_utf8(&frame) {
                                if ws_tx.send(Message::Text(txt.to_owned())).await.is_err() { break; }
                            }
                        }
                    }
                }
            } => {},
            _ = close_rx => {
                let _ = ws_tx.send(Message::Close(None)).await;
            }
        }
    });

    // main loop after join
    while let Some(Ok(msg)) = ws_rx.next().await {
        if !msg.is_text() { continue; }
        let req: ClientRequest = serde_json::from_str(msg.to_text()?)?;
        match req {
            ClientRequest::Message { room, text } => {
                let ev = ServerEvent::NewMessage {
                    room: room.clone(),
                    name: name.clone(),
                    text,
                    ts: chrono::Utc::now().timestamp_millis() as u64,
                };
                hub.send(HubCmd::Send { room, event: ev }).await?;
            }
            ClientRequest::Leave { room } => {
                hub.send(HubCmd::Leave { room: room.clone(), name: name.clone() }).await?;
                if let Some(tx) = close_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
            ClientRequest::Members { room } => {
                let (tx, rx) = oneshot::channel();
                hub.send(HubCmd::GetMembers { room: room.clone(), resp: tx }).await?;
                if let Ok(list) = rx.await {
                    let ev = ServerEvent::MemberList { room, members: list };
                    push_tx.send(Message::Text(serde_json::to_string(&ev)?)).await?;
                }
            }
            ClientRequest::Join { .. } | ClientRequest::RoomList => {}
        }
    }

    if let Some(tx) = close_tx.take() {
        let _ = tx.send(());
    }
    let _ = push_handle.await;
    Ok(())
}
