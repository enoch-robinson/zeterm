//! Known Hosts 管理模块
//!
//! 提供 SSH 主机密钥验证功能，包括：
//! - known_hosts 文件解析和写入
//! - 主机密钥查找和验证
//! - 首次连接处理
//! - 密钥变更检测

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use tracing::{debug, info};

/// 默认 known_hosts 文件名
pub const KNOWN_HOSTS_FILENAME: &str = "known_hosts";

/// 主机密钥类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyType {
    /// RSA 密钥
    Rsa,
    /// Ed25519 密钥
    Ed25519,
    /// ECDSA-SHA2-NISTP256
    EcdsaSha2Nistp256,
    /// ECDSA-SHA2-NISTP384
    EcdsaSha2Nistp384,
    /// ECDSA-SHA2-NISTP521
    EcdsaSha2Nistp521,
    /// 未知类型
    Unknown(String),
}

impl KeyType {
    /// 从字符串解析密钥类型
    pub fn from_str(s: &str) -> Self {
        match s {
            "ssh-rsa" => Self::Rsa,
            "ssh-ed25519" => Self::Ed25519,
            "ecdsa-sha2-nistp256" => Self::EcdsaSha2Nistp256,
            "ecdsa-sha2-nistp384" => Self::EcdsaSha2Nistp384,
            "ecdsa-sha2-nistp521" => Self::EcdsaSha2Nistp521,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &str {
        match self {
            Self::Rsa => "ssh-rsa",
            Self::Ed25519 => "ssh-ed25519",
            Self::EcdsaSha2Nistp256 => "ecdsa-sha2-nistp256",
            Self::EcdsaSha2Nistp384 => "ecdsa-sha2-nistp384",
            Self::EcdsaSha2Nistp521 => "ecdsa-sha2-nistp521",
            Self::Unknown(s) => s,
        }
    }
}

impl std::fmt::Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 主机密钥条目
#[derive(Debug, Clone)]
pub struct HostKeyEntry {
    /// 主机名列表（可能包含多个，用逗号分隔）
    pub hostnames: Vec<String>,
    /// 密钥类型
    pub key_type: KeyType,
    /// Base64 编码的公钥
    pub key_data: String,
    /// 注释（可选）
    pub comment: Option<String>,
    /// 是否为哈希主机名
    pub is_hashed: bool,
}

impl HostKeyEntry {
    /// 创建新的主机密钥条目
    pub fn new(
        hostname: impl Into<String>,
        key_type: KeyType,
        key_data: impl Into<String>,
    ) -> Self {
        Self {
            hostnames: vec![hostname.into()],
            key_type,
            key_data: key_data.into(),
            comment: None,
            is_hashed: false,
        }
    }

    /// 添加主机名
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostnames.push(hostname.into());
        self
    }

    /// 设置注释
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// 检查是否匹配指定主机
    ///
    /// 匹配规则：
    /// - 纯主机名条目（如 "example.com"）只匹配标准端口 22
    /// - 带端口条目（如 "[example.com]:2222"）只匹配指定端口
    pub fn matches_host(&self, hostname: &str, port: u16) -> bool {
        let host_with_port = format!("[{}]:{}", hostname, port);

        for h in &self.hostnames {
            // 检查是否匹配带端口格式
            if h == &host_with_port {
                return true;
            }

            // 只有标准端口 22 才匹配纯主机名格式的条目
            if port == 22 && h == hostname {
                return true;
            }

            // 支持通配符匹配
            if h.contains('*') || h.contains('?') {
                // 带端口格式的通配符匹配
                if Self::wildcard_match(h, &host_with_port) {
                    return true;
                }
                // 只有标准端口 22 才匹配纯主机名格式的通配符
                if port == 22 && Self::wildcard_match(h, hostname) {
                    return true;
                }
            }
        }
        false
    }

    /// 简单的通配符匹配
    fn wildcard_match(pattern: &str, text: &str) -> bool {
        let pattern_chars: Vec<char> = pattern.chars().collect();
        let text_chars: Vec<char> = text.chars().collect();
        Self::wildcard_match_helper(&pattern_chars, &text_chars, 0, 0)
    }

    fn wildcard_match_helper(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
        if pi == pattern.len() && ti == text.len() {
            return true;
        }
        if pi == pattern.len() {
            return false;
        }

        match pattern[pi] {
            '*' => {
                // * 匹配零个或多个字符
                for i in ti..=text.len() {
                    if Self::wildcard_match_helper(pattern, text, pi + 1, i) {
                        return true;
                    }
                }
                false
            },
            '?' => {
                // ? 匹配单个字符
                if ti < text.len() {
                    Self::wildcard_match_helper(pattern, text, pi + 1, ti + 1)
                } else {
                    false
                }
            },
            c => {
                if ti < text.len() && text[ti] == c {
                    Self::wildcard_match_helper(pattern, text, pi + 1, ti + 1)
                } else {
                    false
                }
            },
        }
    }

    /// 格式化为 known_hosts 行
    pub fn to_line(&self) -> String {
        let hostnames = self.hostnames.join(",");
        let mut line = format!("{} {} {}", hostnames, self.key_type, self.key_data);
        if let Some(ref comment) = self.comment {
            line.push(' ');
            line.push_str(comment);
        }
        line
    }

    /// 从 known_hosts 行解析
    pub fn from_line(line: &str) -> Option<Self> {
        let line = line.trim();

        // 跳过空行和注释
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 3 {
            return None;
        }

        let hostnames: Vec<String> = parts[0].split(',').map(|s| s.to_string()).collect();
        let key_type = KeyType::from_str(parts[1]);
        let key_data = parts[2].to_string();
        let comment = parts.get(3).map(|s| s.to_string());

        let is_hashed = hostnames.iter().any(|h| h.starts_with("|1|"));

        Some(Self {
            hostnames,
            key_type,
            key_data,
            comment,
            is_hashed,
        })
    }
}

/// 主机密钥验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// 密钥匹配
    Match,
    /// 首次连接（未知主机）
    Unknown,
    /// 密钥已变更（可能的中间人攻击）
    Changed {
        /// 期望的密钥类型
        expected_type: KeyType,
        /// 期望的密钥数据
        expected_key: String,
    },
    /// 主机被撤销
    Revoked,
}

