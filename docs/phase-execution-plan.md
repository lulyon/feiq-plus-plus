# feiq++ Phase 2-6 — 详细执行计划

## 实现状态

**最后更新**: 2026-07-02

| Phase | 状态 | 关键交付物 |
|-------|:---:|-----------|
| Phase 2 — 文件传输 | ✅ 100% | 文件引擎、TCP、进度面板、拖拽 |
| Phase 3 — 聊天记录+用户管理 | ✅ 100% | 无限滚动、搜索、分组、别名 |
| Phase 4 — 群聊+黑名单 | ✅ 100% | P2P 群组分发、黑名单过滤 |
| Phase 5 — 加密+密封+主题 | ✅ 100% | ECDH+AES-GCM、密封消息、主题 |
| Phase 6 — 功能增强 | ✅ 100% | 14 项功能（详见下方） |
| Phase 7 — 遗留加密 | ⏸️ 延后 | RSA/RC2/Blowfish 互操作 |

**总测试**: 200 Rust + 18 TS = 218，全部通过  
**IPC 命令**: 36  
**前端组件**: 11 / **Zustand stores**: 5

---

> 基于对当前代码库的逐行审计 + 原 feiq C++ 项目 (`/Users/zhihu/code/feiq/`) 的交叉参考。
> Phase 2-5 保留了原始执行计划作为历史记录。Phase 6 记录了实际实现的 14 项功能增强。

---

## Phase 6 — 功能增强（全部完成）

Phase 6 源于 `docs/UNIMPLEMENTED_FEATURES.md`，按优先级分 5 批实现：

```
Priority  High ─  Med ─  Low
          │         │        │
BR_ABSENCE│Relay DL │Typing  │Theme skin
Pinyin   │SpeedLim │Avatar  │Font custom
          │FileShare│Group   │Doodle tool
          │         │Stealth │Legacy cmds
```

**关键设计决策**:
- 中继文件下载: Binary WebSocket tunnel（新 relay 协议消息类型）
- 文件共享鉴权: IPMSG 原生 `IPMSG_PASSWORDOPT` 标志位
- 隐身模式: 全局不可见（不广播 BR_ENTRY）
- 头像: 网络传输（扩展 IPMSG 自定义命令）

### 实现顺序

```
Phase A (快速见效) ─ 3/3 ✅
├── F1. BR_ABSENCE 处理器
├── F2. 拼音搜索
└── F13. READMSG/ANSREADMSG 处理

Phase B (核心功能) ─ 3/3 ✅
├── F3. 输入状态提示
├── F5. 文件共享（密码保护）
└── F9a. 群组文件共享

Phase C (重大功能) ─ 3/3 ✅
├── F4. 中继文件下载
├── F7. 个人头像
└── F8. 隐身模式

Phase D (打磨) ─ 3/3 ✅
├── F6. 传输限速
├── F10. 完整主题皮肤
└── F14. 群组权限控制

Phase E (锦上添花) ─ 3/3 ✅
├── F11. 字体自定义
├── F12. 涂鸦工具
└── F9b. 群组公告
```

---

### F1: BR_ABSENCE 处理器（名称/状态变更检测）

**优先级**: High | **复杂度**: Low | **文件**: 4

当旧版飞秋用户改名或离开时，会广播 `IPMSG_BR_ABSENCE` (0x04)。此前 feiq++ 忽略此包，导致联系人名称从不更新。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 协议 | `protocol/parser.rs` | `RecvBrAbsence` 处理器 — 从 extra 提取新名称，更新 `post.from.name` |
| 网络 | `network/manager.rs` | `handle_packet()` 中 BR_ABSENCE → emit `NetworkEvent::FellowOnline` |
| 引擎 | `engine/engine.rs` | 触发现有联系人 upsert 路径更新名称/状态 |
| 常量 | `protocol/constants.rs` | `IPMSG_BR_ABSENCE` 已定义 |

---

### F2: 拼音联系人搜索

**优先级**: High | **复杂度**: Low-Med | **文件**: 3

`ContactBook::search()` 声称支持拼音，但实际只匹配 name/IP/host。输入 "zs" 无法找到 "张三"。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 依赖 | `feiq-core/Cargo.toml` | 添加 `pinyin = "0.10"` |
| 模型 | `model/contacts.rs` | `search()` 增加首字母 + 全拼匹配，`pc_name` 纳入搜索字段 |
| 前端 | `components/Sidebar.tsx` | 客户端过滤增加 `host`、`pc_name` 字段 |

