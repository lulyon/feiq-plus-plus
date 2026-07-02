# feiq++ Relay Server — 技术方案文档

> **状态**: ✅ 已实现（2026-06-30）。192 个测试全通过，前端 UI 已更新。
>
> **实现分支**: `main`
>
> **关键文件**:
> - `packages/feiq-relay/` — relay 服务器 crate
> - `packages/feiq-core/src/network/relay.rs` — relay 客户端 transport
> - `packages/feiq-core/src/engine/engine.rs` — hybrid 模式引擎
> - `packages/feiq-gui/src/components/SettingsDialog.tsx` — 模式选择 UI

## 决策记录

经过多轮讨论，确认以下方案：

| 决策 | 结论 |
|------|------|
| 服务器方案 | **自建 Rust relay server**（非 Centrifugo/Sockudo/XMPP） |
| 传输协议 | **自定义 JSON over WebSocket** (7 种消息类型) |
| 部署位置 | 公网云服务器 |
| 用户模式 | **三种可选**：LAN Only / Relay Only / Hybrid |
| 身份模型 | 简单房间名（无密码） |
| 离线消息 | 支持，服务器内存队列，24h TTL |
| 文件传输 | Phase 2，当前不做 |
| 分布式 | 不需要，单实例够用 |
| 加密 | ECDH+AES 端到端（relay 透明，不解析 IPMSG 载荷） |
| 兼容性 | IPMSG 协议不变，和飞秋/原版飞鸽互通不受影响 |

---

## 架构总览

```
                          ┌──────────────┐
                          │    Engine    │
                          │   contacts   │
                          │   messages   │
                          │   history    │
                          └──┬───┬───┬──┘
                             │   │   │
                  NetworkEvent (传输无关的统一事件)
                             │   │   │
              ┌──────────────┼───┼───┼──────────────┐
              │              │   │   │              │
      ┌───────▼──────┐ ┌────▼───▼───▼────┐  ┌──────▼──────┐
      │ UDP Transport │ │  Relay Client  │  │  (未来扩展)  │
      │ (已有)        │ │  (新增)        │  │  XMPP/MQTT   │
      │ manager.rs    │ │  relay.rs      │  │              │
      └───────┬──────┘ └───────┬────────┘  └──────────────┘
              │                 │
     UDP :2425 广播        WebSocket :2426
     局域网 P2P          ┌───▼──────┐
                        │  Relay   │
                        │  Server  │
                        │ (新增)   │
                        └──────────┘
                        公网云服务器
```

- 三个 transport 都通过 `NetworkEvent` 和 engine 通信
- engine 不关心消息来源，只需要 `source` 字段决定回复走哪个通道
- 用户启动时选择一种模式（或 Hybrid）

---

## 新增/修改文件明细

### 新增文件（3 个）

| 文件 | 行数(估) | 说明 |
|------|:---:|------|
| `packages/feiq-relay/Cargo.toml` | 20 | relay server crate 定义 |
| `packages/feiq-relay/src/main.rs` | 50 | CLI 入口 (clap: --bind, --port) |
| `packages/feiq-relay/src/server.rs` | 250 | WebSocket 服务器 + 房间管理 + 离线队列 |
| `packages/feiq-core/src/network/relay.rs` | 300 | Relay 客户端：WS 连接、协议、重连、NetworkEvent 生成 |

### 修改文件（8 个）

| 文件 | 改动量 | 说明 |
|------|:---:|------|
| `Cargo.toml` (root) | +2 行 | workspace members 加 `packages/feiq-relay` |
| `packages/feiq-core/Cargo.toml` | +3 行 | 加 `tokio-tungstenite`, `futures-util`, `base64` |
| `packages/feiq-core/src/network/mod.rs` | +5 行 | 导出 `relay` 模块；**移动 `NetworkEvent` enum 从 manager.rs 到此** |
| `packages/feiq-core/src/network/manager.rs` | -20 行 | `NetworkEvent` 定义移除，改 `use super::NetworkEvent` |
| `packages/feiq-core/src/protocol/types.rs` | +15 行 | `Fellow` 加 `source: PeerSource` 枚举 |
| `packages/feiq-core/src/engine/engine.rs` | +80 行 | hybrid 模式启动、联系人合并、消息路由 |
| `packages/feiq-core/src/storage/settings.rs` | +20 行 | `AppConfig` 加 `mode`, `relay_*` 字段 |
| `packages/feiq-app/src/commands.rs` | +10 行 | relay 配置命令（已有 `update_settings` 可复用） |
| `packages/feiq-gui/src/components/SettingsDialog.tsx` | +50 行 | 三种模式选择 + relay 配置 UI |

