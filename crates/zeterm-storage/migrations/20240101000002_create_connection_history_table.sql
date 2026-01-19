-- 创建 connection_history 表
-- 存储 SSH 连接历史记录

CREATE TABLE IF NOT EXISTS connection_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id INTEGER NOT NULL,
    connected_at INTEGER NOT NULL, -- Unix 时间戳（秒）
    disconnected_at INTEGER, -- Unix 时间戳（秒），NULL 表示仍在连接
    duration INTEGER, -- 连接时长（秒），NULL 表示仍在连接
    status TEXT NOT NULL, -- 'success', 'failed', 'disconnected'
    error_message TEXT, -- 错误信息（如果连接失败）
    session_id TEXT, -- 会话 ID（可选）

    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
);

-- 创建索引以优化查询性能
CREATE INDEX IF NOT EXISTS idx_connection_history_host_id ON connection_history(host_id);
CREATE INDEX IF NOT EXISTS idx_connection_history_connected_at ON connection_history(connected_at DESC);
CREATE INDEX IF NOT EXISTS idx_connection_history_status ON connection_history(status);
CREATE INDEX IF NOT EXISTS idx_connection_history_session_id ON connection_history(session_id);

-- 创建视图：最近连接的主机
CREATE VIEW IF NOT EXISTS recent_connections AS
SELECT
    h.id,
    h.name,
    h.host,
    h.username,
    ch.connected_at,
    ch.status
FROM hosts h
INNER JOIN connection_history ch ON h.id = ch.host_id
ORDER BY ch.connected_at DESC
LIMIT 10;
