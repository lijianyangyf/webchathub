use crate::protocol::{ClientRequest, ServerEvent};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use std::io;
use std::time::Duration;

pub async fn start_cli_client(ws_url: &str) -> anyhow::Result<()> {
    // 启用伪图形终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 连接 WebSocket 服务器
    let (ws_stream, _) = connect_async(ws_url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 交互区状态
    let mut messages: Vec<String> = vec!["欢迎来到聊天室！请输入 /rooms 查询房间，或 /join <房间名> <用户名>".to_string()];
    let mut input = String::new();
    let mut joined = false;
    let mut room = String::new();
    let mut name = String::new();

    // tokio channel 用于异步收消息
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel();

    // tokio任务，实时收服务器推送并发到channel
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if msg.is_text() {
                if let Ok(event) = serde_json::from_str::<ServerEvent>(msg.to_text().unwrap()) {
                    match event {
                        ServerEvent::UserJoined { name, .. } => {
                            let _ = msg_tx.send(format!("👤 {} 加入了房间", name));
                        }
                        ServerEvent::UserLeft { name, .. } => {
                            let _ = msg_tx.send(format!("👋 {} 离开了房间", name));
                        }
                        ServerEvent::NewMessage { name, text, .. } => {
                            let _ = msg_tx.send(format!("{}: {}", name, text));
                        }
                        ServerEvent::RoomList { rooms } => {
                            let _ = msg_tx.send(format!("当前房间列表: {:?}", rooms));
                        }
                        ServerEvent::MemberList { room, members } =>{
                            let _ = msg_tx.send(format!("房间 [{}] 成员: {:?}", room, members));
                        }
                    }
                }
            } else if msg.is_close() {
                let _ = msg_tx.send("服务器已关闭连接。".to_string());
                break;
            }
        }
    });

    // 主 UI 事件循环
    loop {
        // 非阻塞地收服务器消息，实时刷消息区
        while let Ok(msg) = msg_rx.try_recv() {
            messages.push(msg);
        }

        // 渲染整个界面
        terminal.draw(|f| {
            let size = f.size();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
                .split(size);

            // 消息区
            let msg_str = messages.iter().rev().take((layout[0].height as usize)-2).collect::<Vec<_>>().into_iter().rev().cloned().collect::<Vec<_>>().join("\n");
            let msg_block = Paragraph::new(msg_str)
                .block(Block::default().borders(Borders::ALL).title("聊天记录"));
            f.render_widget(msg_block, layout[0]);

            // 输入区
            let prompt = if joined {
                format!("{}@{}> {}", name, room, input)
            } else {
                format!("> {}", input)
            };
            let input_block = Paragraph::new(prompt)
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("输入"));
            f.render_widget(input_block, layout[1]);
        })?;

        // 处理输入
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
                // 只处理第一次按下（忽略重复）
                if kind == KeyEventKind::Press {
                    match code {
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Esc => break,
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if trimmed.is_empty() {
                                input.clear();
                                continue;
                            }
                            if !joined {
                                let tokens: Vec<_> = trimmed.split_whitespace().collect();
                                if tokens.len() == 1 && tokens[0] == "/rooms" {
                                    ws_sender.send(Message::Text(serde_json::to_string(&ClientRequest::RoomList)?)).await?;
                                } else if tokens.len() == 3 && tokens[0] == "/join" {
                                    room = tokens[1].to_string();
                                    name = tokens[2].to_string();
                                    let join_msg = ClientRequest::Join { room: room.clone(), name: name.clone() };
                                    ws_sender.send(Message::Text(serde_json::to_string(&join_msg)?)).await?;
                                    joined = true;
                                    messages.push(format!("已加入房间 [{}]，现在可以发送消息，输入 /leave 退出。", room));
                                } else {
                                    messages.push("命令格式错误，请输入 /rooms 或 /join <房间名> <用户名>".to_string());
                                }
                            } else {
                                if trimmed == "/leave" {
                                    let leave_msg = ClientRequest::Leave { room: room.clone() };
                                    ws_sender.send(Message::Text(serde_json::to_string(&leave_msg)?)).await?;
                                    messages.push("你已离开房间。".to_string());
                                    break;
                                } else if trimmed == "/members" {
                                    // 查询当前房间成员
                                    let req = ClientRequest::Members { room: room.clone() };
                                    ws_sender.send(Message::Text(serde_json::to_string(&req)?)).await?;
                                } else if !trimmed.is_empty() {
                                    let msg = ClientRequest::Message { room: room.clone(), text: trimmed.into() };
                                    ws_sender.send(Message::Text(serde_json::to_string(&msg)?)).await?;
                                }
                            }
                            input.clear();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 恢复终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
