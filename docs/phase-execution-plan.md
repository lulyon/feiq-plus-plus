# feiq++ Phase 2-5 — 详细执行计划

> 基于对当前代码库的逐行审计 + 原 feiq C++ 项目 (`/Users/zhihu/code/feiq/`) 的交叉参考。
> 每个功能均标注了：具体改动文件、实现细节、依赖关系、预计复杂度。

---

## Phase 2 — 文件传输 + 表情（当前 60%）

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

## Phase 3 — 聊天记录 + 截图 + 用户管理（当前 25%）

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

### 3.4 — 截图命令

**改动文件**: `commands.rs`, `main.rs`, `InputArea.tsx`

```
方案: Rust std::process::Command (无需额外插件)
  1. commands.rs: capture_screenshot 命令
     - macOS: screencapture -i /tmp/feiq_screenshot_xxx.png
     - Linux: maim -s /tmp/...
     - Windows: 启动 Win+Shift+S (异步，需配合剪贴板或文件)
  2. 返回文件路径 → 前端 show 预览 → 作为文件发送
  3. InputArea: 加截图按钮 (Camera 图标)
  4. 快捷键: tauri-plugin-global-shortcut 注册 CmdOrCtrl+Shift+S
复杂度: 中，平台差异需分别处理
```

### 3.5 — 截图标注工具

**改动文件**: `components/ScreenshotAnnotation.tsx` (新), `InputArea.tsx`

```
方案: 原始 Canvas API (无额外依赖)
  工具: 矩形 / 箭头 / 文字 / 自由绘制
  1. 截完图 → 打开全屏 overlay → <canvas> 显示截图
  2. 上层透明 Canvas 用于标注绘制
  3. 工具栏: 工具切换 + 颜色选择 + 撤销
  4. 完成 → canvas.toBlob() → write to tmp → send as file
复杂度: 中高，~200 行 Canvas 交互代码
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
| 3.4 截图 | 中 | commands | InputArea |
| 3.5 标注 | 中高 | 0 | 新组件 |
| 3.6 分组树 | 中 | engine + commands | Sidebar |
| 3.7 别名签名 | 低 | engine + commands | ChatPanel + Sidebar |

---

## Phase 4 — 群聊 + 文件夹 + 离线消息（当前 20%）

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

### 4.2 — 文件夹传输

**改动文件**: `engine.rs`, `tcp.rs`, `commands.rs`

```
方案: IPMSG GETDIRFILES 协议标准流程
  1. engine.rs: 移除文件夹拒绝逻辑 (line 574-579)
  2. engine.rs: build_folder_manifest(dir_path) → 递归生成 FileContent 清单
  3. engine.rs: handle_network_event 处理 IPMSG_GETDIRFILES:
     - 创建根目录
     - 遍历清单 → 逐文件 TCP 请求 → recv_file
  4. sender: 发送文件清单 → 监听 TCP → 收到 GETDIRFILES 请求 → send_file(offset)
  5. commands.rs: send_folder(ip, dir_path) 命令
复杂度: 高，涉及 TCP 协议双向交互
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
| 4.2 文件夹传输 | 高 | engine + tcp + commands | 0 (复用文件传输 UI) |
| 4.3 黑名单 | 低 | history + engine + commands | Sidebar |

---

## Phase 5 — 加密 + 文件共享 + 打磨（当前 15%）

### 5.1 — 加密接入消息管线 ⚡ 最关键的缺失

**改动文件**: `engine.rs`, `types.rs`, `parser.rs`, `serializer.rs`, `settings.rs`

```
方案: 临时 ECDH 密钥对，通过 ANSENTRY 携带公钥
  1. Fellow 加 public_key: Vec<u8> 字段
  2. Engine 加 key_map: HashMap<String, CryptoState>
  3. ANSENTRY/BR_ENTRY extra 字段追加公钥 (NUL + raw bytes)
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
方案: CSS 自定义属性 + .theme-light / .theme-dark / @media prefers-color-scheme
  1. index.css: 定义 --color-* 变量 (亮色+暗色+强调色)
  2. settings.rs: AppConfig 加 theme_mode / theme_accent
  3. App.tsx: useEffect 读取 theme → document.documentElement.className
  4. SettingsDialog.tsx: 主题选择器 (Auto/Light/Dark + 配色)
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

## 建议实现顺序（跨 Phase）

```
第 1 批 (快速可见改进):
  Phase 3.2  日期分隔线          (30min)
  Phase 5.4  自定义主题          (1h)
  Phase 3.7  别名编辑 + 签名     (1h)
  Phase 5.6  历史导出/导入       (1h)

第 2 批 (核心体验):
  Phase 3.1  无限滚动            (1h)
  Phase 3.6  分组树形视图        (2h)
  Phase 4.3  黑名单              (1h)
  Phase 5.5  托盘未读徽章        (1h)

第 3 批 (功能增强):
  Phase 3.3  消息搜索            (2h)
  Phase 4.1  群组分发 + UI       (3h)
  Phase 2.1  文件引擎层          (3h)
  Phase 3.4  截图命令            (1h)

第 4 批 (需要协议变更):
  Phase 2.2  文件 Tauri 命令     (2h)
  Phase 2.3  文件传输面板        (2h)
  Phase 2.4  可点击文件气泡      (1h)
  Phase 2.5  拖拽发送            (1h)
  Phase 5.1  加密管线 ⚡         (4h)
  Phase 5.2  密封消息            (2h)

第 5 批 (复杂协议):
  Phase 3.5  截图标注            (3h)
  Phase 4.2  文件夹传输          (4h)
  Phase 5.3  文件共享服务        (2h)
```

---

## 技术验证项

| 项 | 方法 | 状态 |
|----|------|:---:|
| Tauri v2 drag-drop API | 查阅 @tauri-apps/api 文档 | 待验证 |
| screencapture 行为 | macOS 实测 screencapture -i | 待验证 |
| CSS var 替换 Tailwind class | 用手动替换还是 tailwind.config | 待验证 |
| Tauri TrayIcon::set_icon API | Tauri 2 API 文档 | 待验证 |
| IPMSG GETDIRFILES 完整协议 | 原 feiq 代码 (`TECHNICAL_DOC.md`) | 已参考 |
| ECDH 密钥格式 (x25519 32 bytes) | ring crate API 已验证 | ✅ |
| IPMSG_ENCRYPTOPT 与旧客户端兼容 | 仅 feiq++ 之间加密，旧客户端明文 | ✅ |

---

> **参考代码**: 原 feiq Qt5/C++ 项目位于 `/Users/zhihu/code/feiq/`，包含完整实现参考
> **核心技术文档**: `/Users/zhihu/code/feiq/TECHNICAL_DOC.md` (31KB)
> **当前 feiq++ 测试**: 30 个全部通过
