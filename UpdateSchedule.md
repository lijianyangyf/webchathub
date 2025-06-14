
# UpdateSchedule

本文件详细列出了将现有 Tokio 聊天室项目迭代为生产级服务的 **三大里程碑**、**子任务** 及 **实施方法**。所有路径均以当前源码目录结构为准。

---

## 总览

| 里程碑 | 目标 | 主要文件 | 关键成果 |
|-------|------|----------|----------|
| **M1** | 功能拓展 | `hub.rs`, `protocol.rs`, `listener.rs`, `ui.rs`, `config.rs` | 成员列表查询、历史消息回放、空房间 TTL |
| **M2** | 性能优化 | `hub.rs`, `memory_pool.rs (new)`, `lib.rs` | 内存池、分房间并发、负载均衡接口 |
| **M3** | 管理系统 | `server_manager.rs (new)`, `static/` | Web 管理面 API + React UI |

---

## 里程碑 1 – 功能拓展

### 1‑A 查询房间成员

| 步骤 | 实现要点 |
|------|---------|
| **协议** | 在 `protocol.rs`：<br>`enum ClientRequest { …, GetMembers { room: String } }`<br>`enum ServerEvent  { …, MemberList { room: String, members: Vec<String> } }` |
| **Hub** | `struct Room { …, members: HashSet<String> }`<br>在 `handle_join/leave` 中插入或移除成员；新增 `fn members(&self, room) -> Option<Vec<String>>`. |
| **Listener** | `match req` 分支追加 `ClientRequest::GetMembers`：查询 Hub 并 `ws_sender.send(MemberList)`. |
| **客户端 TUI** | `/members` 命令 -> 发送 `GetMembers`; `MemberList` 到达后在右侧弹窗展示。 |
| **测试** | 在 `tests/protocol.rs` 增加 roundtrip；集成测试：模拟两用户加入同房间，断言返回成员包含两人。 |

### 1‑B 历史消息环形缓冲

| 步骤 | 实现要点 |
|------|---------|
| **配置** | `config.rs` 新增 `history_limit: usize` (默认 100)。 |
| **Hub** | 为 `Room` 加 `history: VecDeque<ChatMessage>`；广播前 `push_back` 并维持容量；`join_room` 时批量复制历史到 `Vec<ChatMessage>` 返回。 |
| **协议** | `ServerEvent::HistoryBatch { messages: Vec<ChatMessage> }`. |
| **Listener** | 在 `JoinAck` 之后立刻发送 `HistoryBatch`. |
| **客户端** | 进入房间收到 `HistoryBatch` 先渲染；退出房间 `messages.clear()` 释放内存。 |
| **测试 & Bench** | 单元测试容量裁剪；Criterion 基准记录环缓冲吞吐。 |

### 1‑C 空房间 TTL (5 分钟)

| 步骤 | 实现要点 |
|------|---------|
| **配置** | `room_ttl_secs` (默认 300)。 |
| **Hub** | `RoomMeta { last_empty_at: Option<Instant> }`；<br>`interval(Duration::from_secs(60))` 周期检查并 `rooms.remove(room)`。 |
| **日志** | 移除房间时 `info!(room=%s, "room expired after TTL")`. |
| **测试** | 通过 `tokio::time::pause` + `advance` 模拟；覆盖过期删除。 |

---

## 里程碑 2 – 性能优化

### 2‑A 内存池

1. **新增文件 `src/memory_pool.rs`**  
   - 使用 `bytes::BytesMut` + `slab::Slab` 实现简单池。  
   - 公共接口：`alloc(size) -> PooledBytes`, `recycle(PooledBytes)`。
2. **集成点**  
   - Hub 广播时将 `ChatMessage` JSON 编码直接写入 `PooledBytes`; 推送完归还。  
   - 历史环缓冲改存 `PooledBytes`，减少重复分配。
3. **指标**  
   - `metrics` crate：记录 `alloc_total`, `recycle_total`，对比 before/after。

### 2‑B 分房间并发

| 步骤 | 细节 |
|------|------|
| **结构调整** | `ChatHub` 不再维护单一 `mpsc`; 改为 `HashMap<room, RoomHandle>`；<br>`RoomHandle` = `{ tx: mpsc::Sender<RoomCmd>, info: Arc<RoomMeta> }`. |
| **并发模型** | 每个房间 `tokio::spawn(room_task)`；广播 & 历史逻辑迁入该 task。 |
| **调度** | 启动时 `Runtime::new_multi_thread(worker_threads = num_cpus)`。 |
| **测试** | 使用 `cargo bench` 模拟 1k 房间并发推送，对比 QPS。

### 2‑C 负载均衡接口

- **trait RoomRouter** in `lib.rs`：`fn shard(room: &str) -> ShardId`.  
- 默认实现返回 `0`；保留字段 `shard_id` 在 `RoomMeta`，为未来多进程分片预埋。

---

## 里程碑 3 – 服务端管理系统

### 3‑A 新二进制 `server_manager.rs`

- 监听 `0.0.0.0:8080`，依赖 `warp` + `tokio::sync`.  
- 通过 `broadcast::Sender<AdminCmd>` 向 Hub 发送管理命令，使用 `oneshot` 回传结果。

### 3‑B REST API

| Endpoint | Method | 描述 |
|----------|--------|------|
| `/api/rooms` | GET | 列出房间、人数、TTL 倒计时 |
| `/api/rooms/{room}/kick/{user}` | POST | 踢出用户 |
| `/api/rooms/{room}/close` | POST | 强制关闭房间 |

### 3‑C 前端 (React + Vite)

- 放置于 `static/`; `npm run build` 输出至 `dist/`.  
- 使用 `fetch` 调用 REST；`Chart.js` 绘在线人数折线图；`lucide-react` 图标。

### 3‑D 认证

- `AUTH_TOKEN=<uuid>` 环境变量；所有管理 API 需 `Authorization: Bearer`.  
- 未来可切换到 `OIDC`.

---

## 时间线 (预估)

| 阶段 | 周期 | 主要交付 |
|------|------|----------|
| **M1** | Week 1‑3 | 功能全部可测，集成测试通过 |
| **M2** | Week 4‑6 | 基准测试 QPS ↑2‑3×，内存占用 ↓30% |
| **M3** | Week 7‑8 | Web 管理面上线，文档 & Docker 镜像发布 |

---

## 交付物

1. **Merge Requests**：每子任务独立 MR，带 CI 通过与测评报告。  
2. **文档**：更新 `README.md` & 新增 `docs/ARCHITECTURE.md`。  
3. **可执行产物**：`target/release/server`, `client`, `server_manager`.  
4. **Docker**：`docker-compose.yml` 支持一键启动全部组件。  
5. **监控 & 指标**：Prometheus exporter + Grafana dashboard JSON.

---

> 如需调整优先级或修改具体技术选型，请在评审会中反馈。我们会在迭代前锁定需求，保证交付节奏。
