-- Create connection_history table
-- Store SSH connection history records

CREATE TABLE IF NOT EXISTS connection_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id INTEGER NOT NULL,
    connected_at INTEGER NOT NULL, -- Unix timestamp (seconds)
    disconnected_at INTEGER, -- Unix timestamp (seconds), NULL means still connected
    duration INTEGER, -- Connection duration (seconds), NULL means still connected
    status TEXT NOT NULL, -- 'success', 'failed', 'disconnected'
    error_message TEXT, -- Error message (if connection failed)
    session_id TEXT, -- Session ID (optional)

    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
);

-- Create indexes to optimize query performance
CREATE INDEX IF NOT EXISTS idx_connection_history_host_id ON connection_history(host_id);
CREATE INDEX IF NOT EXISTS idx_connection_history_connected_at ON connection_history(connected_at DESC);
CREATE INDEX IF NOT EXISTS idx_connection_history_status ON connection_history(status);
CREATE INDEX IF NOT EXISTS idx_connection_history_session_id ON connection_history(session_id);

-- Create view: recently connected hosts
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
