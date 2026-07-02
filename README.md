# feiq++

经典局域网聊天软件飞秋 (FeiQ) 的现代跨平台增强版本，兼容飞鸽传书 (IP Messenger) 协议。

## 特性

### 即时通讯
- 文本消息收发、96 种 QQ 风格表情内联渲染
- 窗口抖动 (Knock)
- 离线消息（上线后自动送达）
- Enter / Ctrl+Enter 发送切换

### 文件传输
- 单文件传输，支持大文件 (>100GB)
- 实时进度显示、取消/续传
- 拖拽发送
- 64KB 分块传输
- 上传/下载限速（可配置 KB/s）
- 中继文件传输（跨网络通过 WebSocket 二进制隧道）
- 密码保护的文件共享目录浏览

### 群组
- 无服务器 P2P 聊天室，无限创建群组
- 群发消息、群文件共享、群公告、P2P 群组分发
- 群主权限控制、群设置管理

### 跨网络通信
- 自建 Rust WebSocket 中继服务器 `feiq-relay`
- 三种连接模式：纯局域网 / 纯中继 / 混合模式
- 跨子网自动去重 (MAC 地址索引)
- 离线消息 24h TTL 服务器端队列

### 安全
- feiq++ 间 ECDH (x25519) + AES-256-GCM 端到端加密
- 密封消息 (阅后即焚)
- 随机数前缀防重放
- 隐身模式（全局不可见）

### 用户管理
- 好友分组树、自定义备注名/别名编辑、个性签名
- 拼音搜索联系人（支持首字母和全拼）
- 黑名单过滤
- 名称/IP 搜索、自定义广播网段
- 个人头像（SHA256 哈希 + 网络传输）

### 聊天记录
- SQLite 持久化存储、无限滚动加载历史
- 全文搜索、日期分隔线、JSON 导出/导入

### 个性化
- 明/暗/自动/自定义主题 (CSS 变量 + Tailwind + 运行时注入)
- 系统原生通知 + Dock/Taskbar 未读角标
- 系统托盘快捷操作
- 字体自定义（族 + 大小）
- 涂鸦绘画工具（Canvas 自由绘制）

## 平台支持

| 平台 | 状态 |
|------|:---:|
| macOS | ✅ |
| Windows | ✅ |
| Linux | ✅ |

## 技术栈

| 层 | 技术 | 说明 |
|---|------|------|
| 协议引擎 | Rust + tokio | IPMSG v9 完整协议实现 |
| 编码 | encoding_rs (Mozilla) | 纯 Rust GBK/UTF-8 |
| 加密 | ring 0.17 | AES-256-GCM + ECDH |
| 存储 | rusqlite (bundled) | SQLite 内嵌，无系统依赖 |
| 桌面壳 | Tauri 2 | ~5MB 二进制，三平台原生 |
| 前端 | React 18 + TypeScript | Vite 构建 |
| 样式 | Tailwind CSS 3 | 工具类优先 |

## 项目结构

```
feiq-plus-plus/
├── packages/
│   ├── feiq-core/              # Rust: 协议引擎 + 网络 + 存储 + 加密
│   │   └── src/
│   │       ├── protocol/       # IPMSG 协议常量、类型、编解码、解析链
│   │       ├── network/        # UDP/TCP/Relay 通信、加密
│   │       ├── engine/         # 引擎控制器 + 事件系统 + 文件任务
│   │       ├── model/          # 联系人簿 (线程安全)
│   │       └── storage/        # SQLite 聊天记录 + INI 配置
│   ├── feiq-app/               # Tauri 2 桌面壳
│   │   └── src/                # commands (36), events, state, tray
│   ├── feiq-relay/             # Rust WebSocket 中继服务器
│   │   └── src/                # server, main, lib
│   └── feiq-gui/               # React 前端
│       └── src/
│           ├── components/     # Sidebar, ChatPanel, MessageBubble, InputArea,
│           │                   # EmojiPicker, SettingsDialog, CreateGroupDialog,
│           │                   # FileTransferPanel, DoodleDialog, RemoteFileBrowser
│           └── stores/         # Zustand: contactStore, messageStore,
│                               # fileTransferStore, groupStore, typingStore
├── docs/
│   ├── PLAN.md                  # 完整实现计划
│   ├── phase-execution-plan.md  # 详细执行方案
│   ├── relay-server-design.md   # 中继服务器技术方案
│   └── RELEASE_NOTES_v0.1.0.md  # 发布说明
├── README.md                    # 本文件
└── Cargo.toml                   # Rust workspace
```