---

### F3: 输入状态提示

**优先级**: Med | **复杂度**: Med | **文件**: 11

`IPMSG_INPUTING` (0x79) 和 `IPMSG_INPUT_END` (0x7A) 常量存在但无处理逻辑。旧版飞秋支持此功能。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 类型 | `protocol/types.rs` | `Content::Typing { is_typing: bool }` |
| 协议 | `protocol/parser.rs` | `RecvInputing` + `RecvInputEnd` 处理器 |
| 事件 | `engine/events.rs` | `FrontendEvent::TypingIndicator` |
| 引擎 | `engine/engine.rs` | `send_typing_to()` 方法，UDP 发送 INPUTING/INPUT_END |
| IPC | `commands.rs` | `send_typing` 命令 |
| 事件转发 | `events.rs` | `typing-indicator` Tauri 事件 |
| 前端 | `App.tsx` | 监听 typing-indicator 事件 |
| 前端 | `InputArea.tsx` | onChange debounce → 3s 无输入自动发送 INPUT_END |
| 前端 | `ChatPanel.tsx` | "is typing..." 动画提示，5s auto-clear |
| State | `typingStore.ts` (新) | Zustand store，5s 超时自动清除 |

---

### F4: 中继文件下载（Binary WebSocket Tunnel）

**优先级**: Med | **复杂度**: High | **文件**: 8

通过中继连接的节点无法传输文件，`commands.rs:396` 有守卫直接拒绝中继节点下载。

**设计决策**: Binary WebSocket Tunnel —— 新增 relay 协议 `FileStart`/`FileEnd` JSON 消息 + 二进制 `[8B file_id BE][chunk data]` 帧。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| Relay 服务端 | `relay/server.rs` | `FileStart`/`FileEnd` ClientMessage + ServerMessage，`Room.file_transfers: HashMap<(sender,file_id), receiver>`，二进制帧解析 file_id → 查表 → 转发，断开/离开时清理 |
| Relay 客户端 | `network/relay.rs` | `ServerMsg::FileStart/FileEnd`，`send_file_start/send_file_chunk/send_file_end` 方法，`ws_write_tx` 从 `String` 升级为 `Message` 支持 Binary 帧，接收二进制帧 → emit `NetworkEvent::FileChunk` |
| 网络事件 | `network/mod.rs` | `FileStartViaRelay`、`FileEndViaRelay`、`FileChunk` 三个变体 |
| 引擎 | `engine/engine.rs` | `send_file_via_relay`（push 模式：FileStart → 64KB chunk 循环 → FileEnd），`relay_client()` 访问器，事件处理匹配臂 |
| 命令 | `commands.rs` | 移除 relay 守卫，中继节点发送 GETFILEDATA 请求，引擎处理二进制响应 |
| 集成测试 | `relay/tests/` | `test_file_transfer_via_relay` — Alice→Bob 完整文件传输流程 |

---

### F5: 文件共享（密码保护）

**优先级**: Med | **复杂度**: Med | **文件**: 7

后端骨架存在（`handle_file_share_request`、`list_directory`、`shared_dir_password` 配置），但方法从未被调用，密码检查未接入事件循环，无前端 UI。

**设计决策**: IPMSG 原生 `IPMSG_PASSWORDOPT` 标志位 —— 密码通过 GETDIRFILES 的 extra 字段传输，与旧版飞秋兼容。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 引擎 | `engine/engine.rs` | 提取 `check_file_share_request(config, request)` 独立函数，替换 UDP 事件循环内联处理器，密码从 `IPMSG_PASSWORDOPT` + extra 解析 |
| 协议 | `engine/engine.rs` | `build_get_dir_files(packet_no, password)` 构建带密码的 GETDIRFILES 请求 |
| IPC | `commands.rs` | `browse_shared_folder(ip, password?)` 命令 |
| 前端 | `RemoteFileBrowser.tsx` (新) | 模态对话框：发送请求、密码输入、请求状态提示 |
| 前端 | `ChatPanel.tsx` | 聊天头部 "Browse Files" 按钮 |
| 测试 | `engine.rs` | 4 个新增测试：无密码/密码错误/正确密码/无密码配置 |

---

### F6: 传输限速

**优先级**: Med | **复杂度**: Low-Med | **文件**: 4

