# webchathub

> **webchathub** is a lightweight multi‑room WebSocket chat system written in Rust.  
> It ships with a **dependency‑free server** and a **cross‑platform TUI client**, ready to run out‑of‑the‑box.

## ✨ Highlights

- **Tokio + tungstenite** async stack: thousands of concurrent connections on a single host
- **Multi‑room with history replay**: each room keeps the latest _N_ messages and replays them on join
- **Room TTL**: idle rooms are automatically recycled to free resources
- **Memory pool**: shared binary buffer to reduce allocations and copies
- **Slash‑command TUI**: `/join`, `/leave`, `/rooms`, `/members` at your fingertips
- **Pure JSON protocol**: easy to integrate from browsers or any language

## Project Layout

```text
webchathub/
├─ Cargo.toml               # Build metadata & dependencies
├─ src/
│  ├─ bin/                  # Executable entry points
│  │  ├─ server.rs          # Chat server
│  │  └─ client.rs          # TUI client
│  ├─ server/               # Server internals
│  │  ├─ listener.rs
│  │  └─ mod.rs
│  ├─ client/               # Client UI helpers
│  │  ├─ ui.rs
│  │  └─ mod.rs
│  ├─ hub.rs                # ChatHub: routing / dispatch
│  ├─ room.rs               # Room state machine
│  ├─ protocol.rs           # JSON message types
│  ├─ memory_pool.rs        # Bytes pool
│  ├─ config.rs             # Environment config
│  └─ lib.rs                # crate exports
```

## Quick Start

> Requirements: **Rust 1.76+**. CI passes on Linux, macOS, and Windows.

```bash
# clone
clone git@github.com:lijianyangyf/webchathub.git
cd webchathub

# release build
cargo build --release
```

### 1. Start the server

```bash
# listens on 0.0.0.0:9000 by default
cargo run --bin server
```

See [Configuration](#configuration) for environment variables.

### 2. Start the local TUI client

```bash
cargo run --bin client          # connect to ws://127.0.0.1:9000
# or specify explicit ws URL
cargo run --bin client ws://1.2.3.4:9000
```

### 3. Slash Commands

| Command | Description |
|---------|-------------|
| `/join <room> <name>` | Join or create a room |
| `/leave` | Leave the current room |
| `/rooms` | List all rooms |
| `/members` | List members of the current room |

Invalid syntax yields:  
`usage: /join <room> <name> | /leave | /rooms | /members`

## Configuration

The server reads **environment variables** and falls back to defaults:

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SERVER_ADDR` | string | `0.0.0.0:9000` | WebSocket listen address |
| `LOG_LEVEL`   | string | `info`        | log level (`trace`–`error`) |
| `HISTORY_LIMIT` | usize | `100`        | number of historical messages per room |
| `ROOM_TTL_SECS` | u64   | `300`        | idle room recycle TTL (seconds) |

Example:

```bash
SERVER_ADDR=127.0.0.1:8080 LOG_LEVEL=debug cargo run --bin server
```

## Protocol

All messages are **UTF‑8 JSON** text frames.

### Client → Server `ClientRequest`

```jsonc
// join a room
{ "Join": { "room": "rust", "name": "alice" } }

// send a message
{ "Message": { "room": "rust", "text": "hello" } }

// others: Leave | RoomList | Members
```

### Server → Client `ServerEvent`

```jsonc
// regular chat
{ "NewMessage":
  { "room": "rust", "name": "alice", "text": "hello", "ts": 1718620680000 } }

// system events
{ "UserJoined": { "room": "rust", "name": "bob" } }
{ "UserLeft":   { "room": "rust", "name": "bob" } }

// room & member lists
{ "RoomList":   { "rooms": ["rust","golang"] } }
{ "MemberList": { "room": "rust", "members": ["alice","bob"] } }
```

> Timestamps `ts` are milliseconds since Unix epoch (UTC).

## Architecture Overview

```text
┌──────────┐      HubCmd       ┌──────────────┐  RoomCmd  ┌───────────┐
│ listener │ ───────────────▶ │   ChatHub     │──────────▶│   Room    │
│  (WS)    │ ◀─────────────── │ (router/task) │ ◀─────────│ (state)   │
└──────────┘   broadcast bytes └──────────────┘   events   └───────────┘
                     ▲                                │
                     └─ history replay / members list ┘
```

* **listener** — upgrades each TCP stream to WebSocket, handles handshakes & framing  
* **ChatHub** — routes commands to the appropriate room and maintains the room map  
* **Room** — keeps members, message history, and TTL timer, broadcasting events to subscribers  
* **memory_pool** — shared `BytesMut` pool to cut down on allocations & copies  

## Development & Contribution

```bash
# run unit / integration tests
cargo test

# format code
cargo fmt
```

PRs and issues are welcome! Add a Roadmap or contributing guide under this README if desired.

## License

Dual‑licensed as **MIT OR Apache‑2.0**. Adjust as needed for your organization.
