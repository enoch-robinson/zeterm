# Zeterm 设计文档

> 基于 Rust + GPUI 的现代化 SSH 终端平台

---

## 📚 文档索引

### 核心文档

| 文档 | 说明 |
|------|------|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | 架构总览（分层设计 + 组件分工 + 数据流） |
| [API.md](./API.md) | 完整 API 定义（Trait + 实体 + 错误类型） |
| [IMPLEMENTATION.md](./IMPLEMENTATION.md) | 实现指南（开发阶段 + 技术要点） |
| [USER_GUIDE.md](./USER_GUIDE.md) | 用户指南（安装 + 使用 + 快捷键） |
| [PERSISTENCE.md](./PERSISTENCE.md) | 配置与持久化（目录结构 + 配置项 + 数据库） |

### 模块文档

| 文档 | 说明 |
|------|------|
| [SSH 后端实现](./modules/ssh-backend.md) | 基于 russh 的 SSH 连接实现 |
| [SFTP 文件管理](./modules/sftp.md) | 远程文件系统访问与管理 |

### 核心抽象

| 文档 | 说明 |
|------|------|
| [连接状态机](./core/state-machine.md) | 连接生命周期状态与转换规则 |
| [错误处理策略](./core/error-handling.md) | 错误类型定义与处理策略 |

### 参考文档

| 目录 | 说明 |
|------|------|
| `research/` | 研究资料（Zed 终端分析等） |
| `reference/` | 速查资料（按键映射、术语表等） |

---

## 🏗️ 架构概览

Zeterm 采用**五层架构**设计，实现 UI 层与网络层的彻底解耦：

| 层级 | 职责 | 核心组件 |
|------|------|----------|
| 表现层 | UI渲染、事件捕获 | TerminalView, TabView, SplitView |
| 应用层 | 用例协调、状态管理 | SessionCoordinator, TabManager |
| 领域层 | 业务实体、Trait 抽象 | TerminalConnection |
| 适配器层 | 协议转换 | SshAdapter |
| 基础设施层 | IO、外部服务 | SshConnection, SQLite |

> 📖 详细架构设计见 [ARCHITECTURE.md](./ARCHITECTURE.md)

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

## 🧩 组件分工

| 组件类型 | 来源 | 用途 |
|----------|------|------|
| **终端渲染器** | 自实现（参考 Zed） | 字符网格、光标、选择 |
| **Tab/分屏** | 自实现 | 多标签页、分屏布局 |
| **其他 UI 组件** | gpui-component | Modal、Input、Button、Theme 等 |

> ⚠️ 终端渲染器、Tab、分屏均为自实现，gpui-component 不提供这些能力。
>
> 📖 详细实现见 [ARCHITECTURE.md](./ARCHITECTURE.md)

---

## 🔗 相关资源

- [GPUI 文档](https://docs.rs/gpui)
- [GPUI Component](https://github.com/longbridge/gpui-component)
- [Zed Terminal 源码](https://github.com/zed-industries/zed/tree/main/crates/terminal_view)
- [Alacritty Terminal](https://github.com/alacritty/alacritty)
- [Russh](https://github.com/warp-tech/russh)