文件传输占满带宽，无任何速率限制。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 配置 | `storage/settings.rs` | `upload_speed_limit_kbps: u32`、`download_speed_limit_kbps: u32`（0 = 不限） |
| 网络 | `network/tcp.rs` | `send_file`/`recv_file` 新增 `speed_limit_bytes_per_sec: Option<u64>` 参数，每个 chunk 后 `Instant::now()` 计算实际耗时，若快于预期则 `sleep(diff)` |
| 引擎 | `engine/engine.rs` | TCP accept 任务捕获 `config`，将 `upload_speed_limit_kbps * 1024` 传入 `send_file` |
| 命令 | `commands.rs` | 下载前读取 `download_speed_limit_kbps`，传入 `recv_file` |
| 测试 | `network/tcp.rs` | 2 个新增测试：128KB@128KB/s 限速验证(≥400ms)、不限速对比 |

---

### F7: 个人头像（网络传输）

**优先级**: Med | **复杂度**: Med | **文件**: 7

无头像功能。文档声称"数据模型字段存在"但代码中无。

**设计决策**: 网络传输 —— 新增自定义 `IPMSG_GETAVATAR` (0x75) / `IPMSG_SENDAVATAR` (0x76) 命令，头像以 SHA256 哈希标识，通过 extra 字段 `hash:base64_data` 传输。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 类型 | `protocol/types.rs` | `Fellow.avatar_hash: String` |
| 数据库 | `storage/history.rs` | `contact_meta.avatar_hash` 列 + 迁移，`save_avatar_hash()` |
| 配置 | `storage/settings.rs` | `AppConfig.avatar_path: String` |
| 常量 | `protocol/constants.rs` | `IPMSG_GETAVATAR = 0x75`, `IPMSG_SENDAVATAR = 0x76` |
| 协议 | `protocol/parser.rs` | `RecvGetAvatar` + `RecvSendAvatar` 处理器 |
| 引擎 | `engine/engine.rs` | `handle_avatar_request`（读取文件→SHA256→发送 SENDAVATAR），`handle_avatar_response`（解析哈希→保存 contact_meta），消息处理中检测 avatar 内容并过滤 |
| IPC | `commands.rs` | `set_avatar(path)`（≤100KB PNG/JPEG 校验），`get_avatar(ip)` |

---

### F8: 隐身模式（全局不可见）

**优先级**: Med | **复杂度**: High | **文件**: 5

无隐身功能。旧版飞秋支持基于 BR_ABSENCE 的状态变更。

**设计决策**: 全局不可见 —— 启用时停止广播 BR_ENTRY，不应答 BR_ENTRY 的 ANSENTRY。仍可主动发消息，对方可回复（单向可见性）。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 配置 | `storage/settings.rs` | `AppConfig.stealth_mode: bool` |
| 引擎 | `engine/engine.rs` | `start_udp()` 跳过初始广播，周期广播跳过，`handle_network_event` 中 BR_ENTRY → 不返回 ANSENTRY reply，`set_stealth_mode()` 方法 |
| IPC | `commands.rs` | `set_stealth_mode(enabled)` 命令 |

---

### F9a: 群组文件共享

**优先级**: Low | **复杂度**: Med | **文件**: 3

群文本聊天工作正常，文件共享未实现。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 引擎 | `engine/engine.rs` | `send_file_to_group()` — 复用 `send_text_to_group` 模式，遍历成员调用 `send_file_to` |
| IPC | `commands.rs` | `send_group_file` 命令 |
| 前端 | `ChatPanel.tsx` | GroupChatPanel 添加文件按钮 |

---

### F9b: 群组公告

**优先级**: Low | **复杂度**: Med | **文件**: 4

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 数据库 | `storage/history.rs` | `group_announcements` 表（id, group_name, content, sender_ip, created_at），`save_announcement()`/`get_announcements()` |
| 引擎 | `engine/engine.rs` | `send_announcement_to_group(group_name, content)` — P2P 分发 `[announce:groupname]` 前缀消息 + DB 保存，`get_announcements()` 查询 |
| IPC | `commands.rs` | `send_group_announcement`、`get_group_announcements` |

---

### F10: 完整主题皮肤

**优先级**: Low | **复杂度**: Med | **文件**: 4

