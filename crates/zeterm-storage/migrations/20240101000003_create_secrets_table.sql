-- Create secrets table
-- Store sensitive information like passwords and passphrases
-- Replaces Windows Credential Manager on Windows to avoid thread isolation issues

CREATE TABLE IF NOT EXISTS secrets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,          -- 密码标识符 (如 "host_root_192.168.5.200_22")
    password TEXT NOT NULL,            -- 密码值 (明文存储，依赖文件系统权限保护)
    created_at INTEGER NOT NULL,       -- Unix timestamp (seconds)
    updated_at INTEGER NOT NULL        -- Unix timestamp (seconds)
);

-- Create unique index on key for fast lookup
CREATE UNIQUE INDEX IF NOT EXISTS idx_secrets_key ON secrets(key);

-- Create index for cleanup operations
CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(updated_at);
