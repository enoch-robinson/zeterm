# API 文档

> Zeterm 核心模块公开 API 参考

---

## 一、文档概述

### 1.1 模块结构

```
zeterm/
├── zeterm-core/          # 领域层：核心 Trait 和实体
├── zeterm-terminal/      # 终端模型：Alacritty 封装
├── zeterm-ssh/           # SSH 后端：russh 实现
├── zeterm-storage/       # 持久化：SQLite 存储
└── zeterm/# 主程序：UI 和应用层
```

### 1.2 依赖关系

```
┌─────────────────────────────────────────────────────────┐
│                      zeterm (主程序)                     │
│                           │             │
│              ┌────────────┼────────────┐                │
│              ▼            ▼            ▼                │
│      zeterm-terminalzeterm-ssh  zeterm-storage        │
│              │            │            │                │
│              └────────────┼────────────┘                │
│                           ▼                             │
│                zeterm-core                │
└─────────────────────────────────────────────────────────┘
```

---

## 二、zeterm-core (领域层)

### 2.1 TerminalConnection Trait

终端连接的核心抽象接口。

```rust
use async_trait::async_trait;
use futures::stream::BoxStream;

/// 终端连接抽象 Trait
///
/// 所有连接后端（SSH、Local PTY、Mock）都必须实现此 Trait。
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    /// 发送数据到远端
    /// 
    /// # Arguments
    /// * `bytes` - 要发送的字节数据（通常是 ANSI 序列）
    /// 
    /// # Returns
    /// * `Ok(())` - 发送成功
    /// * `Err(ConnectionError)` - 发送失败
    /// 
    /// # Example
    /// ```rust
    /// conn.write(b"ls -la\n").await?;
    /// ```
    async fn write(&mut self, bytes: &[u8]) -> Result<(), ConnectionError>;

    /// 调整终端窗口大小
    /// 
    /// # Arguments
    /// * `rows` - 行数
    /// * `cols` - 列数
    /// 
    /// # Example
    /// ```rust
    /// conn.resize(24, 80).await?;
    /// ```
    async fn resize(&mut self, rows: u16, cols: u16) -> Result<(), ConnectionError>;

    /// 获取数据接收流
    /// 
    /// **注意**: 此方法只能调用一次，后续调用返回空流。
    /// 
    /// # Returns
    /// 返回一个异步字节流，用于接收远端数据。
    /// 
    /// # Example
    /// ```rust
    /// let mut stream = conn.receive_stream();
    /// while let Some(Ok(data)) = stream.next().await {
    ///     terminal.advance_bytes(&data);
    /// }
    /// ```
    fn receive_stream(&mut self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>>;

    /// 关闭连接
    /// 
    /// # Example
    /// ```rust
    /// conn.close().await?;
    /// ```
    async fn close(&mut self) -> Result<(), ConnectionError>;
}
```

### 2.2 ConnectionInfo Trait

连接信息查询接口。

```rust
/// 连接信息查询 Trait
pub trait ConnectionInfo {
    /// 检查连接是否活跃
    fn is_connected(&self) -> bool;
    
    /// 获取连接类型
    fn connection_type(&self) -> ConnectionType;
    
    /// 获取远端地址
    fn remote_address(&self) -> Option<String>;
    
    /// 获取连接建立时间
    fn connected_at(&self) -> Option<Instant>;
}

/// 连接类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {Ssh,
    LocalPty,
    Telnet,
    Serial,
    Mock,
}
```

### 2.3 HostRepository Trait

主机配置仓储接口。

```rust
/// 主机仓储 Trait
#[async_trait]
pub trait HostRepository: Send + Sync {
    /// 获取所有主机
    async fn list_all(&self) -> Result<Vec<HostConfig>, StorageError>;
    
    /// 按分组获取主机
    async fn list_by_group(&self, group: &str) -> Result<Vec<HostConfig>, StorageError>;
    
    /// 搜索主机
    async fn search(&self, query: &str) -> Result<Vec<HostConfig>, StorageError>;
    
    /// 获取单个主机
    async fn get(&self, id: &str) -> Result<Option<HostConfig>, StorageError>;
    
    /// 创建主机
    async fn create(&self, host: &HostConfig) -> Result<(), StorageError>;
    
    /// 更新主机
    async fn update(&self, host: &HostConfig) -> Result<(), StorageError>;
    
    /// 删除主机
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
}
```

### 2.4 SecretStore Trait

凭证存储接口。

```rust
/// 密钥存储 Trait
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// 存储密钥
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError>;
    
    /// 获取密钥
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError>;
    
    /// 删除密钥
    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError>;
    
    /// 检查存储是否可用
    fn is_available(&self) -> bool;
}
```

---

## 三、实体定义

### 3.1 HostConfig

主机配置实体。

```rust
use serde::{Deserialize, Serialize};

/// 主机配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    ///唯一标识符
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 主机地址
    pub host: String,
    /// 端口号 (默认 22)
    #[serde(default = "default_port")]
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 认证配置
    pub auth: AuthConfig,
    /// 分组名称
    #[serde(default)]
    pub group: Option<String>,
    /// 标签列表
    #[serde(default)]
    pub tags: Vec<String>,
    /// 启动命令
    #[serde(default)]
    pub startup_command: Option<String>,
    /// 创建时间
    pub created_at: i64,
    /// 更新时间
    pub updated_at: i64,
}

fn default_port() -> u16 { 22 }