只有亮/暗/自动三种硬编码主题。无自定义配色或背景图片。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 配置 | `storage/settings.rs` | `CustomTheme` 结构体（bg, surface, primary, text, bubble_sent, bubble_recv），`AppConfig.custom_theme` |
| 前端 | `App.tsx` | 主题 useEffect 增加 `custom` 分支：逐字段注入 CSS 变量（`--color-bg`/`--color-surface`/`--color-primary`/`--color-text`/`--color-bubble-sent`/`--color-bubble-recv`） |
| CSS | `index.css` | 已有 `@theme` 定义，runtime `setProperty` 覆盖 |

---

### F11: UI 字体自定义

**优先级**: Low | **复杂度**: Low | **文件**: 3

字体硬编码为系统字体栈。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 配置 | `storage/settings.rs` | `font_family: String`、`font_size: u32`（默认 14） |
| CSS | `index.css` | `body { font-family: var(--font-family, ...); font-size: var(--font-size, 14px); }` |
| 前端 | `App.tsx` | 主题 useEffect 中注入 `--font-family` + `--font-size` |

---

### F12: 涂鸦/绘图工具

**优先级**: Low | **复杂度**: High | **文件**: 2

截图标注已移除（commit 85e23b4），无绘图能力。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 前端 | `DoodleDialog.tsx` (新) | HTML5 Canvas 自由绘制（560×420），工具栏：画笔/橡皮擦、10 色调色板、线宽滑块(1-20)、撤销(20 步历史)、发送按钮 |
| 前端 | `InputArea.tsx` | 画笔按钮触发 DoodleDialog |

---

### F13: READMSG/ANSREADMSG 处理

**优先级**: Low | **复杂度**: Low | **文件**: 2

feiq++ 已发送 READMSG 但未处理入站 READMSG/ANSREADMSG。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 协议 | `protocol/parser.rs` | `RecvReadMessage`（IPMSG_RECVMSG 已读回执）、`RecvReadMsgSealed`（IPMSG_READMSG 密封消息已读）、`RecvAnsReadMsg`（IPMSG_ANSREADMSG 已读确认） |
| 引擎 | `engine/engine.rs` | 处理已读回执 → emit `MessageReadConfirmed` 事件更新 UI |

---

### F14: 群组权限控制

**优先级**: Low | **复杂度**: Med | **文件**: 5

无权限模型。任何成员可发消息，无群主/管理员概念。

**实际实现**:
| 层 | 文件 | 改动 |
|----|------|------|
| 数据库 | `storage/history.rs` | `groups_info` 新增 `owner_ip` + `settings` (JSON) 列，DB迁移 v2→v3，`save_group_with_owner()`/`delete_group()`/`update_group_settings()`/`get_group_info()` |
| 引擎 | `engine/engine.rs` | `create_group_with_owner()`/`delete_group()`/`update_group_settings()` |
| IPC | `commands.rs` | `delete_group_cmd` |

---

### 新增 IPC 命令一览

Phase 6 新增 10 个 IPC 命令，总计 36 个：

| 命令 | 功能 | 所属特性 |
|------|------|---------|
| `send_typing` | 发送输入状态 | F3 |
| `send_read_receipt` | 发送已读回执 | F13 |
| `browse_shared_folder` | 浏览远程共享目录 | F5 |
| `send_group_file` | 群文件发送 | F9a |
| `send_group_announcement` | 群公告发送 | F9b |
| `get_group_announcements` | 群公告查询 | F9b |
| `set_avatar` | 设置头像 | F7 |
| `get_avatar` | 获取头像哈希 | F7 |
| `set_stealth_mode` | 隐身模式开关 | F8 |
| `delete_group_cmd` | 删除群组 | F14 |

---

### Phase 6 新增依赖

| Crate | 版本 | 用途 |
|-------|------|------|
| `pinyin` | 0.10 | 汉字→拼音转换（搜索） |
| `sha2` | 0.10 | SHA256 头像哈希 |
| `hex` | 0.4 | 哈希十六进制编码 |

---

### Phase 6 新增测试

| 测试 | 文件 | 验证 |
|------|------|------|
| `test_check_file_share_request_password_required_but_none` | engine.rs | 无密码拒绝 |
| `test_check_file_share_request_wrong_password` | engine.rs | 错误密码拒绝 |
| `test_check_file_share_request_correct_password` | engine.rs | 正确密码放行 |
| `test_check_file_share_request_no_password_when_unconfigured` | engine.rs | 未配置密码放行 |
| `test_send_file_with_speed_limit` | tcp.rs | 128KB@128KB/s ≥400ms |
| `test_send_file_unlimited` | tcp.rs | 不限速足够快 |
| `test_file_transfer_via_relay` | integration_test.rs | FileStart→Binary→FileEnd |

