# My Chat – Async WebSocket Chat Server & TUI Client

A tiny yet fully‑featured chat application written in **Rust**.  
It demonstrates a clean, actor‑like architecture on top of Tokio,
with a broadcast hub on the server side and a terminal‑based UI on the client
side.

---

## ✨ Features

* **Async everywhere** – built on `tokio` and `tokio‑tungstenite`.
* **Room‑based chat** – users join named rooms; messages are scoped to the room.
* **Broadcast hub** – one task per room, using `tokio::sync::broadcast`.
* **Interactive terminal client** – `tui` + `crossterm`, supports scroll‑back,
  command shortcuts and real‑time updates.
* **Member list query** – `/members` instantly shows who’s online in the room.  *(new in v0.2)*
* **Limited message history** – the server keeps the last *N* messages in a
  ring‑buffer and replays them (with timestamps) to newcomers. *(new in v0.2)*
* **JSON protocol** – a minimal, versioned client/server message format.
* **Config via env** – server address, log level, buffer & history size.
* **Extensible** – loosely‑coupled modules ready for HTTP, TLS, persistence
  or metrics add‑ons.

---

## 🗂 Project Layout

```
my_chat/
├─ Cargo.toml
└─ src/
   ├─ bin/
   │   ├─ server.rs        # start_ws_listener + ChatHub
   │   └─ client.rs        # start_cli_client (TUI)
   ├─ client/
   │   ├─ mod.rs
   │   └─ ui.rs            # terminal UI & websocket driver
   ├─ server/
   │   ├─ mod.rs
   │   └─ listener.rs      # TCP acceptor & per‑connection handler
   ├─ hub.rs               # ChatHub: room registry + command loop (+history)
   ├─ protocol.rs          # ClientRequest / ServerEvent enums
   ├─ config.rs            # Config::from_env()
   ├─ error.rs             # ChatError + Result alias
   └─ lib.rs               # re‑exports
```

---

## 📜 Protocol (v1.1)

### Client → Server (`ClientRequest`)
| Variant | Fields | Notes |
|---------|--------|-------|
| `Join`      | `room`, `name` | first message after connect |
| `Leave`     | `room` | leave room |
| `Message`   | `room`, `text` | UTF‑8, max 2 kB |
| `RoomList`  | – | ask for current room names |
| `Members`   | `room` | **new** – ask for online members |

### Server → Client (`ServerEvent`)
| Variant | Fields | Notes |
|---------|--------|-------|
| `RoomList`   | `rooms` |
| `MemberList` | `room`, `members` | **new** |
| `UserJoined` | `room`, `name` |
| `UserLeft`   | `room`, `name` |
| `NewMessage` | `room`, `name`, `text`, `ts` | `ts` = Unix millis |
| `Error`      | `reason` |

Historical messages are replayed as a burst of `NewMessage` events right after
`Join` acknowledgement.

---

## ⚙️ Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `SERVER_ADDR`   | `0.0.0.0:9000` | TCP address to bind the websocket server |
| `LOG_LEVEL`     | `info`         | `trace`, `debug`, `info`, `warn`, `error` |
| `HISTORY_LIMIT` | `100`          | messages kept per‑room for history replay |
| `ROOM_BUFFER`   | `1024`         | broadcast channel capacity per room |

---

## ▶️ Running Locally

```bash
# 1. Start the server
cargo run --bin server

# 2. In another terminal start two clients
cargo run --bin client ws://127.0.0.1:9000
```

Available commands inside the client:

```
/rooms                   # list rooms
/join <room> <name>      # enter room
/members                 # who’s online
/leave                   # exit room
<text>                   # ordinary chat message
```

Press **Esc** to quit and restore your terminal.

---

## 🧹 Graceful shutdown

* **Server** – Ctrl‑C is caught; tasks are notified and the listener stops
  accepting.  
* **Client** – Esc or `/leave` restores the terminal to the previous state.

---

## 📈 Roadmap

* Prometheus & OpenTelemetry instrumentation
* TLS & HTTP upgrade (`hyper` + `tokio-rustls`)
* Rate‑limiting and auth (JWT)
* Message persistence (Redis Streams or S3 archive)
* Web‑based admin dashboard (planned)

---

## 🤝 Contributing

1. `cargo fmt && cargo clippy --all-targets`
2. `cargo test`
3. Open a PR – thanks!

---

## License

MIT

