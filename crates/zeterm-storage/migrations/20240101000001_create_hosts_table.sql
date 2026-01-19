-- 创建 hosts 表
-- 存储 SSH 主机配置信息

CREATE TABLE IF NOT EXISTS hosts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL,
    auth_config TEXT NOT NULL, -- JSON 格式存储认证配置
    group_name TEXT,
    tags TEXT, -- JSON 数组格式
    description TEXT,
    created_at INTEGER NOT NULL, -- Unix 时间戳（秒）
    updated_at INTEGER NOT NULL  -- Unix 时间戳（秒）
);

-- 创建索引以优化查询性能
CREATE INDEX IF NOT EXISTS idx_hosts_name ON hosts(name);
CREATE INDEX IF NOT EXISTS idx_hosts_group ON hosts(group_name);
CREATE INDEX IF NOT EXISTS idx_hosts_created_at ON hosts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_hosts_host_username ON hosts(host, username);

-- 创建全文搜索支持（可选，用于搜索主机名称和描述）
--注意：SQLite 的FTS5 需要单独的虚拟表
-- CREATE VIRTUAL TABLE IF NOT EXISTS hosts_fts USING fts5(name, description, content=hosts, content_rowid=id);