---

## Phase 2 — 文件传输 + 表情（已完成）

### 架构缺口

1. `engine.rs:67` `file_tasks` HashMap 是死代码 — 从未被填充
2. `IPMSG_GETFILEDATA` 协议处理完全缺失 — parser 中无 handler，manager 中无 dispatch
3. `FileServer.accept()` 从未被调用 — TCP 文件传输监听器闲置
4. 前端 App.tsx 只监听 `contact-update` / `new-message`，缺少 `file-progress` / `file-state-changed`

### 2.1 — 引擎层：文件任务管理

**改动文件**: `engine/engine.rs`, `protocol/types.rs`, `protocol/parser.rs`, `network/manager.rs`

```
问题：收到 Content::File 后，引擎只做过滤，不创建 FileTaskHandle
方案：
  1. types.rs: FileContent 加 local_task_id: Option<u64>
  2. engine.rs: handle_network_event 收到 File → 创建 FileTaskHandle → 存入 file_tasks
     → emit FileStateChanged{NotStart}
  3. parser.rs: 新增 RecvGetFileData handler，解析 IPMSG_GETFILEDATA (0x60)
  4. manager.rs: handle_packet 为 GETFILEDATA dispatch GetFileData 事件
  5. manager.rs: 启动 FileServer accept 循环 (call accept() in a spawned task)
  6. engine.rs: 暴露 file_tasks 访问器 (register/get/cancel)
```

### 2.2 — Tauri 命令

**改动文件**: `commands.rs`, `main.rs`

```
download_file(task_id, save_path) → 接受 TCP → recv_file → 进度回调
cancel_task(task_id)              → request_cancel → emit FileStateChanged{Canceled}
send_file(ip, file_path)          → create_file_content → build_file_message → UDP 通知
                                       → 后台 accept TCP → send_file
```

### 2.3 — 前端文件传输面板

**改动文件**: `stores/fileTransferStore.ts` (新), `App.tsx`, `components/FileTransferPanel.tsx` (新), `ChatPanel.tsx`

```
1. fileTransferStore.ts: 按 task_id 追踪传输状态/进度/方向
2. App.tsx: 监听 file-progress / file-state-changed 事件 → 更新 store
3. FileTransferPanel.tsx: 可折叠面板，进度条 + 取消按钮
4. ChatPanel.tsx: 集成 FileTransferPanel
```

### 2.4 — 前端可点击文件气泡

**改动文件**: `MessageBubble.tsx`, `stores/messageStore.ts`

```
1. messageStore.ts: Content 接口加 local_task_id / file_id / packet_no
2. MessageBubble.tsx: 文件气泡加 onClick → save() 对话框 → invoke("download_file")
3. normalizeContent 提取 local_task_id
```

### 2.5 — 前端拖拽发送

**改动文件**: `InputArea.tsx`

```
1. Tauri v2 内置 drag-drop: getCurrentWebview().onDragDropEvent(...)
2. track isDraggingOver 状态 → 视觉反馈（虚线边框 overlay）
3. drop 事件: 遍历 paths → invoke("send_file", { ip, filePath })
4. 附件按钮: @tauri-apps/plugin-dialog open() → invoke("send_file")
```

### 预计改动量

| 层 | 文件数 | 新增行 |
|----|--------|--------|
| Rust 引擎 + 协议 | 5 | ~200 |
| Tauri 命令 | 2 | ~100 |
| React 前端 | 5 | ~250 |

---

## Phase 3 — 聊天记录 + 用户管理（已完成，截图已移除）

### 3.1 — 无限滚动历史加载

**改动文件**: `ChatPanel.tsx`, `stores/messageStore.ts`

```
方案: scrollTop === 0 检测 + scrollHeight 位置保持
  1. messageStore 加 loadingHistory + historyOffset 状态
  2. ChatPanel 的 messages div 加 ref + onScroll handler
  3. 滚动到顶 → 如果 hasHistory != "allLoaded" 则触发加载
  4. 加载前记录 scrollHeight，prepend 后恢复 scrollTop
  5. 返回 < limit 条 → 标记 hasHistory = "allLoaded"
复杂度: 低，纯前端
```

### 3.2 — 消息日期分隔线

