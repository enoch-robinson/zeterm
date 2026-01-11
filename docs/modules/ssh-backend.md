# SSH 后端实现 (russh)

> 基于 russh 实现 `TerminalConnection` Trait，包含事件驱动转换方案

---

## 一、设计目标

1. **完整实现 `TerminalConnection` Trait** - 提供 SSH 连接能力
2. **事件驱动转流式** - 将 russh 的回调模式转换为 `BoxStream`
3. **支持多种认证** - 密码、公钥、Agent、键盘交互
4. **会话复用** - 单TCP 连接支持多 Channel

---

## 二、核心架构

### 2.1 组件关系图

```
┌─────────────────────────────────────────────────────────────┐
│                      SshConnection│
│  (实现 TerminalConnection Trait)                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐  │
│  │ SshSession  │    │ SshChannel  │    │ EventConverter│  │
│  │ (russh)     │───►│ (PTY)       │───►│ (回调→Stream)│  │
│  └─────────────┘    └─────────────┘    └────────┬────────┘  │
│                │           │
│                ▼           │
│                                BoxStream<Vec<u8>>   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心结构定义

```rust
use russh::{client,ChannelId};
use tokio::sync::mpsc;
use futures::stream::BoxStream;

/// SSH 连接实现
pub struct SshConnection {
    /// russh 会话句柄
    session: client::Handle<SshHandler>,
    
    /// PTY 通道
    channel: Option<ChannelId>,
    
    /// 数据接收通道 (从 Handler 回调接收)
    data_rx: mpsc::Receiver<Vec<u8>>,
    
    /// 连接配置
    config: SshConfig,
    
    /// 终端尺寸
    terminal_size: (u16, u16),
}

/// SSH 连接配置
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthConfig,
    pub timeout: Duration,
    pub keepalive_interval: Option<Duration>,
}

/// 认证配置
#[derive(Debug, Clone)]
pub enum AuthConfig {
    Password(String),
    PublicKey {
        private_key_path: PathBuf,
        passphrase: Option<String>,
    },
    Agent,
}
```

---

## 三、事件驱动转换方案

### 3.1 问题背景

russh 使用**回调模式**处理服务器数据：

```rust
// russh 的Handler trait (简化)
#[async_trait]
trait Handler {
    async fn data(&mut self, channel: ChannelId, data: &[u8]) -> Result<()>;
    async fn eof(&mut self, channel: ChannelId) -> Result<()>;// ...
}
```

但我们的 `TerminalConnection` Trait 需要**流式接口**：

```rust
fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>>;
```

### 3.2 解决方案：Channel Bridge

使用 `tokio::sync::mpsc` 作为桥梁，将回调转换为流：

```rust
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// SSH 事件处理器 - 实现 russh::client::Handler
pub struct SshHandler {
    /// 数据发送端- 将回调数据发送到流
    data_tx: mpsc::Sender<Vec<u8>>,
    
    /// 错误发送端
    error_tx: mpsc::Sender<ConnectionError>,
    
    /// 主机密钥验证回调
    host_key_verifier: Arc<dyn HostKeyVerifier>,
}

#[async_trait]
impl client::Handler for SshHandler {
    type Error = ConnectionError;
    
    /// 接收服务器数据 - 转发到 channel
    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],_session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // 将数据发送到 mpsc channel
        self.data_tx
            .send(data.to_vec())
            .await
            .map_err(|_| ConnectionError::Disconnected)?;
        Ok(())
    }
    
    /// 通道EOF
    async fn eof(
        &mut self,
        _channel: ChannelId,_session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // 关闭发送端，流将自然结束
        Ok(())
    }
    
    /// 主机密钥验证
    async fn check_server_key(
        &mut self,
        server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        self.host_key_verifier
            .verify(server_public_key)
            .await
    }
}
```

### 3.3 流构建

```rust
impl SshConnection {
    /// 创建数据接收流
    fn create_receive_stream(
        data_rx: mpsc::Receiver<Vec<u8>>,
    ) -> BoxStream<'static, Result<Vec<u8>>> {
        // 将 mpsc::Receiver 转换为 Stream
        let stream = ReceiverStream::new(data_rx);
        
