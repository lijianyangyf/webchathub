# my_chat

> _my_chat_ 是用 Rust 编写的轻量级多房间 WebSocket 聊天系统，包含 **无依赖服务器** + **跨平台 TUI 客户端**，零配置即可运行。

## ✨ 特色

- **Tokio + tungstenite** 异步栈，单机即可支撑数千长连接
- **多房间 + 历史回放**：每个房间保留最近 _N_ 条消息，用户加入即重播
- **房间 TTL**：长时间无人自动卸载，释放资源
- **内存池**：二进制帧复用，减少重复分配
- **Slash 命令 TUI**：/join /leave /rooms /members 等一键操作
- **纯 JSON 协议**，易于与浏览器或其它语言集成

## 目录结构

```text
my_chat/
├─ Cargo.toml               # 依赖与构建元数据
├─ src/
│  ├─ bin/                  # 可执行入口
│  │  ├─ server.rs          # 聊天服务器
│  │  └─ client.rs          # TUI 客户端
│  ├─ server/               # 服务器内部实现
│  │  ├─ listener.rs
│  │  └─ mod.rs
│  ├─ client/               # 客户端 UI 与辅助
│  │  ├─ ui.rs
│  │  └─ mod.rs
│  ├─ hub.rs                # ChatHub：房间路由/调度
│  ├─ room.rs               # 单个房间状态机
│  ├─ protocol.rs           # JSON 消息定义
│  ├─ memory_pool.rs        # Bytes 池
│  ├─ config.rs             # 环境变量配置
│  └─ lib.rs                # crate 导出
```

## 快速开始

> 依赖：**Rust 1.76+**；Linux / macOS / Windows 皆通过 CI 验证。

```bash
# 克隆
git clone git@github.com:lijianyangyf/webchathub.git
cd webchathub

# Release 构建
cargo build --release
```

### 1. 启动服务器

```bash
# 默认监听 0.0.0.0:9000
cargo run --bin server
```

可用环境变量见下文 [配置](#配置)。

### 2. 启动本地 TUI 客户端

```bash
cargo run --bin client          # 连接 ws://127.0.0.1:9000
# 或指定 ws URL
cargo run --bin client ws://1.2.3.4:9000
```

### 3. Slash 命令

| 命令 | 说明 |
|------|------|
| `/join <room> <name>` | 加入 / 创建房间 |
| `/leave` | 离开当前房间 |
| `/rooms` | 获取房间列表 |
| `/members` | 查看当前房间成员 |

错误格式会提示：`usage: /join <room> <name> | /leave | /rooms | /members`

## 配置

服务器通过 **环境变量** 读取配置，缺省时使用以下默认值：

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SERVER_ADDR` | 字符串 | `0.0.0.0:9000` | WebSocket 监听地址 |
| `LOG_LEVEL`   | 字符串 | `info`         | 日志级别 (`trace` ~ `error`) |
| `HISTORY_LIMIT` | usize | `100`         | 每房间历史条数 |
| `ROOM_TTL_SECS` | u64   | `300`         | 房间空闲回收 TTL（秒） |

示例：

```bash
SERVER_ADDR=127.0.0.1:8080 LOG_LEVEL=debug cargo run --bin server
```

## 协议

所有消息均为 **UTF‑8 JSON** 文本帧。

### Client → Server `ClientRequest`

```jsonc
// 加入房间
{ "Join": { "room": "rust", "name": "alice" } }

// 发送消息
{ "Message": { "room": "rust", "text": "hello" } }

// 其它：Leave | RoomList | Members
```

### Server → Client `ServerEvent`

```jsonc
// 普通聊天
{ "NewMessage":
  { "room": "rust", "name": "alice", "text": "hello", "ts": 1718620680000 } }

// 系统事件
{ "UserJoined": { "room": "rust", "name": "bob" } }
{ "UserLeft":   { "room": "rust", "name": "bob" } }

// 房间 & 成员列表
{ "RoomList":   { "rooms": ["rust","golang"] } }
{ "MemberList": { "room": "rust", "members": ["alice","bob"] } }
```

> **时间戳** `ts` 为毫秒级 UTC Unix epoch。

## 架构概览

```text
┌──────────┐      HubCmd       ┌──────────────┐  RoomCmd  ┌───────────┐
│ listener │ ───────────────▶ │   ChatHub     │──────────▶│   Room    │
│  (WS)    │ ◀─────────────── │ (router/task) │ ◀─────────│ (state)   │
└──────────┘   broadcast bytes └──────────────┘   events   └───────────┘
                     ▲                                │
                     └─ history replay / members list ┘
```

* **listener** 每条 TCP 连接升级为 WebSocket，完成握手 & 协议解析  
* **ChatHub** 维护房间 Map，负责路由指令到对应房间  
* **Room** 保存成员、历史及 TTL 计时器，广播事件给所有订阅者  
* **memory_pool** 集中管理 `BytesMut`，降低频繁分配 / 复制开销  

## 开发 & 贡献

```bash
# 单元 / 集成测试
cargo test

# 自动格式化
cargo fmt
```

欢迎 PR / Issue！如需 Roadmap 与贡献指南，可在此 README 下方补充。

## License

本仓库默认采用双许可证 **MIT OR Apache‑2.0**。请按团队合规要求调整。
