# TerminalConnection Trait 设计

> 定义终端连接的核心抽象接口

---

## 一、设计目标

1. **协议无关** - 统一 SSH、Local PTY、Telnet 等不同后端
2. **异步优先** - 基于 async/await 的非阻塞 IO
3. **线程安全** - 支持跨线程共享
4. **流式输出** - 使用 Stream 处理连续数据

---

## 二、核心 Trait 定义

```rust
use async_trait::async_trait;
use anyhow::Result;
use futures::stream::BoxStream;

/// 终端连接后端的核心抽象
///
/// 所有连接类型（SSH、Local PTY、Telnet 等）都必须实现此Trait。
/// 这是UI 层与网络层解耦的关键。
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    /// 发送数据到远端
    /// 
    /// # Arguments
    /// * `bytes` - 要发送的字节数据（通常是键盘输入转换的ANSI 序列）
    /// 
    /// # Returns
    /// * `Ok(())` - 发送成功
    /// * `Err(_)` - 发送失败（连接断开、通道关闭等）
    async fn write(&mut self, bytes: &[u8]) -> Result<()>;

    /// 调整终端窗口大小
    /// 
    /// 当UI 层检测到窗口大小变化时调用，通知远端调整 PTY 尺寸。
    /// 
    /// # Arguments
    /// * `rows` - 行数
    /// * `cols` - 列数
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<()>;

    /// 获取数据接收流
    /// 
    /// 返回一个异步流，用于接收远端发送的数据。
    /// 调用者应持续消费此流以获取终端输出。
    /// 
    /// # Note
    /// 此方法会转移流的所有权，只能调用一次。
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>>;
    /// 关闭连接
    /// 
    /// 优雅地关闭连接，释放相关资源。
    async fn close(&mut self) -> Result<()>;
}
```

---

## 三、扩展接口

### 3.1 连接信息查询

```rust
/// 连接信息扩展
pub trait ConnectionInfo {
    /// 连接是否活跃
    fn is_connected(&self) -> bool;
    
    /// 获取连接类型
    fn connection_type(&self) -> ConnectionType;
    
    /// 获取远端地址
    fn remote_address(&self) -> Option<String>;
    
    /// 获取连接时长
    fn connected_duration(&self) -> Option<Duration>;
}

/// 连接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {Ssh,
    LocalPty,
    Telnet,
    Serial,
}
```

### 3.2 心跳保活

```rust
/// 心跳保活扩展
#[async_trait]
pub trait Keepalive {
    /// 发送心跳包
    async fn send_keepalive(&mut self) -> Result<()>;
    
    /// 设置心跳间隔
    fn set_keepalive_interval(&mut self, interval: Duration);
}
```

---

## 四、实现指南

### 4.1 实现清单

| 后端类型 | 实现结构体 | 依赖库 |
|----------|------------|--------|
| SSH | `SshConnection` | russh |
| Local PTY | `LocalPtyConnection` | portable-pty |
| Telnet | `TelnetConnection` | tokio |
| Serial | `SerialConnection` | tokio-serial |

### 4.2 实现要点

1. **`Send + Sync` 约束**
   - 确保内部状态可安全跨线程访问
   - 使用 `Arc<Mutex<_>>` 或 `tokio::sync` 原语

2. **`receive_stream` 实现**
   - 使用 `tokio::sync::mpsc` 桥接回调与流
   - 返回 `BoxStream` 以隐藏具体类型

3. **错误处理**
   - 将底层库错误转换为统一的 `ConnectionError`
   - 保留错误上下文便于调试

---

## 五、Mock 实现示例

```rust
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Mock 连接 - 用于测试和开发
pub struct MockConnection {
    data_rx: Option<mpsc::Receiver<Vec<u8>>>,
    data_tx: mpsc::Sender<Vec<u8>>,
    connected: bool,
}

impl MockConnection {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(256);
        Self {
            data_rx: Some(rx),
            data_tx: tx,
            connected: true,
        }
    }
    
    /// 模拟服务器输出
    pub async fn inject_output(&self, data: &[u8]) {
        let _ = self.data_tx.send(data.to_vec()).await;
    }
}

#[async_trait]
impl TerminalConnection for MockConnection {
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        // 回显输入
        self.data_tx.send(bytes.to_vec()).await?;
        Ok(())
    }
    
    async fn resize(&mut self, _rows: u16, _cols: u16) -> Result<()> {
        Ok(())
    }
    
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>> {
        let rx = self.data_rx.take().expect("receive_stream 只能调用一次");
        Box::pin(ReceiverStream::new(rx).map(Ok))
    }
    
    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }
}
```

---

## 六、使用示例

```rust
async fn run_terminal(mut conn: Box<dyn TerminalConnection>) {
    // 获取输出流
    let mut stream = conn.receive_stream();
    
    // 启动输出处理任务
    tokio::spawn(async move {
        while let Some(result) = stream.next().await {
            match result {
                Ok(data) => {
                    // 将数据送入终端解析器
                    terminal.advance_bytes(&data);
                }
                Err(e) => {
                    eprintln!("接收错误: {}", e);break;
                }
            }
        }
    });
    
    // 发送命令
    conn.write(b"ls -la\n").await?;
    
    // 调整窗口大小
    conn.resize(24, 80).await?;
}
```

---

## 七、相关文档

- [SSH 后端实现](../modules/ssh-backend.md) - russh 实现细节
- [连接状态机](./state-machine.md) - 连接生命周期管理
- [错误处理](./error-handling.md) - 错误类型定义