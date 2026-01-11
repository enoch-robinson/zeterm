# 配置管理系统

> 定义 Zeterm 的配置结构、加载机制与安全存储策略

---

## 一、设计目标

| 目标 | 说明 |
|------|------|
| 层次化配置 | 支持全局配置、主机配置、会话配置 |
| 多格式支持 | TOML 为主，兼容 JSON |
| 安全存储 | 敏感信息加密或使用系统密钥链 |
| 热更新 | 部分配置支持运行时更新 |

---

## 二、目录结构

```
~/.config/zeterm/          # Linux
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\          # Windows
├── config.toml            # 全局配置
├── hosts.toml             # 主机列表
└── themes/                # 主题配置
    └── custom.toml
```

---

## 三、全局配置 (`config.toml`)

### 3.1 配置分类

| 分类 | 说明 |
|------|------|
| `[general]` | 通用设置 (语言、日志等) |
| `[terminal]` | 终端设置 (滚动、光标等) |
| `[appearance]` | 外观设置 (主题、字体等) |
| `[network]` | 网络设置 (超时、重连等) |
| `[keybindings]` | 快捷键绑定 |

### 3.2 配置项参考

#### General

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `language` | "zh-CN" | 界面语言 |
| `restore_session` | true | 启动时恢复上次会话 |
| `log_level` | "info" | 日志级别 |

#### Terminal

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `scrollback_lines` | 10000 | 滚动缓冲区行数 |
| `cursor_style` | "block" | 光标样式 (block/beam/underline) |
| `cursor_blink` | true | 光标闪烁 |
| `bell_enabled` | false | 启用响铃 |

#### Appearance

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `theme` | "default" | 主题名称 |
| `font_family` | "JetBrains Mono" | 字体族 |
| `font_size` | 14.0 | 字体大小 |
| `line_height` | 1.2 | 行高倍数 |
| `opacity` | 1.0 |窗口透明度 |

#### Network

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `connect_timeout` | 30| 连接超时 (秒) |
| `keepalive_interval` | 60 | 心跳间隔 (秒) |
| `auto_reconnect` | true | 自动重连 |
| `max_reconnect_attempts` | 3 | 最大重连次数 |

### 3.3 示例配置

```toml
[general]
language = "zh-CN"
restore_session = true
log_level = "info"

[terminal]
scrollback_lines = 10000
cursor_style = "block"
cursor_blink = true

[appearance]
theme = "default"
font_family = "JetBrains Mono"
font_size = 14.0

[network]
connect_timeout = 30
keepalive_interval = 60
auto_reconnect = true
```

---

## 四、主机配置 (`hosts.toml`)

### 4.1 主机字段

| 字段 | 必填 | 说明 |
|------|------|------|
| `id` | ✅ | 唯一标识 |
| `name` | ✅ | 显示名称 |
| `host` | ✅ | 主机地址 |
| `port` | ❌ | 端口 (默认 22) |
| `username` | ✅ | 用户名 |
| `auth` | ✅ | 认证配置 |
| `group` | ❌ | 分组名称 |
| `tags` | ❌ | 标签列表 |
| `startup_command` | ❌ | 启动命令 |

### 4.2 认证方式

| 类型 | 配置 |
|------|------|
|密码 | `type = "password"`, `password = "..."` |
| 公钥 | `type = "publickey"`, `key_path = "..."` |
| Agent | `type = "agent"` |
| 每次询问 | `type = "ask"` |

### 4.3 示例

```toml
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
username = "developer"

[hosts.auth]
type = "password"
password = "keychain:dev-server-password"  # 引用系统密钥链
```

---

## 五、敏感信息处理

### 5.1 存储方式

| 方式 | 格式 | 说明 |
|------|------|------|
| 系统密钥链 | `keychain:key-name` | 推荐，最安全 |
| 环境变量 | `env:VAR_NAME` | 适合CI/CD |
| 明文 | 直接写入 | 不推荐 |

### 5.2 密钥链集成

- **macOS**: Keychain
- **Windows**: Credential Manager
- **Linux**: Secret Service (GNOME Keyring / KWallet)

---

## 六、配置加载流程

```
1. 确定配置目录 (平台相关)
2. 加载config.toml (不存在则使用默认)
3. 加载 hosts.toml (不存在则为空)
4. 解析敏感信息引用 (keychain:/env:)
5. 验证配置有效性
6. 启动文件监听 (热更新)
```

---

## 七、热更新支持

| 配置类型 | 热更新 | 说明 |
|----------|--------|------|
| 外观设置 | ✅ | 立即生效 |
| 快捷键 | ✅ | 立即生效 |
| 终端设置 | ⚠️ | 新会话生效 |
| 网络设置 | ⚠️ | 新连接生效 |
| 主机列表 | ✅ | 立即生效 |

---

## 八、相关文档

- [持久化设计](./persistence.md) - 数据存储方案
- [错误处理](../core/error-handling.md) - ConfigError 定义