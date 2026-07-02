# Feiq-Plus-Plus 实现计划

## 背景

飞秋（FeiQ）是经典的局域网聊天软件，兼容飞鸽传书（IP Messenger）协议。由白水啓章（H.Shirouzu）于 1996 年创建，BSD 协议开源。飞秋基于此协议做了大量扩展，最后一版发布于 2013 年（Windows）/ 2015 年（Mac/Linux），此后停止更新。

本项目基于原 feiq (Qt5/C++ macOS 飞秋) 的全面代码审查 + 联网调研飞秋/飞鸽传书完整功能，重写一个跨平台、现代化的局域网聊天软件。

---

## 一、经典飞秋完整功能矩阵（调研整理）

### 1.1 即时通讯

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 文本消息收发 | ✅ | ✅ | ✅ P1 |
| 自定义表情 (96个QQ表情) | ✅ | ❌ | ✅ P2 |
| GIF 动画表情 | ✅ | ❌ | ✅ P2 |
| 窗口抖动/闪屏振动 | ✅ | ❌ | ✅ P2 |
| 正在输入状态提示 | ✅ | ❌ | ✅ P3 |
| 离线消息 (对方上线后送达) | ✅ | ❌ | ✅ P4 |

### 1.2 文件传输

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 单文件发送/接收 | ✅ | ✅ | ✅ P2 |
| 文件夹传输 | ✅ | ✅ (v9) | ✅ P3 |
| 超大文件 (>4GB) | ✅ | ✅ | ✅ P2 |
| 断点续传 | ✅ | ✅ (offset) | ✅ P4 |
| 传输进度实时显示 | ✅ | ❌ | ✅ P2 |
| 速度限制 | ✅ | ❌ | ✅ P4 |
| 文件共享 (设置密码) | ✅ | ❌ | ✅ P5 |
| 拖拽发送文件 | ✅ | ❌ | ✅ P2 |

### 1.3 群组功能

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 无服务器聊天室 | ✅ | ❌ | ✅ P4 |
| 无限创建群组 | ✅ | ❌ | ✅ P4 |
| 群内文件共享 | ✅ | ❌ | ✅ P5 |
| 群公告 | ✅ | ❌ | ✅ P5 |
| 全员群发消息 | ✅ | ❌ | ✅ P4 |
| 分组群发 | ✅ | ❌ | ✅ P4 |

### 1.4 用户管理

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 好友分组管理 | ✅ | ❌ | ✅ P3 |
| 组权限控制 (屏蔽/隐身) | ✅ | ❌ | ✅ P5 |
| 黑名单 | ✅ | ❌ | ✅ P4 |
| 隐身模式 (全局+分组) | ✅ | ❌ | ✅ P5 |
| 自定义备注名 | ✅ | ❌ | ✅ P3 |
| 用户搜索 (名称/IP/拼音首字母) | ✅ | ❌ | ✅ P3 |
| 上线/下线自动通知 | ✅ | ✅ | ✅ P1 |

### 1.5 个性化

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 个性头像 | ✅ | ❌ | ✅ P3 |
| 个性签名 | ✅ | ❌ | ✅ P3 |
| 皮肤/主题换色 | ✅ | ❌ | ✅ P5 |
| 界面字体自定义 | ✅ | ❌ | ✅ P5 |
| 个性形象照片 | ✅ | ❌ | ❌ (过时) |

### 1.6 截图与涂鸦

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 截图发送 | ✅ | ❌ | ✅ P3 |
| 涂鸦/画图 | ✅ | ❌ | ✅ P5 |
| 截图标注 (矩形/箭头/文字) | ❌ | ❌ | ✅ P3 |

### 1.7 远程与语音

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 远程协助/共享桌面 | ✅ | ❌ | ❌ (超出IM范围) |
| 语音聊天 | ✅ | ❌ | ❌ (P6可考虑) |

### 1.8 日程与提醒

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| 日程安排 | ✅ | ❌ | ❌ (超出IM范围) |
| 复杂提醒 (年月周日时分秒) | ✅ | ❌ | ❌ (超出IM范围) |
| 提醒动作 (弹窗/音乐/执行程序/关机) | ✅ | ❌ | ❌ (超出IM范围) |

### 1.9 安全与加密

