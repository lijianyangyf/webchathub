# My Chat – Async WebSocket Chat Server & TUI Client

A tiny yet fully‑featured chat application written in **Rust**.  
It demonstrates a clean, actor‑like architecture on top of Tokio,
with a broadcast hub on the server side and a terminal‑based UI on the client
side.

---

## ✨ Features

* **Async everywhere** – built on `tokio` and `tokio‑tungstenite`.
* **Room‑based chat** – users join named rooms, messages are scoped to the room.
* **Broadcast hub** – one task per room, using `tokio::sync::broadcast`.
* **Interactive terminal client** – `tui` + `crossterm`, supports scroll‑back,
  command shortcuts and real‑time updates.
* **JSON protocol** – a minimal, versioned client/server message format.
* **Config via env** – server address, log level, buffer sizes.
* **Extensible** – traits/structs are loosely coupled, ready for HTTP, TLS,
  persistence or metrics add‑ons.

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
   ├─ hub.rs               # ChatHub: room registry + command loop
   ├─ protocol.rs          # ClientRequest / ServerEvent enums
   ├─ error.rs             # ChatError + Result alias
   ├─ config.rs            # Config::from_env()
   └─ lib.rs               # re‑exports
```

---

## 📜 Protocol (v1)

### Client → Server (`ClientRequest`)
| Variant | Fields | Notes |
|---------|--------|-------|
| `ListRooms` | – | ask for current room names |
| `Join`      | `room: String`, `nick: String` | first message after connect |
| `SendMsg`   | `text: String` | UTF‑8, max 2 kB |
| `Leave`     | – | leave current room |

### Server → Client (`ServerEvent`)
| Variant | Fields | Notes |
|---------|--------|-------|
| `RoomList`   | `rooms: Vec<String>` |
| `Joined`     | `room`, `members` |
| `UserJoined` | `nick` |
| `UserLeft`   | `nick` |
| `Message`    | `from`, `text`, `ts` |
| `Error`      | `reason` |

Ping/Pong frames are handled automatically by tungstenite; application‑level
heart‑beats can be added in the future.

---

## ⚙️ Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `CHAT_ADDR`   | `0.0.0.0:9000` | TCP address to bind the websocket server |
| `CHAT_LOG`    | `info`         | `trace`, `debug`, `info`, `warn`, `error` |
| `CHAT_ROOM_BUFFER` | `128` | broadcast channel capacity per room |

---

## ▶️ Running Locally

```bash
# 1. Start the server
cargo run --bin server

# 2. In another terminal start two clients
cargo run --bin client ws://127.0.0.1:9000
```

> Press **`?`** inside the client for a quick help screen.  
> Supported commands: `/rooms`, `/join <room>`, `/leave`, `/quit`.

---

## 🧹 Graceful shutdown

* **Server** – Ctrl‑C is caught; tasks are notified and the listener stops
  accepting.  
* **Client** – Esc or `/quit` restores the terminal to the previous state.

---

## 📈 Roadmap / Ideas

* Prometheus & OpenTelemetry instrumentation.
* TLS & HTTP upgrade with `hyper` + `tokio‑rustls`.
* Rate‑limiting and auth (JWT).
* Message persistence (Redis Streams or S3 archive).

---

## 🤝 Contributing

1. `cargo fmt && cargo clippy --all-targets`
2. `cargo test`
3. Open a PR – thanks!

---

## License

MIT
