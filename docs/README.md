# Zeterm 设计文档

> 基于 Rust + GPUI 的现代化 SSH 终端平台

---

## 📚 文档索引

### 总览

| 文档 | 说明 |
|------|------|
| [design.md](./design.md) | 总体设计概述 |
| [roadmap.md](./roadmap.md) | 实现路径与里程碑 |
| [api.md](./api.md) | API 文档 ⭐ NEW |
| [quick-reference.md](./quick-reference.md) | 快速参考卡 ⭐ NEW |

### 架构设计 (`architecture/`)

| 文档 | 说明 |
|------|------|
| [layers.md](./architecture/layers.md) | 五层架构详解 |
| [data-flow.md](./architecture/data-flow.md) | 数据流与线程模型 |

### 核心抽象 (`core/`)

| 文档 | 说明 |
|------|------|
| [connection-trait.md](./core/connection-trait.md) | TerminalConnection Trait 设计 |
| [state-machine.md](./core/state-machine.md) | 连接状态机设计 |
| [error-handling.md](./core/error-handling.md) | 错误处理策略 |
| [testing.md](./core/testing.md) | 测试策略与Mock 指南 |
| [security.md](./core/security.md) | 安全设计与凭证保护 |

### 功能模块 (`modules/`)

| 文档 | 说明 |
|------|------|
| [session-model.md](./modules/session-model.md) | SessionCoordinator 会话模型 |
| [terminal-view.md](./modules/terminal-view.md) | TerminalView 渲染视图 (参考 Zed) |
| [terminal-rendering.md](./modules/terminal-rendering.md) | 终端渲染实现细节 ⭐ NEW |
| [key-mappings.md](./modules/key-mappings.md) | 按键映射表 ⭐ NEW |
| [zed-terminal-analysis.md](./modules/zed-terminal-analysis.md) | Zed 终端渲染技术分析 |
| [ssh-backend.md](./modules/ssh-backend.md) | SSH 后端实现 (russh) |
| [sftp.md](./modules/sftp.md) | SFTP 文件管理模块 |

### 基础设施 (`infrastructure/`)

| 文档 | 说明 |
|------|------|
| [config.md](./infrastructure/config.md) | 配置管理系统 |
| [persistence.md](./infrastructure/persistence.md) | 数据持久化设计 |
| [platform.md](./infrastructure/platform.md) | 跨平台差异处理 ⭐ NEW |

---

## 🏗️ 架构概览

Zeterm 采用**五层架构**设计，实现 UI 层与网络层的彻底解耦：

| 层级 | 职责 | 核心组件 |
|------|------|----------|
| 表现层 | UI渲染、事件捕获 | TerminalView, SftpView |
| 应用层 | 用例协调、状态管理 | SessionCoordinator |
| 领域层 | 业务实体、Trait 抽象 | TerminalConnection |
| 适配器层 | 协议转换 | SshAdapter |
| 基础设施层 | IO、外部服务 | SshConnection, SQLite |

> 📖 详细架构设计见 [五层架构详解](./architecture/layers.md)

---

## 🎯 设计原则

| 原则 | 说明 |
|------|------|
| 分层解耦 | UI 层与网络层通过 Trait 彻底解耦 |
| 异步优先 | 基于 Tokio 的全异步 IO 模型 |
| 类型安全 | 利用 Rust 类型系统在编译期捕获错误 |
| 可测试性 | 所有核心组件可 Mock、可单测 |
| 职责分离 | 终端状态与连接管理解耦 |

> 📖 详细设计见 [总体设计](./design.md)

---

##🧩 组件分工

| 组件类型 | 来源 | 用途 |
|----------|------|------|
| **终端渲染器** | 参考 Zed `terminal_view` | 字符网格、光标、选择 |
| **窗口 UI 组件** | gpui-component | Tab、Dock、Modal、按钮等 |

> ⚠️ 终端渲染器需参考 Zed 实现，gpui-component 不提供终端渲染能力。
>
> 📖 详细实现见 [TerminalView](./modules/terminal-view.md)

---

## 🔗 相关资源

- [GPUI 文档](https://docs.rs/gpui)
- [GPUI Component](https://github.com/longbridge/gpui-component)
- [Zed Terminal 源码](https://github.com/zed-industries/zed/tree/main/crates/terminal_view)
- [Alacritty Terminal](https://github.com/alacritty/alacritty)
- [Russh](https://github.com/warp-tech/russh)