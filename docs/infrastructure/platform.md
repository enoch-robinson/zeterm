# 跨平台差异处理

> Windows、macOS、Linux 平台差异与适配策略

---

## 一、概述

### 1.1 支持平台

| 平台 | 最低版本 | 架构 |
|------|----------|------|
| Windows | 10 (1903+) | x64, ARM64 |
| macOS | 11.0+ | x64, ARM64 |
| Linux | glibc 2.31+ | x64 |

### 1.2 主要差异领域

| 领域 | 差异程度 | 说明 |
|------|----------|------|
| SSH Agent | 高 | 各平台实现完全不同 |
| 凭证存储 | 高 | 不同的系统 API |
| 文件路径 | 中 | 配置目录位置不同 |
| 字体渲染 | 中 | 渲染引擎差异 |
| 快捷键 | 低 | 修饰键习惯不同 |

---

## 二、SSH Agent 集成

### 2.1 平台差异

| 平台 | Agent 类型 | 通信方式 |
|------|------------|----------|
| macOS | ssh-agent | Unix Socket (`SSH_AUTH_SOCK`) |
| Linux | ssh-agent / gnome-keyring | Unix Socket (`SSH_AUTH_SOCK`) |
| Windows | OpenSSH Agent | Named Pipe (`\\.\pipe\openssh-ssh-agent`) |
| Windows | Pageant (PuTTY) | Shared Memory |

### 2.2 Agent 检测实现

```rust
use std::env;

#[cfg(unix)]
pub fn detect_ssh_agent() -> Option<AgentConnection> {
    // Unix:检查 SSH_AUTH_SOCK 环境变量
    let socket_path = env::var("SSH_AUTH_SOCK").ok()?;
    if std::path::Path::new(&socket_path).exists() {
        Some(AgentConnection::UnixSocket(socket_path))
    } else {
        None
    }
}

#[cfg(windows)]
pub fn detect_ssh_agent() -> Option<AgentConnection> {
    // Windows: 优先检查 OpenSSH Agent
    if check_openssh_agent() {
        return Some(AgentConnection::NamedPipe(
            r"\\.\pipe\openssh-ssh-agent".to_string()
        ));
    }
    
    // 备选:检查 Pageant
    if check_pageant() {
        return Some(AgentConnection::Pageant);
    }
    
    None
}

#[cfg(windows)]
fn check_openssh_agent() -> bool {
    use windows::Win32::Storage::FileSystem::*;
    // 检查 Named Pipe 是否存在
    let pipe_path = r"\\.\pipe\openssh-ssh-agent";
    std::path::Path::new(pipe_path).exists()
}

#[cfg(windows)]
fn check_pageant() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::*;
    // 检查 Pageant 窗口是否存在
    unsafe {
        let hwnd = FindWindowA(
            windows::core::s!("Pageant"),
            windows::core::s!("Pageant")
        );
        hwnd.0 != 0
    }
}
```

### 2.3 Agent 连接抽象

```rust
pub enum AgentConnection {
    UnixSocket(String),
    NamedPipe(String),
    Pageant,
}

#[async_trait]
pub trait SshAgent: Send + Sync {
    /// 列出所有可用密钥
    async fn list_identities(&self) -> Result<Vec<PublicKey>>;
    
    /// 使用 Agent 中的密钥签名
    async fn sign(&self, key: &PublicKey, data: &[u8]) -> Result<Vec<u8>>;
}

// 平台特定实现
#[cfg(unix)]
pub struct UnixSshAgent { /* ... */ }

#[cfg(windows)]
pub struct WindowsSshAgent { /* ... */ }

#[cfg(windows)]
pub struct PageantAgent { /* ... */ }
```

---

## 三、凭证存储

### 3.1 平台API

| 平台 | API | 库 |
|------|-----|-----|
| macOS | Keychain Services | `security-framework` |
| Windows | Credential Manager | `windows-credentials` |
| Linux | Secret Service (D-Bus) | `secret-service` |

### 3.2 统一接口

