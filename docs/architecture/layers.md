# 五层架构详解

> Zeterm 的分层架构设计与职责划分

---

## 一、架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                    表现层 (Presentation)                    │
│GPUI Views│
├─────────────────────────────────────────────────────────────┤
│                    应用层 (Application)                     │
│                   Use Cases & Coordinators                  │
├─────────────────────────────────────────────────────────────┤
│                    领域层 (Domain)│
│                 Entities, Traits & Rules│
├─────────────────────────────────────────────────────────────┤
│                适配器层 (Adapter)                        │
│                  Protocol Converters                │
├─────────────────────────────────────────────────────────────┤
│                基础设施层 (Infrastructure)                  │
│                   IO & External Services│
└─────────────────────────────────────────────────────────────┘
```

---

## 二、各层职责

### 2.1 表现层 (Presentation)

| 职责 | 说明 |
|------|------|
| 纯渲染 | 将状态绘制到屏幕 |
| 事件捕获 | 接收用户输入并向下传递 |
| 无业务逻辑 | 不知道 SSH、网络等概念 |

**核心组件**: `TerminalView`, `SftpView`, `HostListView`, `TabBar`

### 2.2 应用层 (Application)

| 职责 | 说明 |
|------|------|
| 用例协调 | 编排领域对象完成业务流程 |
| 状态管理 | 管理 UI 状态与领域状态的映射 |
| 事件分发 | 处理用户操作并调用领域服务 |

**核心组件**: `SessionCoordinator`, `WorkspaceManager`, `ConnectionStore`

### 2.3 领域层 (Domain)

| 职责 | 说明 |
|------|------|
| 业务实体 | 定义核心数据结构 |
| 领域接口 | 声明 Trait 抽象 |
| 业务规则 | 封装领域逻辑 |

**核心组件**: `TerminalConnection` Trait, `HostConfig`, `SessionConfig`, `ConnectionState`

### 2.4 适配器层 (Adapter)

| 职责 | 说明 |
|------|------|
| 协议转换 | 将外部协议转换为领域接口 |
| 格式适配 | 数据格式转换 |

**核心组件**: `SshAdapter`, `EventStreamConverter`

### 2.5 基础设施层 (Infrastructure)

| 职责 | 说明 |
|------|------|
| 具体实现 | 实现领域层定义的 Trait |
| 外部交互 | 网络、文件系统、数据库 |

**核心组件**: `SshConnection`, `SqliteHostRepository`, `SftpClient`

---

## 三、层级依赖规则

```
表现层 ──► 应用层 ──► 领域层 ◄── 适配器层 ◄── 基础设施层▲                │
                        └─────────────────────────┘
                依赖倒置 (DIP)
```

### 依赖原则

| 规则 | 说明 |
|------|------|
| 向下依赖 | 上层只能依赖下层 |
| 依赖倒置 | 基础设施层依赖领域层接口，而非反向 |
| 接口隔离 | 每层只暴露必要接口 |

---

## 四、关键接口定义

### 4.1 领域层核心 Trait

```rust
// 终端连接抽象 (领域层)
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    async fn write(&mut self, bytes: &[u8]) -> Result<()>;
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>>;
    async fn close(&mut self) -> Result<()>;
}

// 主机仓储抽象 (领域层)
#[async_trait]
pub trait HostRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<HostConfig>>;
    async fn get(&self, id: &str) -> Result<Option<HostConfig>>;
    async fn save(&self, host: &HostConfig) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
```

### 4.2 应用层协调器

```rust
// 会话协调器 (应用层)
pub struct SessionCoordinator {
    terminal: Model<TerminalState>,       // 终端状态
    connection: Model<ConnectionManager>, // 连接管理
}
```

---

## 五、模块映射

```
zeterm/
├── crates/
│   ├── zeterm/              # 表现层 + 应用层
│   │   └── src/
│   │       ├── ui/          # 表现层: Views
│   │       └── app/         # 应用层: Coordinators
│   ├── zeterm-core/         # 领域层
│   │   └── src/
│   │       ├── traits/      # 核心 Trait
│   │       ├── entities/    # 业务实体
│   │       └── state/       # 状态定义
│   ├── zeterm-ssh/          # 适配器层 + 基础设施层
│   │   └── src/
│   │       ├── adapter.rs   # 协议适配
│   │       └── connection.rs # SSH 实现
│   └── zeterm-storage/      # 基础设施层
│       └── src/
│           └── sqlite.rs    # 持久化实现
```

---

## 六、相关文档

- [数据流设计](./data-flow.md) - 跨层数据流转
- [TerminalConnection Trait](../core/connection-trait.md) - 核心接口详解
- [SessionModel](../modules/session-model.md) - 应用层设计