impl HostConfig {
    /// 创建新的主机配置
    pub fn new(name: &str, host: &str, username: &str) -> Self;
    /// 获取连接地址
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    
    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), ValidationError>;
}
```

### 3.2 AuthConfig

认证配置。

```rust
/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// 密码认证
    Password {
        /// 密码（可以是引用格式）
        password: String,},
    /// 公钥认证
    PublicKey {
        /// 私钥路径
        key_path: String,
        /// 私钥密码
        #[serde(default)]
        passphrase: Option<String>,
    },
    /// SSH Agent 认证
    Agent,/// 每次询问
    Ask,
}

impl AuthConfig {
    /// 检查是否需要交互输入
    pub fn requires_interaction(&self) -> bool {
        matches!(self, AuthConfig::Ask)
    }
    
    /// 解析密码引用
    pub async fn resolve_password(&self, store: &dyn SecretStore) -> Result<Option<String>, Error>;
}
```

### 3.3 SessionConfig

会话配置。

```rust
/// 会话配置
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 终端类型
    pub term_type: String,
    /// 滚动缓冲区行数
    pub scrollback_lines: usize,
    /// 是否启用自动重连
    pub auto_reconnect: bool,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 重连初始延迟 (毫秒)
    pub reconnect_initial_delay_ms: u64,
    /// 重连最大延迟 (毫秒)
    pub reconnect_max_delay_ms: u64,
    /// 重连退避倍数
    pub reconnect_backoff_multiplier: f64,
    /// 心跳间隔（秒）
    pub keepalive_interval: Option<u64>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            term_type: "xterm-256color".to_string(),
            scrollback_lines: 10000,
            auto_reconnect: true,
            max_reconnect_attempts: 3,
            reconnect_initial_delay_ms: 1000,
            reconnect_max_delay_ms: 30000,
            reconnect_backoff_multiplier: 2.0,
            keepalive_interval: Some(60),
        }
    }
}
```

### 3.4 TerminalSize

终端尺寸。

```rust
/// 终端尺寸
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    /// 行数
    pub rows: u16,
    /// 列数
    pub cols: u16,
}

impl TerminalSize {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
    
    /// 检查尺寸是否有效
    pub fn is_valid(&self) -> bool {
        self.rows > 0 && self.cols > 0
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}
```

---

## 四、状态机

### 4.1 ConnectionState

连接状态枚举。

```rust
use std::time::Instant;

/// 连接状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// 空闲状态
    Idle,
    /// 正在连接
    Connecting { attempt: u32 },
    /// 正在认证
    Authenticating,
    /// 已连接
    Connected { connected_at: Instant },
    /// 正在断开
    Disconnecting,
    /// 已断开
    Disconnected { reason: DisconnectReason },
    /// 正在重连
    Reconnecting { attempt: u32 },
}

impl ConnectionState {
    /// 检查是否处于活跃状态
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
    
    /// 检查是否可以发送数据
    pub fn can_send(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
    
    /// 转换为 UI 显示状态
    pub fn to_ui_status(&self) -> UiConnectionStatus;
}
```

### 4.2 DisconnectReason

断开原因。

```rust
/// 断开原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    /// 用户主动断开
    UserInitiated,
    /// 服务器关闭
    ServerClosed,
    /// 网络错误
    NetworkError,
    /// 认证失败
    AuthFailed,
    /// 超时
    Timeout,
    /// 主机密钥验证失败
    HostKeyRejected,
}
```

### 4.3 ConnectionStateMachine

状态机实现。

```rust
use tokio::sync::watch;

/// 连接状态机
pub struct ConnectionStateMachine {
    state: ConnectionState,
    state_tx: watch::Sender<ConnectionState>,
    reconnect_policy: ReconnectPolicy,
}

impl ConnectionStateMachine {
    /// 创建新的状态机
    pub fn new() -> Self;
    
    /// 获取当前状态
    pub fn state(&self) -> &ConnectionState;
    
    /// 订阅状态变更
    pub fn subscribe(&self) -> watch::Receiver<ConnectionState>;
    
    /// 处理事件
    pub fn handle_event(&mut self, event: ConnectionEvent) -> Option<StateTransition>;
    
    /// 重置状态机
    pub fn reset(&mut self);
}

/// 连接事件
pub enum ConnectionEvent {
    Connect,
    TcpConnected,
    AuthSuccess,
    AuthFailed,
    Disconnect,
    ConnectionLost,
    ReconnectSuccess,
    ReconnectFailed,
}
```

---

## 五、错误类型

### 5.1 ConnectionError

连接错误。

```rust
use thiserror::Error;

/// 连接错误
#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),
    
    #[error("Connection timeout")]
    Timeout,
    
    #[error("Connection refused")]
    Refused,
    
    #[error("Network unreachable")]
    NetworkUnreachable,
    
    #[error("Connection disconnected")]
    Disconnected,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("SSH error: {0}")]
    Ssh(String),
}

impl ConnectionError {
    /// 检查是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::Disconnected)
    }
}
```

### 5.2 AuthError

认证错误。

```rust
/// 认证错误
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("Invalid private key")]
    InvalidPrivateKey,
    
    #[error("Invalid key passphrase")]
    InvalidKeyPassphrase,
    
    #[error("Key file not found: {0}")]
    KeyFileNotFound(String),
    
    #[error("Host key verification failed")]
    HostKeyVerificationFailed,
    
    #[error("Unsupported authentication method")]
    UnsupportedMethod,
    
    #[error("Authentication cancelled")]
    Cancelled,
}
```

### 5.3 StorageError

存储错误。

```rust
/// 存储错误
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Record not found: {0}")]
    NotFound(String),
    
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 六、相关文档

- [TerminalConnection Trait](./core/connection-trait.md) - 详细设计
- [连接状态机](./core/state-machine.md) - 状态转换规则
- [错误处理](./core/error-handling.md) - 错误处理策略