---

## Relay 协议详设（JSON over WebSocket）

### 消息类型一览

**客户端 → 服务器：**

| type | 用途 | 关键字段 |
|------|------|---------|
| `join` | 加入房间 | `room`, `name`, `host`, `version` |
| `leave` | 离开房间 | (无) |
| `send` | 单播给指定 peer | `to`(client_id), `ipmsg_cmd`, `ipmsg_data`(base64) |
| `broadcast` | 广播给房间所有人 | `ipmsg_cmd`, `ipmsg_data`(base64) |
| `ping` | 心跳 | (无) |

**服务器 → 客户端：**

| type | 用途 | 关键字段 |
|------|------|---------|
| `joined` | 加入确认 + 在线列表 | `client_id`, `peers[]` |
| `peer_online` | 新 peer 上线 | `peer{id, name, host, version}` |
| `peer_offline` | Peer 离线 | `peer_id` |
| `message` | 转发单播消息 | `from`(client_id), `from_name`, `ipmsg_cmd`, `ipmsg_data` |
| `broadcast` | 转发广播消息 | `from`, `from_name`, `ipmsg_cmd`, `ipmsg_data` |
| `offline_msgs` | 离线消息批量推送 | `messages[{from,from_name,ipmsg_cmd,ipmsg_data,timestamp}]` |
| `pong` | 心跳响应 | (无) |
| `error` | 错误 | `message` |

### 协议设计原则

1. **`ipmsg_data` 是 IPMSG 原始报文 `extra` 字段的 base64**。Relay 不解析、不修改、不加密。
2. **`ipmsg_cmd` 是 IPMSG 命令号**（`BR_ENTRY=1`, `SENDMSG=32` 等），server 用它做离线消息分类。
3. **Relay 永远不主动发 IPMSG 报文**。它只是转发隧道。
4. **E2E 加密完全透明**：Alice 加密 → base64 → relay 转发（看不懂）→ base64 解码 → Bob 解密。

### 完整交互序列

```
Alice                    Relay Server                 Bob
  │                          │                         │
  │── WS connect ──────────→│                         │
  │                          │                         │
  │── join{room:"office",   │                         │
  │    name:"Alice",...} ──→│                         │
  │                          │ 创建/加入房间            │
  │←─ joined{client_id:     │ 分配 UUID               │
  │    "a1", peers:[]} ─────│                         │
  │                          │                         │
  │                          │←─── join{room:"office", │
  │                          │     name:"Bob",...} ────│
  │                          │                         │
  │                          │──── joined{client_id:   │
  │                          │     "b2", peers:[       │
  │                          │     {id:"a1",name:      │
  │                          │      "Alice",...}]} ──→│
  │                          │                         │
  │←─ peer_online{peer:      │                         │
  │    {id:"b2",name:"Bob"}}─│                         │
  │                          │                         │
  │  [双方互相可见]            │                         │
  │                          │                         │
  │── send{to:"b2",          │                         │
  │   ipmsg_cmd:32,          │                         │
  │   ipmsg_data:"ZmVp..."}─→│                         │
  │                          │── message{from:"a1",    │
  │                          │   from_name:"Alice",    │
  │                          │   ipmsg_cmd:32,        │
  │                          │   ipmsg_data:"ZmVp..."}→│
  │                          │                         │
  │                          │←── send{to:"a1",...} ──│
  │←─ message{from:"b2",...}─│                         │
  │                          │                         │
  │                          │  [Bob 断开]              │
  │←─ peer_offline{          │                         │
  │    peer_id:"b2"} ───────│                         │
  │                          │                         │
  │── send{to:"b2",...} ──→│  [Bob 不在线]            │
  │                          │  入 offline_queue       │
  │                          │                         │
  │                          │←─── join... ────────────│  (Bob 重连)
  │                          │──── offline_msgs[...]──→│
  │                          │    + joined +           │
  │                          │    peer_online(Alice)   │
```