**改动文件**: `ChatPanel.tsx`

```
方案: 遍历时比较相邻消息的日期部分
  1. messages.map 内: 比较当前 msg.timestamp 和前一条的日期
  2. 日期不同时插入 DateSeparator:
     - 今天 → "Today"
     - 昨天 → "Yesterday"
     - 本周 → "星期X"
     - 更早 → "2024-01-15"
复杂度: 低，纯前端，~30 行
```

### 3.3 — 消息搜索

**改动文件**: `engine.rs`, `commands.rs`, `main.rs`, `ChatPanel.tsx`

```
方案: ChatPanel 内搜索栏 (option A)
  1. engine.rs: 加 search_chat_history(query, limit) → HistoryDb::search_messages
  2. commands.rs: 加 search_messages IPC 命令
  3. ChatPanel: 标题栏旁加搜索图标 → 搜索输入框 → 防抖调用 search_messages
  4. 结果显示为列表 (联系人 + 时间 + 内容片段)
复杂度: 中，前后端均需改动
```

### 3.6 — 联系人分组树形视图

**改动文件**: `Sidebar.tsx`, `contactStore.ts`, `commands.rs`, `engine.rs`

```
方案: React 本地分组 + 可折叠 CollapsibleGroup 组件
  1. 按 group_name 分组联系人 (空 group → "未分组")
  2. CollapsibleGroup: useState 控制展开/折叠，显示组名 + 人数
  3. 在线联系人在组内排前面
  4. 组间拖拽: HTML5 DnD (onDragStart/onDrop) → 更新 group_name
  5. 持久化: HistoryDb 加 contact_meta 表 (ip, alias, group_name)
复杂度: 中，无额外依赖
```

### 3.7 — 别名编辑 + 签名显示

**改动文件**: `ChatPanel.tsx`, `Sidebar.tsx`, `commands.rs`, `engine.rs`

```
方案: 双击内联编辑 + 右键菜单
  1. ChatPanel 标题: 双击名称 → 切换 input → 回车保存 → invoke("set_alias")
  2. Sidebar 联系项: 右键菜单 → "编辑备注名" → 小弹窗
  3. 签名显示: ChatPanel 标题下 + Sidebar 联系项内
  4. 持久化: contact_meta 表
复杂度: 低
```

### 预计改动量

| 功能 | 复杂度 | Rust 改动 | 前端改动 |
|------|--------|-----------|----------|
| 3.1 无限滚动 | 低 | 0 | ChatPanel + store |
| 3.2 日期分隔线 | 低 | 0 | ChatPanel |
| 3.3 消息搜索 | 中 | engine + commands | ChatPanel |
| 3.6 分组树 | 中 | engine + commands | Sidebar |
| 3.7 别名签名 | 低 | engine + commands | ChatPanel + Sidebar |

---

## Phase 4 — 群聊 + 离线消息（已完成，文件夹传输已移除）

### 4.1 — P2P 群组分发

**改动文件**: `engine.rs`, `commands.rs`, `main.rs`, `Sidebar.tsx`, `ChatPanel.tsx`

```
方案: 遍历成员逐人发送 (与原始 feiq 一致)
  1. engine.rs: send_text_to_group(group_name, text)
     - 从 HistoryDb 读 member_ips
     - 逐人调用 send_text_to (text 前缀 [groupname])
  2. 群消息识别: 前端用 from_ip = "group:groupname" 作为 store key
  3. 群聊视图: ChatPanel 检测 group: 前缀 → 始终显示发送者名称
  4. 群创建: CreateGroupDialog (新组件) → 多选联系人 → invoke("create_group")
  5. Sidebar: "Groups" 区域 + "创建群组" 按钮
复杂度: 中
```

### 4.3 — 黑名单

**改动文件**: `history.rs`, `engine.rs`, `commands.rs`, `Sidebar.tsx`

```
方案: SQLite blacklist 表 + handle_network_event 入口过滤
  1. history.rs: 加 blacklist 表 + CRUD 方法
  2. engine.rs: handle_network_event 最开头检查 is_blacklisted:
     - FellowOnline → skip
     - Message → drop
  3. commands.rs: add_to_blacklist / remove_from_blacklist / get_blacklist
  4. Sidebar: 右键 → "拉黑" / "取消拉黑"
复杂度: 低
```

### 预计改动量

