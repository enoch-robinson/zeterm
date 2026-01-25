# Zeterm 产品开发实现步骤

> 基于 Rust + GPUI 的现代化SSH 终端平台 - 完整开发指南

---

## 一、产品概述

### 1.1 产品定位

**Zeterm** 是一个基于 GPUI 框架的跨平台 SSH 终端客户端，目标是成为开发者日常使用的高效工具。

### 1.2 核心特性

| 特性 | 说明 |
|------|------|
| 🚀 高性能 | 基于 Rust 和 GPU 加速渲染 |
| 🔌 可扩展 | Trait抽象支持多种连接后端 (SSH/Local PTY/Telnet) |
| 🧪 可测试 | 分层架构便于单元测试和Mock |
| 🎨 现代 UI | 基于 Zed 编辑器的 GPUI 框架 |
| 📁 文件管理 | 集成 SFTP 文件传输功能 |
| 🔐 安全存储 | 系统密钥链保护敏感信息 |

### 1.3 目标用户

- 后端开发工程师
- 运维工程师
- DevOps 工程师
- 需要频繁 SSH 连接的技术人员

---

## 二、技术架构

### 2.1 五层架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    表现层 (Presentation)                    │
│                      GPUI Views                             │
│         TerminalView | SftpView | HostListView              │
├─────────────────────────────────────────────────────────────┤
│                    应用层 (Application)                     │
│                   Use Cases & Coordinators                  │
│      SessionCoordinator | WorkspaceManager | ConnectionStore│
├─────────────────────────────────────────────────────────────┤
│                      领域层 (Domain)                        │
│                 Entities, Traits & Rules                    │
│    TerminalConnection | HostConfig | ConnectionState        │
├─────────────────────────────────────────────────────────────┤
│                     适配器层 (Adapter)                       │
│                   Protocol Converters                       │
│              SshAdapter | EventStreamConverter              │
├─────────────────────────────────────────────────────────────┤
│                  基础设施层 (Infrastructure)                  │
│                   IO & External Services                    │
│       SshConnection | SqliteRepository | SftpClient         │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 层级职责

| 层级 | 职责 | 核心组件 |
|------|------|----------|
| 表现层 | UI 渲染、事件捕获、无业务逻辑 | TerminalView, SftpView |
| 应用层 | 用例协调、状态管理、事件分发 | SessionCoordinator |
| 领域层 | 业务实体、Trait 抽象、业务规则 | TerminalConnection |
| 适配器层 | 协议转换、格式适配 | SshAdapter |
| 基础设施层 | 具体实现、外部交互 | SshConnection, SQLite |

### 2.3 依赖规则

- **向下依赖**: 上层只能依赖下层
- **依赖倒置**: 基础设施层依赖领域层接口，而非反向
- **接口隔离**: 每层只暴露必要接口

---

## 三、技术栈选型

### 3.1 核心依赖

| 领域 | 技术选型 | 版本 | 说明 |
|------|----------|------|------|
| UI 框架 | GPUI | 0.2 | Zed 编辑器的 GPU 加速渲染引擎 |
| UI 组件库 | gpui-component | 0.4 | 60+ 现成UI 组件 |
| 终端模拟 | alacritty_terminal | 0.24 | 成熟的终端状态机 |
| SSH 协议 | russh | 0.45 | 纯 Rust 异步 SSH 实现 |
| 异步运行时 | Tokio | 1.x | Rust 生态标准异步运行时 |
| 数据存储 | SQLite (sqlx) | 0.8 | 轻量级跨平台数据库 |
| 配置格式 | TOML | 0.8 | 人类可读的配置格式 |
| 错误处理 | thiserror | 2.x | 类型安全的错误定义 |
| 日志系统 | tracing | 0.1 | 结构化日志框架 |

### 3.2 组件分工说明

| 组件类型 | 来源 | 用途 |
|----------|------|------|
| **终端渲染器** | 参考 Zed 自行实现 | 字符网格、光标、选择 |
| **窗口 UI 组件** | gpui-component | Tab、Dock、Modal、按钮等 |

