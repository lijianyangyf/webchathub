# 说明文档

---

## 1 技术选型
- 异步运行时：tokio
- 网络协议：
  - WebSocket（基于 tokio-tungstenite）
  - TCP（基于 tokio::net::TcpListener）
- 消息编码：JSON（serde + serde_json）
- 同步/广播：tokio::sync::{mpsc, broadcast}
- 日志：log + env_logger
- （可选）HTTP 接口：warp 或 axum

---

## 2 工程目录结构

```text
my_chat/
├─ Cargo.toml
└─ src/
├─ bin/
│ ├─ server.rs # Server 可执行入口
│ └─ client.rs # Client 可执行入口（CLI）
├─ lib.rs # 公共库（protocol、error、config）
├─ protocol.rs # 消息格式定义 (serde)
├─ error.rs # 公共错误类型
├─ config.rs # 配置（端口、日志级别等）
├─ hub.rs # 聊天中心（ChatHub）
├─ server/
│ ├─ mod.rs # Server 模块声明
│ └─ listener.rs # 监听 & 握手 & 连接分发
└─ client/
├─ mod.rs # Client 模块声明
└─ ui.rs # CLI 输入/输出处理
```

---

## 3 核心模块职责
### 3.1 protocol.rs

定义客户端 ↔ 服务器 之间交换的消息结构，如：
```rust
enum ClientRequest { Join { room: String, name: String }, 
                     Message { room: String, text: String }, 
                     Leave { room: String } }
enum ServerEvent  { UserJoined{…}, UserLeft{…}, NewMessage{…}, 
                     RoomList{ Vec<String> } }
```
使用 serde derive 实现 JSON 序列化/反序列化。


### 3.2 hub.rs（ChatHub）

作为全局聊天室管理者，维护：
```rust
rooms: HashMap<String, broadcast::Sender>
client_tx: mpsc::Sender
```
HubCommand 枚举：JoinRoom, LeaveRoom, SendMessage……

主循环 task：不断从 mpsc::Receiver 读取命令，更新 rooms 或向指定 room 的 broadcast::Sender 广播事件。

### 3.3 server/listener.rs

建立 WebSocket/TCP 监听（tokio::spawn）

每接入一个连接，握手后 spawn 一个 connection_handler(task)，并将其 client-side 的 mpsc::Sender 传给 handler。

### 3.4 connection_handler (可在 listener.rs 中)

拆分读（reader）写（writer）半部：

- a) reader 循环：读取到一条 ClientRequest → 转换成 HubCommand 通过 hub_tx 发送给 ChatHub
- b) writer 循环：先在连接初期创建一个 broadcast::Receiver，

不断接收来自该房间的 ServerEvent 并推送给 WebSocket/TCP 客户端。

在 JoinRoom 时：从 hub 端拿到对应 broadcast::Sender，clone 一个 Receiver。

### 3.5 client/ui.rs

CLI 客户端，读取 stdin（用户输入指令，如 /join room1 Alice、hello world）

与服务器 WebSocket 连接，读 stdin → 序列化成 ClientRequest → send

spawn 两个 task：一个处理输入并发消息，一个接收服务器返还的 ServerEvent 并打印到终端。

---

## 4 并发 & 通信流程
### 4.1 启动：

main(server.rs)：初始化日志、加载配置 → 启动 ChatHub (spawn hub task) → 启动 listener
### 4.2 客户端连入：

listener 接入 → WebSocket/TCP 握手 → spawn connection_handler(task)
### 4.3 加入房间：

client 发送 ``` { "type":"Join", "room":"rust", "name":"alice" } ```

connection_handler 将其封装成 ``` HubCommand::JoinRoom(room, name, reply_tx) → hub_tx.send ```

ChatHub 收到 → 如果 room 不存在则创建 broadcast::channel → clone sender

将消息广播给该房间的所有成员
### 4.4 发消息：

client 发送 Message 命令 → ChatHub 转发到指定房间的 broadcast::Sender → 各 client handler 的 Receiver 收到 → 推送给各 WebSocket
### 4.5 退出房间/断开：

client 发送 Leave 或者断连 → connection_handler 通知 ChatHub → hub 清理该客户端资源

---

### 4.6 数据结构示例
#### 4.6.1 protocol.rs
```rust
// protocol.rs
#[derive(Serialize,Deserialize)]
pub enum ClientRequest {
  Join  { room: String, name: String },
  Leave { room: String        },
  Message { room: String, text: String },
}

#[derive(Serialize,Deserialize,Clone)]
pub enum ServerEvent {
  UserJoined { room: String, name: String },
  UserLeft   { room: String, name: String },
  NewMessage { room: String, name: String, text: String, ts: u64 },
  RoomList   { rooms: Vec<String> },
}
```
#### 4.6.2 hub.rs
```rust
// hub.rs
enum HubCommand {
  JoinRoom { room: String, name: String, resp: mpsc::Sender<broadcast::Receiver<ServerEvent>> },
  SendMsg  { room: String, event: ServerEvent },
  LeaveRoom{ room: String, name: String },
}

struct ChatHub {
  rooms: HashMap<String, broadcast::Sender<ServerEvent>>,
  cmd_rx: mpsc::Receiver<HubCommand>,
}

impl ChatHub {
  async fn run(&mut self) {
    while let Some(cmd) = self.cmd_rx.recv().await {
      match cmd {
        HubCommand::JoinRoom{room,name,resp} => { … }
        HubCommand::SendMsg{room,event}   => { … }
        HubCommand::LeaveRoom{…}          => { … }
      }
    }
  }
}
```

---

#### 4.6.3 依赖示例（Cargo.toml）
```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.18"
tungstenite = "0.17"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.9"
```

---

## 5 可选扩展

- 持久化（聊天记录）→ Redis / SQLite / Postgres

- HTTP/REST 管理接口 → warp / axum

- TLS 加密 → tokio-native-tls 或 rustls

- Web 界面客户端 → 前端 + WebSocket

- 用户认证 → token / OAuth / JWT

- 以上架构已覆盖一个基本的异步多房间聊天室核心逻辑，后续可在此基础上逐步扩展持久化、权限、GUI 等功能。