        // 包装为 Result 类型
        let stream = stream.map(Ok);
        
        Box::pin(stream)
    }
}

---

## 四、TerminalConnection Trait 实现

```rust
use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
impl TerminalConnection for SshConnection {
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let channel_id = self.channel
            .ok_or_else(|| anyhow::anyhow!("通道未打开"))?;
        
        self.session
            .data(channel_id, bytes.into())
            .await?;
        
        Ok(())
    }
    
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let channel_id = self.channel
            .ok_or_else(|| anyhow::anyhow!("通道未打开"))?;
        
        self.session
            .window_change(channel_id, cols as u32, rows as u32, 0, 0)
            .await?;
        
        self.terminal_size = (rows, cols);
        Ok(())
    }
    
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>>> {
        // 取出receiver，转换为流
        let rx = std::mem::replace(
            &mut self.data_rx,
            mpsc::channel(1).1, // 替换为空receiver
        );
        Self::create_receive_stream(rx)
    }
    
    async fn close(&mut self) -> Result<()> {
        if let Some(channel_id) = self.channel.take() {
            let _ = self.session.eof(channel_id).await;let _ = self.session.close(channel_id).await;
        }
        self.session.disconnect(
            russh::Disconnect::ByApplication,
            "用户断开连接","en",
        ).await?;
        Ok(())
    }
}
```

---

## 五、连接建立流程

```rust
impl SshConnection {
    /// 建立 SSH 连接
    pub async fn connect(config: SshConfig) -> Result<Self> {
        // 1. 创建数据通道
        let (data_tx, data_rx) = mpsc::channel(1024);
        let (error_tx, _error_rx) = mpsc::channel(16);
        
        // 2. 创建 Handler
        let handler = SshHandler {
            data_tx,
            error_tx,
            host_key_verifier: Arc::new(DefaultHostKeyVerifier),
        };
        
        // 3. 配置 russh
        let ssh_config = Arc::new(client::Config {
            connection_timeout: Some(config.timeout),
            keepalive_interval: config.keepalive_interval,
            ..Default::default()
        });
        
        // 4. 建立 TCP 连接
        let addr = format!("{}:{}", config.host, config.port);
        let session = client::connect(ssh_config, &addr, handler).await?;
        
        // 5. 认证
        let authenticated = Self::authenticate(&session, &config).await?;
        if !authenticated {
            return Err(anyhow::anyhow!("认证失败"));
        }
        
        // 6. 打开 PTY 通道
        let channel = session.channel_open_session().await?;
        channel.request_pty(
            false,
            "xterm-256color",
            80, 24, 0, 0,
            &[],
        ).await?;
        channel.request_shell(false).await?;
        
        Ok(Self {
            session,
            channel: Some(channel.id()),
            data_rx,
            config,
            terminal_size: (24, 80),
        })
    }
}
```

---

## 六、认证实现

```rust
impl SshConnection {
    async fn authenticate(
        session: &client::Handle<SshHandler>,
        config: &SshConfig,
    ) -> Result<bool> {
        match &config.auth {
            AuthConfig::Password(password) => {
                session
                    .authenticate_password(&config.username, password)
                    .await
            }
            
            AuthConfig::PublicKey { private_key_path, passphrase } => {
                let key = russh_keys::load_secret_key(
                    private_key_path,
                    passphrase.as_deref(),
                )?;
                session
                    .authenticate_publickey(&config.username, Arc::new(key))
                    .await
            }
            
            AuthConfig::Agent => {
                let mut agent = russh_keys::agent::client::AgentClient::connect_env()
                    .await?;
                let identities = agent.request_identities().await?;
                
                for identity in identities {
                    if session
                        .authenticate_publickey_with(&config.username, identity, &mut agent)
                        .await?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}
```

---

## 七、相关文档

- [TerminalConnection Trait](../core/connection-trait.md) - 接口定义
- [连接状态机](../core/state-machine.md) - 状态管理
- [错误处理](../core/error-handling.md) - 错误类型