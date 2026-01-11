# Zeterm 总体设计

> 基于 Rust + GPUI 的现代化SSH 终端平台

---

## 一、设计愿景

**Zeterm** 是一个基于 GPUI 框架的跨平台 SSH 终端客户端，目标是成为开发者日常使用的高效工具。

### 核心特性

| 特性 | 说明 |
|------|------|
| 🚀 高性能 | 基于 Rust 和 GPU 加速渲染 |
| 🔌 可扩展 | Trait抽象支持多种连接后端 |
| 🧪 可测试 | 分层架构便于单元测试 |
| 🎨 现代 UI | 基于 Zed 编辑器的 GPUI 框架 |

---

## 二、五层架构

```
┌─────────────────────────────────────────────────────────────┐
│                表现层 (Presentation)                    │
│GPUI Views│
├─────────────────────────────────────────────────────────────┤
│                    应用层 (Application)                     │
│                   Use Cases & Coordinators                  │
├─────────────────────────────────────────────────────────────┤
│                    领域层 (Domain)                          │
│                 Entities, Traits & Rules                    │
├─────────────────────────────────────────────────────────────┤
│                    适配器层 (Adapter)                       │
│                   Protocol Converters                       │
├─────────────────────────────────────────────────────────────┤
│                基础设施层 (Infrastructure)                  │
│                   IO & External Services                    │
└─────────────────────────────────────────────────────────────┘
```

>📖 详细设计见 [五层架构详解](./architecture/layers.md)

---

## 三、核心抽象

整个系统最关键的设计是 `TerminalConnection` Trait：

```rust
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), ConnectionError>;
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<(), ConnectionError>;
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>>;
    async fn close(&mut self) -> Result<(), ConnectionError>;
}
```

>📖 完整 API 定义（含详细注释）见 [API 文档](./api.md)

> 📖 详细设计见 [TerminalConnection Trait](./core/connection-trait.md)

---

## 四、关键组件

| 组件 | 层级 | 职责 |
|------|------|------|
| `TerminalView` | 表现层 | 终端渲染视图 (参考 Zed) |
| `SessionCoordinator` | 应用层 | 协调终端状态与连接管理 |
| `TerminalState` | 应用层 | 管理 Alacritty 终端状态 |
| `ConnectionManager` | 应用层 | 管理连接后端和数据泵 |
| `TerminalConnection` | 领域层 | 连接抽象 Trait |
| `ConnectionStateMachine` | 领域层 | 连接生命周期状态机 |
| `SshConnection` | 基础设施层 | SSH 后端实现 (russh) |

---

## 五、组件分工

### 5.1 终端渲染 vs窗口 UI

| 组件类型 | 来源 | 用途 |
|----------|------|------|
| **终端渲染器** | 参考 Zed `terminal_view` | 字符网格、光标、选择 |
| **窗口 UI 组件** | gpui-component | Tab、Dock、Modal、按钮等 |

>⚠️ 终端渲染器需参考 Zed 实现，gpui-component 不提供终端渲染能力。

### 5.2 gpui-component 复用

| 组件 | 用途 |
|------|------|
| `Tab` | 多标签页管理 |
| `Dock` | 分屏布局 |
| `Tree` | 主机列表 |
| `Table` | SFTP 文件列表 |
| `Modal` + `Input` | 连接对话框 |
| `ContextMenu` | 右键菜单 |
| `Theme` | 颜色主题 |

---

## 六、技术栈

| 领域 | 技术选型 | 说明 |
|------|----------|------|
| UI框架 | GPUI (Zed) | GPU 加速渲染引擎 |
| UI 组件库 | gpui-component | 60+ 现成组件 |
| 终端模拟 | alacritty_terminal | 成熟的终端状态机 |
| SSH 协议 | russh | 纯 Rust 异步 SSH |
| 异步运行时 | Tokio | Rust 生态标准 |
| 数据存储 | SQLite (sqlx) | 轻量跨平台 |
| 配置格式 | TOML | 人类可读 |

---

## 七、设计原则

| 原则 | 说明 |
|------|------|
| 分层解耦 | UI 层与网络层通过 Trait 彻底解耦 |
| 异步优先 | 基于 Tokio 的全异步 IO 模型 |
| 类型安全 | 利用 Rust 类型系统在编译期捕获错误 |
| 可测试性 | 所有核心组件可Mock、可单测 |
| 职责分离 | 终端状态与连接管理解耦 |

---

## 八、文档导航

### 架构设计

- [五层架构详解](./architecture/layers.md)
- [数据流与线程模型](./architecture/data-flow.md)

### 核心抽象

- [TerminalConnection Trait](./core/connection-trait.md)
- [连接状态机](./core/state-machine.md)
- [错误处理策略](./core/error-handling.md)

### 功能模块

- [SessionModel](./modules/session-model.md)
- [TerminalView](./modules/terminal-view.md)
- [SSH 后端实现](./modules/ssh-backend.md)
- [SFTP 模块](./modules/sftp.md)

### 基础设施

- [配置管理](./infrastructure/config.md)
- [数据持久化](./infrastructure/persistence.md)

### 实现计划

- [实现路径](./roadmap.md)

---

## 九、快速开始

```bash
# 克隆项目
git clone https://github.com/example/zeterm.git
cd zeterm

# 构建运行
cargo run

# 运行测试
cargo test
```

---

## 十、参考资源

> 📖 完整的参考资源列表见 [README.md](./README.md#-相关资源)