| 功能 | 复杂度 | Rust 改动 | 前端改动 |
|------|--------|-----------|----------|
| 4.1 群组分发 | 中 | engine + commands | Sidebar + ChatPanel + Dialog |
| 4.3 黑名单 | 低 | history + engine + commands | Sidebar |

---

## Phase 5 — 加密 + 密封 + 主题（已完成）

### 5.1 — 加密接入消息管线

**改动文件**: `engine.rs`, `types.rs`, `parser.rs`, `serializer.rs`, `settings.rs`

```
方案: 临时 ECDH 密钥对 (当前阶段)，通过 ANSENTRY 携带公钥

密钥策略:
  - 每次启动生成新的 x25519 keypair (内存中，不写盘)
  - 重启后 keypair 丢失 → 重新握手，前向安全性
  - 对等体公钥只存内存 (Fellow.public_key)，离线即清除
  - LAN 场景双方几乎同时在线 → 临时密钥够用

  1. Fellow 加 public_key: Vec<u8> 字段
  2. Engine 加 key_map: HashMap<String, CryptoState> (纯内存)
  3. ANSENTRY/BR_ENTRY extra 字段追加公钥 (NUL + raw 32 bytes)
  4. send_text_to: 检测 is_feiq_plus_plus → 查 key_map → encrypt 载荷 → 加 ENCRYPTOPT
  5. handle_network_event Message: 检测 ENCRYPTOPT → decrypt 载荷 → 正常处理
  6. 版本检测: version.contains("feiq_plus_plus")

复杂度: 高，涉及协议变更和密钥状态机
```

### 5.2 — 密封消息（阅后即焚）

**改动文件**: `engine.rs`, `types.rs`, `parser.rs`, `serializer.rs`, `commands.rs`, `MessageBubble.tsx`

```
方案: IPMSG_SECRETEXOPT 标记 + 前端倒计时
  1. Content 加 Sealed 变体 { text, ttl_seconds }
  2. parser.rs: RecvReadCheck 检测 SECRETEXOPT → 标记为 Sealed
  3. engine.rs: send_sealed_text_to → 加 SECRETEXOPT | READCHECKOPT
  4. 前端: MessageBubble 显示倒计时 → setTimeout 移除消息
  5. 用户查看 → send_readmsg → 发送者收到 MessageRead 事件
复杂度: 中
```

### 5.3 — 文件共享服务

**改动文件**: `tcp.rs`, `engine.rs`, `settings.rs`, `commands.rs`

```
方案: IPMSG GETDIRFILES 扩展为目录浏览
  1. AppConfig 加 shared_dir / shared_dir_password
  2. engine.rs: 处理 GETDIRFILES (file_id=0) → 返回目录列表
  3. tcp.rs: FileServer accept 循环加目录列表响应
  4. 密码验证: GETDIRFILES 携带 :password: 字段
复杂度: 中
```

### 5.4 — 自定义主题

**改动文件**: `index.css`, `settings.rs`, `App.tsx`, `SettingsDialog.tsx`

```
方案: CSS 自定义属性 + @media prefers-color-scheme
  1. index.css: 定义 --color-* 变量 (亮色+暗色+强调色)
  2. settings.rs: AppConfig 加 theme
  3. App.tsx: useEffect 读取 theme → document.documentElement.className
  4. SettingsDialog.tsx: 主题选择器 (Auto/Light/Dark)
复杂度: 低，纯前端 + 1 配置字段
```

### 5.5 — 托盘未读徽章

**改动文件**: `tray.rs`, `events.rs`, `main.rs`

```
方案: Tauri TrayIcon::set_icon + macOS dock badge
  1. tray.rs: 存储 TrayIcon 引用到 AppState
  2. events.rs: 维护 unread_total 计数器
  3. 收到 new-message 且非当前选中 IP → unread_total += 1
  4. 用户切换聊天 → unread_total -= 对应未读数
  5. macOS: app.set_badge(unread_total)
复杂度: 中
```

### 5.6 — 聊天记录 JSON 导出/导入

**改动文件**: `history.rs`, `commands.rs`, `SettingsDialog.tsx`

```
方案: HistoryDb::export_all → JSON 文件 + import_messages ← JSON 文件
  1. history.rs: export_all() → serde_json::Value; import_messages(Value) → count
  2. commands.rs: export_history(path) / import_history(path)
  3. SettingsDialog.tsx: 导出按钮 (save() 对话框) + 导入按钮 (open() 对话框)
复杂度: 低
```