| 功能 | 原 feiq | IPMSG 原生 | 本计划 |
|------|:---:|:---:|:---:|
| RSA 公钥加密 (512/1024/2048) | ❌ | ✅ (v9) | ✅ P5 |
| RC2/Blowfish 对称加密 | ❌ | ✅ (v9) | ✅ P5 |
| AES 128/192/256 | ❌ | ✅ (v9) | ✅ P5 |
| 数字签名 (MD5/SHA1) | ❌ | ✅ (v9) | ✅ P5 |
| 密封消息 (阅后即焚) | ❌ | ✅ | ✅ P5 |
| 垃圾信息屏蔽 | ✅ | ❌ | ✅ P4 |

### 1.10 飞秋特有功能

| 功能 | 说明 | 本计划 |
|------|------|:---:|
| 飞秋空间日志 | 局域网博客，HTML内容发布 | ❌ (过时概念) |
| 飞秋应用管理器 | 插件下载管理 | ❌ |
| 飞秋机器人 | 命令行自动化接口 | ❌ |
| 聊天记录备份/还原 | 历史消息导入导出 | ✅ P5 |
| 通讯录 | 动态企业通讯录 | ✅ P5 |
| 跨网段支持 | 自定义广播IP段 | ✅ P1 |

---

## 二、IPMSG 协议完整命令参考 (v9)

### 2.1 基础命令

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_NOOPERATION` | 0x00000000 | 无操作 |
| `IPMSG_BR_ENTRY` | 0x00000001 | 上线广播 |
| `IPMSG_BR_EXIT` | 0x00000002 | 下线广播 |
| `IPMSG_ANSENTRY` | 0x00000003 | 上线应答 |
| `IPMSG_BR_ABSENCE` | 0x00000004 | 离开模式 / 状态变更 |
| `IPMSG_BR_ISGETLIST` | 0x00000010 | 请求成员列表 |
| `IPMSG_OKGETLIST` | 0x00000011 | 确认收到列表 |
| `IPMSG_GETLIST` | 0x00000012 | 获取成员列表 |
| `IPMSG_ANSLIST` | 0x00000013 | 返回成员列表 |
| `IPMSG_BR_ISGETLIST2` | 0x00000018 | 请求成员列表 v2 |

### 2.2 消息命令

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_SENDMSG` | 0x00000020 | 发送消息 |
| `IPMSG_RECVMSG` | 0x00000021 | 消息已接收 (回复确认) |
| `IPMSG_READMSG` | 0x00000030 | 消息已读 (密封消息) |
| `IPMSG_DELMSG` | 0x00000031 | 删除消息 |
| `IPMSG_ANSREADMSG` | 0x00000032 | 确认已读 (v8+) |

### 2.3 信息查询

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_GETINFO` | 0x00000040 | 请求协议版本 |
| `IPMSG_SENDINFO` | 0x00000041 | 返回协议版本 |
| `IPMSG_GETABSENCEINFO` | 0x00000050 | 询问离开状态 |
| `IPMSG_SENDABSENCEINFO` | 0x00000051 | 返回离开状态 |

### 2.4 文件传输

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_GETFILEDATA` | 0x00000060 | 请求文件数据 (TCP) |
| `IPMSG_RELEASEFILES` | 0x00000061 | 释放文件 |
| `IPMSG_GETDIRFILES` | 0x00000062 | 请求文件夹数据 (TCP) |