>⚠️ **重要**: 终端渲染器需参考 Zed 实现，gpui-component 不提供终端渲染能力。

---

## 四、项目结构

### 4.1 Cargo Workspace布局

```
zeterm/
├── Cargo.toml                # Workspace 配置
├── crates/
│   ├── zeterm/# 主程序 (表现层 + 应用层)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs        # 程序入口
│   │       ├── ui/            # 表现层
│   │       │   ├── mod.rs
│   │       │   ├── terminal_view.rs
│   │       │   ├── terminal_element.rs
│   │       │   ├── sftp_view.rs
│   │       │   ├── host_list.rs
│   │       │   └── workspace.rs
│   │       └── app/           # 应用层
│   │           ├── mod.rs
│   │           ├── session_coordinator.rs
│   │           ├── terminal_state.rs
│   │           └── connection_manager.rs
│   │
│   ├── zeterm-core/           # 领域层
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits/# 核心 Trait
│   │       │   ├── mod.rs
│   │       │   ├── connection.rs
│   │       │   └── repository.rs
│   │       ├── entities/      # 业务实体
│   │       │   ├── mod.rs
│   │       │   ├── host.rs
│   │       │   └── session.rs
│   │       ├── state/         # 状态定义
│   │       │   ├── mod.rs
│   │       │   └── connection_state.rs
│   │       └── errors/        # 错误类型
│   │           ├── mod.rs
│   │           └── types.rs
│   │
│   ├── zeterm-ssh/            # SSH 后端 (适配器 + 基础设施)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs  # SshConnection 实现
│   │       ├── adapter.rs     # 协议适配
│   │       ├── auth.rs        # 认证处理
│   │       └── sftp.rs        # SFTP 客户端
│   │
│   ├── zeterm-storage/        # 持久化 (基础设施)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite.rs      # SQLite 实现
│   │       └── migrations/    # 数据库迁移
│   │
│   └── zeterm-mock/           # Mock 实现 (测试用)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── mock_connection.rs
│
├── docs/                      # 文档目录
├── migrations/                # 数据库迁移脚本
└── resources/                 # 资源文件 (图标、字体等)
```

### 4.2 模块依赖关系

```
                    ┌─────────────────┐
                    │     zeterm      │
                    │     (主程序)     │
                    └────────┬────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
          ▼                  ▼                  ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  zeterm-ssh     │ │ zeterm-storage  │ │  zeterm-mock    │
│(SSH 后端)       │ │  (持久化)         │ │  (Mock 测试)    │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │   zeterm-core   │
                    │   (领域层)       │
                    └─────────────────┘
```

---

## 五、开发阶段规划

### 5.1 阶段总览

```
Phase 1        Phase 2        Phase 3        Phase 4        Phase 5
核心骨架   ──► 终端渲染   ──►  SSH 接入   ──►  交互完善  ──►  产品化
 (2周)          (2周)          (2周)          (1周)          (2周)
   ││              │              │              │▼              ▼              ▼
MockConnectionTerminalView  真实SSH连接    鼠标/复制粘贴   主机管理
Alacritty集成   (参考Zed)     密码/密钥认证  窗口ResizeSFTP/Tab
gpui-component  字符网格渲染                滚动缓冲区      (gpui-component)
```

**总工期: 约 9 周**

### 5.2 开发原则

| 原则 | 说明 |
|------|------|
| 渐进式开发 | 从最小可行产品开始，逐步增强|
| Mock驱动 | 先用Mock 验证架构，再接入真实实现 |
| 里程碑驱动 | 每个阶段有明确的可验证目标 |
| 测试先行 | 核心模块必须有单元测试覆盖 |

---

## 六、Phase 1: 核心骨架 (2周)

### 6.1 阶段目标

搭建项目基础架构，验证核心抽象设计，确保各层之间的协作正常。

### 6.2 核心交付物

| 交付物 | 说明 |
|--------|------|
| Cargo Workspace | 完整的项目结构 |
| GPUI 基础窗口 | 可运行的空白窗口 |
|TerminalConnection Trait | 核心抽象接口定义 |
| MockConnection | 测试用 Mock 后端 |
| Alacritty 集成 | 终端状态机集成 |
| SessionCoordinator | 会话协调器基础实现 |