### 预计改动量

| 功能 | 复杂度 | Rust 改动 | 前端改动 |
|------|--------|-----------|----------|
| 5.1 加密管线 | 高 | engine + types + parser + serializer | 0 |
| 5.2 密封消息 | 中 | engine + types + parser | MessageBubble |
| 5.3 文件共享 | 中 | tcp + engine + settings | 0 |
| 5.4 主题 | 低 | settings | index.css + App + Settings |
| 5.5 托盘徽章 | 中 | tray + events | 0 |
| 5.6 历史导出 | 低 | history + commands | SettingsDialog |

---

## 已确认决策

| 决策 | 结论 |
|------|------|
| 截图方案 | 已移除（commit 85e23b4） |
| 加密密钥持久化 | 当前阶段临时密钥（前向安全） |
| Canvas 标注 | 已移除（commit 85e23b4），现由 DoodleDialog 替代 |
| 群聊方案 | 纯 P2P 无服务器：`send_text_to_group` 遍历成员逐人发送 |
| 文件夹传输 | **已移除**（commit f6c7616） |
| 4 个严重 bug | ✅ 已修复 |
| 中继文件传输 | Binary WebSocket tunnel（push 模式） |
| 文件共享鉴权 | `IPMSG_PASSWORDOPT` 原生标志位 |
| 隐身模式 | 全局不可见（不广播 + 不应答） |
| 头像 | 自定义协议扩展 `GETAVATAR/SENDAVATAR` |

## 跨 Agent 审计发现的问题

### 🔴 严重问题（已全部修复）

| # | 问题 | 位置 | 状态 |
|---|------|------|:---:|
| 1 | **crypto.rs nonce 重用 bug** | `crypto.rs:89` | ✅ 已修复 |
| 2 | **前端 upsertContact 别名覆盖** | `contactStore.ts:38` | ✅ 已修复 |
| 3 | **HistoryDb 群组重复** | `history.rs:206` | ✅ 已修复 |
| 4 | **search_messages 不搜索联系人姓名** | `history.rs:130` | ✅ 已修复 |

### 🟡 中等问题

| # | 问题 | 位置 | 状态 |
|---|------|------|:---:|
| 5 | **drain_pending 静默丢弃数据库错误** | `engine.rs:506` | 🟡 已知 |
| 6 | **MessageRecord 去重使用时间戳** | `messageStore.ts:57` | 🟡 已知 |
| 7 | **加密公钥追加到 ANSENTRY 损坏名称解析** | `parser.rs:84,105` | ✅ 已修复 |

### 🟢 确认可行 / 无阻塞

| # | 发现 |
|---|------|
| 8 | Tauri v2 托盘/停靠栏 API 已确认 |
| 9 | `screencapture -i` 已验证 |
| 10 | Tauri v2 拖放 API 已确认 |
| 11 | Tailwind v4 `@theme` CSS 变量方法已确认 |
| 12 | Canvas 标注可行 → DoodleDialog 已实现 |
| 13 | 原始 feiq 无真正的群聊 — 我们自建 P2P 分发 |
| 14 | 原始 feiq 拒绝文件夹传输 — 我们自建目录共享 |
| 15 | `std::sync::Mutex` 从未跨 await 持有 — 线程安全 ✅ |

## 技术验证项

| 项 | 方法 | 状态 |
|----|------|:---:|
| Tauri v2 drag-drop API | 查阅 @tauri-apps/api 文档 | ✅ |
| screencapture 行为 | macOS 实测 | ✅ |
| CSS var 替换 Tailwind class | 12 个语义变量 | ✅ |
| Tauri TrayIcon API | set_badge_count / set_tooltip | ✅ |
| ECDH 密钥格式 (x25519 32 bytes) | ring crate API 已验证 | ✅ |
| IPMSG_ENCRYPTOPT 兼容性 | 仅 feiq++ 间加密 | ✅ |
| Relay 二进制隧道 | WebSocket Message::Binary | ✅ |
| IPMSG_PASSWORDOPT 兼容性 | 旧版飞秋兼容 | ✅ |

---

> **参考代码**: 原 feiq Qt5/C++ 项目位于 `/Users/zhihu/code/feiq/`
> **当前 feiq++ 测试**: 200 Rust + 18 TS = 218，全部通过
> **Phase 6 执行计划原文**: `.claude/plans/luminous-kindling-teacup.md`