---

## Relay Server 详设 (`packages/feiq-relay`)

### CLI

```bash
feiq-relay --bind 0.0.0.0 --port 2426 --history-ttl 86400
```

### 数据结构

```rust
struct Server {
    rooms: Arc<Mutex<HashMap<String, Room>>>,
}

struct Room {
    name: String,
    clients: HashMap<String, ClientState>,  // client_id → ClientState
    offline_queue: Vec<PendingMessage>,
}

struct ClientState {
    id: String,       // UUID v4
    name: String,
    host: String,
    version: String,
    tx: UnboundedSender<String>,  // → WS write half
    connected_at: i64,
}

struct PendingMessage {
    to: String,
    from: String,
    from_name: String,
    ipmsg_cmd: u32,
    ipmsg_data: String,
    timestamp: i64,
}
```

### 核心处理流程

```
handle_message(ws_msg, client_id, room, server):
  match ws_msg.type:
    "join"    → handle_join
    "leave"   → handle_leave
    "send"    → handle_send
    "broadcast" → handle_broadcast
    "ping"    → reply pong

handle_join:
  1. 创建/获取房间
  2. 分配 UUID → 插入 clients
  3. 回复 joined{client_id, peers: 房间内其他人}
  4. 广播 peer_online 给其他人
  5. 推送 offline_msgs（发往该 client 的离线消息）
  6. 清空该 client 的离线队列

handle_send:
  1. 查找 to → 在线则转发 message
  2. 不在线则入 offline_queue

handle_broadcast:
  1. 转发给房间内所有其他人

handle_leave / WS disconnect:
  1. 从 clients 移除
  2. 广播 peer_offline 给其他人
```

### 离线消息 TTL 清理

```rust
// 每 60 秒清理一次过期消息
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let cutoff = now() - ttl;
        for room in rooms.iter_mut() {
            room.offline_queue.retain(|m| m.timestamp > cutoff);
        }
    }
});
```

---

## Relay Client 详设 (`feiq-core/src/network/relay.rs`)

### 数据结构

```rust
pub struct RelayClient {
    server_url: String,
    room: String,
    client_id: Option<String>,
    tx: UnboundedSender<String>,     // → WS write half
    event_tx: UnboundedSender<NetworkEvent>,
    shutdown: Arc<AtomicBool>,
}
```

### 公开方法

```rust
impl RelayClient {
    /// 连接 relay 服务器并加入房间
    pub async fn connect(
        url: &str, room: &str, name: &str, host: &str,
        version: &str, event_tx: UnboundedSender<NetworkEvent>,
    ) -> Result<Self>;

    /// 单播 IPMSG 报文到指定 peer
    pub async fn send_to(&self, peer_id: &str, cmd: u32, data: &[u8]) -> Result<()>;

    /// 广播 IPMSG 报文到房间所有人
    pub async fn broadcast(&self, cmd: u32, data: &[u8]) -> Result<()>;

    /// 接收循环（对标 NetworkManager::run()）
    pub async fn run(&self) -> Result<()>;

    /// 优雅关闭
    pub fn shutdown(&self);
}
```

### run() 接收循环

```
loop {
    ws_msg ← WebSocket 接收
    match ws_msg.type:
      "joined"    → 记录 client_id
                    peers[] → 逐个构造 Fellow → emit FellowOnline
      "peer_online" → peer → 构造 Fellow → emit FellowOnline
      "peer_offline" → peer_id → 查本地映射 → emit FellowOffline
      "message"    → base64_decode(ipmsg_data) → parse_raw → protocol_chain
                    → 构建 Post（from_name 来自 message 的 from_name 字段）
                    → emit Message(post)
      "offline_msgs" → messages[] → 逐条同上处理
      "error"      → emit Error
      "pong"       → 更新心跳时间戳
}
```

### 重连机制

