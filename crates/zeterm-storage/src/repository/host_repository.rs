//! 主机配置仓库
//!
//! 提供主机配置的数据访问接口和实现。

use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use zeterm_core::entities::{AuthConfig, HostConfig, HostId};

/// 主机配置仓库接口
///
/// 定义主机配置的 CRUD 操作。
#[async_trait]
pub trait HostRepository: Send + Sync {
    /// 列出所有主机配置
    async fn list_all(&self) -> Result<Vec<HostConfig>>;

    /// 按分组列出主机配置
    async fn list_by_group(&self, group: &str) -> Result<Vec<HostConfig>>;

    /// 搜索主机配置（按名称、主机地址、用户名）
    async fn search(&self, query: &str) -> Result<Vec<HostConfig>>;

    /// 获取指定 ID 的主机配置
    async fn get(&self, id: HostId) -> Result<Option<HostConfig>>;

    /// 创建新的主机配置
    async fn create(&self, config: &HostConfig) -> Result<HostId>;

    /// 更新主机配置
    async fn update(&self, config: &HostConfig) -> Result<()>;

    /// 删除主机配置
    async fn delete(&self, id: HostId) -> Result<()>;

    /// 统计主机数量
    async fn count(&self) -> Result<i64>;

    /// 获取所有分组名称
    async fn list_groups(&self) -> Result<Vec<String>>;
}

/// SQLite 主机配置仓库实现
pub struct SqliteHostRepository {
    pool: SqlitePool,
}

impl SqliteHostRepository {
    /// 创建新的仓库实例
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 将数据库行转换为 HostConfig
    fn row_to_host_config(row: &sqlx::sqlite::SqliteRow) -> Result<HostConfig> {
        let id: i64 = row.try_get("id").context("获取 id 失败")?;
        let name: String = row.try_get("name").context("获取 name 失败")?;
        let host: String = row.try_get("host").context("获取 host 失败")?;
        let port: i64 = row.try_get("port").context("获取 port 失败")?;
        let username: String = row.try_get("username").context("获取 username 失败")?;
        let auth_config_json: String = row
            .try_get("auth_config")
            .context("获取 auth_config 失败")?;
        let group_name: Option<String> =
            row.try_get("group_name").context("获取 group_name 失败")?;
        let tags_json: Option<String> = row.try_get("tags").context("获取 tags 失败")?;
        let description: Option<String> = row
            .try_get("description")
            .context("获取 description 失败")?;
        let created_at: i64 = row.try_get("created_at").context("获取 created_at 失败")?;
        let updated_at: i64 = row.try_get("updated_at").context("获取 updated_at 失败")?;

        let auth_config: AuthConfig =
            serde_json::from_str(&auth_config_json).context("解析认证配置失败")?;

        let tags: Vec<String> = if let Some(json) = tags_json {
            serde_json::from_str(&json).context("解析标签失败")?
        } else {
            Vec::new()
        };

        Ok(HostConfig {
            id: Some(HostId::new(id)),
            name,
            host,
            port: port as u16,
            username,
            auth_config,
            group: group_name,
            tags,
            description,
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    /// 获取当前 Unix 时间戳（秒）
    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

#[async_trait]
impl HostRepository for SqliteHostRepository {
    async fn list_all(&self) -> Result<Vec<HostConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, host, port, username, auth_config,
                   group_name, tags, description, created_at, updated_at
            FROM hosts
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("查询所有主机配置失败")?;

        let mut configs = Vec::new();
        for row in &rows {
            let config = Self::row_to_host_config(row)?;
            configs.push(config);
        }

        Ok(configs)
    }

    async fn list_by_group(&self, group: &str) -> Result<Vec<HostConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, host, port, username, auth_config,
                   group_name, tags, description, created_at, updated_at
            FROM hosts
            WHERE group_name = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(group)
        .fetch_all(&self.pool)
        .await
        .context("按分组查询主机配置失败")?;

        let mut configs = Vec::new();
        for row in &rows {
            let config = Self::row_to_host_config(row)?;
            configs.push(config);
        }

        Ok(configs)
    }

    async fn search(&self, query: &str) -> Result<Vec<HostConfig>> {
        let search_pattern = format!("%{}%", query);

        let rows = sqlx::query(
            r#"
            SELECT id, name, host, port, username, auth_config,
                   group_name, tags, description, created_at, updated_at
            FROM hosts
            WHERE name LIKE ? OR host LIKE ? OR username LIKE ? OR description LIKE ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await
        .context("搜索主机配置失败")?;

        let mut configs = Vec::new();
        for row in &rows {
            let config = Self::row_to_host_config(row)?;
            configs.push(config);
        }

        Ok(configs)
    }

    async fn get(&self, id: HostId) -> Result<Option<HostConfig>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, host, port, username, auth_config,
                   group_name, tags, description, created_at, updated_at
            FROM hosts
            WHERE id = ?
            "#,
        )
        .bind(id.as_i64())
        .fetch_optional(&self.pool)
        .await
        .context("查询主机配置失败")?;

        if let Some(row) = row {
            let config = Self::row_to_host_config(&row)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, config: &HostConfig) -> Result<HostId> {
        // 验证配置
        config.validate().context("主机配置验证失败")?;

        // 序列化认证配置和标签
        let auth_config_json =
            serde_json::to_string(&config.auth_config).context("序列化认证配置失败")?;

        let tags_json = if config.tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&config.tags).context("序列化标签失败")?)
        };

