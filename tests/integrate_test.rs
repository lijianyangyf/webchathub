// tests/integrated_test.rs – end‑to‑end happy‑path scenario
// --------------------------------------------------------
// This integration test exercises the full message flow across Hub ➜ Room ➜
// broadcast channel; it joins a room, sends a chat message, leaves, queries
// history, and finally asserts TTL cleanup.
//
//   cargo test --test integrated_test
//
// Requires: ChatHub::spawn() -> (Sender<HubCmd>, JoinHandle<()>).

use bytes::Bytes;
use my_chat::hub::{ChatHub, HubCmd};
use my_chat::protocol::{ClientRequest, ServerEvent};
use serde_json::from_slice;
use tokio::sync::{broadcast, oneshot};
use std::time::Duration;

#[tokio::test]

async fn integrated_happy_path() {

    unsafe{std::env::set_var("ROOM_TTL_SECS", "1");}
    // 1. start hub
    let hub_tx = ChatHub::spawn();

    // 2. join room
    let (join_tx, join_rx) = oneshot::channel::<broadcast::Receiver<Bytes>>();
    hub_tx
        .send(HubCmd::Join {
            room: "rust".into(),
            name: "alice".into(),
            resp: join_tx,
        })
        .await
        .unwrap();
    let mut bcast_rx = join_rx.await.unwrap();

    // 3. send message
    let chat_evt = ServerEvent::NewMessage {
        room: "rust".into(),
        name: "alice".into(),
        text: "hello".into(),
        ts: 42,
    };
    hub_tx
        .send(HubCmd::Send {
            room: "rust".into(),
            event: chat_evt.clone(),
        })
        .await
        .unwrap();

    // 4. receive broadcast
    let frame = bcast_rx.recv().await.unwrap();
    let evt: ServerEvent = from_slice(&frame).unwrap();
    assert_eq!(evt, chat_evt);

    // 5. leave room (triggers TTL countdown inside room)
    hub_tx
        .send(HubCmd::Leave {
            room: "rust".into(),
            name: "alice".into(),
        })
        .await
        .unwrap();

    // 6. query history – expect exactly one message (the one we sent)
    let (hist_tx, hist_rx) = oneshot::channel::<Vec<Bytes>>();
    hub_tx
        .send(HubCmd::GetHistory {
            room: "rust".into(),
            resp: hist_tx,
        })
        .await
        .unwrap();
    let history = hist_rx.await.unwrap();
    assert_eq!(history.len(), 1);
    let evt_hist: ServerEvent = from_slice(&history[0]).unwrap();
    assert_eq!(evt_hist, chat_evt);

    // 7. wait > TTL (5s default) then confirm room list empty
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;

    let (rooms_tx, rooms_rx) = oneshot::channel::<Vec<String>>();
    hub_tx.send(HubCmd::GetRoomList { resp: rooms_tx }).await.unwrap();
    let rooms = rooms_rx.await.unwrap();
    assert!(rooms.is_empty(), "room not cleaned up after TTL");
}
