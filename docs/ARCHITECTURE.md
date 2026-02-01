# Zeterm 架构设计

> 基于 Rust + GPUI 的 SSH 终端平台架构说明

---

## 一、架构总览

### 1.1 五层架构

```
┌─────────────────────────────────────────────────────────────┐
│                    表现层 (Presentation)                    │
│                         GPUI Views                         │
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

### 1.2 层级职责

| 层级 | 职责 | 核心组件 |
|------|------|----------|
| **表现层** | 纯渲染、事件捕获 | `TerminalView`, `TabView`, `SplitView` |
| **应用层** | 用例协调、状态管理 | `SessionCoordinator`, `TabManager`, `SplitManager` |
| **领域层** | 业务实体、Trait 抽象 | `TerminalConnection`, `ConnectionState` |
| **适配器层** | 协议转换 | `SshAdapter` |
| **基础设施层** | 具体实现、外部交互 | `SshConnection`, `SqliteHostRepository` |

### 1.3 依赖规则

```
表现层 ──► 应用层 ──► 领域层 ◄── 适配器层 ◄── 基础设施层
                        ▲
                        └── 依赖倒置 (DIP)
```

---

## 二、Crate 结构

```
zeterm/
├── crates/
│   ├── zeterm/              # 表现层 + 应用层
│   │   └── src/
│   │       ├── ui/          # Views (Tab, Split, TerminalView 自实现)
│   │       │   ├── tab_view.rs
│   │       │   ├── split_pane/
│   │       │   └── terminal_view/
│   │       └── app/         # Coordinators
│   │           └── session/
│   ├── zeterm-core/         # 领域层
│   │   └── src/
│   │       ├── traits/      # TerminalConnection
│   │       ├── entities/    # HostConfig
│   │       └── state/       # ConnectionState
│   ├── zeterm-ssh/          # 适配器层 + 基础设施层
│   │   └── src/
│   │       ├── adapter.rs
│   │       └── connection.rs
│   ├── zeterm-storage/      # 基础设施层
│   │   └── src/
│   │       └── sqlite.rs
│   └── zeterm-mock/         # 测试支持
│       └── src/
│           └── mock_connection.rs
```

---

## 三、核心组件

### 3.1 组件分工

| 组件 | 层级 | 实现方式 | 说明 |
|------|------|----------|------|
| `TerminalView` | 表现层 | 自实现 | 终端渲染（参考 Zed） |
| `TabView` | 表现层 | 自实现 | 多标签页管理 |
| `SplitView` | 表现层 | 自实现 | 分屏布局 |
| `SessionCoordinator` | 应用层 | 自实现 | 会话协调 |
| `TabManager` | 应用层 | 自实现 | Tab 状态管理 |
| `SplitManager` | 应用层 | 自实现 | 分屏状态管理 |
| `TerminalConnection` | 领域层 | Trait | 连接抽象 |
| `Modal`/`Input` | 表现层 | gpui-component | 对话框组件 |

### 3.2 UI 组件来源

- **自实现**: TerminalView、TabView、SplitView（终端特殊需求）
- **gpui-component**: Modal、Input、Button、Theme、Tree、Table

---

## 四、数据流

### 4.1 输入数据流（用户 → 远端）

```
用户按键 → TerminalView → ANSI序列 → SessionCoordinator → TerminalConnection → SSH → 远端
```

### 4.2 输出数据流（远端 → 显示）

```
远端数据 → SSH → TerminalConnection → Data Pump → TerminalState → cx.notify() → 重绘
```

### 4.3 线程模型

| 线程 | 职责 | 特点 |
|------|------|------|
| GPUI 主线程 | UI 渲染、事件处理 | <16ms 响应 |
| Tokio Runtime | 网络 IO、SSH 协议 | 多线程并发 |

---

## 五、关键接口

### 5.1 TerminalConnection Trait

```rust
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), ConnectionError>;
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<(), ConnectionError>;
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>>;
    async fn close(&mut self) -> Result<(), ConnectionError>;
}
```

### 5.2 HostRepository Trait

```rust
#[async_trait]
pub trait HostRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<HostConfig>, StorageError>;
    async fn list_by_group(&self, group: &str) -> Result<Vec<HostConfig>, StorageError>;
    async fn search(&self, query: &str) -> Result<Vec<HostConfig>, StorageError>;
    async fn get(&self, id: &str) -> Result<Option<HostConfig>, StorageError>;
    async fn create(&self, host: &HostConfig) -> Result<(), StorageError>;
    async fn update(&self, host: &HostConfig) -> Result<(), StorageError>;
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
}
```

---

## 六、设计原则

| 原则 | 说明 |
|------|------|
| 分层解耦 | UI 层与网络层通过 Trait 彻底解耦 |
| 异步优先 | 基于 Tokio 的全异步 IO 模型 |
| 类型安全 | 利用 Rust 类型系统在编译期捕获错误 |
| 可测试性 | 所有核心组件可 Mock、可单测 |
| 职责分离 | 终端状态与连接管理解耦 |

---

## 七、相关文档

- [API 文档](./api.md) - 完整接口定义
- [实现路径](./roadmap.md) - 开发阶段规划
- [配置管理](./infrastructure/config.md) - 配置说明