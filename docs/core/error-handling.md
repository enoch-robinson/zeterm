# 错误处理策略

> 定义 Zeterm 的错误类型体系与处理策略

---

## 一、设计目标

1. **类型安全** - 利用 Rust 的 `Result` 和自定义错误类型
2. **可追溯** - 错误链保留完整上下文
3. **用户友好** - 错误信息可直接展示给用户
4. **可恢复** - 区分可恢复与不可恢复错误

---

## 二、错误类型定义

### 2.1 顶层错误枚举

```rust
use thiserror::Error;

/// Zeterm 顶层错误类型
#[derive(Error, Debug)]
pub enum ZetermError {
    #[error("连接错误: {0}")]
    Connection(#[from] ConnectionError),

    #[error("认证错误: {0}")]
    Auth(#[from] AuthError),

    #[error("终端错误: {0}")]
    Terminal(#[from] TerminalError),

    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),

    #[error("SFTP 错误: {0}")]
    Sftp(#[from] SftpError),
}
```

### 2.2 连接错误

```rust
/// 连接相关错误
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("无法解析主机名: {host}")]
    DnsResolution { host: String },

    #[error("连接超时: {host}:{port} ({timeout_secs}秒)")]
    Timeout {
        host: String,
        port: u16,
        timeout_secs: u64,
    },

    #[error("连接被拒绝: {host}:{port}")]
    Refused { host: String, port: u16 },

    #[error("网络不可达: {0}")]
    NetworkUnreachable(String),

    #[error("连接已断开")]Disconnected,

    #[error("IO 错误: {0}")]Io(#[from] std::io::Error),
}
```

### 2.3 认证错误

```rust
/// 认证相关错误
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("密码错误")]
    InvalidPassword,

    #[error("私钥无效或格式错误: {path}")]
    InvalidPrivateKey { path: String },

    #[error("私钥密码错误")]
    InvalidKeyPassphrase,

    #[error("私钥文件不存在: {path}")]
    KeyFileNotFound { path: String },

    #[error("主机密钥验证失败: {fingerprint}")]
    HostKeyVerificationFailed { fingerprint: String },

    #[error("认证方式不支持: {method}")]
    UnsupportedMethod { method: String },

    #[error("认证超时")]
    Timeout,

    #[error("认证被拒绝: {reason}")]
    Rejected { reason: String },
}
```

### 2.4 终端错误

```rust
/// 终端相关错误
#[derive(Error, Debug)]
pub enum TerminalError {
    #[error("PTY 分配失败")]
    PtyAllocationFailed,

    #[error("Shell 启动失败: {shell}")]
    ShellStartFailed { shell: String },

    #[error("终端大小无效: {rows}x{cols}")]
    InvalidSize { rows: u16, cols: u16 },

    #[error("写入失败: {0}")]
    WriteFailed(String),

    #[error("通道已关闭")]
    ChannelClosed,
}
```

### 2.5 配置错误

```rust
/// 配置相关错误
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("配置文件不存在: {path}")]
    FileNotFound { path: String },

    #[error("配置解析失败: {0}")]
    ParseError(String),

    #[error("配置项无效: {key} = {value}")]
    InvalidValue { key: String, value: String },

    #[error("必填配置项缺失: {key}")]
    MissingRequired { key: String },
}
```

---

## 三、错误传播策略

### 3.1 层级传播规则

```
┌─────────────────────────────────────────────────────────┐
│  UI Layer│
│  -捕获所有错误                                          │
│  - 转换为用户可读消息                                    │
│  - 显示 Toast/Dialog│
└─────────────────────┬───────────────────────────────────┘
                      │ ZetermError
┌─────────────────────▼───────────────────────────────────┐
│  Model Layer                                            │
│  - 聚合底层错误                                          │
│  - 添加业务上下文                                        │
│  - 触发状态转换                                          │
└─────────────────────┬───────────────────────────────────┘
                      │ ConnectionError / AuthError / ...
┌─────────────────────▼───────────────────────────────────┐
│Adapter Layer                                          │
│  - 转换第三方库错误                                      │
│  - 统一错误类型                                          │
└─────────────────────┬───────────────────────────────────┘
                      │ russh::Error / std::io::Error
┌─────────────────────▼───────────────────────────────────┐
│  Infrastructure Layer                                   │
│  - 原始错误产生处                                        │
└─────────────────────────────────────────────────────────┘
```

### 3.2 错误转换示例

```rust
impl From<russh::Error> for ConnectionError {
    fn from(err: russh::Error) -> Self {
        match err {
            russh::Error::Disconnect => ConnectionError::Disconnected,
            russh::Error::Timeout => ConnectionError::Timeout {
                host: "unknown".into(),
                port: 0,
                timeout_secs: 30,
            },
            other => ConnectionError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, other.to_string())
            ),
        }
    }
}
```

---

## 四、错误恢复机制

### 4.1 可恢复错误分类

| 错误类型 | 可恢复 | 恢复策略 |
|----------|--------|----------|
| `ConnectionError::Timeout` | ✅ | 自动重试 (最多3次) |
| `ConnectionError::Disconnected` | ✅ | 提示用户重连 |
| `AuthError::InvalidPassword` | ✅ | 重新输入密码 |
| `AuthError::HostKeyVerificationFailed` | ✅ | 用户确认后信任 |
| `TerminalError::ChannelClosed` | ❌ | 关闭会话 |
| `ConfigError::ParseError` | ❌ | 使用默认配置 |

### 4.2 重试策略

```rust
/// 重试配置
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始延迟 (毫秒)
    pub initial_delay_ms: u64,
    /// 延迟倍数 (指数退避)
    pub backoff_multiplier: f64,/// 最大延迟 (毫秒)
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30000,
        }
    }
}
```

---

## 五、用户错误展示

### 5.1 错误消息结构

```rust
/// 用户友好的错误消息
pub struct UserErrorMessage {
    /// 简短标题
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 建议操作
    pub suggestion: Option<String>,
    /// 是否可重试
    pub retryable: bool,
}

impl From<&ZetermError> for UserErrorMessage {
    fn from(err: &ZetermError) -> Self {
        match err {
            ZetermError::Auth(AuthError::InvalidPassword) => Self {
                title: "认证失败".into(),
                description: "密码不正确".into(),
                suggestion: Some("请检查密码是否正确".into()),
                retryable: true,
            },
            // ... 其他错误映射
            _ => Self {
                title: "发生错误".into(),
                description: err.to_string(),
                suggestion: None,
                retryable: false,
            },
        }
    }
}
```

---

## 六、日志与监控

### 6.1 错误日志级别

| 错误类型 | 日志级别 |
|----------|----------|
|可恢复 + 自动重试成功 | `DEBUG` |
| 可恢复 + 需用户介入 | `WARN` |
| 不可恢复 | `ERROR` |

### 6.2 结构化日志

```rust
use tracing::{error, warn, instrument};

#[instrument(skip(self), fields(host = %self.host))]
async fn connect(&mut self) -> Result<(), ConnectionError> {
    match self.try_connect().await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!(error = %e, "连接失败");
            Err(e)
        }
    }
}
```

---

## 七、相关文档

- [连接状态机](./state-machine.md) - 错误触发的状态转换
- [SessionModel](../modules/session-model.md) - 错误处理的业务逻辑