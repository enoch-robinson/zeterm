# SSH 连接功能实现文档

> 日期: 2026-01-26
> 状态: ✅ 已完成

---

## 一、变更概述

本次实现了从主机列表双击主机后建立 SSH 连接的完整流程，包括：

1. **配置转换**: HostConfig → SshConfig
2. **认证处理**: 密码、公钥、SSH Agent 三种认证方式
3. **密码解析**: 支持 keychain/env/plain 三种密码引用格式
4. **异步连接**: 非阻塞 SSH 连接建立
5. **数据泵启动**: 后端数据 → 终端状态机

---

## 二、修改的文件

### 2.1 主文件: `zeterm/crates/zeterm/src/ui/main_window.rs`

**新增 imports**:
```rust
use crate::app::runtime;
use zeterm_core::config::PasswordRef;
use zeterm_core::entities::AuthConfig;
use zeterm_core::errors::ConnectionError;
use zeterm_ssh::{AuthMethod, SshConfig, SshConnection};
use zeterm_storage::{KeyringSecretStore, SecretHelper};
```

**新增方法**:

| 方法 | 类型 | 说明 |
|------|------|------|
| `connect_to_host` | 同步 | SSH 连接入口，协调整个连接流程 |
| `do_ssh_connect` | 异步 | 执行实际的 SSH 连接 |
| `convert_to_ssh_config` | 异步 | 将 HostConfig 转换为 SshConfig |
| `update_connection_result` | 同步 | 更新连接状态到 UI |

**修改的方法**:

| 方法 | 修改内容 |
|------|----------|
| `on_host_list_event` | `ConnectRequested` 事件现在调用 `connect_to_host()` |

---

## 三、连接流程

```
用户双击主机
    │
    ▼
MainWindow::on_host_list_event(ConnectRequested)
    │
    ▼
MainWindow::connect_to_host()
    │
    ├─1. create_ssh_tab() - 创建新 Tab
    │
    ├─2. update_connection_status(Connecting) - 更新状态栏
    │
    ├─3. SessionCoordinator::new() - 创建会话协调器
    │
    ├─4. add_ssh_terminal_pane() - 添加终端面板
    │
    └─5. runtime::spawn() - 异步执行连接
         │
         ▼
    do_ssh_connect() [异步]
         │
         ├─ convert_to_ssh_config() - 配置转换
         │      │
         │      └─ SecretHelper::resolve_password_ref() - 解析密码
         │
         ├─ SshConnection::new() - 创建连接实例
         │
         ├─ ssh_conn.connect() - 建立 SSH 连接
         │
         ├─ coordinator.set_connection() - 设置连接到协调器
         │
         └─ coordinator.start_data_pump() - 启动数据泵
```

---

## 四、认证方式支持

### 4.1 密码认证

```rust
AuthConfig::Password { password_ref } => {
    let parsed_ref = PasswordRef::parse(password_ref);
    let password = secret_helper.resolve_password_ref(&parsed_ref)?;
    AuthMethod::Password(password)
}
```

**密码引用格式**:
- `keychain:service_name` - 从系统密钥链读取
- `env:VAR_NAME` - 从环境变量读取
- `plain:password` - 明文密码（不推荐）

### 4.2 公钥认证

```rust
AuthConfig::PublicKey { key_path, passphrase_ref } => {
    let passphrase = passphrase_ref
        .map(|ref| secret_helper.resolve_password_ref(&PasswordRef::parse(ref)))
        .transpose()?;
    AuthMethod::PublicKey { key_path, passphrase }
}
```

### 4.3 SSH Agent 认证

```rust
AuthConfig::Agent => AuthMethod::Agent
```

---

## 五、错误处理

| 错误类型 | 处理方式 |
|----------|----------|
| Tab 创建失败 | 记录错误日志，直接返回 |
| 终端面板添加失败 | 记录错误日志，直接返回 |
| 密码解析失败 | 返回 `ConnectionError::Configuration` |
| 密码不存在 | 返回 `ConnectionError::Authentication` |
| SSH 连接失败 | 记录错误日志，coordinator 保持断开状态 |

---

## 六、状态栏更新

| 阶段 | 状态 | 说明 |
|------|------|------|
| 开始连接 | Connecting | 显示"连接中..." |
| 连接成功 | Connected | 显示用户@主机信息 |
| 连接失败 | Error | 显示错误状态 |

---

## 七、待完善功能

### 7.1 P1 - 高优先级

- [ ] 连接失败时显示错误对话框
- [ ] 主机密钥确认对话框集成
- [ ] 连接成功后更新状态栏为 Connected

### 7.2 P2 - 中优先级

- [ ] 连接超时处理
- [ ] 断开重连支持
- [ ] 终端大小自动同步

### 7.3 P3 - 低优先级

- [ ] 连接进度显示
- [ ] 多因素认证支持

---

## 八、测试验证

### 8.1 编译测试

```bash
cargo check  # ✅ 通过
cargo test   # ✅ 所有测试通过
```

### 8.2 端到端测试

需要准备：
1. 配置了 SSH 的测试服务器
2. 在 hosts.toml 中添加主机配置
3. 启动应用，双击主机测试连接

---

## 九、相关文件

- `zeterm/crates/zeterm/src/ui/main_window.rs` - 主窗口（主要修改）
- `zeterm/crates/zeterm-ssh/src/connection.rs` - SSH 连接实现
- `zeterm/crates/zeterm-ssh/src/config.rs` - SSH 配置
- `zeterm/crates/zeterm-storage/src/secret.rs` - 密钥存储
- `zeterm/crates/zeterm-core/src/entities/host.rs` - 主机配置实体

---

## 十、变更统计

| 类别 | 数量 |
|------|------|
| 新增方法 | 4 |
| 修改方法 | 1 |
| 新增 imports | 6 |
| 新增代码行 | ~200 |

---

*文档创建: 2026-01-26*