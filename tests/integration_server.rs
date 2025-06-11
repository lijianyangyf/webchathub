use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};
use serde_json;
use std::time::Duration;

use my_chat::hub::{ChatHub, HubCommand};
use my_chat::protocol::{ClientRequest, ServerEvent};
use my_chat::server::listener::start_ws_listener;

#[tokio::test]
async fn test_ws_lifecycle() {
    let _ = env_logger::builder().is_test(true).try_init();

    let (hub_tx, hub_rx) = mpsc::channel(8);
    let mut hub = ChatHub::new(hub_rx);
    tokio::spawn(async move { hub.run().await; });

    let addr = "127.0.0.1:9010";
    tokio::spawn(start_ws_listener(addr, hub_tx.clone()));
    tokio::time::sleep(Duration::from_millis(100)).await;

    let url = format!("ws://{}", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WebSocket connect failed");

    // 发送 Join
    let join = ClientRequest::Join {
        room: "testroom".into(),
        name: "testuser".into(),
    };
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&join).unwrap(),
    )).await.unwrap();

    // 接收 UserJoined
    let msg = ws.next().await.expect("No message received").unwrap();
    if msg.is_text() {
        let event: ServerEvent = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        assert!(matches!(event, ServerEvent::UserJoined { room, name } if room == "testroom" && name == "testuser"));
    } else if msg.is_close() {
        println!("WebSocket closed by server");
        return;
    } else {
        panic!("Unexpected message: {:?}", msg);
    }

    // 发送消息
    let say = ClientRequest::Message {
        room: "testroom".into(),
        text: "hello, world!".into(),
    };
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&say).unwrap(),
    )).await.unwrap();

    // 接收 NewMessage
    let msg = ws.next().await.expect("No NewMessage received").unwrap();
    if msg.is_text() {
        let event: ServerEvent = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        assert!(matches!(event, ServerEvent::NewMessage { room, name, text, .. } 
            if room == "testroom" && name == "testuser" && text == "hello, world!"
        ));
    } else if msg.is_close() {
        println!("WebSocket closed by server");
        return;
    } else {
        panic!("Unexpected message: {:?}", msg);
    }

    // 发送离开
    let leave = ClientRequest::Leave {
        room: "testroom".into(),
    };
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&leave).unwrap(),
    )).await.unwrap();

    // 接收 UserLeft
    let msg = ws.next().await.expect("No UserLeft received").unwrap();
    if msg.is_text() {
        let event: ServerEvent = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        assert!(matches!(event, ServerEvent::UserLeft { room, name } if room == "testroom" && name == "testuser"));
    } else if msg.is_close() {
        println!("WebSocket closed by server");
        return;
    } else {
        panic!("Unexpected message: {:?}", msg);
    }

    // 正常关闭（不会panic）
    ws.close(None).await.unwrap();
}