### 2.5 加密 (v9)

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_GETPUBKEY` | 0x00000072 | 请求 RSA 公钥 |
| `IPMSG_ANSPUBKEY` | 0x00000073 | 返回 RSA 公钥 |

### 2.6 飞秋扩展

| 命令 | 值 | 说明 |
|------|-----|------|
| `IPMSG_OPEN_YOU` | 0x00000077 | 打开聊天窗口 |
| `IPMSG_INPUTING` | 0x00000079 | 正在输入 |
| `IPMSG_INPUT_END` | 0x0000007A | 输入结束 |
| `IPMSG_SENDIMAGE` | 0x000000C0 | 发送图片 (8字节ID) |
| `IPMSG_KNOCK` | 0x000000D1 | 窗口抖动 |

### 2.7 选项标志

| 标志 | 值 | 说明 |
|------|-----|------|
| `IPMSG_ABSENCEOPT` | 0x00000100 | 离开模式 (命令用) |
| `IPMSG_SERVEROPT` | 0x00000200 | 服务器模式 |
| `IPMSG_DIALUPOPT` | 0x00010000 | 拨号用户 |
| `IPMSG_FILEATTACHOPT` | 0x00200000 | 文件附件 |
| `IPMSG_ENCRYPTOPT` | 0x00400000 | 加密消息 |
| `IPMSG_UTF8OPT` | 0x00800000 | UTF-8 编码 |
| `IPMSG_SENDCHECKOPT` | 0x00000100 | 需回执确认 (发送用) |
| `IPMSG_SECRETOPT` | 0x00000200 | 密封消息 |
| `IPMSG_BROADCASTOPT` | 0x00000400 | 广播 |
| `IPMSG_MULTICASTOPT` | 0x00000800 | 组播 |
| `IPMSG_NOPOPUPOPT` | 0x00001000 | 不弹出窗口 |
| `IPMSG_AUTORETOPT` | 0x00002000 | 自动回复 (防乒乓) |
| `IPMSG_PASSWORDOPT` | 0x00008000 | 密码保护 |
| `IPMSG_NOLOGOPT` | 0x00020000 | 不记日志 |
| `IPMSG_NOADDLISTOPT` | 0x00080000 | 不加入列表 |
| `IPMSG_READCHECKOPT` | 0x00100000 | 需已读回执 (v8+) |

### 2.8 加密算法标志 (v9)

| 标志 | 说明 |
|------|------|
| `IPMSG_RSA_512/1024/2048` | RSA 密钥长度 |
| `IPMSG_RC2_40/128/256` | RC2 加密 |
| `IPMSG_BLOWFISH_128/256` | Blowfish 加密 |
| `IPMSG_AES_128/192/256` | AES 加密 |
| `IPMSG_SIGN_MD5/SHA1` | 数字签名算法 |

### 2.9 文件类型标志

| 标志 | 说明 |
|------|------|
| `IPMSG_FILE_REGULAR` (0x01) | 普通文件 |
| `IPMSG_FILE_DIR` (0x02) | 目录 |
| `IPMSG_FILE_RETPARENT` (0x03) | 返回上级目录 |
| `IPMSG_FILE_SYMLINK` (0x04) | 符号链接 |
| `IPMSG_FILE_CDEV/BDEV/FIFO` | Unix 设备文件 |
| `IPMSG_FILE_RESFORK` (0x10) | Mac 资源分支 |

---

## 三、技术选型

| 层 | 选择 | 理由 |
|---|------|------|
| 后端 | Rust (tokio 异步) | 内存安全、高性能、跨平台 |
| 协议编码 | encoding_rs (Mozilla) | 纯 Rust GBK/UTF-8，Firefox/Servo 验证 |
| 现代加密 (feiq++间) | ring (AES-256-GCM + ECDH) | 纯 Rust，仅 feiq++ 之间使用 |
| 遗留加密 (IPMSG v9兼容) | rsa 0.9 + blowfish 0.10 | 仅 P5 可选实现，与旧客户端互通 |
| 存储 | rusqlite (bundled) | SQLite 内嵌编译，无系统依赖 |
| 桌面壳 | Tauri 2.0 | 小二进制(~5MB)、三平台、原生通知/tray/快捷键 |
| 前端 | React 18 + TypeScript + Vite | 成熟生态、Tauri 官方支持 |
| 样式 | Tailwind CSS + shadcn/ui (Radix) | 无障碍、可定制、暗色模式内置 |
| 状态管理 | Zustand 4 | 轻量、TS 原生 |

---

## 四、项目结构

```
feiq-plus-plus/
├── Cargo.toml                       # workspace (feiq-core, feiq-app, feiq-relay)
├── CLAUDE.md                         # AI 上下文文档
├── README.md
├── docs/
│   ├── PLAN.md                       # 本文件
│   ├── relay-server-design.md        # Relay 服务器技术方案
│   └── RELEASE_NOTES_v0.1.0.md      # v0.1.0 发布说明
├── scripts/
│   └── dev-multi.sh                  # 多实例开发脚本
├── packages/
│   ├── feiq-core/                    # Rust: 协议引擎 + 网络 + 存储
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── protocol/
│   │       │   ├── constants.rs      # 完整 IPMSG 常量 (含 v9 加密)
│   │       │   ├── types.rs          # Fellow, Content, Post, FileContent, ContactSource
│   │       │   ├── parser.rs         # 责任链解析器 (12 handler)
│   │       │   ├── serializer.rs     # 消息打包/解包
│   │       │   ├── encoding.rs       # GBK/UTF-8 via encoding_rs
│   │       │   └── emoji.rs          # 96 个 QQ 表情映射
│   │       ├── network/
│   │       │   ├── udp.rs            # UDP socket, 广播, 异步接收
│   │       │   ├── tcp.rs            # TCP 监听, 文件传输流 (64KB chunk)
│   │       │   ├── relay.rs          # WebSocket relay 客户端 transport (432 行)
│   │       │   ├── crypto.rs         # ECDH (x25519) + AES-256-GCM 加密
│   │       │   └── manager.rs        # 网络生命周期, 协调 UDP+TCP+Relay
│   │       ├── engine/
│   │       │   ├── engine.rs         # 主控制器, hybrid LAN+Relay 模式
│   │       │   ├── events.rs         # FrontendEvent 定义
│   │       │   └── tasks.rs          # FileTask 状态机 + 进度节流
│   │       ├── model/
│   │       │   └── contacts.rs       # 线程安全联系人簿 (IP+MAC 索引, Relay peers)
│   │       └── storage/
│   │           ├── history.rs        # SQLite 聊天记录 + 离线消息 + 群组
│   │           └── settings.rs       # 配置持久化 + ConnectionMode
│   │
│   ├── feiq-app/                     # Tauri 桌面壳 (Rust)
│   │   ├── src/
│   │   │   ├── main.rs              # Tauri 入口
│   │   │   ├── commands.rs          # IPC commands
│   │   │   ├── state.rs             # AppState (Arc<Engine>)
│   │   │   ├── events.rs            # 事件发射
│   │   │   └── tray.rs              # 系统托盘
│   │   ├── icons/                    # 应用图标 (SVG + PNG + ICNS + ICO)
│   │   ├── capabilities/
│   │   └── tauri.conf.json
│   │
│   ├── feiq-relay/                   # Relay 服务器 (独立 crate)
│   │   ├── src/
│   │   │   ├── main.rs              # CLI 入口 (clap: --host --port)
│   │   │   ├── server.rs            # 核心 RelayServer (WebSocket 路由 + 房间)
│   │   │   └── lib.rs               # 公开导出
│   │   └── tests/
│   │       └── integration_test.rs  # 2 个集成测试
│   │
│   └── feiq-gui/                     # React 前端
│       └── src/
│           ├── main.tsx
│           ├── App.tsx
│           ├── components/
│           │   ├── Sidebar.tsx       # 联系人列表 + 搜索 + 在线数 + 未读徽标
│           │   ├── ChatPanel.tsx     # 聊天头部 + 消息列表 + 输入区
│           │   ├── MessageBubble.tsx # 文本/抖动/文件气泡 + 表情内联渲染
│           │   ├── InputArea.tsx     # 文本输入 + 表情选择器 + 发送按钮
│           │   ├── EmojiPicker.tsx   # 16×6 表情网格 + 悬浮预览
│           │   └── SettingsDialog.tsx# 配置 (用户名/主机/连接模式/Relay URL/网段/Enter发送)
│           └── stores/
│               ├── contactStore.ts   # Zustand: 联系人列表 + 增删改选
│               └── messageStore.ts   # Zustand: 按 IP 索引的消息 + 未读数
```

---

## 五、实现阶段

### Phase 1 — MVP: 基础消息 ✅ 已完成

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust 协议层 | IPMSG 全部常量、Fellow/Content/Post 类型、encoding_rs GBK转换、pack/parse 序列化 | ✅ |
| Rust 网络层 | UDP 端口2425、广播支持、MAC地址获取(mac_address crate)、自我消息过滤 | ✅ |
| Rust 引擎 | 启停、send_text、send_im_online、联系人自动发现、ansi_entry 应答 | ✅ |
| Rust 存储 | QSettings 风格 INI 配置 (用户名/主机名/自定义网段) | ✅ |
| Tauri | 脚手架、AppState、start_engine/get_contacts/send_message commands | ✅ |
| 前端 | 联系人侧边栏(在线/离线条目)、聊天面板(文本气泡)、消息输入框(Enter发送) | ✅ |
| **交付** | 两台机器同一局域网互相发现、收发文本、上线/下线通知 | ✅ |

### Phase 2 — 文件传输 + 表情 ✅ 已完成

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust | TCP 异步文件传输、FileTask 状态机(NotStart→Running→Finish/Error/Canceled)、进度节流(1%/100KB) | ✅ |
| Rust | SendFileContent/RecvFile 协议 | ✅ |
| Rust | 96个QQ表情定义、KnockContent、SendKnockContent | ✅ |
| Rust | download_file、cancel_task | ✅ |
| 前端 | 文件传输对话框(实时进度条) | ✅ |
| 前端 | 文件消息气泡(可点击下载) | ✅ |
| 前端 | 拖拽发送文件 | ✅ |
| 前端 | 表情选择器(6x16网格)、消息中表情渲染(/:) → GIF) | ✅ |
| 前端 | 窗口抖动动画(CSS shake) | ✅ |
| **交付** | 文件收发+进度、表情选择+渲染、窗口抖动 | ✅ |

> ✅ 已实现

### Phase 3 — 聊天记录 + 截图 + 用户管理 ✅ 已完成

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust | SQLite 聊天记录(自动保存所有收发消息) | ✅ |
| Rust | 分页查询(get_chat_history) | ✅ |
| Rust | 联系人搜索(名称/IP，无拼音) | ✅ |
| Rust | 联系人分组存储、备注名、个性签名字段（数据模型已定义、无 setter/UI） | ✅ |
| Rust | 截图命令(调用平台截图工具) | ❌ (removed) |
| 前端 | 无限滚动加载历史消息 | ✅ |
| 前端 | 消息内搜索 | ✅ |
| 前端 | 日期分隔线 | ✅ |
| 前端 | 切换联系人时自动加载最近 100 条历史 | ✅ |
| 前端 | 截图标注工具(Canvas) | ❌ (removed) |
| 前端 | 联系人分组树形视图 | ✅ |
| 前端 | 备注名编辑 UI、个性签名显示 | ✅ |
| **交付** | 完整聊天记录、联系人分组管理 | ✅ |

> ✅ 已实现（截图功能已移除，commit 85e23b4）

### Phase 4 — 群聊 + 离线消息 ✅ 100% (文件夹传输已移除)

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust | 离线消息缓存(对方离线时暂存SQLite，上线后自动重发) | ✅ |
| Rust | 群组元数据 SQLite 存储(群名/成员列表) | ✅ |
| Rust | P2P 群组分发(逐人发送消息/文件) | ✅ |
| Rust | 黑名单过滤、垃圾消息检测 | ✅ |
| 前端 | 群组创建对话框 | ✅ |
| 前端 | 群聊视图(显示发送者名称前缀) | ✅ |
| 前端 | 群发消息 | ✅ |
| **交付** | 群聊、离线消息、黑名单 | ✅ 100% |

> 文件夹传输已从代码库移除（commit f6c7616），详见 `UNIMPLEMENTED_FEATURES.md`

### Phase 4.5 — 🚀 Relay 服务器 ✅ 已完成 (v0.1.4)

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust relay server | 独立 `feiq-relay` crate。WebSocket 服务器，7 种 JSON 消息类型。房间模型 | ✅ |
| Rust relay client | `network/relay.rs` (432 行)。自动重连 (exponential backoff) | ✅ |
| Engine hybrid mode | 同时运行 LAN (UDP) + Relay (WebSocket) transport，跨 transport 去重 | ✅ |
| Settings | `ConnectionMode` 枚举: LanOnly / RelayOnly / Hybrid | ✅ |
| Frontend UI | SettingsDialog 中连接模式选择器 + Relay URL 输入框 | ✅ |
| 离线消息 | 服务器内存队列，24h TTL，客户端重连后自动推送 | ✅ |
| 加密 | ECDH+AES 端到端加密对 relay 透明 | ✅ |
| **交付** | 跨网络通信、混合模式、relay 服务器独立部署 | ✅ |

### Phase 5 — 加密 + 文件共享 + 打磨 ✅ 已完成

| 模块 | 内容 | 状态 |
|------|------|:---:|
| Rust | ECDH (x25519) 密钥交换 + AES-256-GCM 加密 (`ring` crate) | ✅ |
| Rust | 断点续传 (offset 恢复) | ✅ |
| Rust | `IPMSG_ENCRYPTOPT` 标记接入消息管线 | ✅ |
| Rust | 密封消息 (阅后即焚)、文件共享服务 | ✅ |
| Rust | 传输速度限制 (token bucket) | ❌ (未实现) |
| 前端 | 加密会话标识 (🔒图标)、密封消息倒计时 UI | ✅ |
| 前端 | 文件共享浏览对话框 | ✅ |
| 前端 | 自定义主题 (明/暗 + 多配色) | ✅ |
| 前端 | 系统托盘未读角标、聊天记录 JSON 导出/导入 | ✅ |
| **交付** | 端到端加密、文件共享、暗色主题、全平台发布 | ✅ |

> ✅ 已实现

---

## 六、已验证的技术假设

| 假设 | 方法 | 结果 |
|------|------|------|
| `encoding_rs` GBK 编解码 | `cargo run` 实测 GBK ↔ UTF-8 往返 | ✅ 完美，与 iconv 等价 |
| `mac_address` crate | 本机 `cargo run` 测试 | ✅ 正常获取 en0 MAC |
| Tauri 2 CLI 可用性 | `npm view @tauri-apps/cli version` | ✅ 2.11.3 稳定 |
| Tauri 插件 (notification/dialog/shortcut) | npm view 版本查询 | ✅ 全部 v2 稳定版 |
| `ring` 加密能力 | 编译验证 API | ✅ AES-256-GCM / ECDH 可用；❌ 无 RSA/Blowfish |
| `rsa` crate | `cargo check` 0.9.10 | ✅ 纯 Rust，稳定版 |
| `blowfish` crate | crates.io 查询 | ✅ 0.10.0 纯 Rust |
| 截图工具 | `which screencapture` | ✅ macOS 自带 |

## 七、关键技术挑战与方案

| 挑战 | 方案 |
|------|------|
| **GBK/UTF-8 编码** | `encoding_rs` 纯 Rust，Mozilla 出品，Firefox 同款。✅ 已实测验证 |
| **图片传输** | 原协议 `IPMSG_SENDIMAGE` (0xC0) 只传 8 个 ASCII 字符的图片 ID，实际图片数据传输通道从未被破解。**决策**：feiq++ 之间用文件传输通道发送图片，接收端检测 jpg/png/gif 扩展名自动内联预览。与旧客户端互发时友好提示用文件方式 |
| **文件夹传输** | `IPMSG_GETDIRFILES` (0x62) 协议已定义。发送方递归扫描 → 生成文件清单(manifest) → TCP 流式发送每个文件(长度前缀帧)。接收方解析清单 → 创建目录结构 → 逐文件写入 |
| **群聊 (无服务器)** | 群组存在本地 SQLite。发消息时遍历群成员逐人 UDP 发送。前端按 `[群名] 发送者:` 格式聚合显示 |
| **加密** | **决策**：仅 feiq++ 之间加密，不做 IPMSG v9 老旧加密栈兼容。方案：ECDH (x25519) 密钥交换 + AES-256-GCM 对称加密。`ring` crate 原生支持这两个算法。旧客户端走明文 |
| **跨平台 MAC 地址** | `mac_address` crate，macOS/Win/Linux 统一 API。✅ 已实测验证 |
| **大文件 OOM** | TCP 64KB 分块流式读写，永不加载完整文件到内存。feiq++ 间用 64KB，与旧客户端通信降级到它们的块大小 |
| **截图跨平台** | macOS: `screencapture -i`; Windows: Win+Shift+S; Linux: `maim -s` / `gnome-screenshot -a` |
| **离线消息** | 对方不在线时存入 SQLite `pending_messages` 表，对方上线(收到 BR_ENTRY)后自动重发 |
| **断点续传** | IPMSG GETFILEDATA 请求支持 `offset` 参数，TCP 发送方 `seek(offset)` 后继续传输 |
| **版本字符串** | 原格式 `1_lbt6_0#128#MAC#0#0#0#4001#9`。feiq++ 使用 `feiq_plus_plus#128#MAC#0#0#0#1#9` 标识自己，可与旧客户端区分 |
| **跨网络通信** | 自建 Rust WebSocket relay server。客户端通过 `ContactSource::Lan` / `ContactSource::Relay` 区分来源，MAC+name 跨 transport 去重。7 种 JSON 消息类型 (Join/Joined/PeerJoin/PeerLeave/RelayMessage/Ack/Error)。服务端 24h TTL 离线队列 |
| **Hybrid 模式** | Engine 同时运行 LAN (UDP) 和 Relay (WebSocket) transport，同一 mpsc channel 产出 NetworkEvent。两种 transport 完全对等 — engine 不关心消息来自哪个 transport |

