//! 实体定义模块
//!
//! 定义 Zeterm 的核心业务实体。

mod host;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// 导出主机配置相关类型
pub use host::{AuthConfig, HostConfig, HostId};

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
    /// 最小行数
    pub const MIN_ROWS: u16 = 1;

    /// 最小列数
    pub const MIN_COLS: u16 = 1;

    /// 最大行数（1000 行，足以满足大多数终端使用场景）
    pub const MAX_ROWS: u16 = 1000;

    /// 最大列数（2000 列，足以满足宽屏终端使用场景）
    pub const MAX_COLS: u16 = 2000;

    /// 创建新的终端尺寸
    ///
    /// 会自动将超出范围的值限制在有效范围内。
    ///
    /// # 示例
    ///
    /// ```
    /// # use zeterm_core::entities::TerminalSize;
    /// let size = TerminalSize::new(24, 80);
    /// assert_eq!(size.rows, 24);
    /// assert_eq!(size.cols, 80);
    /// ```
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows: rows.clamp(Self::MIN_ROWS, Self::MAX_ROWS),
            cols: cols.clamp(Self::MIN_COLS, Self::MAX_COLS),
        }
    }

    /// 创建新的终端尺寸（带验证）
    ///
    /// 如果尺寸超出有效范围，返回错误。
    ///
    /// # 错误
    ///
    /// - `TerminalSizeError::RowsTooSmall` - 行数小于最小值
    /// - `TerminalSizeError::RowsTooLarge` - 行数超过最大值
    /// - `TerminalSizeError::ColsTooSmall` - 列数小于最小值
    /// - `TerminalSizeError::ColsTooLarge` - 列数超过最大值
    ///
    /// # 示例
    ///
    /// ```
    /// # use zeterm_core::entities::TerminalSize;
    /// let size = TerminalSize::try_new(24, 80).unwrap();
    /// assert_eq!(size.rows, 24);
    /// assert_eq!(size.cols, 80);
    /// ```
    pub fn try_new(rows: u16, cols: u16) -> Result<Self, TerminalSizeError> {
        if rows < Self::MIN_ROWS {
            return Err(TerminalSizeError::RowsTooSmall {
                actual: rows,
                min: Self::MIN_ROWS,
            });
        }
        if rows > Self::MAX_ROWS {
            return Err(TerminalSizeError::RowsTooLarge {
                actual: rows,
                max: Self::MAX_ROWS,
            });
        }
        if cols < Self::MIN_COLS {
            return Err(TerminalSizeError::ColsTooSmall {
                actual: cols,
                min: Self::MIN_COLS,
            });
        }
        if cols > Self::MAX_COLS {
            return Err(TerminalSizeError::ColsTooLarge {
                actual: cols,
                max: Self::MAX_COLS,
            });
        }
        Ok(Self { rows, cols })
    }

    /// 检查尺寸是否有效
    ///
    /// 尺寸必须在有效范围内（既不为零，也不超过最大值）。
    pub fn is_valid(&self) -> bool {
        self.rows >= Self::MIN_ROWS
            && self.rows <= Self::MAX_ROWS
            && self.cols >= Self::MIN_COLS
            && self.cols <= Self::MAX_COLS
    }

    /// 计算总单元格数
    ///
    /// 注意：如果值过大，此计算可能会溢出。
    pub fn cell_count(&self) -> usize {
        self.rows as usize * self.cols as usize
    }

    /// 获取行数（带边界检查）
    pub fn rows_clamped(&self) -> u16 {
        self.rows.clamp(Self::MIN_ROWS, Self::MAX_ROWS)
    }

    /// 获取列数（带边界检查）
    pub fn cols_clamped(&self) -> u16 {
        self.cols.clamp(Self::MIN_COLS, Self::MAX_COLS)
    }

    /// 调整尺寸（限制在有效范围内）
    pub fn clamp(&self) -> Self {
        Self {
            rows: self.rows_clamped(),
            cols: self.cols_clamped(),
        }
    }
}

