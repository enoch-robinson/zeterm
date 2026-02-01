-- Create hosts table
-- Store SSH host configuration information

CREATE TABLE IF NOT EXISTS hosts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL,
    auth_config TEXT NOT NULL, -- JSON format for authentication config
    group_name TEXT,
    tags TEXT, -- JSON array format
    description TEXT,
    created_at INTEGER NOT NULL, -- Unix timestamp (seconds)
    updated_at INTEGER NOT NULL  -- Unix timestamp (seconds)
);

-- Create indexes to optimize query performance
CREATE INDEX IF NOT EXISTS idx_hosts_name ON hosts(name);
CREATE INDEX IF NOT EXISTS idx_hosts_group ON hosts(group_name);
CREATE INDEX IF NOT EXISTS idx_hosts_created_at ON hosts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_hosts_host_username ON hosts(host, username);

-- Full-text search support (optional, for searching host names and descriptions)
-- Note: SQLite FTS5 requires a separate virtual table
-- CREATE VIRTUAL TABLE IF NOT EXISTS hosts_fts USING fts5(name, description, content=hosts, content_rowid=id);