---

## 八、关键依赖

### Rust (feiq-core)

```
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"         # WebSocket 客户端 (relay)
futures-util = "0.3"               # Stream/Sink 组合子
base64 = "0.22"                     # 载荷编码 (relay 传输)
encoding_rs = "0.8"                # GBK/UTF-8 ✅ 已验证
rusqlite = { version = "0.32", features = ["bundled"] }
ring = "0.17"                       # AES-256-GCM + ECDH (x25519) ✅ 已验证
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
mac_address = "1.1"                 # 跨平台 MAC ✅ 已验证
bitflags = "2"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

# P5 可选：IPMSG v9 老旧加密兼容
# rsa = "0.9"                    # RSA 密钥交换 (纯 Rust)
# blowfish = "0.10"              # Blowfish 对称加密 (纯 Rust)
# sha1 = "0.11"                  # SHA-1 签名
# md5 = "0.8"                    # MD5 签名
```

### Rust (feiq-relay)

```
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"         # WebSocket 服务器
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
clap = { version = "4", features = ["derive"] }
```

### Tauri (feiq-app)

```
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-notification = "2"    # 系统通知 ✅ v2.3.3
tauri-plugin-global-shortcut = "2" # 全局快捷键 ✅ v2.3.2
tauri-plugin-dialog = "2"          # 原生文件对话框 ✅ v2.7.1
tauri-plugin-fs = "2"              # 文件系统访问
```