>📖 完整 API 定义见 [API 文档](../api.md)

```rust
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// 存储密钥
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError>;
    
    /// 获取密钥
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError>;
    
    /// 删除密钥
    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError>;
    
    /// 检查是否可用
    fn is_available(&self) -> bool;
}
```

### 3.3 macOS Keychain 实现

```rust
#[cfg(target_os = "macos")]
pub struct KeychainStore;

#[cfg(target_os = "macos")]
impl SecretStore for KeychainStore {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError> {
        use security_framework::passwords::*;
        
        // 删除已存在的条目
        let _ = delete_generic_password(service, key);
        
        // 添加新条目
        set_generic_password(service, key, value.as_bytes()).map_err(|e| Error::Keychain(e.to_string()))
    }
    
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError> {
        use security_framework::passwords::*;
        
        match get_generic_password(service, key) {
            Ok(bytes) => {
                let value = String::from_utf8(bytes)
                    .map_err(|_| Error::InvalidUtf8)?;
                Ok(Some(value))
            }
            Err(e) if e.code() == errSecItemNotFound => Ok(None),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }
    
    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError> {
        use security_framework::passwords::*;
        
        delete_generic_password(service, key)
            .map_err(|e| Error::Keychain(e.to_string()))
    }
    
    fn is_available(&self) -> bool {
        true // macOS 始终可用
    }
}
```

### 3.4 Windows Credential Manager 实现

```rust
#[cfg(target_os = "windows")]
pub struct CredentialStore;

#[cfg(target_os = "windows")]
impl SecretStore for CredentialStore {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError> {
        use windows_credentials::*;
        
        let target = format!("{}:{}", service, key);
        let credential = Credential::new(&target)
            .username(key)
            .secret(value);
        
        credential.save()
            .map_err(|e| Error::Credential(e.to_string()))
    }
    
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, SecretError> {
        use windows_credentials::*;
        
        let target = format!("{}:{}", service, key);
        
        match Credential::load(&target) {
            Ok(cred) => Ok(Some(cred.secret().to_string())),
            Err(CredentialError::NotFound) => Ok(None),
            Err(e) => Err(Error::Credential(e.to_string())),
        }
    }
    
    async fn delete(&self, service: &str, key: &str) -> Result<(), SecretError> {
        use windows_credentials::*;
        
        let target = format!("{}:{}", service, key);
        Credential::delete(&target)
            .map_err(|e| Error::Credential(e.to_string()))
    }
    
    fn is_available(&self) -> bool {
        true // Windows 始终可用
    }
}
```

### 3.5 Linux Secret Service 实现

```rust
#[cfg(target_os = "linux")]
pub struct SecretServiceStore {
    available: bool,
}

#[cfg(target_os = "linux")]
impl SecretServiceStore {
    pub fn new() -> Self {
        // 检查 D-Bus Secret Service 是否可用
        let available = Self::check_availability();
        Self { available }
    }
    
    fn check_availability() -> bool {
        // 尝试连接 Secret Service
        secret_service::SecretService::connect(
            secret_service::EncryptionType::Dh
        ).is_ok()
    }
}

#[cfg(target_os = "linux")]
impl SecretStore for SecretServiceStore {
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), SecretError> {
        use secret_service::*;
        
        let ss = SecretService::connect(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        
        // 解锁集合
        collection.unlock()?;
        
        // 创建属性
        let mut attributes = HashMap::new();
        attributes.insert("service", service);
        attributes.insert("key", key);
        
        // 存储密钥
        collection.create_item(
            &format!("{}/{}", service, key),
            attributes,
            value.as_bytes(),
            true, // 替换已存在的"text/plain",
        )?;
        
        Ok(())
    }
    
    fn is_available(&self) -> bool {
        self.available
    }
}
```

### 3.6 工厂函数