### 6.3 技术要点

#### 6.3.1 TerminalConnection Trait 设计

这是整个系统最关键的抽象，定义了终端连接的统一接口：

- `write()`: 发送数据到远端
- `resize()`: 调整终端窗口大小
- `receive_stream()`: 获取数据接收流
- `close()`: 关闭连接

#### 6.3.2 MockConnection 实现

用于开发和测试阶段，无需真实 SSH 服务器：

- 使用 `mpsc::channel` 模拟数据流
- 支持注入测试数据
- 可配置输出频率和内容

#### 6.3.3 Alacritty 终端状态机

集成 `alacritty_terminal` 处理 ANSI 转义序列：

- 创建 `Term` 实例
- 实现 `EventListener` 处理终端事件
- 使用 `Arc<Mutex<Term>>` 实现线程安全访问

### 6.4 里程碑验证

-✅ 运行程序，看到 GPUI 窗口
- ✅ gpui-component 基础组件正常渲染
- ✅ MockConnection 每秒输出 "Hello World\n"
- ✅控制台打印 Alacritty 解析后的内容
- ✅ 单元测试通过

---

## 七、Phase 2: 终端渲染 (2周)

### 7.1 阶段目标

实现终端字符网格渲染，与 Alacritty 终端状态机对接，在屏幕上正确显示终端内容。

### 7.2 核心交付物

| 交付物 | 说明 |
|--------|------|
| TerminalView | 终端主视图组件 |
| TerminalElement | 底层渲染元素 |
| 字体度量系统 | 等宽字体单元格计算 |
| 字符渲染 | ASCII 和 Unicode 支持 |
| 颜色系统 | 256色/TrueColor 支持 |
| 光标渲染 | Block/Beam/Underline 样式 |

### 7.3 Zed 参考文件

必须深入研究 Zed 编辑器的终端实现：

```
zed/crates/terminal_view/src/
├── terminal_view.rs      # 主视图组件 →参考
├── terminal_element.rs   # 渲染元素 → 核心参考
└── persistence.rs        # 可忽略

zed/crates/terminal/src/
├── terminal.rs           # 终端模型 → 参考
└── mappings/             # 按键映射 → 参考
```

### 7.4 渲染架构

```
┌─────────────────────────────────────────────────────────┐
│  TerminalView (gpui::Render)                            │
│  ├──读取 SessionCoordinator 状态                         │
│  ├── 创建 TerminalElement                               │
│  └── 处理键盘/鼠标事件                                    │
├─────────────────────────────────────────────────────────┤
│  TerminalElement (gpui::Element)                        │
│  ├── prepaint(): 计算字体度量、布局                        │
│  └── paint(): 绘制背景、字符、光标                         │
├─────────────────────────────────────────────────────────┤
│  alacritty_terminal::Term                               │
│  └── 提供 renderable_content()                          │
└─────────────────────────────────────────────────────────┘
```

### 7.5 子阶段划分

| 子阶段 | 内容 | 时间 |
|--------|------|------|
| 2a | 基础字符网格渲染 (ASCII) | 3天 |
| 2b | Unicode/宽字符支持 | 3天 |
| 2c | 颜色和样式支持 | 2天 |
| 2d | 光标渲染 | 2天 |

### 7.6 技术要点

#### 7.6.1 字体度量计算

- 使用等宽字体 (如 JetBrains Mono)
- 计算单元格宽度和高度
- 处理基线位置

#### 7.6.2 Element绘制流程

1. **prepaint()**: 计算字体度量、布局尺寸
2. **paint_background()**: 填充终端背景色
3. **paint_selection()**: 高亮选中区域
4. **paint_cells()**: 逐单元格绘制字符
5. **paint_cursor()**: 绘制光标

#### 7.6.3 颜色处理

- 支持 16 色基础色
- 支持 256 色扩展色
- 支持 TrueColor (24位)
- 处理前景色和背景色

### 7.7 里程碑验证