### 前端

```
react 18, react-dom 18
zustand 4                        # 状态管理
tailwindcss 3                    # 工具类样式
lucide-react                     # 图标
@radix-ui/react-dialog           # 对话框
@radix-ui/react-popover          # 弹出层(表情选择器)
@radix-ui/react-progress         # 进度条
@radix-ui/react-tabs             # 标签页(文件传输)
@radix-ui/react-toast            # 提示
@radix-ui/react-tooltip          # 工具提示
@tauri-apps/api ^2.0.0
@tauri-apps/plugin-notification ^2.0.0
@tauri-apps/plugin-global-shortcut ^2.0.0
@tauri-apps/plugin-dialog ^2.0.0
```

---

---

## 九、经典飞秋 UI 设计参考（联网调研收集）

> 飞秋界面仿早期 QQ 风格，蓝灰色调，双栏布局，支持换肤/自定义字体。
> 以下基于飞秋官方网站、百度百科、CSDN 教程、ZOL 软件百科等整合。

### 9.1 主窗口布局

飞秋主窗口采用 QQ 式上下结构 + 分组树形列表：

- **标题栏**: 显示"飞秋 FeiQ"，含 [更换外观] 按钮（打开换肤面板：颜色面板/皮肤面板/风格面板/字体设置）
- **搜索栏**: 支持按用户名、组名、IP、**拼音首字母**搜索好友
- **好友列表** (主体): 分组折叠树，每项含头像 + 名称 + 个性签名。在线状态标识: 🟢在线 / 🔴离线 / ⚫离开。右键菜单: 添加好友、创建讨论组、群发消息、增加其他网段好友
- **底部快捷栏**: [聊天记录] [查找好友] [群聊] [设置]
- **状态栏**: 在线人数统计
- **系统托盘**: 最小化到托盘，新消息时图标闪烁。右键菜单: 隐身/离线切换、备份/还原数据

