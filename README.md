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

client 发送 ```rust { "type":"Join", "room":"rust", "name":"alice" } ```

connection_handler 将其封装成 ```rust HubCommand::JoinRoom(room, name, reply_tx) → hub_tx.send ```

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

## 6 实现规划

### 6.1 初始化工程 & 配置依赖

- 在项目根目录 cargo init --bin my_chat，或用 workspace 管理 server/client 两个 bin。
- 在 Cargo.toml 中添加 tokio、tokio-tungstenite、serde/serde_json、log/env_logger、等依赖；
- 建立基本目录结构：src/{bin/{server.rs,client.rs}, lib.rs, protocol.rs, hub.rs, error.rs, config.rs, server/, client/}
- 要点：
  - workspace 下可共享 lib；
  - 版本锁定、fmt/clippy 预配置。

### 6.2 共用模块：config.rs + error.rs

- config.rs：
  - 定义服务器端口、最大连接数、日志级别等，用 struct Config { port: u16, .. } 从 env 或 toml/yaml 文件读取（可用 config crate）
- error.rs：
  - 定义 enum ChatError（I/O、Serde、WebSocket、Channel 断连等）
  - 实现 From<io::Error>、From<tungstenite::Error>、Display
- 要点：提前统一错误类型，后续模块可 Result<T, ChatError>
- 
### 6.3 消息协议：protocol.rs

- 定义 ClientRequest、ServerEvent enum，派生 Serialize, Deserialize, Clone；
- 如需附加字段（timestamp、用户 ID），一并设计在此；
- 要点：
  - 保证兼容性：为 enum 版本升级预留字段；
  - 组织 message envelope，如 { “type”: “…”, “payload”: {...} }

### 6.4 聊天枢纽 ChatHub：hub.rs

- 定义 HubCommand（JoinRoom, SendMsg, LeaveRoom…)，包含必要数据和一个可选回执 channel；
- ChatHub 结构：
  - rooms: HashMap<String, broadcast::Sender<ServerEvent>>>
  - cmd_rx: mpsc::Receiver<HubCommand>
- 实现 async fn run(&mut self)：
- 收到 JoinRoom：如 room 不在 map，broadcast::channel(100) 新建 → clone sender → 发送 UserJoined；
- SendMsg：lookup sender，sender.send(event)；
- LeaveRoom：broadcast UserLeft，如房间空则删除 entry；
- 要点：
  - broadcast channel 容量够大，避免消息积压丢失；
  - 监听订阅者错误（RecvError::Lagged、Closed）；
  - Hub 启动后通过 tokio::spawn(hub.run())。

### 6.5 服务器监听与连接分发：server/listener.rs

- 在 server.rs 中读取 Config，初始化 log → 启动 hub task → 监听 TCP 或 WebSocket；
- 每接入一条连接：
  - WebSocket 握手（tokio-tungstenite）或直接 TCP；
  - 为该连接创建一个 mpsc::Sender<HubCommand>（clone hub_tx）；
  - spawn 一个 handle_connection(stream, hub_tx_clone) task；
要点：
  - 合理控制最大并发连接（Semaphore）；
  - 捕捉 handshake 错误并优雅返回；
  - 连接断开时通知 Hub 清理用户。

### 6.6 单连接处理逻辑：connection_handler

- 将 socket 拆成 reader/writer；
- 先读取客户端发送的 Join 指令，申请 broadcast::Receiver<ServerEvent> via resp channel；
- Spawn 两个子任务：
  - Reader task：
    - 循环 reader.next_message() → 反序列化成 ClientRequest → 转成 HubCommand → hub_tx.send().
    - Catch 解析/通道错误后退出，向 Hub 发送 LeaveRoom。
  - Writer task：
    - 从 Join 时得到的 broadcast::Receiver 循环 recv().await → 序列化 ServerEvent → writer.send()。
    - 通过 tokio::select! 监控 reader/writer 任何一方退出则关闭另一方。
- 要点：
  - 确保每个房间 Join 时都 clone 一个新的 Receiver；
  - 考虑心跳或 Ping/Pong 保活；
  - 处理 broadcast::RecvError（Lagged/Closed）

### 6.7 客户端 CLI：client/ui.rs + client.rs

- 在 client.rs 中读取配置（server 地址） → 建立 WebSocket/TCP 连接；
- 在 ui.rs：
  - stdin 读入 Task：解析 /join room name、/leave room、纯文本 → 封装 ClientRequest → 通过 socket 发送；
  - socket 读入 Task：反序列化 ServerEvent → 打印（可带颜色、高亮）；
- 要点：
  - 输入命令格式校验；
  - reconnect 逻辑（断线后自动重连）；
  - 用户体验：命令行提示符、历史记录（可用 rustyline）

### 6.8 主程序入口 wiring：bin/server.rs & bin/client.rs

- server.rs：
  - init logger、parse args/config → 构建 mpsc channels → spawn hub → listener.listen().await
- client.rs：
  - init logger → 建立连接 → 调用 ui::run(socket).await。
- 要点：优雅关机：捕捉 Ctrl-C，通知各 task shutdown
  
### 6.9 测试与调试

- 单元测试：
  - protocol 序列化测试；
  - hub.run() 模拟命令流；
- 集成测试：
  - 启动一个 server，多个 client 并发 join/send → 验证消息互通；
- 要点：使用 tokio::test 异步测试；引入 tracing 可视化执行流程

### 6.10 日志与监控

- 全局使用 log::{info,warn,error}
- env_logger 配置：按模块/级别过滤；
- 可选：用 prometheus + hyper 暴露 /metrics；
- 迭代扩展（可选）
- 持久化聊天记录 → 在 hub 或后置服务写入 SQLite/Redis；
- 用户认证 → 在 JoinRoom 前校验 token；
- HTTP/REST 管理界面 → 用 warp/axum 增加 admin API；
- TLS 加密 → tokio-rustls；
- Web 前端 → 改写成前端 SPA + WebSocket。