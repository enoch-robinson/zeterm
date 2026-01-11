# Zeterm 总体设计

> 利用 Rust 强大的类型系统（Traits）切断 UI 层与网络层的直接耦合，实现一个高性能、易维护、易测试的现代化SSH 终端平台。

---

## 一、设计愿景

**Zeterm** 是一个基于 GPUI 框架的跨平台 SSH 终端客户端，目标是成为开发者日常使用的高效工具。

### 核心特性

-🚀 **高性能** - 基于 Rust 和 GPU 加速渲染
- 🔌 **可扩展** - Trait 抽象支持多种连接后端
- 🧪 **可测试** - 分层架构便于单元测试
- 🎨 **现代 UI** - 基于 Zed 编辑器的GPUI 框架

---

## 二、架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                表现层 (Presentation)│
│GPUI Views│
├─────────────────────────────────────────────────────────────┤
│                   状态模型层 (Model)                         │
│                    App State & Logic│
├─────────────────────────────────────────────────────────────┤
│                适配器层 (Adapter)                        │
│                    Traits & Interfaces│
├─────────────────────────────────────────────────────────────┤
│                基础设施层 (Infrastructure)                 │
│                   IO & External Services│
└─────────────────────────────────────────────────────────────┘
```

>📖 详细设计见 [四层架构详解](./architecture/layers.md)

---

## 三、核心抽象

整个系统最关键的设计是 `TerminalConnection` Trait，它将UI 与具体连接实现彻底解耦：

```rust
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    async fn write(&mut self, bytes: &[u8]) -> Result<()>;
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>>;
    async fn close(&mut self) -> Result<()>;
}
```

> 📖 详细设计见 [TerminalConnection Trait](./core/connection-trait.md)

---

## 四、关键组件

| 组件 | 职责 | 文档 |
|------|------|------|
| `SessionModel` | 会话状态管理，连接 UI 与后端 | [详情](./modules/session-model.md) |
| `TerminalView` | 终端渲染视图 (移植自 Zed) | [详情](./modules/terminal-view.md) |
| `SshConnection` | SSH 后端实现 (russh) | [详情](./modules/ssh-backend.md) |
| `ConnectionStateMachine` | 连接生命周期状态机 | [详情](./core/state-machine.md) |

---

## 五、技术栈

| 领域 | 技术选型 | 说明 |
|------|----------|------|
| UI 框架 | GPUI (Zed) | GPU 加速渲染引擎 |
| UI 组件库 | gpui-component | 60+ 现成组件，Dock 布局 |
| 终端模拟 | alacritty_terminal | 成熟的终端状态机 |
| SSH 协议 | russh | 纯 Rust 异步 SSH |
| 异步运行时 | Tokio | Rust 生态标准 |
| 数据存储 | SQLite (sqlx) | 轻量跨平台 |
| 配置格式 | TOML | 人类可读 |

---

## 六、gpui-component 组件复用

| 组件 | 用途 |
|------|------|
| `Button`, `Input`, `Modal` | 连接对话框、设置界面 |
| `Table` | 主机列表、SFTP 文件列表 |
| `Tree` | 文件浏览器树形结构 |
| `Tab` | 多标签页管理 |
| `Dock` | 分屏布局（水平/垂直分割） |
| `Dropdown`, `ContextMenu` | 右键菜单、下拉选择 |
| `Notification`, `Toast` | 连接状态提示 |
| `Progress` | 文件传输进度 |

>📖 组件文档: https://docs.rs/gpui-component

---

## 七、文档导航

### 架构设计

- [四层架构详解](./architecture/layers.md) - 分层设计与职责划分
- [数据流与线程模型](./architecture/data-flow.md) - 数据流转与并发处理

### 核心抽象

- [TerminalConnection Trait](./core/connection-trait.md) - 连接接口设计
- [连接状态机](./core/state-machine.md) - 生命周期管理
- [错误处理策略](./core/error-handling.md) - 错误类型与传播

### 功能模块

- [SSH 后端实现](./modules/ssh-backend.md) - russh 集成方案
- [SFTP 模块](./modules/sftp.md) - 文件管理功能

### 基础设施

- [配置管理](./infrastructure/config.md) - 配置系统设计
- [数据持久化](./infrastructure/persistence.md) - 存储方案

### 实现计划

- [实现路径](./roadmap.md) - 分阶段开发计划与里程碑

---

## 八、快速开始

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

## 九、参考资源

- [GPUI 文档](https://docs.rs/gpui)
- [GPUI Component](https://github.com/longbridge/gpui-component) - UI 组件库
- [Alacritty Terminal](https://github.com/alacritty/alacritty)
- [Russh](https://github.com/warp-tech/russh)
- [Zed Editor](https://github.com/zed-industries/zed)