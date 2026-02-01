# 错误处理策略

> Zeterm 错误类型定义与处理策略

---

## 一、错误类型概览

```rust
pub enum ConnectionError {
    DnsResolution(String),
    Timeout,
    Refused,
    NetworkUnreachable,
    Disconnected,
    Io(std::io::Error),
    Ssh(String),
}

pub enum AuthError {
    InvalidPassword,
    InvalidPrivateKey,
    InvalidKeyPassphrase,
    KeyFileNotFound(String),
    HostKeyVerificationFailed,
    UnsupportedMethod,
    Cancelled,
}

pub enum StorageError {
    Database(String),
    NotFound(String),
    DuplicateKey(String),
    Serialization(String),
    Io(std::io::Error),
}
```

---

## 二、错误处理原则

| 原则 | 说明 |
|------|------|
| 类型安全 | 使用 `thiserror` 定义强类型错误 |
| 错误传播 | 使用 `?` 操作符自动传播 |
| 上下文保留 | 使用 `anyhow::Context` 添加上下文 |
| 用户友好 | 错误消息应清晰可读 |

---

## 三、错误转换

```rust
impl From<std::io::Error> for ConnectionError {
    fn from(e: std::io::Error) -> Self {
        ConnectionError::Io(e)
    }
}

impl ConnectionError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::Disconnected)
    }
}
```

---

## 四、相关文档

- [API.md](../API.md) - 完整错误类型定义
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 架构总览