### 9.2 聊天对话框布局

飞秋聊天对话框采用**左右分栏**设计（独有特色）：

**左侧 (聊天区, ~70%宽度)**:
- 消息显示区: 按时间戳显示对话，支持富文本、表情内联、文件链接
- 文本输入区: 支持多行输入
- 工具栏: 😊表情 | 🖊涂鸦 | ✂截图 | 📎发送文件 | [发送▼]下拉(设置回执)
- 底部: [聊天记录] [全部聊天] [字体设置]

**右侧 (文件面板, ~30%宽度)**:
- "发送文件"区: 显示待发送文件列表，支持拖拽添加。每个文件显示名称+大小
- "接收文件"区: 对方发来的文件，含 [全部接收] [全部拒收] [中断传送] 按钮
- "已接收文件"区: 已完成列表，右键菜单可打开目录/打开文件/删除

### 9.3 表情选择器

- 96 个内置 QQ 风格 GIF 动画表情（代码如 `/:)`, `/:~`, `/:love`，描述如微笑/撇嘴/爱情）
- 6x16 网格分页展示
- 标签页切换: 默认表情 | 自定义表情 | 导入表情
- 三种添加方式: 本地文件、导入 QQ 表情目录、右键聊天图片"增加至表情库"
- 右键删除表情
- 2013 版新增表情分组功能

