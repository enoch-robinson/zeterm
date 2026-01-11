#TerminalConnection Trait 设计

> 定义终端连接的核心抽象接口

---

## 一、设计目标

| 目标 | 说明 |
|------|------|
| 协议无关 | 统一 SSH、Local PTY、Telnet 等不同后端 |
| 异步优先 | 基于 async/await 的非阻塞 IO |
| 线程安全 | 支持跨线程共享 (`Send + Sync`) |
| 流式输出 | 使用 Stream 处理连续数据 |

---

## 二、核心Trait 定义

```rust
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    /// 发送数据到远端(键盘输入转换的ANSI 序列)
    async fn write(&mut self, bytes: &[u8]) -> Result<(), ConnectionError>;

    /// 调整终端窗口大小
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<(), ConnectionError>;

    /// 获取数据接收流(只能调用一次)
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>>;

    /// 关闭连接
    async fn close(&mut self) -> Result<(), ConnectionError>;
}
```

> 📖 完整 API 定义（含详细注释）见 [API 文档](../api.md)

---

## 三、扩展接口

### 3.1 连接信息查询

```rust
pub trait ConnectionInfo {
    fn is_connected(&self) -> bool;
    fn connection_type(&self) -> ConnectionType;
    fn remote_address(&self) -> Option<String>;
}

pub enum ConnectionType { Ssh, LocalPty, Telnet, Serial }
```

### 3.2 心跳保活

```rust
#[async_trait]
pub trait Keepalive {
    async fn send_keepalive(&mut self) -> Result<()>;
    fn set_keepalive_interval(&mut self, interval: Duration);
}
```

---

## 四、实现清单

| 后端类型 | 实现结构体 | 依赖库 |
|----------|------------|--------|
| SSH | `SshConnection` | russh |
| Local PTY | `LocalPtyConnection` | portable-pty |
| Mock | `MockConnection` | - |

---

## 五、实现要点

### 5.1 `Send + Sync` 约束

- 确保内部状态可安全跨线程访问
- 使用 `Arc<Mutex<_>>` 或 `tokio::sync` 原语

### 5.2 `receive_stream` 实现

- 使用 `tokio::sync::mpsc` 桥接回调与流
- 返回 `BoxStream` 以隐藏具体类型
- **只能调用一次**，后续调用返回空流

### 5.3 错误处理

- 将底层库错误转换为统一的 `ConnectionError`
- 保留错误上下文便于调试

---

## 六、Mock 实现示例

```rust
pub struct MockConnection {
    data_rx: Option<mpsc::Receiver<Vec<u8>>>,
    data_tx: mpsc::Sender<Vec<u8>>,
}

impl MockConnection {
    pub fn new() -> Self { /*创建 channel，启动模拟输出任务 */ }
    pub async fn inject_output(&self, data: &[u8]) { /* 模拟服务器输出 */ }
}
```

**用途**: 测试和开发阶段验证架构，无需真实 SSH 服务器。

---

## 七、使用模式

```rust
async fn run_session(mut conn: Box<dyn TerminalConnection>) {
    // 1. 获取输出流
    let mut stream = conn.receive_stream();
    
    // 2. 启动输出处理任务
    tokio::spawn(async move {
        while let Some(Ok(data)) = stream.next().await {
            terminal.advance_bytes(&data);
        }
    });
    
    // 3. 发送命令
    conn.write(b"ls -la\n").await?;
    
    // 4. 调整窗口
    conn.resize(24, 80).await?;
}
```

---

## 八、相关文档

- [SSH 后端实现](../modules/ssh-backend.md) - russh 实现细节
- [连接状态机](./state-machine.md) - 连接生命周期管理
- [错误处理](./error-handling.md) - 错误类型定义