```
连接断开:
  delay = 1s
  loop:
    try connect + join
    成功 → break
    失败 → delay *= 2 (max 60s)
    sleep(delay)
```

---

## Engine 改造 (`engine.rs`)

### 三种模式

```rust
// settings.rs
pub enum ConnectionMode {
    LanOnly,     // 纯局域网（默认，行为不变）
    RelayOnly,   // 纯 Relay
    Hybrid,      // LAN + Relay 同时启用
}
```

### 启动流程

```rust
pub async fn start(&mut self) -> Result<()> {
    let config = &self.config;

    // 始终启动 UDP（RelayOnly 除外）
    if config.mode != ConnectionMode::RelayOnly {
        self.start_udp().await?;  // 现有逻辑
    }

    // 按需启动 Relay
    if config.mode != ConnectionMode::LanOnly
        && config.relay_enabled
    {
        self.start_relay().await?;
    }
}
```

### 消息发送路由

```rust
async fn send_text_to(&self, ip: &str, text: &str) -> Result<()> {
    let fellow = self.contacts.find_by_ip(ip).ok_or("contact not found")?;

    // 构建 IPMSG 报文（和现在一样）
    let data = build_text_message(self.packet_id(), &self.config.name,
                                   &self.config.host, &self.version, text);

    match fellow.source {
        PeerSource::LanPeer => {
            // 走 UDP（现有逻辑）
            self.network.send_to(&fellow.ip, fellow.port, &data).await
        }
        PeerSource::RelayPeer(ref peer_id) => {
            // 走 Relay
            self.relay_client.send_to(peer_id, IPMSG_SENDMSG | IPMSG_SENDCHECKOPT, &data).await
        }
    }
}
```

### 联系人合并 (Hybrid 模式)

```rust
// 同一台机器可能同时从 LAN 和 Relay 被发现
// ContactBook.upsert() 已有 MAC-based 去重逻辑（find_same）

// 当 LAN 发现 Bob (MAC=BBCCDD):
//   → 新建 Fellow{mac:BBCCDD, source:LanPeer, ip:"192.168.1.8", port:2425}

// 当 Relay 也发现 Bob (MAC=BBCCDD):
//   → find_same(BBCCDD) 找到已有
//   → update: 追加 source, 但不覆盖 LAN 的 ip/port
//   → Fellow{mac:BBCCDD, sources:[LanPeer, RelayPeer("b2")], ip:"192.168.1.8"}

// 发消息时：
//   preferred_source() → LanPeer 优先（更低延迟）
//   如果 LAN 不可达（peer 离线并只出现在 relay），走 relay
```

### `Fellow` 类型改动

```rust
// protocol/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerSource {
    LanPeer,
    RelayPeer(String),  // relay client_id
}

// Fellow 新增字段
pub struct Fellow {
    // ... 现有字段 ...
    #[serde(default)]
    pub source: PeerSource,  // 默认 LanPeer
}
```

---

## 前端改动

### 设置面板 UI

```
┌─ Connection ────────────────────────┐
│  ○ 局域网直连 (默认)                 │
│     ┌──────────────────────────┐    │
│     │ Port: [2425        ]     │    │
│     │ Custom IPs: [192.168.]   │    │
│     └──────────────────────────┘    │
│                                      │
│  ○ Relay 中转                       │
│  ● Hybrid (推荐)                    │
│     ┌──────────────────────────┐    │
│     │ Server: [ws://x.x:2426]   │    │
│     │ Room:   [my-office  ]     │    │
│     └──────────────────────────┘    │
└──────────────────────────────────────┘
```

### 联系人显示

- Relay peer 的 IP 列显示 `☁️ Relay`，hover 显示 client_id
- LAN peer 显示正常 IP（现有行为）

---

## 实现顺序

### Step 1: 轻量重构（前置）

- [x] 将 `NetworkEvent` enum 从 `manager.rs` 移到 `network/mod.rs`
- [x] `Fellow` 加 `source: PeerSource` 字段（默认 `LanPeer`）
- [x] 跑 `cargo test --workspace` 确认无回归

### Step 2: Relay Server（独立可测）

