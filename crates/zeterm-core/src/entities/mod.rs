//! 实体定义模块
//!
//! 定义 Zeterm 的核心业务实体。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 终端尺寸
///
/// 表示终端的行数和列数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalSize {
    /// 行数
    pub rows: u16,
    /// 列数
    pub cols: u16,
}

impl TerminalSize {
    /// 创建新的终端尺寸
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }

    /// 检查尺寸是否有效
    pub fn is_valid(&self) -> bool {
        self.rows > 0 && self.cols > 0
    }

    /// 计算总单元格数
    pub fn cell_count(&self) -> usize {
        self.rows as usize * self.cols as usize
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl std::fmt::Display for TerminalSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

/// 会话 ID
///
/// 唯一标识一个终端会话。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    /// 创建新的会话 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从字符串解析会话 ID
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// 获取内部 UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_terminal_size_valid() {
        assert!(TerminalSize::new(24, 80).is_valid());
        assert!(!TerminalSize::new(0, 80).is_valid());
        assert!(!TerminalSize::new(24, 0).is_valid());
    }

    #[test]
    fn test_terminal_size_display() {
        let size = TerminalSize::new(24, 80);
        assert_eq!(size.to_string(), "80x24");
    }

    #[test]
    fn test_session_id_unique() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_parse() {
        let id = SessionId::new();
        let parsed = SessionId::parse(&id.to_string()).unwrap();
        assert_eq!(id, parsed);
    }
}