- ✅屏幕显示终端背景
- ✅ Mock 输出的"Hello World" 正确显示
- ✅ 光标可见且位置正确
- ✅ 支持基本ANSI 颜色
- ✅ 字体渲染清晰 (等宽字体)

---

## 八、Phase 3: SSH 接入 (2周)

### 8.1 阶段目标

实现真实的 SSH 连接，替换MockConnection，支持密码和公钥认证。

### 8.2 核心交付物

| 交付物 | 说明 |
|--------|------|
| SshConnection | TerminalConnection 的 SSH 实现 |
| 密码认证 | 基础认证方式 |
| 公钥认证 | RSA/Ed25519 密钥支持 |
| SSH Agent | 系统密钥代理集成 |
| 主机密钥验证 | known_hosts 支持 |
| 连接状态机 | 完整的状态管理 |

### 8.3 russh 回调转流式方案

russh 使用回调模式，需要转换为流式接口：

```
russh Handler (回调)
        │
        │ data_tx.send(data)
        ▼
   mpsc::channel
        │
        │ ReceiverStream
        ▼
BoxStream<Vec<u8>> (流式)
```

### 8.4 认证流程

```
用户输入 → 认证方式选择 → 执行认证
                        │
    ┌───────────────────┼────┐
    ▼           ▼          ▼
  密码认证    公钥认证    Agent认证
    │           │           │
    └───────────┴───────────┘
                │
                ▼
          打开 PTY → 请求 Shell
```

### 8.5 连接状态机

| 状态 | 说明 |
|------|------|
| `Idle` | 空闲，未连接 |
| `Connecting` | 正在建立 TCP 连接 |
| `Authenticating` | TCP 已连接，正在认证 |
| `Connected` | 认证成功，会话可用 |
| `Disconnecting` | 正在优雅关闭 |
| `Disconnected` | 已断开 |
| `Reconnecting` | 自动重连中 |

### 8.6 技术要点

#### 8.6.1 SshHandler 实现

- 实现 russh 的`Handler` trait
- 在 `data()` 回调中发送数据到 channel
- 在 `eof()` 回调中关闭发送端

#### 8.6.2 认证方式支持

| 方式 | 方法 | 说明 |
|------|------|------|
| 密码 | `authenticate_password()` | 最简单，安全性较低 |
| 公钥 | `authenticate_publickey()` | 推荐，需加载私钥 |
| Agent | `authenticate_publickey_with()` | 使用系统 SSH Agent |

#### 8.6.3 主机密钥验证

- 首次连接提示用户确认
- 保存到 known_hosts 文件
- 后续连接自动验证

### 8.7 里程碑验证

- ✅ 成功连接到真实 SSH 服务器
- ✅ 密码认证工作正常
- ✅ 公钥认证工作正常
- ✅ 运行 htop，画面流畅，颜色正确
- ✅ 运行 vim，编辑文件正常

---

## 九、Phase 4: 交互完善 (1周)

### 9.1 阶段目标

完善用户交互体验，实现窗口调整、复制粘贴、鼠标支持等功能。

### 9.2 核心交付物

| 交付物 | 说明 |
|--------|------|
| 窗口 Resize | 终端尺寸同步 |
| 复制粘贴 | 系统剪贴板集成 |
| 鼠标支持 | 点击、选择、滚轮 |
| 滚动缓冲区 | 历史内容查看 |
| 快捷键系统 | 自定义快捷键 |

### 9.3 键盘映射

| 按键 | ANSI 序列 | 说明 |
|------|-----------|------|
| Enter | `\r` | 回车 |
| Backspace | `\x7f` | 退格 |
| Tab | `\t` | 制表符 |
| Escape | `\x1b` | 转义 |
| Arrow Up | `\x1b[A` | 上箭头 |
| Arrow Down | `\x1b[B` | 下箭头 |
| Arrow Right | `\x1b[C` | 右箭头 |
| Arrow Left | `\x1b[D` | 左箭头 |
| Ctrl+C | `\x03` | 中断 |
| Ctrl+D | `\x04` | EOF |
| Ctrl+Z | `\x1a` | 挂起 |

