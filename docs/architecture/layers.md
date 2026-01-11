# 四层架构详解

> Zeterm 的分层架构设计与职责划分

---

## 一、架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                    表现层 (Presentation)│
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

---

## 二、表现层 (Presentation Layer)

### 2.1 职责

- **纯渲染** - 只负责将状态绘制到屏幕
- **事件捕获** - 接收用户输入并向下传递
- **无业务逻辑** - 不知道 SSH、网络等概念

### 2.2 核心组件

| 组件 | 职责 |
|------|------|
| `TerminalView` | 终端字符网格渲染 |
| `SftpView` | 文件管理器视图 |
| `TabBar` | 标签页管理 |
| `HostList` | 主机列表侧边栏 |
| `StatusBar` | 状态栏显示 |

### 2.3 设计原则

```rust
// ✅ 正确：View 只读取Model 状态
impl Render for TerminalView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let content = self.session.read(cx).renderable_content();
        // 渲染 content...
    }
}

// ❌ 错误：View 直接操作网络
impl Render for TerminalView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        self.ssh_client.send_data(...); // 不应该在这里！}
}
```

---

## 三、状态模型层 (Model Layer)

### 3.1 职责

- **状态持有** - 管理应用的核心状态
- **业务协调** - 协调 UI 与后端的交互
- **事件分发** - 处理用户操作并更新状态

### 3.2 核心组件

| 组件 | 职责 |
|------|------|
| `SessionModel` | 单个终端会话的状态 |
| `ConnectionStore` | 管理所有连接 |
| `HostStore` | 主机配置管理 |
| `WorkspaceModel` | 窗口布局状态 |

### 3.3 SessionModel 结构

```rust
pub struct SessionModel {
    /// 终端状态机(Alacritty)
    term: Arc<Mutex<Term<EventProxy>>>,
    
    /// 连接后端 (多态)
    backend: Box<dyn TerminalConnection>,
    
    /// 连接状态机
    state: ConnectionStateMachine,
    
    /// 配置
    config: TerminalConfig,
}
```

### 3.4 数据流

```
用户输入 → View.dispatch_event()    ↓
         Model.handle_input()
                    ↓Backend.write()──────→远端服务器
                                ↓
         Backend.receive_stream() ←───┘
                    ↓
         Term.advance_bytes()
                    ↓
         cx.notify() → View.render()
```

---

## 四、适配器层 (Adapter Layer)

### 4.1 职责

- **定义接口** - 声明标准化的 Trait
- **屏蔽差异** - 隐藏不同后端的实现细节
- **依赖倒置** - 上层依赖抽象而非具体实现

### 4.2 核心 Trait

```rust
/// 终端连接抽象
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    async fn write(&mut self, bytes: &[u8]) -> Result<()>;
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>>;
    async fn close(&mut self) -> Result<()>;
}

/// 文件系统抽象
#[async_trait]
pub trait FileSystemBackend: Send + Sync {
    async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn delete(&self, path: &str) -> Result<()>;
}

/// 主机存储抽象
#[async_trait]
pub trait HostRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<HostConfig>>;
    async fn get(&self, id: &str) -> Result<Option<HostConfig>>;
    async fn save(&self, host: &HostConfig) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
```

### 4.3 优势

1. **可测试** - 使用 Mock 实现进行单元测试
2. **可扩展** - 新增后端只需实现 Trait
3. **解耦** - UI 层完全不知道具体实现

---

## 五、基础设施层 (Infrastructure Layer)

### 5.1 职责

- **具体实现** - 实现适配器层定义的 Trait
- **外部交互** - 处理网络、文件系统、数据库
- **协议处理** - SSH、SFTP、Telnet 等协议细节

### 5.2 核心组件

| 组件 | 实现 Trait | 依赖 |
|------|------------|------|
| `SshConnection` | `TerminalConnection` | russh |
| `LocalPtyConnection` | `TerminalConnection` | portable-pty |
| `SftpClient` | `FileSystemBackend` | russh-sftp |
| `SqliteHostRepo` | `HostRepository` | sqlx |

### 5.3 实现示例

```rust
// SSH 后端实现
pub struct SshConnection {
    session: client::Handle<SshHandler>,
    channel: Option<ChannelId>,
    data_rx: mpsc::Receiver<Vec<u8>>,
}

#[async_trait]
impl TerminalConnection for SshConnection {
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        // russh 具体实现...
    }
    // ...
}

// Local PTY 后端实现
pub struct LocalPtyConnection {pty: Box<dyn PtyMaster>,
    reader: Box<dyn Read + Send>,
}

#[async_trait]
impl TerminalConnection for LocalPtyConnection {
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        // portable-pty 具体实现...
    }
    // ...
}
```

---

## 六、层级依赖规则

### 6.1 依赖方向

```
表现层 ──────► 状态模型层 ──────► 适配器层 ◄────── 基础设施层
   ││                ▲                │
   │               │                 │                │
   └───────────────┴─────────────────┴────────────────┘
                只能向下依赖
```

### 6.2 禁止事项

|禁止 | 原因 |
|------|------|
| 表现层直接依赖基础设施层 | 破坏分层，难以测试 |
| 状态模型层依赖具体实现 | 应依赖 Trait 抽象 |
| 基础设施层依赖上层 | 违反依赖倒置原则 |

---

## 七、相关文档

- [数据流设计](./data-flow.md) - 详细的数据流转
- [TerminalConnection Trait](../core/connection-trait.md) - 核心接口
- [SessionModel](../modules/session-model.md) - 会话模型设计