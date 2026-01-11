# 数据持久化设计

> 定义 Zeterm 的数据存储方案与持久化策略

---

## 一、设计目标

| 目标 | 说明 |
|------|------|
| 可靠存储 | 确保用户数据不丢失 |
| 快速访问 | 启动时快速加载 |
| 跨平台 | 支持 Windows/macOS/Linux |
| 可迁移 | 支持导入导出 |

---

## 二、存储架构

### 2.1 数据分类

| 数据类型 | 存储方式 | 说明 |
|----------|----------|------|
| 全局配置 | TOML 文件 | 用户可编辑 (`config.toml`) |
| 主机列表 | TOML + SQLite | TOML 用于导入导出，SQLite 用于运行时 |
| 会话历史 | SQLite | 连接记录 |
| 敏感信息 | 系统密钥链 | 密码、私钥密码 |

> **说明**: 主机配置支持两种存储方式：
> - **TOML 文件** (`hosts.toml`): 便于用户手动编辑、版本控制和跨设备同步
> - **SQLite 数据库**: 运行时使用，支持高效搜索、分组和历史记录
> 
> 启动时会将 `hosts.toml` 同步到 SQLite，UI 修改会同时更新两者。

### 2.2 目录结构

配置目录 (用户可编辑):
```
~/.config/zeterm/              # Linux (XDG_CONFIG_HOME)
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\              # Windows
├── config.toml                # 全局配置
├── hosts.toml                 # 主机列表 (可选，用于导入导出)
├── known_hosts                # SSH 主机密钥
└── themes/                    # 自定义主题
```

数据目录 (程序管理):
```
~/.local/share/zeterm/         # Linux (XDG_DATA_HOME)
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\# Windows
├── zeterm.db                  # SQLite 数据库
├── sessions/                  # 会话快照
└── logs/                      # 日志文件
```

---

## 三、数据库设计

### 3.1 主机表 (`hosts`)

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT PK | 唯一标识 |
| `name` | TEXT | 显示名称 |
| `host` | TEXT | 主机地址 |
| `port` | INTEGER | 端口 (默认 22) |
| `username` | TEXT | 用户名 |
| `auth_type` | TEXT | 认证类型 |
| `auth_data` | TEXT | 认证数据 (JSON, 加密) |
| `group_name` | TEXT | 分组名称 |
| `tags` | TEXT | 标签 (JSON 数组) |
| `created_at` | INTEGER | 创建时间 |
| `updated_at` | INTEGER | 更新时间 |

### 3.2 连接历史表 (`connection_history`)

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 自增 ID |
| `host_id` | TEXT FK | 关联主机 |
| `connected_at` | INTEGER | 连接时间 |
| `disconnected_at` | INTEGER | 断开时间 |
| `duration_secs` | INTEGER | 持续时长 |
| `disconnect_reason` | TEXT | 断开原因 |

### 3.3 索引

| 索引 | 字段 | 用途 |
|------|------|------|
| `idx_hosts_group` | `group_name` | 按分组查询 |
| `idx_hosts_name` | `name` | 按名称搜索 |
| `idx_history_time` | `connected_at DESC` | 最近连接 |

---

## 四、数据访问层

### 4.1HostRepository接口

| 方法 | 说明 |
|------|------|
| `list_all()` | 获取所有主机 |
| `list_by_group(group)` | 按分组获取|
| `search(query)` | 搜索主机 |
| `get(id)` | 获取单个主机 |
| `create(host)` | 创建主机 |
| `update(host)` | 更新主机 |
| `delete(id)` | 删除主机 |

### 4.2 实现

使用 `sqlx` 实现 SQLite 访问：

- 连接池管理 (`SqlitePool`)
- 编译时 SQL 检查 (`query_as!`)
- 自动迁移 (`sqlx::migrate!`)

---

## 五、会话恢复

### 5.1 会话快照内容

| 字段 | 说明 |
|------|------|
| `id` | 会话 ID |
| `host_id` | 关联主机 |
| `layout` | 窗口布局 (JSON) |
| `scroll_positions` | 各终端滚动位置 |
| `created_at` | 创建时间 |

### 5.2 自动保存策略

- **保存间隔**: 30 秒
- **触发条件**: 布局变化、滚动位置变化
- **保存内容**: 窗口布局、滚动位置

### 5.3 恢复流程

```
1. 启动时检查 restore_session 配置
2. 加载最近的会话快照
3. 恢复窗口布局
4. 重新建立连接 (可选)
```

---

## 六、数据迁移

### 6.1 版本管理

-数据库版本号存储在 `_meta` 表
- 启动时检查版本，执行增量迁移
- 迁移脚本位于 `migrations/` 目录

### 6.2 导入导出

| 操作 | 格式 | 内容 |
|------|------|------|
| 导出 | JSON | 主机列表 (不含密码) |
| 导入 | JSON | 合并或覆盖主机 |

**注意**: 敏感信息不包含在导出文件中。

---

## 七、相关文档

- [配置管理](./config.md) - 配置文件设计
- [错误处理](../core/error-handling.md) - 存储相关错误