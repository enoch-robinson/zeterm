# Phase 3 新功能快速参考

> 版本：1.0  
> 更新日期：2026-01-19

---

## 📖 目录

- [ECDSA 密钥支持](#ecdsa-密钥支持)
- [跳过验证选项](#跳过验证选项)
- [常见问题](#常见问题)

---

## ECDSA 密钥支持

### 概述

SSH 连接现在支持 ECDSA (椭圆曲线数字签名算法) 公钥认证，包括：
- **ECDSA-NISTP256** (默认推荐)
- **ECDSA-NISTP384**
- **ECDSA-NISTP521**

### 生成 ECDSA 密钥

```bash
# 生成 ECDSA P-256 密钥（推荐）
ssh-keygen -t ecdsa -b 256 -f ~/.ssh/id_ecdsa -N ""

# 生成 ECDSA P-384 密钥
ssh-keygen -t ecdsa -b 384 -f ~/.ssh/id_ecdsa -N ""

# 生成 ECDSA P-521 密钥
ssh-keygen -t ecdsa -b 521 -f ~/.ssh/id_ecdsa -N ""
```

### 添加到服务器

```bash
# 方法 1: 使用 ssh-copy-id
ssh-copy-id -i ~/.ssh/id_ecdsa.pub user@server

# 方法 2: 手动复制公钥
cat ~/.ssh/id_ecdsa.pub | ssh user@server 'cat >> ~/.ssh/authorized_keys'
```

### 在 Zeterm 中使用

```rust
use zeterm_ssh::SshConfig;
use std::path::Path;

// 不带密码的 ECDSA 密钥
let config = SshConfig::new("example.com", "user")
    .with_key_file(Path::new("~/.ssh/id_ecdsa"));

// 带密码的 ECDSA 密钥
let config = SshConfig::new("example.com", "user")
    .with_key_file_and_passphrase(
        Path::new("~/.ssh/id_ecdsa"),
        "my-passphrase"
    );
```

### 支持的密钥类型对比

| 密钥类型 | 密钥文件 | Curve/Size | 速度 | 安全性 | 推荐度 |
|---------|---------|------------|------|--------|--------|
| RSA | `id_rsa` | 2048+ | 慢 | 中 | ⭐⭐⭐ |
| Ed25519 | `id_ed25519` | Ed25519 | 快 | 高 | ⭐⭐⭐⭐⭐ |
| ECDSA | `id_ecdsa` | P-256 | 中 | 高 | ⭐⭐⭐⭐ |
| ECDSA | `id_ecdsa` | P-384 | 慢 | 高 | ⭐⭐⭐ |
| ECDSA | `id_ecdsa` | P-521 | 很慢 | 很高 | ⭐⭐ |

**推荐顺序：** Ed25519 > ECDSA-P256 > RSA

### 技术要求

✅ **自动支持** - 无需额外配置，代码已自动识别所有密钥类型  
✅ **内置支持** - `russh 0.56.0` 内置 ECDSA 支持（通过 p256/p384/p521 依赖）  
✅ **统一 API** - 与 RSA/Ed25519 使用相同的方法

---

## 跳过验证选项

### ⚠️ 安全警告

> **此功能会使连接容易受到中间人攻击！**  
> 仅在受信任的测试环境中使用，切勿在生产环境使用！

### 基本用法

```rust
use zeterm_ssh::SshConfig;

// 启用不安全模式（跳过主机密钥验证）
let config = SshConfig::new("localhost", "testuser")
    .with_password("testpass")
    .allow_insecure();  // ⚠️ 仅限测试环境！
```

### 警告日志示例

启用时会输出以下警告（仅记录一次）：

```
ERROR @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
ERROR @       WARNING: INSECURE MODE ENABLED!       @
ERROR @@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
ERROR Host key verification is DISABLED for localhost:22
ERROR This makes your connection vulnerable to
ERROR man-in-the-middle attacks!
ERROR Use this option ONLY in trusted test environments!
WARN  Skipping host key verification for localhost:22 (insecure mode)
```

### 适用场景

✅ **可以使用：**
- 本地开发测试
- 受信任的内网环境
- CI/CD 自动化测试
- 临时调试环境

❌ **不要使用：**
- 生产环境
- 公网服务器连接
- 不受信任的网络
- 处理敏感数据的场景

### 安全替代方案

如果遇到主机密钥验证问题，建议先尝试以下安全方案：

#### 方案 1: 首次连接手动确认
```rust
let config = SshConfig::new("server.com", "user")
    .with_key_file(Path::new("~/.ssh/id_ed25519"))
    .with_host_key_verification(
        HostKeyVerification::AskOnFirstConnect  // 首次连接时询问用户
    );
```

#### 方案 2: 使用 known_hosts 文件
```rust
let config = SshConfig::new("server.com", "user")
    .with_key_file(Path::new("~/.ssh/id_ed25519"))
    .with_host_key_verification(
        HostKeyVerification::KnownHostsFile(
            Path::new("~/.ssh/known_hosts").to_path_buf()
        )
    );
```

#### 方案 3: 手动添加主机密钥
```bash
# 手动连接一次，让 SSH 保存密钥
ssh-keyscan server.com >> ~/.ssh/known_hosts
```

### 配置对比

| 验证模式 | 安全性 | 适用场景 | 配置方法 |
|---------|--------|---------|---------|
| `Strict` | 🔒🔒🔒🔒🔒 | 生产环境 | `.with_host_key_verification(HostKeyVerification::Strict)` |
| `AskOnFirstConnect` | 🔒🔒🔒🔒 | 默认推荐 | 默认配置 |
| `KnownHostsFile` | 🔒🔒🔒🔒 | 自定义路径 | `.with_host_key_verification(HostKeyVerification::KnownHostsFile(path))` |
| `AutoAccept` | 🔒 | 测试环境 | `.with_host_key_verification(HostKeyVerification::AutoAccept)` |
| `allow_insecure()` | 🔓🔓🔓🔓🔓 | 仅限测试 | `.allow_insecure()` |

---

## 常见问题

### ECDSA 相关

**Q: Zeterm 支持哪些 ECDSA 曲线？**  
A: 支持所有 NIST 标准曲线：P-256、P-384、P-521。

**Q: ECDSA 和 RSA 哪个更好？**  
A: Ed25519 是最佳选择（速度快，安全性高）。如果需要 NIST 标准，ECDSA-P256 是 RSA-2048 的良好替代。

**Q: 我的 ECDSA 密钥无法使用怎么办？**  
A: 检查以下几点：
1. 确认私钥文件格式正确（OpenSSH 格式）
2. 确认服务器 `authorized_keys` 中包含对应的公钥
3. 尝试使用 `ssh -i ~/.ssh/id_ecdsa user@server` 手动连接测试

**Q: 是否需要为 ECDSA 启用特殊 feature？**  
A: 不需要。`russh 0.56.0` 已内置 ECDSA 支持（通过 p256/p384/p521 依赖），无需任何额外配置。

### 跳过验证相关

**Q: 为什么要提供这个危险的功能？**  
A: 1. 方便本地测试和开发；2. 与其他 SSH 客户端保持兼容；3. 在受信任环境中简化配置。

**Q: 警告会每次连接都显示吗？**  
A: 不会。警告日志仅记录一次，避免日志污染。

**Q: 可以在生产环境临时使用吗？**  
A: 强烈不建议。遇到问题应优先解决 known_hosts 配置，而不是跳过验证。

**Q: `allow_insecure()` 和 `AutoAccept` 有什么区别？**  
A: `AutoAccept` 会接受并保存密钥到 known_hosts，而 `allow_insecure()` 完全跳过验证。

**Q: 如何确认当前是否处于不安全模式？**  
A: 检查日志，如果有 "WARNING: INSECURE MODE ENABLED!" 错误，说明已启用。

---

## 代码示例集合

### 完整连接示例（ECDSA + 安全验证）

```rust
use zeterm_ssh::{SshConnection, SshConfig};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ECDSA 密钥，首次连接时询问用户
    let config = SshConfig::new("example.com", "user")
        .with_key_file(Path::new("~/.ssh/id_ecdsa"))
        .with_terminal_size(120, 40)
        .with_keepalive(Some(std::time::Duration::from_secs(60)));

    let connection = SshConnection::new(config)?;
    connection.connect().await?;

    println!("Connected successfully!");

    // ... 使用连接 ...

    Ok(())
}
```

### 测试环境示例（跳过验证）

```rust
use zeterm_ssh::{SshConnection, SshConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 测试环境：跳过主机密钥验证
    let config = SshConfig::new("localhost", "testuser")
        .with_password("testpass")
        .allow_insecure();  // ⚠️ 仅限测试！

    let connection = SshConnection::new(config)?;
    connection.connect().await?;

    println!("Connected to test server!");

    // ... 测试代码 ...

    Ok(())
}
```

### 多认证方式回退示例

```rust
use zeterm_ssh::{SshConfig, AuthMethod, HostKeyVerification};
use std::path::Path;

// 主认证方式：ECDSA 密钥
// 回退方式：密码认证
let config = SshConfig::new("server.com", "user")
    .with_key_file_and_passphrase(
        Path::new("~/.ssh/id_ecdsa"),
        "key-passphrase"
    )
    .with_password_fallback("backup-password");  // ECDSA 失败时尝试密码

let connection = SshConnection::new(config)?;
connection.connect().await?;
```

---

## 进一步阅读

- **[implementation-notes.md](./implementation-notes.md)** - 详细实现文档
- **[task-checklist.md](../task-checklist.md)** - 完整任务清单
- **russh 文档** - https://docs.rs/russh

---

## 快速检查清单

使用 ECDSA 密钥：
- [ ] 确认私钥文件存在且格式正确
- [ ] 确认服务器已添加公钥到 `authorized_keys`
- [ ] 使用 `ssh -i` 手动测试连接
- [ ] 在 Zeterm 中配置密钥路径

使用跳过验证：
- [ ] ⚠️ 确认在受信任的测试环境
- [ ] 确认了解安全风险
- [ ] 配置 `allow_insecure()` 选项
- [ ] 检查日志中的警告信息

---

**文档版本：** 1.0  
**最后更新：** 2026-01-19  
**维护者：** Zeterm Team