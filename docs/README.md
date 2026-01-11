# Zeterm 设计文档

> 基于 Rust + GPUI 的现代化SSH 终端平台

---

## 📚 文档索引

### 总览

| 文档 | 说明 |
|------|------|
| [design.md](./design.md) | 总体设计概述 |
| [roadmap.md](./roadmap.md) | 实现路径与里程碑 |

### 架构设计 (`architecture/`)

| 文档 | 说明 |
|------|------|
| [layers.md](./architecture/layers.md) | 四层架构详解 |
| [data-flow.md](./architecture/data-flow.md) | 数据流与线程模型 |

### 核心抽象 (`core/`)

| 文档 | 说明 |
|------|------|
| [connection-trait.md](./core/connection-trait.md) | TerminalConnection Trait 设计 |
| [state-machine.md](./core/state-machine.md) | 连接状态机设计 |
| [error-handling.md](./core/error-handling.md) | 错误处理策略 |

### 功能模块 (`modules/`)

| 文档 | 说明 |
|------|------|
| [session-model.md](./modules/session-model.md) | SessionModel 会话模型 |
| [terminal-view.md](./modules/terminal-view.md) | TerminalView 渲染视图 |
| [ssh-backend.md](./modules/ssh-backend.md) | SSH 后端实现 (russh) |
| [sftp.md](./modules/sftp.md) | SFTP 文件管理模块 |

### 基础设施 (`infrastructure/`)

| 文档 | 说明 |
|------|------|
| [config.md](./infrastructure/config.md) | 配置管理系统 |
| [persistence.md](./infrastructure/persistence.md) | 数据持久化设计 |

---

## 🎯 设计原则

1. **分层解耦** - UI 层与网络层通过 Trait 彻底解耦
2. **异步优先** - 基于 Tokio 的全异步 IO 模型
3. **类型安全** - 利用 Rust 类型系统在编译期捕获错误
4. **可测试性** - 所有核心组件可Mock、可单测

---

## 🔗 相关资源

- [GPUI 文档](https://docs.rs/gpui)
- [Alacritty Terminal](https://github.com/alacritty/alacritty)
- [Russh](https://github.com/warp-tech/russh)
- [Zed Editor](https://github.com/zed-industries/zed)