### 9.4 文件传输监视器

独立弹窗，三区域展示：
- **发送中**: 文件名、目标用户、进度条+百分比、实时速度 (MB/s)、[取消] 按钮
- **接收中**: 同上，[取消]/[接收] 按钮
- **已完成**: 文件名列表，[打开] 按钮，[清空完成] 按钮
- 特色: 局域网速度可达 **10-100MB/s**、支持 **4GB+** 大文件、**断点续传**（秒传已传部分）、**速度限制**

### 9.5 系统设置对话框

标签页形式:
- **个人资料**: 用户名、组名、头像、个性形象照片、个性签名、联系方式
- **发送/接收**: 消息弹窗开关、回执设置、Enter/Ctrl+Enter切换、缓冲区大小、图片压缩选项
- **网络**: 绑定网卡/IP/MAC、端口(2425)、群聊组播地址(D类IP)、自定义广播IP段
- **其他**: 聊天记录保存路径、开机自启、关闭到托盘、截图快捷键(默认无，可自定义)

### 9.6 换肤/外观系统

- 预设多套配色 (蓝/粉/黑等)
- 自定义颜色面板
- 支持图片做皮肤背景 (DIY 皮肤)
- 窗口透明度调节 (滑块)
- 字体类型+大小自定义

### 9.7 截图工具