```rust
/// 获取当前平台的密钥存储实现
pub fn get_secret_store() -> Box<dyn SecretStore> {
    #[cfg(target_os = "macos")]
    { Box::new(KeychainStore) }
    
    #[cfg(target_os = "windows")]
    { Box::new(CredentialStore) }
    
    #[cfg(target_os = "linux")]
    {
        let store = SecretServiceStore::new();
        if store.is_available() {
            Box::new(store)
        } else {
            // 回退到加密文件存储
            Box::new(EncryptedFileStore::new())
        }
    }
}
```

---

## 四、文件路径

### 4.1 配置目录

| 平台 | 配置目录 | 数据目录 |
|------|----------|----------|
| macOS | `~/Library/Application Support/zeterm` | 同配置目录 |
| Windows | `%APPDATA%\zeterm` | `%LOCALAPPDATA%\zeterm` |
| Linux | `~/.config/zeterm` | `~/.local/share/zeterm` |

### 4.2 路径获取实现

```rust
use std::path::PathBuf;

/// 获取配置目录
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~")).join("zeterm")
    }
    
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zeterm")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            })
            .join("zeterm")
    }
}

/// 获取数据目录
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    { config_dir() }
    
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zeterm")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            })
            .join("zeterm")
    }
}

/// 获取日志目录
pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs/zeterm")
    }
    
    #[cfg(target_os = "windows")]
    {
        data_dir().join("logs")
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::state_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/state")
            })
            .join("zeterm/logs")
    }
}

/// SSH 密钥默认路径
pub fn ssh_keys_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
}
```

---

## 五、字体渲染

### 5.1 平台差异

| 平台 | 渲染引擎 | 特点 |
|------|----------|------|
| macOS | Core Text | 平滑、略粗 |
| Windows | DirectWrite | 清晰、ClearType |
| Linux | FreeType + fontconfig | 可配置 |

### 5.2 字体回退链

```rust
/// 获取平台默认等宽字体
pub fn default_monospace_fonts() -> Vec<&'static str> {
    #[cfg(target_os = "macos")]
    {
        vec![
            "JetBrains Mono",
            "SF Mono",
            "Menlo",
            "Monaco",
            "Courier New",
        ]
    }
    
    #[cfg(target_os = "windows")]
    {
        vec![
            "JetBrains Mono",
            "Cascadia Code",
            "Consolas",
            "Courier New",
        ]
    }
    
    #[cfg(target_os = "linux")]
    {
        vec![
            "JetBrains Mono",
            "Fira Code",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Noto Sans Mono",
            "monospace",
        ]
    }
}
```

### 5.3 字体配置

```rust
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub line_height: f32,
    /// Windows 特有: 是否启用 ClearType
    #[cfg(target_os = "windows")]
    pub cleartype: bool,
    /// Linux 特有: 抗锯齿模式
    #[cfg(target_os = "linux")]
    pub antialias: AntialiasMode,
}

#[cfg(target_os = "linux")]
pub enum AntialiasMode {
    None,
    Grayscale,
    Subpixel,
}
```

---

## 六、快捷键

### 6.1 修饰键映射

| 功能 | macOS | Windows/Linux |
|------|-------|---------------|
| 复制 | Cmd+C | Ctrl+Shift+C |
| 粘贴 | Cmd+V | Ctrl+Shift+V |
| 新建标签 | Cmd+T | Ctrl+Shift+T |
| 关闭标签 | Cmd+W | Ctrl+Shift+W |
| 设置 | Cmd+, | Ctrl+, |
| 全选 | Cmd+A | Ctrl+Shift+A |
| 搜索 | Cmd+F | Ctrl+Shift+F |

### 6.2 快捷键配置

