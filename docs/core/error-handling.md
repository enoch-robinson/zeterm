# 错误处理策略

> 定义 Zeterm 的错误类型体系与处理策略

---

## 一、设计目标

| 目标 | 说明 |
|------|------|
| 类型安全 | 利用 Rust 的 `Result` 和自定义错误类型 |
| 可追溯 | 错误链保留完整上下文 |
| 用户友好 | 错误信息可直接展示给用户 |
| 可恢复 | 区分可恢复与不可恢复错误 |

---

## 二、错误类型层次

```
ZetermError (顶层)
├── ConnectionError   // 连接相关
├── AuthError// 认证相关
├── TerminalError     // 终端相关
├── ConfigError       // 配置相关
└── SftpError         // 文件传输相关
```

---

## 三、错误类型定义

### 3.1 连接错误

| 错误 | 说明 | 可恢复 |
|------|------|--------|
| `DnsResolution` | 无法解析主机名 | ✅ |
| `Timeout` | 连接超时 | ✅ |
| `Refused` | 连接被拒绝 | ✅ |
| `NetworkUnreachable` | 网络不可达 | ✅ |
| `Disconnected` | 连接已断开 | ✅ |

### 3.2 认证错误

| 错误 | 说明 | 可恢复 |
|------|------|--------|
| `InvalidPassword` | 密码错误 | ✅ |
| `InvalidPrivateKey` | 私钥无效或格式错误 | ❌ |
| `InvalidKeyPassphrase` | 私钥密码错误 | ✅ |
| `KeyFileNotFound` | 私钥文件不存在 | ❌ |
| `HostKeyVerificationFailed` | 主机密钥验证失败 | ✅ (用户确认) |
| `UnsupportedMethod` | 认证方式不支持 | ❌ |

### 3.3 终端错误

| 错误 | 说明 | 可恢复 |
|------|------|--------|
| `PtyAllocationFailed` | PTY 分配失败 |❌ |
| `ShellStartFailed` | Shell 启动失败 | ❌ |
| `InvalidSize` | 终端大小无效 | ✅ |
| `ChannelClosed` | 通道已关闭 | ❌ |

### 3.4 配置错误

| 错误 | 说明 | 可恢复 |
|------|------|--------|
| `FileNotFound` | 配置文件不存在 | ✅ (使用默认) |
| `ParseError` | 配置解析失败 | ❌ |
| `InvalidValue` | 配置项无效 | ❌ |
| `MissingRequired` | 必填配置项缺失 | ❌ |

---

## 四、错误传播策略

```
┌─────────────────────────────────────────────────┐
│  UI Layer                                       │
│  -捕获所有错误                │
│  - 转换为用户可读消息                            │
│  - 显示Toast/Dialog│
└─────────────────────┬───────────────────────────┘│ ZetermError
┌─────────────────────▼───────────────────────────┐
│  Application Layer                              │
│  - 聚合底层错误                                  │
│  - 添加业务上下文                                │
│  - 触发状态转换                                  │
└─────────────────────┬───────────────────────────┘
                      │ ConnectionError / AuthError
┌─────────────────────▼───────────────────────────┐
│  Infrastructure Layer                           │
│  - 转换第三方库错误                              │
│  - 统一错误类型                                  │
└─────────────────────────────────────────────────┘
```

---

## 五、错误恢复机制

### 5.1 重试策略

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `max_attempts` | 3 | 最大重试次数 |
| `initial_delay_ms` | 1000 | 初始延迟 (毫秒) |
| `backoff_multiplier` | 2.0 | 退避倍数 |
| `max_delay_ms` | 30000 | 最大延迟 (毫秒) |

> **注意**: 这些参数与 [连接状态机](./state-machine.md) 和 [API 文档](../api.md) 中的定义保持一致。

### 5.2 恢复决策

| 错误类型 | 恢复策略 |
|----------|----------|
| `Timeout` | 自动重试 (最多3次) |
| `Disconnected` | 提示用户重连 |
| `InvalidPassword` | 重新输入密码 |
| `HostKeyVerificationFailed` | 用户确认后信任 |
| `ChannelClosed` | 关闭会话 |
| `ParseError` | 使用默认配置 |

---

## 六、用户错误展示

### 6.1 错误消息结构

```rust
pub struct UserErrorMessage {
    pub title: String,        // 简短标题
    pub description: String,  // 详细描述
    pub suggestion: Option<String>,  // 建议操作
    pub retryable: bool,      // 是否可重试
}
```

### 6.2 示例映射

| 错误 | 标题 | 建议 |
|------|------|------|
| `InvalidPassword` | 认证失败 | 请检查密码是否正确 |
| `Timeout` | 连接超时 | 请检查网络连接 |
| `HostKeyVerificationFailed` | 主机密钥变更 | 确认是否信任此主机 |

---

## 七、日志级别

| 错误类型 | 日志级别 |
|----------|----------|
| 可恢复 + 自动重试成功 | `DEBUG` |
| 可恢复 + 需用户介入 | `WARN` |
| 不可恢复 | `ERROR` |

---

## 八、相关文档

- [连接状态机](./state-machine.md) - 错误触发的状态转换
- [SessionModel](../modules/session-model.md) - 错误处理的业务逻辑