- 快捷键激活 (Ctrl+Shift+A 或自定义)
- 全屏变暗 + 十字光标选择区域
- 自动识别窗口边界
- Shift 键微调选区大小
- ESC 取消
- 截图后编辑工具: 矩形框/箭头/文字/画笔/马赛克/撤销/[保存]/[发送]

### 9.8 群聊窗口

- 无服务器聊天室，组播自动发现
- 窗口显示: 群名 + 成员列表(人数)
- 消息带发送者名称前缀
- 系统提示: "某某加入了群聊"、"某某上传了文件"
- 工具栏: [群文件] [群成员] [群公告]
- 群消息提醒可配置: 弹窗/仅计数/气泡提示
- 跨网段需互相添加 IP

### 9.9 其他 UI 细节

- **涂鸦窗口**: 独立绘图窗口，铅笔/橡皮/颜色选择，绘图结果直接发送
- **在线用户列表弹窗**: 双击状态栏在线人数可查看完整在线用户列表
- **消息到来时**: 可选直接弹出对话框 or 仅右下角通知 or 只显示未读数
- **传输缓冲**: 发送/接收缓冲区可调 (传输假死问题解决方案)
- **右键菜单丰富**: 好友列表右键、聊天区右键、文件面板右键各有一套菜单

### 9.10 设计对齐要点

| 设计要素 | 飞秋风格 | feiq++ 采纳建议 |
|---------|---------|----------------|
| 整体色调 | 蓝灰色调，仿 QQ 2008 | 现代扁平蓝白，保留品牌蓝色 |
| 布局模式 | 列表+对话双栏 | 双栏 + 文件面板按需展开 (响应式) |
| 好友列表 | 分组折叠 + 头像圆点 | 分组折叠 + 头像 + 状态圆点 + 未读徽标 |
| 对话窗口 | 左右分栏 (文件面板常驻) | 文件面板默认折叠，有任务时 badge 提示展开 |
| 表情 | 96个GIF, 6x16网格 | 复用96表情映射表，PNG静态(避GIF性能坑) |
| 文件传输 | 独立监视器弹窗 | 对话内嵌入进度 + 侧边栏传输管理 |
| 主题 | 颜色+图片换肤 | Tailwind CSS变量，明/暗 + 6主题色 |
| 通知 | 托盘闪烁+弹窗 | 系统原生通知 + Dock/Taskbar 角标 |
| 托盘 | 右键隐身/离线 | 系统托盘 + 快捷状态切换 |
| 快捷键 | Enter发送(可配) | 保留可配置Enter/Ctrl+Enter |

---

## 十、验证方法

1. `cargo build --workspace` 三平台编译通过
2. 两台设备同一局域网：自动发现、文本收发、表情渲染一致
3. 文件传输：大文件(>1GB)进度显示正确、MD5 校验一致
4. 与原始飞鸽传书 (IPMSG Windows/Mac) 互通：文本、文件、在线状态
5. 与原始飞秋 (FeiQ Windows) 互通：文本、表情、文件、抖动
6. 群聊：3人以上群组消息送达、文件分发
7. 加密：feiq++ 之间端到端加密通信，第三方抓包不可读
8. 离线：B 离线时 A 发消息，B 上线后自动收到

---

> **调研来源**: 飞秋百度百科、飞鸽传书协议规范 (Draft-9, 1996-2003)、ZOL软件百科、CSDN/博客园技术文章、原 feiq 项目源码