## 快速开始

### 前置要求

- [Rust](https://rustup.rs/) 1.70+
- [Node.js](https://nodejs.org/) 18+
- macOS / Windows / Linux

### 编译运行

```bash
# 克隆项目
git clone https://github.com/lulyon/feiq-plus-plus.git
cd feiq-plus-plus

# 安装前端依赖
cd packages/feiq-gui && npm install && cd ../..

# 开发模式 (前端热更新 + Rust 后端)
cargo tauri dev

# 仅编译 Rust 核心库
cargo build --workspace

# 运行全部测试 (200 Rust + 18 TypeScript)
cargo test --workspace
npm --prefix packages/feiq-gui test

# 生产构建 + 打包
cargo tauri build
```

### 配置

首次启动会自动创建 `~/.feiq_setting.ini`，兼容原飞秋配置格式：

```ini
[user]
name = 你的名字
host = 你的主机名

[network]
custom_group = 192.168.74.|192.168.82.

[app]
send_by_enter = 1
```

## 协议兼容性

完全实现 IP Messenger Draft-9 (v9) 协议，可与以下客户端互通：

| 客户端 | 文本 | 文件 | 表情 | 抖动 |
|--------|:---:|:---:|:---:|:---:|
| 飞鸽传书 (IPMSG) | ✅ | ✅ | — | — |
| 飞秋 (FeiQ) Windows | ✅ | ✅ | ✅ | ✅ |
| 飞秋 (FeiQ) Mac | ✅ | ✅ | ✅ | ✅ |
| feiq++ ↔ feiq++ | ✅ | ✅ | ✅ | ✅ |

feiq++ 间额外支持端到端加密通信。

## 版本标识

feiq++ 在协议中使用的版本字符串：

```
feiq_plus_plus#128#MAC地址#0#0#0#1#9
```

通过此标识可区分 feiq++ 和对端是否支持加密。

## 开发

详见 [`docs/PLAN.md`](docs/PLAN.md) 获取完整实现计划和技术决策记录。

### 运行测试

```bash
cargo test --workspace                    # 全部 200 个 Rust 测试
cargo test -p feiq-core                   # 仅核心库测试
npm --prefix packages/feiq-gui test       # 前端 18 个 TypeScript 测试
```

### 代码统计

```
Rust 源码:    ~11,000 行 (35+ 文件, 3 crates)
React/TS:     ~3,000 行 (11 组件 + 5 stores)
测试覆盖:     218 个测试 (200 Rust + 18 TS), 全部通过
IPC 命令:     36 个
```

## 最近更新 (v0.1.4)

- **中继文件传输**: WebSocket 二进制隧道，支持跨网络文件收发，移除中继下载守卫
- **文件共享**: 密码保护共享目录浏览 (`browse_shared_folder`)，兼容 IPMSG PASSWORDOPT
- **传输限速**: 可配置文件上传/下载限速 (KB/s)，sleep-based pacing
- **输入状态提示**: `IPMSG_INPUTING`/`IPMSG_INPUT_END` 协议支持，前端 debounce + 5s 自动清除
- **隐身模式**: 全局不可见，不广播 BR_ENTRY，不应答 ANSENTRY
- **拼音搜索**: 首字母 + 全拼匹配，`pinyin` crate 集成
- **个人头像**: `IPMSG_GETAVATAR`/`IPMSG_SENDAVATAR` 协议扩展，SHA256 哈希交换
- **主题增强**: 自定义主题 `CustomTheme` 结构体，运行时 CSS 变量注入
- **字体自定义**: `--font-family` / `--font-size` CSS 变量，配置持久化
- **涂鸦工具**: HTML5 Canvas 自由绘制 (画笔/橡皮擦/颜色/线宽/撤销)
- **群组增强**: 群文件共享、群公告、群主权限控制 (owner_ip + settings)
- **BR_ABSENCE**: 名称/状态变更检测，兼容飞秋旧版客户端
- **READMSG/ANSREADMSG**: 密封消息已读回执处理
- **审计修复**: AES-GCM nonce 重用、中继离线消息丢失、引擎停止泄漏等 17 个关键问题修复

## License

本项目参考了以下开源项目：
- 飞鸽传书 (IP Messenger) — BSD License, 白水啓章 (H.Shirouzu), 1996
- 飞秋 (FeiQ) for Mac — GPL, 开源项目