- [x] 创建 `packages/feiq-relay` crate
- [x] 实现 `server.rs`：WS 监听、房间管理、消息路由、离线队列、TTL 清理
- [x] CLI 参数 `--bind`, `--port`
- [x] 用 `websocat` 手动测试

```bash
# 测试
cargo run --package feiq-relay -- --port 2426
# 另一个终端:
websocat ws://localhost:2426
# 输入: {"type":"join","room":"test","name":"Alice","host":"mbp","version":"..."}
# 预期收到: {"type":"joined",...}
```

### Step 3: Relay Client（单元测试）

- [x] 新增 `packages/feiq-core/src/network/relay.rs`
- [x] WebSocket 连接、join、send_to、broadcast
- [x] 接收循环 + NetworkEvent 生成
- [x] 自动重连（指数退避）
- [x] 单元测试（连接真实 relay server 做集成测试）

### Step 4: Engine 集成

- [x] `AppConfig` 加 `mode`, `relay_enabled`, `relay_server_url`, `relay_room`
- [x] `Engine::start()` 集成 relay 启动逻辑
- [x] 联系人合并（hybrid 模式 MAC 去重）
- [x] 消息路由（`source` 字段选通道）

### Step 5: 前端

- [x] `SettingsDialog` 三种模式 + relay 配置
- [x] Sidebar Relay peer 图标标记
- [x] 设置保存/加载 relay 配置

### Step 6: 集成测试

- [x] 启动 relay server
- [x] 两个 feiq++ 实例 relay 模式互通
- [x] 离线消息测试
- [x] 断线重连测试
- [x] Hybrid 模式 LAN + Relay 双通道测试

---

## 验证方案

```bash
# 1. 编译
cargo build --workspace --release

# 2. 启动 relay
./target/release/feiq-relay --port 2426 &

# 3. 启动 Alice（relay 模式）
FEIQ_NAME=Alice cargo run --package feiq-app
# UI: 选择 Relay, ws://localhost:2426, room=test

# 4. 启动 Bob（relay 模式）
FEIQ_NAME=Bob FEIQ_PORT=2426 cargo run --package feiq-app
# UI: 选择 Relay, ws://localhost:2426, room=test

# 5. 验证
#   ✓ Alice 侧边栏出现 Bob（带 ☁️ 标记）
#   ✓ Alice → Bob 消息送达
#   ✓ Bob → Alice 消息送达
#   ✓ 关闭 Bob → Alice 侧边栏 Bob 变灰（离线）
#   ✓ Alice 给离线 Bob 发消息
#   ✓ Bob 重新启动 → 收到离线消息

# 6. Hybrid 验证
#   ✓ 同 LAN 的两个实例能以 LAN 方式通信（和现在行为一样）
#   ✓ 同时连着 relay，不会出现重复联系人

# 7. 回归测试
cargo test --workspace
```

---

## 决策确认（已确认）

| 问题 | 结论 |
|------|------|
| TLS/WSS | MVP 明文 `ws://`，不加密 |
| 认证 | 不认证。知道房间名就能加入 |
| 多房间 | 不支持。一个实例一个房间 |
| 离线消息 TTL | Server 端 `--history-ttl` 参数，默认 86400 秒 |
| 部署 | 单二进制 `cargo build --release`，scp 到服务器直接跑 |

---

## 依赖汇总

| Crate | 版本 | 用于 |
|-------|------|------|
| `tokio-tungstenite` | 0.24 | WebSocket client + server |
| `futures-util` | 0.3 | Stream combinators |
| `base64` | 0.22 | IPMSG 载荷编解码 |
| `uuid` | 1 (server only) | Client ID 生成 |
| `clap` | 4 (server only) | CLI 参数解析 |

无新增前端依赖。

## 总代码量估算

| 模块 | 行数 |
|------|:---:|
| Relay Server (`feiq-relay`) | ~320 |
| Relay Client (`network/relay.rs`) | ~300 |
| Engine 改动 | ~80 |
| Settings 改动 | ~20 |
| 前端改动 | ~50 |
| 重构（NetworkEvent 移动等） | ~30 |
| 测试代码 | ~150 |
| **合计** | **~950 行** |