### 9.4 鼠标事件处理

| 事件 | 处理 |
|------|------|
| 左键按下 | 开始选择 |
| 拖动 | 更新选择范围 |
| 左键释放 | 结束选择 |
| 双击 | 选择单词 |
| 三击 | 选择整行 |
| 滚轮 | 滚动历史 |

### 9.5 Resize 流程

1. 检测视图尺寸变化
2. 计算新的行列数: `cols = view_width / cell_width`
3. 更新 TerminalState (Alacritty)
4. 通知远端(`ConnectionManager.resize()`)
5. 触发重绘

### 9.6 技术要点

#### 9.6.1 剪贴板集成

- 使用 GPUI 提供的剪贴板 API
- 复制:获取选中文本，写入剪贴板
- 粘贴: 读取剪贴板，发送到终端

#### 9.6.2 选择状态管理

- 记录选择起点和终点
- 支持字符级和行级选择
- 渲染时高亮选中区域

#### 9.6.3 滚动缓冲区

- 配置滚动行数(默认 10000)
- 滚轮事件调整偏移量
- 渲染时考虑滚动偏移

### 9.7 里程碑验证

- ✅ 拖拽窗口边缘，终端内容自适应
- ✅ Ctrl+Shift+C 复制选中内容
- ✅ Ctrl+Shift+V 粘贴剪贴板
- ✅ 鼠标点击定位光标 (支持的程序)
- ✅ 滚轮查看历史输出

---

## 十、Phase 5: 产品化 (2周)

### 10.1 阶段目标

完成产品级功能，实现主机管理、多标签页、SFTP 等功能，使产品可供日常使用。

### 10.2 核心交付物

| 交付物 | gpui-component | 说明 |
|--------|----------------|------|
| 主机列表 | ✅ `Table` + `Tree` | 侧边栏主机管理 |
| 配置系统 | - | TOML 配置文件 |
| Tab 管理 | ✅ `Tab` | 多标签页 |
| 分屏布局 | ✅ `Dock` | 水平/垂直分割 |
| SFTP 面板 | ✅ `Table` + `Tree` | 文件管理 |
| 主题系统 | ✅ `Theme` | 颜色主题切换 |
| 数据持久化 | - | SQLite 存储 |
| 连接对话框 | ✅ `Modal` + `Input` | 新建/编辑主机 |

### 10.3 UI 布局设计

```
┌─────────────────────────────────────────────────────────┐
│  [Tab1] [Tab2] [Tab3] [+][≡]                            │  ← Tab
├──────────┬──────────────────────────────────────────────┤
│          │                                              │  ← Dock
│  主机列表 │              终端区域                          │
│ (Tree)   │           (TerminalView)                     │
│          │                                              │
│▼生产环境  │user@server:~$ ls -la                         │
│   Server1│  total 32                                    │
│   Server2│  drwxr-xr-x 5 user user 4096 Jan 1 00:00 .   │
│          │                                              │
├──────────┴──────────────────────────────────────────────┤
│🟢 Connected | UTF-8 | 80x24                             │  ← StatusBar
└─────────────────────────────────────────────────────────┘
```

### 10.4 配置系统

#### 配置目录结构

```
~/.config/zeterm/              # Linux
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\              # Windows
├── config.toml                # 全局配置
├── hosts.toml                 # 主机列表
├── known_hosts                # SSH 主机密钥
└── themes/                    # 自定义主题
```

#### 主要配置项

| 分类 | 配置项 | 默认值 |
|------|--------|--------|
| 终端 | scrollback_lines | 10000 |
| 终端 | cursor_style | block |
| 外观 | font_family | JetBrains Mono |
| 外观 | font_size | 14.0 |
| 网络 | connect_timeout | 30s |
| 网络 | auto_reconnect | true |

### 10.5 数据持久化

#### 数据库表设计

| 表名 | 用途 |
|------|------|
| `hosts` | 主机配置存储 |
| `connection_history` | 连接历史记录 |
| `_meta` | 数据库版本信息 |

#### 敏感信息存储