```rust
/// 平台相关的修饰键
pub fn platform_modifier() -> Modifiers {
    #[cfg(target_os = "macos")]
    { Modifiers::COMMAND }
    
    #[cfg(not(target_os = "macos"))]
    { Modifiers::CONTROL | Modifiers::SHIFT }
}

/// 默认快捷键绑定
pub fn default_keybindings() -> HashMap<String, Action> {
    let modifier = platform_modifier();
    
    let mut bindings = HashMap::new();
    
    #[cfg(target_os = "macos")]
    {
        bindings.insert("cmd-c".to_string(), Action::Copy);
        bindings.insert("cmd-v".to_string(), Action::Paste);
        bindings.insert("cmd-t".to_string(), Action::NewTab);
        bindings.insert("cmd-w".to_string(), Action::CloseTab);
        bindings.insert("cmd-,".to_string(), Action::Settings);
        bindings.insert("cmd-f".to_string(), Action::Search);
        bindings.insert("cmd-n".to_string(), Action::NewWindow);
        bindings.insert("cmd-q".to_string(), Action::Quit);
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        bindings.insert("ctrl-shift-c".to_string(), Action::Copy);
        bindings.insert("ctrl-shift-v".to_string(), Action::Paste);
        bindings.insert("ctrl-shift-t".to_string(), Action::NewTab);
        bindings.insert("ctrl-shift-w".to_string(), Action::CloseTab);
        bindings.insert("ctrl-,".to_string(), Action::Settings);
        bindings.insert("ctrl-shift-f".to_string(), Action::Search);
        bindings.insert("ctrl-shift-n".to_string(), Action::NewWindow);
        bindings.insert("alt-f4".to_string(), Action::Quit);
    }
    
    bindings
}
```

---

## 七、终端行为差异

### 7.1 换行符

| 平台 | 换行符 | 说明 |
|------|--------|------|
| Unix (macOS/Linux) | `\n` (LF) | 标准 |
| Windows | `\r\n` (CRLF) | 需要转换 |

### 7.2 Shell 环境

| 平台 |默认 Shell | 环境变量 |
|------|------------|----------|
| macOS | zsh | `SHELL` |
| Linux | bash/zsh | `SHELL` |
| Windows | PowerShell | `COMSPEC` |

### 7.3 环境检测

```rust
/// 获取默认 Shell
pub fn default_shell() -> String {
    #[cfg(unix)]
    {
        std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/sh".to_string())
    }
    
    #[cfg(windows)]
    {
        std::env::var("COMSPEC")
            .unwrap_or_else(|_| "cmd.exe".to_string())
    }
}

/// 获取用户主目录
pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| {
        #[cfg(unix)]
        { PathBuf::from("/") }
        
        #[cfg(windows)]
        { PathBuf::from("C:\\") }
    })
}
```

---

## 八、known_hosts 处理

### 8.1 文件位置

| 平台 | 路径 |
|------|------|
| macOS/Linux | `~/.ssh/known_hosts` |
| Windows | `%USERPROFILE%\.ssh\known_hosts` |

### 8.2 兼容性处理

```rust
pub fn known_hosts_path() -> PathBuf {
    ssh_keys_dir().join("known_hosts")
}

/// 读取 known_hosts 文件
pub fn load_known_hosts() -> Result<KnownHosts> {
    let path = known_hosts_path();
    
    if !path.exists() {
        return Ok(KnownHosts::new());
    }
    
    let content = std::fs::read_to_string(&path)?;
    
    // 处理不同平台的换行符
    let content = content.replace("\r\n", "\n");
    
    KnownHosts::parse(&content)
}
```

---

## 九、构建配置

### 9.1 Cargo.toml 平台特定依赖

```toml
[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "2"
cocoa = "0.25"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Security_Credentials",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Storage_FileSystem",
]}

[target.'cfg(target_os = "linux")'.dependencies]
secret-service = "3"
```

### 9.2 条件编译示例

```rust
// 平台特定模块
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

// 统一导出
pub use platform::*;

#[cfg(target_os = "macos")]
mod platform {
    pub use super::macos::*;
}

#[cfg(target_os = "windows")]
mod platform {
    pub use super::windows::*;
}

#[cfg(target_os = "linux")]
mod platform {
    pub use super::linux::*;
}
```

---

## 十、相关文档

- [配置管理](./config.md) - 配置文件设计
- [安全设计](../core/security.md) - 凭证保护
- [SSH 后端](../modules/ssh-backend.md) - SSH 实现