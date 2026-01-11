# 数据持久化设计

> 定义 Zeterm 的数据存储方案与持久化策略

---

## 一、设计目标

1. **可靠存储** - 确保用户数据不丢失
2. **快速访问** - 启动时快速加载
3. **跨平台** - 支持 Windows/macOS/Linux
4. **可迁移** - 支持导入导出

---

## 二、存储架构

### 2.1 数据分类

| 数据类型 | 存储方式 | 说明 |
|----------|----------|------|
| 配置文件 | TOML 文件 | 用户可编辑 |
| 主机列表 | SQLite | 支持搜索和分组 |
| 会话历史 | SQLite | 连接记录 |
| 命令历史 | SQLite | 可选功能 |
| 敏感信息 | 系统密钥链 | 密码、私钥密码 |

### 2.2 目录结构

```
~/.local/share/zeterm/     # Linux
~/Library/Application Support/zeterm/  # macOS
%APPDATA%\zeterm\          # Windows
├── zeterm.db              # SQLite 数据库
├── sessions/              # 会话快照
│   └── {session_id}.json
└── logs/                  # 日志文件
    └── zeterm.log
```

---

## 三、数据库设计

### 3.1 主机表 (`hosts`)

```sql
CREATE TABLE hosts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER DEFAULT 22,
    username TEXT NOT NULL,
    auth_type TEXT NOT NULL,  -- 'password', 'publickey', 'agent'
    auth_data TEXT,           -- JSON,加密存储
    group_name TEXT,
    tags TEXT,-- JSON数组
    terminal_config TEXT,     -- JSON,覆盖配置
    startup_command TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_hosts_group ON hosts(group_name);
CREATE INDEX idx_hosts_name ON hosts(name);
```

### 3.2 连接历史表 (`connection_history`)

```sql
CREATE TABLE connection_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id TEXT NOT NULL,
    connected_at INTEGER NOT NULL,
    disconnected_at INTEGER,
    duration_secs INTEGER,
    disconnect_reason TEXT,
    FOREIGN KEY (host_id) REFERENCES hosts(id)
);

CREATE INDEX idx_history_host ON connection_history(host_id);
CREATE INDEX idx_history_time ON connection_history(connected_at DESC);
```

### 3.3 会话快照表 (`session_snapshots`)

```sql
CREATE TABLE session_snapshots (
    id TEXT PRIMARY KEY,
    host_id TEXT NOT NULL,
    layout TEXT NOT NULL,     -- JSON,窗口布局
    scroll_position INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (host_id) REFERENCES hosts(id)
);
```

---

## 四、数据访问层

### 4.1 Repository 接口

```rust
use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait HostRepository: Send + Sync {
    /// 获取所有主机
    async fn list_all(&self) -> Result<Vec<HostConfig>>;
    
    /// 按分组获取
    async fn list_by_group(&self, group: &str) -> Result<Vec<HostConfig>>;
    
    /// 搜索主机
    async fn search(&self, query: &str) -> Result<Vec<HostConfig>>;
    
    /// 获取单个主机
    async fn get(&self, id: &str) -> Result<Option<HostConfig>>;
    
    /// 创建主机
    async fn create(&self, host: &HostConfig) -> Result<()>;
    
    /// 更新主机
    async fn update(&self, host: &HostConfig) -> Result<()>;
    
    /// 删除主机
    async fn delete(&self, id: &str) -> Result<()>;
}
```

### 4.2 SQLite 实现

```rust
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub struct SqliteHostRepository {
    pool: SqlitePool,
}

impl SqliteHostRepository {
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", db_path))
            .await?;
        
        // 运行迁移
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl HostRepository for SqliteHostRepository {
    async fn list_all(&self) -> Result<Vec<HostConfig>> {
        let rows = sqlx::query_as!(
            HostRow,
            "SELECT * FROM hosts ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;
        
        rows.into_iter()
            .map(|r| r.try_into())
            .collect()
    }
    
    async fn search(&self, query: &str) -> Result<Vec<HostConfig>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query_as!(
            HostRow,
            r#"
            SELECT * FROM hosts 
            WHERE name LIKE ? OR host LIKE ? OR tags LIKE ?
            ORDER BY name
            "#,
            pattern, pattern, pattern
        )
        .fetch_all(&self.pool)
        .await?;
        
        rows.into_iter()
            .map(|r| r.try_into())
            .collect()
    }
    
    // ... 其他方法实现
}
```

---

## 五、会话恢复

### 5.1 会话状态结构

```rust
/// 可持久化的会话状态
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// 会话 ID
    pub id: String,
    /// 关联的主机 ID
    pub host_id: String,
    
    /// 窗口布局
    pub layout: WindowLayout,
    
    /// 各终端的滚动位置
    pub scroll_positions: HashMap<String, u32>,
    
    /// 创建时间
    pub created_at: i64,
}

/// 窗口布局
#[derive(Debug, Serialize, Deserialize)]
pub enum WindowLayout {
    Single { terminal_id: String },
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<WindowLayout>,
        second: Box<WindowLayout>,
    },
}
```

### 5.2 自动保存

```rust
impl SessionManager {
    /// 启动自动保存任务
    pub fn start_auto_save(&self, interval: Duration) {
        let sessions = self.sessions.clone();
        let db = self.db.clone();
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                
                let snapshots: Vec<_> = sessions
                    .read()
                    .await
                    .values()
                    .map(|s| s.to_snapshot())
                    .collect();
                
                for snapshot in snapshots {
                    if let Err(e) = db.save_snapshot(&snapshot).await {
                        tracing::warn!("保存会话快照失败: {}", e);
                    }
                }
            }
        });
    }
}
```

---

## 六、数据迁移

### 6.1 版本管理

```rust
/// 数据库版本
const CURRENT_VERSION: u32 = 1;

/// 检查并执行迁移
pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    let version = get_db_version(pool).await?;
    
    if version < CURRENT_VERSION {
        for v in version..CURRENT_VERSION {
            run_migration(pool, v + 1).await?;
        }
    }
    
    Ok(())
}
```

### 6.2 导入导出

```rust
/// 导出数据
pub async fn export_data(db: &Database, path: &Path) -> Result<()> {
    let export = ExportData {
        version: CURRENT_VERSION,
        hosts: db.hosts().list_all().await?,
        // 不导出敏感信息
    };
    
    let json = serde_json::to_string_pretty(&export)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// 导入数据
pub async fn import_data(db: &Database, path: &Path) -> Result<ImportResult> {
    let content = std::fs::read_to_string(path)?;
    let data: ExportData = serde_json::from_str(&content)?;
    
    let mut imported = 0;
    let mut skipped = 0;
    
    for host in data.hosts {
        if db.hosts().get(&host.id).await?.is_none() {
            db.hosts().create(&host).await?;
            imported += 1;
        } else {
            skipped += 1;
        }
    }
    
    Ok(ImportResult { imported, skipped })
}
```

---

## 七、相关文档

- [配置管理](./config.md) - 配置文件设计
- [错误处理](../core/error-handling.md) - 存储相关错误