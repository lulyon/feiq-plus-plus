# feiq++

经典局域网聊天软件飞秋 (FeiQ) 的现代平替增强版本，兼容飞鸽传书 (IP Messenger) 协议。

**跨平台** · **Rust 后端** · **React 前端** · **Tauri 2 桌面壳** · **零服务器 P2P 架构**

## 特性

### 即时通讯
- 文本消息收发、96 种 QQ 风格表情内联渲染
- 窗口抖动 (Knock)、正在输入状态提示
- 离线消息（上线后自动送达）
- Enter / Ctrl+Enter 发送切换

### 文件传输
- 单文件 + 文件夹传输，支持 4GB+ 大文件
- 实时进度显示、断点续传
- 拖拽发送、速度限制
- 局域网极速 (10~100MB/s)

### 群组
- 无服务器 P2P 聊天室，无限创建群组
- 群发消息、分组群发、群文件共享

### 安全
- feiq++ 间 ECDH (x25519) + AES-256-GCM 端到端加密
- 密封消息 (阅后即焚)

### 用户管理
- 好友分组、自定义备注名、个性签名
- 黑名单、隐身模式
- 拼音首字母搜索、自定义广播网段

### 聊天记录
- SQLite 持久化存储、无限滚动加载历史
- 全文搜索、JSON 导出/导入

### 个性化
- 明/暗主题 + 多配色
- 系统原生通知 + Dock/Taskbar 未读角标
- 系统托盘快捷操作

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
│   │       ├── network/        # UDP/TCP 通信、加密
│   │       ├── engine/         # 引擎控制器 + 事件系统 + 文件任务
│   │       ├── model/          # 联系人簿 (线程安全)
│   │       └── storage/        # SQLite 聊天记录 + INI 配置
│   ├── feiq-app/               # Tauri 2 桌面壳
│   │   └── src/                # commands, events, state, tray
│   └── feiq-gui/               # React 前端
│       └── src/
│           ├── components/     # Sidebar, ChatPanel, MessageBubble, InputArea,
│           │                   # EmojiPicker, SettingsDialog
│           └── stores/         # Zustand: contactStore, messageStore
├── PLAN.md                     # 完整实现计划
├── README.md                   # 本文件
└── Cargo.toml                  # Rust workspace
```

## 快速开始

### 前置要求

- [Rust](https://rustup.rs/) 1.70+
- [Node.js](https://nodejs.org/) 18+
- macOS / Windows / Linux

### 编译运行

```bash
# 克隆项目
git clone <repo-url>
cd feiq-plus-plus

# 安装前端依赖
cd packages/feiq-gui && npm install && cd ../..

# 开发模式 (前端热更新 + Rust 后端)
cargo tauri dev

# 仅编译 Rust 核心库
cargo build --workspace

# 运行测试 (27 个单元测试)
cargo test --workspace

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

详见 [`PLAN.md`](PLAN.md) 获取完整实现计划和技术决策记录。

### 运行测试

```bash
cargo test --workspace                    # 全部 27 个测试
cargo test -p feiq-core                   # 仅核心库测试
```

### 代码统计

```
Rust 源码:    ~3,300 行 (17 文件)
React/TS:     ~800 行  (7 组件 + 2 stores)
测试覆盖:     27 个单元测试, 全部通过
```

## License

本项目参考了以下开源项目：
- 飞鸽传书 (IP Messenger) — BSD License, 白水啓章 (H.Shirouzu), 1996
- 飞秋 (FeiQ) for Mac — GPL, 开源项目
