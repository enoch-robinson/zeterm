# PERSISTENCE.md

> 配置管理与数据持久化

---

## 一、目录结构

### 1.1 配置目录（用户可编辑）

```
~/.config/zeterm/              # Linux (XDG_CONFIG_HOME)
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\              # Windows
├── config.toml                # 全局配置
├── hosts.toml                 # 主机列表
├── known_hosts                # SSH 主机密钥
└── themes/                    # 自定义主题
    └── custom.toml
```

### 1.2 数据目录（程序管理）

```
~/.local/share/zeterm/         # Linux (XDG_DATA_HOME)
~/Library/Application Support/zeterm/data/  # macOS
%LOCALAPPDATA%\zeterm\         # Windows
├── zeterm.db                  # SQLite 数据库
├── sessions/                  # 会话快照
└── logs/                      # 日志文件
```

---

## 二、全局配置 (`config.toml`)

### 2.1 配置项

| 分类 | 配置项 | 默认值 | 说明 |
|------|--------|--------|------|
| `[general]` | `language` | "zh-CN" | 界面语言 |
| `[general]` | `restore_session` | true | 启动时恢复会话 |
| `[terminal]` | `scrollback_lines` | 10000 | 滚动缓冲区行数 |
| `[terminal]` | `cursor_style` | "block" | 光标样式 |
| `[appearance]` | `theme` | "default" | 主题名称 |
| `[appearance]` | `font_family` | "JetBrains Mono" | 字体族 |
| `[appearance]` | `font_size` | 14.0 | 字体大小 |
| `[network]` | `connect_timeout_ms` | 30000 | 连接超时（毫秒） |
| `[network]` | `keepalive_interval_ms` | 60000 | 心跳间隔（毫秒） |
| `[network]` | `auto_reconnect` | true | 自动重连 |
| `[network]` | `max_reconnect_attempts` | 3 | 最大重连次数 |
| `[network]` | `reconnect_initial_delay_ms` | 1000 | 重连初始延迟（毫秒） |
| `[network]` | `reconnect_max_delay_ms` | 30000 | 重连最大延迟（毫秒） |

### 2.2 示例

```toml
[general]
language = "zh-CN"
restore_session = true

[terminal]
scrollback_lines = 10000
cursor_style = "block"

[appearance]
theme = "default"
font_family = "JetBrains Mono"
font_size = 14.0

[network]
connect_timeout_ms = 30000
keepalive_interval_ms = 60000
auto_reconnect = true
max_reconnect_attempts = 3
reconnect_initial_delay_ms = 1000
reconnect_max_delay_ms = 30000
```

---

## 三、主机配置 (`hosts.toml`)

### 3.1 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `id` | ✅ | 唯一标识 |
| `name` | ✅ | 显示名称 |
| `host` | ✅ | 主机地址 |
| `port` | ❌ | 端口（默认 22） |
| `username` | ✅ | 用户名 |
| `auth` | ✅ | 认证配置 |
| `group` | ❌ | 分组名称 |

### 3.2 认证方式

```toml
# 密码认证
[hosts.auth]
type = "password"
password = "keychain:server_name"  # 推荐：系统密钥链

# 公钥认证
[hosts.auth]
type = "publickey"
key_path = "~/.ssh/id_rsa"

# SSH Agent
[hosts.auth]
type = "agent"
```

### 3.3 示例

```toml
[[hosts]]
id = "prod-server"
name = "生产服务器"
host = "192.168.1.100"
port = 22
username = "admin"
group = "Production"

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
password = "keychain:dev-password"
```

---

## 四、数据库存储

### 4.1 表结构

**hosts 表**

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT PK | 唯一标识 |
| `name` | TEXT | 显示名称 |
| `host` | TEXT | 主机地址 |
| `port` | INTEGER | 端口 |
| `username` | TEXT | 用户名 |
| `auth_type` | TEXT | 认证类型 |
| `group_name` | TEXT | 分组名称 |
| `created_at` | INTEGER | 创建时间 |
| `updated_at` | INTEGER | 更新时间 |

**connection_history 表**

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 自增 ID |
| `host_id` | TEXT FK | 关联主机 |
| `connected_at` | INTEGER | 连接时间 |
| `disconnected_at` | INTEGER | 断开时间 |
| `duration_secs` | INTEGER | 持续时长 |

### 4.2 同步机制

启动时：`hosts.toml` → SQLite  
运行时：UI 修改 → 同时更新 TOML 和 SQLite

---

## 五、敏感信息存储

| 方式 | 格式 | 说明 |
|------|------|------|
| 系统密钥链 | `keychain:key-name` | 推荐，最安全 |
| 环境变量 | `env:VAR_NAME` | 适合 CI/CD |
| 明文 | 直接写入 | 不推荐 |

**平台支持**：
- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service

---

## 六、热更新

| 配置类型 | 热更新 | 说明 |
|----------|--------|------|
| 外观设置 | ✅ | 立即生效 |
| 快捷键 | ✅ | 立即生效 |
| 终端设置 | ⚠️ | 新会话生效 |
| 主机列表 | ✅ | 立即生效 |

---

## 七、相关文档

- [ARCHITECTURE.md](./ARCHITECTURE.md) - 架构设计
- [API.md](./API.md) - 配置实体定义
- [USER_GUIDE.md](./USER_GUIDE.md) - 用户配置指南