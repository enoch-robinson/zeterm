# 安全设计

> Zeterm 的安全机制与敏感数据保护策略

---

## 一、安全原则

| 原则 | 说明 |
|------|------|
| 最小权限 | 仅请求必要的系统权限 |
| 纵深防御 | 多层安全机制叠加 |
| 默认安全 | 安全选项默认开启 |
| 透明可控 | 用户可审计所有安全决策 |

---

## 二、主机密钥验证

### 2.1 验证流程

```
首次连接                再次连接
    ││
    ▼                            ▼
获取服务器公钥              获取服务器公钥
    │                            │
    ▼                            ▼
known_hosts 中无记录?与已存储密钥比对
    │                            │
    ├─► 提示用户确认             ├─► 匹配 → 允许连接
    │       │                    │
    │       ├─► 信任 → 存储      └─► 不匹配 → 警告!
    │       │                            │
    │       └─► 拒绝 → 中止▼
    │                            用户决策:
    └─────────────────────────── 信任/拒绝/查看详情
```

### 2.2 密钥指纹显示

```rust
pub structHostKeyInfo {
    pub algorithm: String,      // ssh-ed25519, ssh-rsa
    pub fingerprint_sha256: String,
    pub fingerprint_md5: String,
}

// 显示格式
// SHA256:nThbg6kXUpJWGl7E1IGOCspRomTxdCARLviKw6E5SY8
// MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48
```

### 2.3 known_hosts 管理

| 操作 | 说明 |
|------|------|
| 自动添加 | 用户确认后写入 |
| 手动移除 | 设置界面或命令行 |
| 批量导入 | 支持 OpenSSH 格式 |

**存储位置**: `~/.config/zeterm/known_hosts`

---

## 三、凭证安全

### 3.1 存储层级

```
┌─────────────────────────────────────────┐
│         系统密钥链(推荐)               │
│macOS Keychain / Windows Credential   │
│  Linux Secret Service                │
├─────────────────────────────────────────┤
│         加密文件存储                │
│  AES-256-GCM +主密码派生密钥          │
├─────────────────────────────────────────┤
│         内存临时存储                    │
│  会话期间，退出即清除                  │
└─────────────────────────────────────────┘
```

### 3.2 密钥链集成

```rust
pub trait SecretStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}

// 平台实现
pub struct KeychainStore;// macOS
pub struct CredentialStore;    // Windows  
pub struct SecretServiceStore; // Linux
```

### 3.3 配置引用语法

```toml
[hosts.auth]
type = "password"
# 引用密钥链
password = "keychain:zeterm/server-1/password"

# 引用环境变量
password = "env:SERVER_PASSWORD"

# 每次询问 (不存储)
password = "ask:"
```

---

## 四、私钥保护

### 4.1 支持的密钥格式

| 格式 | 加密 | 说明 |
|------|------|------|
| OpenSSH | ✅ | 推荐，现代格式 |
| PEM (PKCS#8) | ✅ | 传统格式 |
| PuTTY PPK | ✅ | Windows 常见|

### 4.2 密钥加载流程

```
读取私钥文件│
      ▼
  是否加密?
      │
  ├─► 否 → 直接加载
  │
  └─► 是 → 请求密码短语
            │
            ├─► 密钥链中有? → 自动解密
            │
            └─► 无 → 提示用户输入
                │
                      └─► 可选: 存入密钥链
```

### 4.3 SSH Agent 支持

| 功能 | 说明 |
|------|------|
| 自动检测 | 读取 `SSH_AUTH_SOCK` |
| 密钥列表 | 显示 Agent 中可用密钥 |
| 签名委托 | 私钥不离开 Agent |

---

## 五、传输安全

### 5.1 支持的算法

| 类型 | 推荐 | 支持 |
|------|------|------|
| 密钥交换 | curve25519-sha256 | diffie-hellman-group14-sha256 |
| 加密 | chacha20-poly1305 | aes256-gcm, aes256-ctr |
| MAC | (AEAD 内置) | hmac-sha2-256 |
| 主机密钥 | ssh-ed25519 | rsa-sha2-512 |

### 5.2 算法协商

```rust
pub struct SecurityPolicy {
    pub min_key_size: u32,           // RSA 最小位数
    pub allowed_kex: Vec<String>,    // 允许的密钥交换
    pub allowed_ciphers: Vec<String>,
    pub allowed_macs: Vec<String>,
}

// 默认策略:仅允许现代安全算法
pub fn default_policy() -> SecurityPolicy;

// 兼容策略: 支持旧服务器
pub fn legacy_policy() -> SecurityPolicy;
```

---

## 六、内存安全

### 6.1 敏感数据处理

```rust
use zeroize::Zeroize;

pub struct SecureString(String);

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize(); // 内存清零
    }
}
```

### 6.2 保护措施

| 措施 | 说明 |
|------|------|
| 内存清零 | 敏感数据用后立即清除 |
| 禁止交换 | 可选: `mlock()` 锁定内存页 |
| 短生命周期 | 密码仅在认证时存在 |

---

## 七、审计日志

### 7.1 记录事件

| 事件 | 级别 | 内容 |
|------|------|------|
| 连接建立 | INFO | 主机、用户、时间 |
| 认证成功/失败 | INFO/WARN | 认证方式、结果 |
| 主机密钥变更 | WARN | 新旧指纹 |
| 连接断开 | INFO | 原因、持续时间 |

### 7.2 日志脱敏

```
✅ 记录: user@192.168.1.100:22 认证成功 (publickey)
❌ 不记录: 密码、私钥内容、会话数据
```

---

## 八、安全配置

```toml
[security]
# 主机密钥验证
strict_host_key_checking = true# 严格模式
auto_add_host_key = false        # 不自动添加

# 算法策略
policy = "default"  # default | legacy | paranoid

# 凭证存储
credential_store = "keychain"  # keychain | encrypted | memory

# 会话安全
clipboard_clear_timeout = 60# 剪贴板自动清除 (秒)
idle_timeout = 1800            # 空闲断开 (秒, 0=禁用)
```

---

## 九、威胁模型

| 威胁 | 缓解措施 |
|------|----------|
| 中间人攻击 | 主机密钥验证 |
|凭证泄露 | 系统密钥链 + 内存清零 |
| 弱加密 | 默认仅允许强算法 |
| 会话劫持 | 空闲超时 + 重认证 |
| 日志泄露 | 敏感信息脱敏 |

---

## 十、相关文档

- [配置管理](../infrastructure/config.md) - 安全配置项
- [错误处理](./error-handling.md) - AuthError 定义
- [SSH 后端](../modules/ssh-backend.md) -认证实现