impl VerificationResult {
    /// 是否安全
    pub fn is_safe(&self) -> bool {
        matches!(self, Self::Match)
    }

    /// 是否需要用户确认
    pub fn needs_confirmation(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// 是否为安全警告
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::Changed { .. } | Self::Revoked)
    }

    /// 获取用户消息
    pub fn message(&self) -> &'static str {
        match self {
            Self::Match => "Host key verified",
            Self::Unknown => "Unknown host, key not in known_hosts",
            Self::Changed { .. } => "WARNING: Host key has changed!",
            Self::Revoked => "Host key has been revoked",
        }
    }

    /// 获取用户消息（中文）
    pub fn message_cn(&self) -> &'static str {
        match self {
            Self::Match => "主机密钥验证通过",
            Self::Unknown => "未知主机，密钥不在 known_hosts 中",
            Self::Changed { .. } => "警告：主机密钥已变更！",
            Self::Revoked => "主机密钥已被撤销",
        }
    }
}

/// Known Hosts 存储
#[derive(Debug)]
pub struct KnownHostsStore {
    /// 文件路径
    path: PathBuf,
    /// 主机密钥条目
    entries: Vec<HostKeyEntry>,
    /// 是否已修改
    modified: bool,
}

impl KnownHostsStore {
    /// 获取默认 known_hosts 路径
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ssh")
            .join(KNOWN_HOSTS_FILENAME)
    }

    /// 创建新的存储（使用默认路径）
    pub fn new() -> Self {
        Self::with_path(Self::default_path())
    }

    /// 使用指定路径创建
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            entries: Vec::new(),
            modified: false,
        }
    }

    /// 获取文件路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 加载 known_hosts 文件
    pub fn load(&mut self) -> Result<(), KnownHostsError> {
        if !self.path.exists() {
            debug!("known_hosts file does not exist: {:?}", self.path);
            return Ok(());
        }

        let file = File::open(&self.path).map_err(|e| KnownHostsError::IoError(e.to_string()))?;

        let reader = BufReader::new(file);
        self.entries.clear();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| KnownHostsError::IoError(e.to_string()))?;

            if let Some(entry) = HostKeyEntry::from_line(&line) {
                self.entries.push(entry);
            } else if !line.trim().is_empty() && !line.trim().starts_with('#') {
                debug!("Skipping invalid line {} in known_hosts", line_num + 1);
            }
        }

        info!("Loaded {} entries from known_hosts", self.entries.len());
        self.modified = false;
        Ok(())
    }

    /// 保存到文件
    pub fn save(&mut self) -> Result<(), KnownHostsError> {
        // 确保目录存在
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| KnownHostsError::IoError(e.to_string()))?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| KnownHostsError::IoError(e.to_string()))?;

        for entry in &self.entries {
            writeln!(file, "{}", entry.to_line())
                .map_err(|e| KnownHostsError::IoError(e.to_string()))?;
        }

        self.modified = false;
        info!("Saved {} entries to known_hosts", self.entries.len());
        Ok(())
    }

    /// 查找主机密钥
    pub fn find(&self, hostname: &str, port: u16) -> Option<&HostKeyEntry> {
        self.entries.iter().find(|e| e.matches_host(hostname, port))
    }

    /// 验证主机密钥
    pub fn verify(
        &self,
        hostname: &str,
        port: u16,
        key_type: &KeyType,
        key_data: &str,
    ) -> VerificationResult {
        match self.find(hostname, port) {
            Some(entry) => {
                if &entry.key_type == key_type && entry.key_data == key_data {
                    VerificationResult::Match
                } else {
                    VerificationResult::Changed {
                        expected_type: entry.key_type.clone(),
                        expected_key: entry.key_data.clone(),
                    }
                }
            },
            None => VerificationResult::Unknown,
        }
    }

    /// 添加主机密钥
    pub fn add(&mut self, entry: HostKeyEntry) {
        self.entries.push(entry);
        self.modified = true;
    }

    /// 添加或更新主机密钥
    pub fn add_or_update(
        &mut self,
        hostname: &str,
        port: u16,
        key_type: KeyType,
        key_data: String,
    ) {
        // 移除旧条目
        self.entries.retain(|e| !e.matches_host(hostname, port));

        // 添加新条目
        let host = if port != 22 {
            format!("[{}]:{}", hostname, port)
        } else {
            hostname.to_string()
        };

        self.entries
            .push(HostKeyEntry::new(host, key_type, key_data));
        self.modified = true;
    }

    /// 移除主机密钥
    pub fn remove(&mut self, hostname: &str, port: u16) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| !e.matches_host(hostname, port));
        let removed = self.entries.len() < len_before;
        if removed {
            self.modified = true;
        }
        removed
    }

    /// 获取所有条目
    pub fn entries(&self) -> &[HostKeyEntry] {
        &self.entries
    }

    /// 是否已修改
    pub fn is_modified(&self) -> bool {
        self.modified
    }
}