        let now = Self::current_timestamp();

        let result = sqlx::query(
            r#"
            INSERT INTO hosts (name, host, port, username, auth_config, group_name, tags, description, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&config.name)
        .bind(&config.host)
        .bind(config.port)
        .bind(&config.username)
        .bind(&auth_config_json)
        .bind(&config.group)
        .bind(&tags_json)
        .bind(&config.description)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("创建主机配置失败")?;

        Ok(HostId::new(result.last_insert_rowid()))
    }

    async fn update(&self, config: &HostConfig) -> Result<()> {
        // 验证配置
        config.validate().context("主机配置验证失败")?;

        let id = config
            .id
            .ok_or_else(|| anyhow::anyhow!("主机配置缺少 ID"))?;

        // 序列化认证配置和标签
        let auth_config_json =
            serde_json::to_string(&config.auth_config).context("序列化认证配置失败")?;

        let tags_json = if config.tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&config.tags).context("序列化标签失败")?)
        };

        let now = Self::current_timestamp();

        let result = sqlx::query(
            r#"
            UPDATE hosts
            SET name = ?, host = ?, port = ?, username = ?, auth_config = ?,
                group_name = ?, tags = ?, description = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&config.name)
        .bind(&config.host)
        .bind(config.port)
        .bind(&config.username)
        .bind(&auth_config_json)
        .bind(&config.group)
        .bind(&tags_json)
        .bind(&config.description)
        .bind(now)
        .bind(id.as_i64())
        .execute(&self.pool)
        .await
        .context("更新主机配置失败")?;

        if result.rows_affected() == 0 {
            anyhow::bail!("主机配置不存在: {}", id);
        }

        Ok(())
    }

    async fn delete(&self, id: HostId) -> Result<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM hosts WHERE id = ?
            "#,
        )
        .bind(id.as_i64())
        .execute(&self.pool)
        .await
        .context("删除主机配置失败")?;

        if result.rows_affected() == 0 {
            anyhow::bail!("主机配置不存在: {}", id);
        }

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM hosts")
            .fetch_one(&self.pool)
            .await
            .context("统计主机数量失败")?;

        let count: i64 = row.try_get("count").context("获取 count 失败")?;
        Ok(count)
    }

    async fn list_groups(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT group_name
            FROM hosts
            WHERE group_name IS NOT NULL
            ORDER BY group_name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("查询分组列表失败")?;

        let mut groups = Vec::new();
        for row in &rows {
            if let Ok(Some(group_name)) = row.try_get::<Option<String>, _>("group_name") {
                groups.push(group_name);
            }
        }
        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    use tempfile::TempDir;

    async fn setup_test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(db_path).await.unwrap();
        db.init().await.unwrap();
        (db, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_get_host() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let config = HostConfig::new(
            "测试服务器".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::password("keychain:test".to_string()),
        );

        let id = repo.create(&config).await.unwrap();
        let retrieved = repo.get(id).await.unwrap().unwrap();

        assert_eq!(retrieved.name, "测试服务器");
        assert_eq!(retrieved.host, "192.168.1.100");
        assert_eq!(retrieved.username, "root");
    }

    #[tokio::test]
    async fn test_list_all_hosts() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let config1 = HostConfig::new(
            "服务器1".to_string(),
            "192.168.1.1".to_string(),
            "user1".to_string(),
            AuthConfig::agent(),
        );

        let config2 = HostConfig::new(
            "服务器2".to_string(),
            "192.168.1.2".to_string(),
            "user2".to_string(),
            AuthConfig::agent(),
        );

        repo.create(&config1).await.unwrap();
        repo.create(&config2).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_host() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let mut config = HostConfig::new(
            "原始名称".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::agent(),
        );

        let id = repo.create(&config).await.unwrap();
        config.id = Some(id);
        config.name = "更新后的名称".to_string();

        repo.update(&config).await.unwrap();

        let retrieved = repo.get(id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "更新后的名称");
    }

    #[tokio::test]
    async fn test_delete_host() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let config = HostConfig::new(
            "待删除".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::agent(),
        );

        let id = repo.create(&config).await.unwrap();
        repo.delete(id).await.unwrap();

        let retrieved = repo.get(id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_search_hosts() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let config = HostConfig::new(
            "生产服务器".to_string(),
            "prod.example.com".to_string(),
            "admin".to_string(),
            AuthConfig::agent(),
        );

        repo.create(&config).await.unwrap();

        let results = repo.search("生产").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "生产服务器");
    }

    #[tokio::test]
    async fn test_list_by_group() {
        let (db, _temp_dir) = setup_test_db().await;
        let repo = SqliteHostRepository::new(db.pool().clone());

        let config1 = HostConfig::new(
            "服务器1".to_string(),
            "192.168.1.1".to_string(),
            "user1".to_string(),
            AuthConfig::agent(),
        )
        .with_group("生产环境".to_string());

        let config2 = HostConfig::new(
            "服务器2".to_string(),
            "192.168.1.2".to_string(),
            "user2".to_string(),
            AuthConfig::agent(),
        )
        .with_group("测试环境".to_string());

        repo.create(&config1).await.unwrap();
        repo.create(&config2).await.unwrap();

        let prod_hosts = repo.list_by_group("生产环境").await.unwrap();
        assert_eq!(prod_hosts.len(), 1);
        assert_eq!(prod_hosts[0].name, "服务器1");
    }
}
