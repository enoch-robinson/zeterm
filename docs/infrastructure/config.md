# 配置管理系统

> 定义 Zeterm 的配置结构、加载机制与安全存储策略

---

## 一、设计目标

1. **层次化配置** - 支持全局配置、主机配置、会话配置
2. **多格式支持** - TOML 为主，兼容 JSON
3. **安全存储** - 敏感信息加密或使用系统密钥链
4. **热更新** - 部分配置支持运行时更新

---

## 二、配置层次结构

```
~/.config/zeterm/
├── config.toml          # 全局配置
├── hosts.toml           # 主机列表
├── themes/# 主题配置
│   ├── default.toml
│   └── custom.toml
└── keys/                # SSH 密钥 (可选)
    └── ...
```

---

## 三、全局配置 (`config.toml`)

### 3.1 配置结构

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 通用设置
    pub general: GeneralConfig,
    
    /// 终端设置
    pub terminal: TerminalConfig,
    
    /// 外观设置
    pub appearance: AppearanceConfig,
    
    /// 网络设置
    pub network: NetworkConfig,
    
    /// 快捷键
    pub keybindings: KeybindingsConfig,
}

/// 通用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// 语言
    pub language: String,
    
    /// 启动时恢复上次会话
    pub restore_session: bool,
    
    /// 日志级别
    pub log_level: LogLevel,
    
    /// 数据目录
    pub data_dir: Option<PathBuf>,
}

/// 终端设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// 默认 Shell
    pub default_shell: Option<String>,
    
    ///滚动缓冲区行数
    pub scrollback_lines: u32,
    
    /// 光标样式
    pub cursor_style: CursorStyle,
    
    /// 光标闪烁
    pub cursor_blink: bool,
    
    /// 启用响铃
    pub bell_enabled: bool,
}

/// 外观设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// 主题名称
    pub theme: String,
    
    /// 字体族
    pub font_family: String,
    
    /// 字体大小
    pub font_size: f32,
    
    /// 行高
    pub line_height: f32,
    
    /// 窗口透明度
    pub opacity: f32,
}

/// 网络设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// 连接超时 (秒)
    pub connect_timeout: u64,
    
    /// 心跳间隔 (秒)
    pub keepalive_interval: u64,
    
    /// 自动重连
    pub auto_reconnect: bool,
    
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    
    /// 代理设置
    pub proxy: Option<ProxyConfig>,
}
```

### 3.2 示例配置文件

```toml
# Zeterm 全局配置

[general]
language = "zh-CN"
restore_session = true
log_level = "info"

[terminal]
scrollback_lines = 10000
cursor_style = "block"
cursor_blink = true
bell_enabled = false

[appearance]
theme = "default"
font_family = "JetBrains Mono"
font_size = 14.0
line_height = 1.2
opacity = 1.0

[network]
connect_timeout = 30
keepalive_interval = 60
auto_reconnect = true
max_reconnect_attempts = 3

[keybindings]
copy = "ctrl+shift+c"
paste = "ctrl+shift+v"
new_tab = "ctrl+shift+t"
close_tab = "ctrl+shift+w"
split_horizontal = "ctrl+shift+h"
split_vertical = "ctrl+shift+v"
```

---

## 四、主机配置 (`hosts.toml`)

### 4.1 结构定义

```rust
/// 主机配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// 唯一标识
    pub id: String,
    
    /// 显示名称
    pub name: String,
    
    /// 主机地址
    pub host: String,
    
    /// 端口
    pub port: u16,
    
    /// 用户名
    pub username: String,
    
    /// 认证方式
    pub auth: HostAuthConfig,
    
    /// 分组
    pub group: Option<String>,
    
    /// 标签
    pub tags: Vec<String>,
    
    /// 终端覆盖配置
    pub terminal: Option<TerminalOverride>,
    
    /// 启动命令
    pub startup_command: Option<String>,
}

/// 主机认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostAuthConfig {
    #[serde(rename = "password")]
    Password {
        /// 密码 (加密存储或引用密钥链)
        password: SecretString,
    },
    
    #[serde(rename = "publickey")]
    PublicKey {
        /// 私钥路径
        key_path: PathBuf,
        /// 私钥密码
        passphrase: Option<SecretString>,
    },
    
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "ask")]
    AskOnConnect,
}
```

### 4.2 示例主机配置

```toml
# 主机列表