impl Default for KnownHostsStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Known Hosts 错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum KnownHostsError {
    /// IO 错误
    #[error("IO error: {0}")]
    IoError(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    ParseError(String),

    /// 验证失败
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_type_from_str() {
        assert_eq!(KeyType::from_str("ssh-rsa"), KeyType::Rsa);
        assert_eq!(KeyType::from_str("ssh-ed25519"), KeyType::Ed25519);
        assert!(matches!(KeyType::from_str("unknown"), KeyType::Unknown(_)));
    }

    #[test]
    fn test_key_type_as_str() {
        assert_eq!(KeyType::Rsa.as_str(), "ssh-rsa");
        assert_eq!(KeyType::Ed25519.as_str(), "ssh-ed25519");
    }

    #[test]
    fn test_host_key_entry_new() {
        let entry = HostKeyEntry::new("example.com", KeyType::Rsa, "AAAAB3...");
        assert_eq!(entry.hostnames, vec!["example.com"]);
        assert_eq!(entry.key_type, KeyType::Rsa);
    }

    #[test]
    fn test_host_key_entry_matches_host() {
        let entry = HostKeyEntry::new("example.com", KeyType::Rsa, "key");

        assert!(entry.matches_host("example.com", 22));
        assert!(!entry.matches_host("other.com", 22));
    }

    #[test]
    fn test_host_key_entry_matches_port() {
        let entry = HostKeyEntry::new("[example.com]:2222", KeyType::Rsa, "key");

        assert!(entry.matches_host("example.com", 2222));
        assert!(!entry.matches_host("example.com", 22));
    }

    #[test]
    fn test_host_key_entry_to_line() {
        let entry =
            HostKeyEntry::new("example.com", KeyType::Ed25519, "AAAAC3...").with_comment("my key");

        let line = entry.to_line();
        assert!(line.contains("example.com"));
        assert!(line.contains("ssh-ed25519"));
        assert!(line.contains("my key"));
    }

    #[test]
    fn test_host_key_entry_from_line() {
        let line = "example.com ssh-rsa AAAAB3... comment";
        let entry = HostKeyEntry::from_line(line).unwrap();

        assert_eq!(entry.hostnames, vec!["example.com"]);
        assert_eq!(entry.key_type, KeyType::Rsa);
        assert_eq!(entry.key_data, "AAAAB3...");
        assert_eq!(entry.comment, Some("comment".to_string()));
    }

    #[test]
    fn test_host_key_entry_from_line_multiple_hosts() {
        let line = "host1,host2,host3 ssh-ed25519 AAAAC3...";
        let entry = HostKeyEntry::from_line(line).unwrap();

        assert_eq!(entry.hostnames.len(), 3);
        assert!(entry.hostnames.contains(&"host1".to_string()));
    }

    #[test]
    fn test_host_key_entry_from_line_skip_comments() {
        assert!(HostKeyEntry::from_line("# comment").is_none());
        assert!(HostKeyEntry::from_line("").is_none());
    }

    #[test]
    fn test_verification_result() {
        assert!(VerificationResult::Match.is_safe());
        assert!(!VerificationResult::Unknown.is_safe());

        assert!(VerificationResult::Unknown.needs_confirmation());
        assert!(!VerificationResult::Match.needs_confirmation());

        let changed = VerificationResult::Changed {
            expected_type: KeyType::Rsa,
            expected_key: "old".to_string(),
        };
        assert!(changed.is_warning());
    }

    #[test]
    fn test_known_hosts_store_default_path() {
        let path = KnownHostsStore::default_path();
        assert!(path.to_string_lossy().contains("known_hosts"));
    }

    #[test]
    fn test_known_hosts_store_verify_unknown() {
        let store = KnownHostsStore::new();
        let result = store.verify("unknown.com", 22, &KeyType::Rsa, "key");
        assert_eq!(result, VerificationResult::Unknown);
    }
}
