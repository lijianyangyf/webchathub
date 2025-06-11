// src/client/ui.rs

use crate::protocol::{ClientRequest, ServerEvent};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use std::io::{self, BufRead};
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;

pub async fn start_cli_client(ws_url: &str) -> anyhow::Result<()> {
    // 连接服务器
    let (ws_stream, _) = connect_async(ws_url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 提示用户输入 /join room name
    println!("请输入 /rooms 查询房间，或 /join <房间名> <用户名> 加入房间。");

    let mut room = String::new();
    let mut name = String::new();

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let tokens: Vec<_> = input.trim().split_whitespace().collect();
        if tokens.len() == 1 && tokens[0] == "/rooms" {
            ws_sender.send(Message::Text(serde_json::to_string(&ClientRequest::RoomList)?)).await?;
            if let Some(Ok(msg)) = ws_receiver.next().await {
                if msg.is_text() {
                    if let Ok(ServerEvent::RoomList { rooms }) = serde_json::from_str(msg.to_text().unwrap()) {
                        println!("当前房间列表: {:?}", rooms);
                    }
                }
            }
            println!("请继续输入 /join <房间名> <用户名> 加入房间。");
            continue;
        } else if tokens.len() == 3 && tokens[0] == "/join" {
            room = tokens[1].to_string();
            name = tokens[2].to_string();
            break;
        } else {
            println!("命令格式错误，请输入 /rooms 或 /join <房间名> <用户名>");
        }
    }
    // 发送 Join 请求
    let join_msg = ClientRequest::Join { room: room.clone(), name: name.clone() };
    ws_sender.send(Message::Text(serde_json::to_string(&join_msg)?)).await?;
    println!("已加入房间 [{}]，现在可以发送消息，输入 /leave 退出。", room);

    // 2. 启动异步任务：接收服务器推送并打印
    let ws_receiver = ws_receiver; // 只做一次 move
    tokio::spawn(async move {
        let mut ws_receiver = ws_receiver;
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if msg.is_text() {
                if let Ok(event) = serde_json::from_str::<ServerEvent>(msg.to_text().unwrap()) {
                    match event {
                        ServerEvent::UserJoined { name, .. } => println!("👤 {} 加入了房间", name),
                        ServerEvent::UserLeft { name, .. } => println!("👋 {} 离开了房间", name),
                        ServerEvent::NewMessage { name, text, .. } => println!("{}: {}", name, text),
                        ServerEvent::RoomList { rooms } => println!("当前房间列表: {:?}", rooms),
                    }
                }
            } else if msg.is_close() {
                println!("服务器已关闭连接。");
                break;
            }
        }
    });

    // 3. 主循环读取用户输入
    let mut input = String::new();
    loop {
        input.clear();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed == "/leave" {
            let leave_msg = ClientRequest::Leave { room: room.clone() };
            ws_sender.send(Message::Text(serde_json::to_string(&leave_msg)?)).await?;
            println!("你已离开房间。");
            break;
        } else if !trimmed.is_empty() {
            let msg = ClientRequest::Message { room: room.clone(), text: trimmed.into() };
            ws_sender.send(Message::Text(serde_json::to_string(&msg)?)).await?;
        }
    }

    Ok(())
}