| 平台 | 存储方式 |
|------|----------|
| macOS | Keychain |
| Windows | Credential Manager |
| Linux | Secret Service |

### 10.6 SFTP 功能

| 功能 | 优先级 |
|------|--------|
| 目录浏览 | P0 |
| 文件上传/下载 | P0 |
| 进度显示 | P1 |
| 拖放支持 | P1 |
| 断点续传 | P2 |

### 10.7 里程碑验证

- ✅ 可以添加、编辑、删除主机
- ✅ 双击主机快速连接
- ✅ 多标签页切换
- ✅ 分屏同时查看多个终端
- ✅ SFTP 上传下载文件
- ✅ 深色/浅色主题切换

---

## 十一、技术风险与应对

### 11.1 风险评估

| 风险 | 影响 | 概率 | 应对策略 |
|------|------|------|----------|
| Zed 渲染器理解困难 | Phase 2 延期 | 中| 预留额外时间，逐步理解源码 |
| russh兼容性问题 | Phase 3阻塞 | 低 | 提前调研，准备 thrussh 备选 |
| GPUI 学习曲线 | 整体延期 | 中 | 参考 gpui-component 示例 |
| 性能问题 | 用户体验差 | 中 | 持续性能测试，及时优化 |
| Unicode宽字符处理 | Phase 2 延期 | 中 | 参考 Zed 和Alacritty 实现 |
| 跨平台兼容性 | 发布延期 | 低 | 早期在多平台测试 |

### 11.2 风险缓解措施

#### Zed 渲染器理解困难

- 先阅读 Zed 终端相关文档和注释
- 从简单功能开始，逐步增加复杂度
- 必要时在社区寻求帮助

#### russh 兼容性问题

- Phase 3 开始前进行技术预研
- 准备 thrussh 作为备选方案
- 关注 russh 项目的 issue和更新

#### 性能问题

- 建立性能基准测试
- 使用 profiler 定位瓶颈
- 参考 Alacritty 的性能优化策略

---

## 十二、性能目标

### 12.1 关键指标

| 指标 | 目标值 | 测试方法 |
|------|--------|----------|
| 渲染延迟 | < 16ms (60fps) | 大量输出时测量帧率 |
| 输入延迟 | < 50ms | 按键到显示的时间 |
| 内存占用 | < 100MB (单会话) | 长时间运行监控 |
| 启动时间 | < 1s | 冷启动到可用 |
| 连接建立 | < 3s | SSH 握手完成时间 |

### 12.2 性能优化策略

| 策略 | 说明 |
|------|------|
| 批量渲染 | 合并多次数据更新为一次渲染 |
| 脏区域重绘 | 只重绘变化的区域 |
| 异步 IO | 网络操作不阻塞主线程 |
| 内存池 | 复用缓冲区减少分配 |

---

## 十三、相关文档

### 13.1 内部文档

| 文档 | 说明 |
|------|------|
| [design.md](./design.md) | 总体设计概述 |
| [roadmap.md](./roadmap.md) | 实现路径与里程碑 |
| [api.md](./api.md) | API 文档 |
| [architecture/layers.md](./architecture/layers.md) | 五层架构详解 |
| [architecture/data-flow.md](./architecture/data-flow.md) | 数据流与线程模型 |
| [core/connection-trait.md](./core/connection-trait.md) | TerminalConnection Trait |
| [core/state-machine.md](./core/state-machine.md) | 连接状态机 |
| [modules/terminal-view.md](./modules/terminal-view.md) | 终端渲染视图 |
| [modules/ssh-backend.md](./modules/ssh-backend.md) | SSH 后端实现 |

### 13.2 外部资源

| 资源 | 链接 |
|------|------|
| GPUI 文档 | https://docs.rs/gpui |
| gpui-component | https://github.com/longbridge/gpui-component |
| Zed Terminal源码 | https://github.com/zed-industries/zed/tree/main/crates/terminal_view |
| Alacritty Terminal | https://github.com/alacritty/alacritty |
| Russh | https://github.com/warp-tech/russh |

---

## 十四、版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v1.0 | - | 初始版本 |