[[hosts]]
id = "prod-server-1"
name = "生产服务器 1"
host = "192.168.1.100"
port = 22
username = "admin"
group = "Production"
tags = ["linux", "web"]

[hosts.auth]
type = "publickey"
key_path = "~/.ssh/id_rsa"

[[hosts]]
id = "dev-server"
name = "开发服务器"
host = "dev.example.com"
port = 22
username = "developer"
group = "Development"

[hosts.auth]
type = "password"
password = "keychain:dev-server-password"  # 引用系统密钥链
```

---

## 五、配置加载器

```rust
use std::path::Path;
use anyhow::Result;

pub struct ConfigManager {
    /// 配置目录
    config_dir: PathBuf,
    
    /// 当前配置
    app_config: AppConfig,
    
    /// 主机列表
    hosts: Vec<HostConfig>,
    
    /// 配置变更通知
    change_tx: broadcast::Sender<ConfigChange>,
}

impl ConfigManager {
    /// 加载配置
    pub fn load(config_dir: impl AsRef<Path>) -> Result<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        
        // 加载全局配置
        let config_path = config_dir.join("config.toml");
        let app_config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            AppConfig::default()
        };
        
        // 加载主机配置
        let hosts_path = config_dir.join("hosts.toml");
        let hosts = if hosts_path.exists() {
            let content = std::fs::read_to_string(&hosts_path)?;
            let hosts_file: HostsFile = toml::from_str(&content)?;
            hosts_file.hosts
        } else {
            Vec::new()
        };
        
        let (change_tx, _) = broadcast::channel(16);
        
        Ok(Self {
            config_dir,
            app_config,
            hosts,
            change_tx,
        })
    }
    
    /// 保存配置
    pub fn save(&self) -> Result<()> {
        // 保存全局配置
        let config_content = toml::to_string_pretty(&self.app_config)?;
        std::fs::write(
            self.config_dir.join("config.toml"),
            config_content,
        )?;
        
        // 保存主机配置
        let hosts_file = HostsFile { hosts: self.hosts.clone() };
        let hosts_content = toml::to_string_pretty(&hosts_file)?;
        std::fs::write(
            self.config_dir.join("hosts.toml"),
            hosts_content,
        )?;
        
        Ok(())
    }
    
    /// 订阅配置变更
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChange> {
        self.change_tx.subscribe()
    }
}
```

---

## 六、敏感信息处理

### 6.1 密钥链集成

```rust
use keyring::Entry;

/// 密钥链管理器
pub struct SecretStore {
    service_name: String,
}

impl SecretStore {
    pub fn new() -> Self {
        Self {
            service_name: "zeterm".to_string(),
        }
    }
    
    /// 存储密码
    pub fn set_password(&self, key: &str, password: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, key)?;
        entry.set_password(password)?;
        Ok(())
    }
    
    /// 获取密码
    pub fn get_password(&self, key: &str) -> Result<String> {
        let entry = Entry::new(&self.service_name, key)?;
        Ok(entry.get_password()?)
    }
    
    /// 删除密码
    pub fn delete_password(&self, key: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, key)?;
        entry.delete_password()?;
        Ok(())
    }
}
```

### 6.2 密码引用解析

```rust
/// 解析密码配置
pub fn resolve_password(value: &str, store: &SecretStore) -> Result<String> {
    if value.starts_with("keychain:") {
        // 从密钥链获取
        let key = value.strip_prefix("keychain:").unwrap();
        store.get_password(key)
    } else if value.starts_with("env:") {
        // 从环境变量获取
        let var = value.strip_prefix("env:").unwrap();
        std::env::var(var).map_err(|e| anyhow::anyhow!("环境变量不存在: {}", e))
    } else {
        // 直接使用 (不推荐)
        Ok(value.to_string())
    }
}
```

---

## 七、相关文档

- [持久化设计](./persistence.md) - 数据存储方案
- [错误处理](../core/error-handling.md) - ConfigError 定义