/// 终端尺寸错误
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TerminalSizeError {
    /// 行数过小
    #[error("行数 {actual} 小于最小值 {min}")]
    RowsTooSmall { actual: u16, min: u16 },

    /// 行数过大
    #[error("行数 {actual} 超过最大值 {max}")]
    RowsTooLarge { actual: u16, max: u16 },

    /// 列数过小
    #[error("列数 {actual} 小于最小值 {min}")]
    ColsTooSmall { actual: u16, min: u16 },

    /// 列数过大
    #[error("列数 {actual} 超过最大值 {max}")]
    ColsTooLarge { actual: u16, max: u16 },
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
    fn test_terminal_size_new() {
        let size = TerminalSize::new(24, 80);
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_clamp_too_small() {
        // 测试自动限制最小值
        let size = TerminalSize::new(0, 0);
        assert_eq!(size.rows, TerminalSize::MIN_ROWS);
        assert_eq!(size.cols, TerminalSize::MIN_COLS);
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_clamp_too_large() {
        // 测试自动限制最大值
        let size = TerminalSize::new(2000, 3000);
        assert_eq!(size.rows, TerminalSize::MAX_ROWS);
        assert_eq!(size.cols, TerminalSize::MAX_COLS);
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_clamp_partial() {
        // 测试部分超出的情况
        let size = TerminalSize::new(50, 3000);
        assert_eq!(size.rows, 50); // 行数不变
        assert_eq!(size.cols, TerminalSize::MAX_COLS); // 列数被限制
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_try_new_valid() {
        let size = TerminalSize::try_new(24, 80).unwrap();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_terminal_size_try_new_rows_too_small() {
        let result = TerminalSize::try_new(0, 80);
        assert!(matches!(
            result,
            Err(TerminalSizeError::RowsTooSmall { actual: 0, .. })
        ));
    }

    #[test]
    fn test_terminal_size_try_new_rows_too_large() {
        let result = TerminalSize::try_new(2000, 80);
        assert!(matches!(
            result,
            Err(TerminalSizeError::RowsTooLarge { actual: 2000, .. })
        ));
    }

    #[test]
    fn test_terminal_size_try_new_cols_too_small() {
        let result = TerminalSize::try_new(24, 0);
        assert!(matches!(
            result,
            Err(TerminalSizeError::ColsTooSmall { actual: 0, .. })
        ));
    }

    #[test]
    fn test_terminal_size_try_new_cols_too_large() {
        let result = TerminalSize::try_new(24, 3000);
        assert!(matches!(
            result,
            Err(TerminalSizeError::ColsTooLarge { actual: 3000, .. })
        ));
    }

    #[test]
    fn test_terminal_size_boundary_min() {
        let size = TerminalSize::new(TerminalSize::MIN_ROWS, TerminalSize::MIN_COLS);
        assert_eq!(size.rows, TerminalSize::MIN_ROWS);
        assert_eq!(size.cols, TerminalSize::MIN_COLS);
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_boundary_max() {
        let size = TerminalSize::new(TerminalSize::MAX_ROWS, TerminalSize::MAX_COLS);
        assert_eq!(size.rows, TerminalSize::MAX_ROWS);
        assert_eq!(size.cols, TerminalSize::MAX_COLS);
        assert!(size.is_valid());
    }

    #[test]
    fn test_terminal_size_cell_count() {
        let size = TerminalSize::new(24, 80);
        assert_eq!(size.cell_count(), 1920);
    }

    #[test]
    fn test_terminal_size_clamp_method() {
        let invalid = TerminalSize {
            rows: 0,
            cols: 3000,
        };
        let valid = invalid.clamp();
        assert_eq!(valid.rows, TerminalSize::MIN_ROWS);
        assert_eq!(valid.cols, TerminalSize::MAX_COLS);
        assert!(valid.is_valid());
    }

    #[test]
    fn test_terminal_size_valid() {
        assert!(TerminalSize::new(24, 80).is_valid());

        // 使用直接构造创建无效尺寸来测试 is_valid() 方法
        assert!(!TerminalSize { rows: 0, cols: 80 }.is_valid());
        assert!(!TerminalSize { rows: 24, cols: 0 }.is_valid());
        assert!(
            !TerminalSize {
                rows: 2000,
                cols: 80
            }
            .is_valid()
        );
        assert!(
            !TerminalSize {
                rows: 24,
                cols: 3000
            }
            .is_valid()
        );
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
