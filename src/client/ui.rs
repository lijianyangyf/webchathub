use std::io::{self, Stdout};
use std::time::Duration;

use chrono::{Local, TimeZone};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,KeyEvent,KeyEventKind},
    execute, terminal,
};
use futures_util::{Sink, SinkExt, StreamExt};
use tokio::{
    select,
    sync::mpsc,
    task,
    time::{sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Spans,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

use crate::protocol::{ClientRequest, ServerEvent};

const WS_DEFAULT: &str = "ws://127.0.0.1:9000";

pub async fn start_cli_client(ws_addr: Option<String>) -> anyhow::Result<()> {
    let ws_addr = ws_addr.unwrap_or_else(|| WS_DEFAULT.to_string());
    let (ws_stream, _) = connect_async(&ws_addr).await?;
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // UI channel
    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<String>();

    // Spawn reader task
    {
        let ui_tx = ui_tx.clone();
        task::spawn(async move {
            while let Some(Ok(msg)) = ws_stream.next().await {
                if !msg.is_text() {
                    continue;
                }
                let evt: ServerEvent = match serde_json::from_str(msg.to_text().unwrap()) {
                    Ok(ev) => ev,
                    Err(e) => {
                        let _ = ui_tx.send(format!("⚠️  bad event: {e}"));
                        continue;
                    }
                };

                match evt {
                    ServerEvent::NewMessage { name, text, ts, .. } => {
                        if let Some(dt) = Local.timestamp_millis_opt(ts as i64).single() {
                            let _ =
                                ui_tx.send(format!("[{}] {}: {}", dt.format("%H:%M:%S"), name, text));
                        }
                    }
                    ServerEvent::UserJoined { name, room } => {
                        let _ = ui_tx.send(format!("🔔 {name} joined {room}"));
                    }
                    ServerEvent::UserLeft { name, room } => {
                        let _ = ui_tx.send(format!("🔕 {name} left {room}"));
                    }
                    ServerEvent::RoomList { rooms } => {
                        let _ = ui_tx.send(format!("📄 rooms: {:?}", rooms));
                    }
                    ServerEvent::MemberList { room, members } => {
                        let _ = ui_tx.send(format!("👥 members in {room}: {:?}", members));
                    }
                }
            }
        });
    }

    // --- Terminal UI setup ---
    enable_tui()?;
    let mut terminal = init_terminal()?;
    let mut input = String::new();
    let mut messages: Vec<String> = Vec::new();
    let mut room: Option<String> = None;

    loop {
        // Draw
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(85), Constraint::Percentage(15)].as_ref())
                .split(f.size());

            let items: Vec<ListItem> =
                messages.iter().map(|m| ListItem::new(Spans::from(m.as_str()))).collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Messages"));
            f.render_widget(list, chunks[0]);

            let inp = Paragraph::new(input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(inp, chunks[1]);
        })?;

        select! {
            Some(line) = ui_rx.recv() => {
                messages.push(line);
            }

            _ = sleep(Duration::from_millis(10)) => {
                while event::poll(Duration::from_millis(0))? {
                    if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
                        if kind == KeyEventKind::Press {
                            match code {
                                KeyCode::Char(c) => input.push(c),
                                KeyCode::Backspace => { input.pop(); }
                                KeyCode::Enter => {
                                    let cmd = input.trim().to_string();
                                    input.clear();
                                    if cmd.starts_with('/') {
                                        handle_command(&cmd, &mut ws_sink, &mut room, &mut messages).await?;
                                        if cmd == "/leave" {
                                            messages.clear(); // free history memory
                                            disable_tui()?;
                                            return Ok(());
                                        }
                                    } else if let Some(r) = &room {
                                        let req = ClientRequest::Message { room: r.clone(), text: cmd };
                                        ws_sink.send(Message::Text(serde_json::to_string(&req)?)).await?;
                                    } else {
                                        messages.push("❗ join a room first".into());
                                    }
                                }
                                KeyCode::Esc => {
                                    if let Some(r) = &room {
                                        let leave = ClientRequest::Leave { room: r.clone() };
                                        ws_sink
                                            .send(Message::Text(serde_json::to_string(&leave)?))
                                            .await?;
                                    }
                                    disable_tui()?;
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Parse and run slash commands.
async fn handle_command<S>(
    cmd: &str,
    ws_sink: &mut S,
    room: &mut Option<String>,
    messages: &mut Vec<String>,
) -> anyhow::Result<()>
where
    S: Sink<Message> + Unpin + Send,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    match parts.as_slice() {
        ["/join", room_name, name] => {
            let req = ClientRequest::Join {
                room: room_name.to_string(),
                name: name.to_string(),
            };
            ws_sink.send(Message::Text(serde_json::to_string(&req)?)).await?;
            *room = Some(room_name.to_string());
        }
        ["/leave"] => {
            if let Some(r) = room.take() {
                ws_sink
                    .send(Message::Text(
                        serde_json::to_string(&ClientRequest::Leave { room: r })?,
                    ))
                    .await?;
            }
        }
        ["/rooms"] => {
            ws_sink
                .send(Message::Text(serde_json::to_string(&ClientRequest::RoomList)?))
                .await?;
        }
        ["/members"] => {
            if let Some(r) = room {
                ws_sink
                    .send(Message::Text(
                        serde_json::to_string(&ClientRequest::Members { room: r.clone() })?,
                    ))
                    .await?;
            } else {
                messages.push("❗ not in any room".into());
            }
        }
        _ => {
            messages.push(
                "❗ usage: /join <room> <name> | /leave | /rooms | /members".into(),
            );
        }
    }
    Ok(())
}

/// Terminal helpers
fn enable_tui() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(
        io::stdout(),
        terminal::EnterAlternateScreen,
        EnableMouseCapture
    )?;
    Ok(())
}

fn disable_tui() -> io::Result<()> {
    execute!(
        io::stdout(),
        terminal::LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal::disable_raw_mode()
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}
