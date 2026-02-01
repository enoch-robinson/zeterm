//! 文本选择模块
//!
//! 提供终端文本选择功能，支持：
//! - 字符级选择（拖动）
//! - 单词选择（双击）
//! - 行级选择（三击）
//!
//! # 示例
//!
//! ```ignore
//! let mut selection = Selection::new();
//!
//! // 开始选择
//! selection.start(SelectionPoint::new(0, 5));
//!
//! // 更新选择范围
//! selection.update(SelectionPoint::new(2, 10));
//!
//! // 获取选择范围
//! if let Some(range) = selection.range() {
//!     println!("Selection: {:?} to {:?}", range.start, range.end);
//! }
//! ```

use std::cmp::{max, min};

/// 选择点
///
/// 表示终端中的一个位置（行和列）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    /// 行号（0-based，可以为负数表示滚动缓冲区）
    pub line: i32,
    /// 列号（0-based）
    pub col: i32,
}

impl SelectionPoint {
    /// 创建新的选择点
    pub fn new(line: i32, col: i32) -> Self {
        Self { line, col }
    }

    /// 创建原点（0, 0）
    pub fn origin() -> Self {
        Self::new(0, 0)
    }

    /// 比较两个点的顺序
    ///
    /// 返回 true 如果 self 在 other 之前（或相同位置）
    pub fn is_before_or_equal(&self, other: &Self) -> bool {
        if self.line < other.line {
            true
        } else if self.line > other.line {
            false
        } else {
            self.col <= other.col
        }
    }

    /// 比较两个点的顺序
    ///
    /// 返回 true 如果 self 严格在 other 之前
    pub fn is_before(&self, other: &Self) -> bool {
        if self.line < other.line {
            true
        } else if self.line > other.line {
            false
        } else {
            self.col < other.col
        }
    }
}

impl Default for SelectionPoint {
    fn default() -> Self {
        Self::origin()
    }
}

impl PartialOrd for SelectionPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SelectionPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.line.cmp(&other.line) {
            std::cmp::Ordering::Equal => self.col.cmp(&other.col),
            ord => ord,
        }
    }
}

/// 选择范围
///
/// 表示一个有序的选择范围，start 总是在 end 之前或相同位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    /// 选择起点（较小的点）
    pub start: SelectionPoint,
    /// 选择终点（较大的点）
    pub end: SelectionPoint,
}

impl SelectionRange {
    /// 创建新的选择范围
    ///
    /// 自动排序确保 start <= end
    pub fn new(p1: SelectionPoint, p2: SelectionPoint) -> Self {
        if p1.is_before_or_equal(&p2) {
            Self { start: p1, end: p2 }
        } else {
            Self { start: p2, end: p1 }
        }
    }

    /// 检查范围是否为空（起点和终点相同）
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// 检查指定点是否在范围内
    pub fn contains(&self, point: &SelectionPoint) -> bool {
        self.start.is_before_or_equal(point) && point.is_before_or_equal(&self.end)
    }

    /// 检查指定行是否与范围相交
    pub fn intersects_line(&self, line: i32) -> bool {
        line >= self.start.line && line <= self.end.line
    }

    /// 获取指定行的选择列范围
    ///
    /// 返回 (start_col, end_col)，如果该行不在选择范围内则返回 None
    pub fn columns_for_line(&self, line: i32, max_col: i32) -> Option<(i32, i32)> {
        if !self.intersects_line(line) {
            return None;
        }

        let start_col = if line == self.start.line {
            self.start.col
        } else {
            0
        };

        let end_col = if line == self.end.line {
            self.end.col
        } else {
            max_col
        };

        Some((start_col, end_col))
    }

    /// 获取选择的行数
    pub fn line_count(&self) -> usize {
        (self.end.line - self.start.line + 1) as usize
    }
}

/// 选择类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    /// 字符级选择（默认拖动选择）
    Character,
    /// 单词选择（双击）
    Word,
    /// 行级选择（三击）
    Line,
    /// 块选择（Alt+拖动，可选功能）
    Block,
}

impl Default for SelectionType {
    fn default() -> Self {
        Self::Character
    }
}

/// 选择状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// 无选择
    None,
    /// 正在选择（鼠标按下，拖动中）
    Selecting,
    /// 选择完成（鼠标释放）
    Selected,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::None
    }
}

/// 文本选择
///
/// 管理终端中的文本选择状态
#[derive(Debug, Clone)]
pub struct Selection {
    /// 选择锚点（鼠标按下的位置）
    anchor: Option<SelectionPoint>,
    /// 选择终点（当前鼠标位置）
    endpoint: Option<SelectionPoint>,
    /// 选择类型
    selection_type: SelectionType,
    /// 选择状态
    state: SelectionState,
    /// 终端列数（用于行选择）
    cols: i32,
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

impl Selection {
    /// 创建新的选择实例
    pub fn new() -> Self {
        Self {
            anchor: None,
            endpoint: None,
            selection_type: SelectionType::Character,
            state: SelectionState::None,
            cols: 80,
        }
    }

    /// 设置终端列数
    pub fn set_cols(&mut self, cols: i32) {
        self.cols = cols;
    }

    /// 获取终端列数
    pub fn cols(&self) -> i32 {
        self.cols
    }

    /// 开始字符级选择
    pub fn start(&mut self, point: SelectionPoint) {
        self.anchor = Some(point);
        self.endpoint = Some(point);
        self.selection_type = SelectionType::Character;
        self.state = SelectionState::Selecting;
    }

    /// 开始单词选择（双击）
    pub fn start_word(&mut self, point: SelectionPoint, word_bounds: Option<(i32, i32)>) {
        self.selection_type = SelectionType::Word;
        self.state = SelectionState::Selecting;

        if let Some((start_col, end_col)) = word_bounds {
            self.anchor = Some(SelectionPoint::new(point.line, start_col));
            self.endpoint = Some(SelectionPoint::new(point.line, end_col));
        } else {
            self.anchor = Some(point);
            self.endpoint = Some(point);
        }
    }

    /// 开始行级选择（三击）
    pub fn start_line(&mut self, line: i32) {
        self.selection_type = SelectionType::Line;
        self.state = SelectionState::Selecting;
        self.anchor = Some(SelectionPoint::new(line, 0));
        self.endpoint = Some(SelectionPoint::new(line, self.cols));
    }

    /// 更新选择范围
    pub fn update(&mut self, point: SelectionPoint) {
        if self.state != SelectionState::Selecting {
            return;
        }

        match self.selection_type {
            SelectionType::Character => {
                self.endpoint = Some(point);
            },
            SelectionType::Word => {
                // 单词选择模式下，扩展到包含新位置的单词
                self.endpoint = Some(point);
            },
            SelectionType::Line => {
                // 行选择模式下，扩展到整行
                if let Some(anchor) = self.anchor {
                    let start_line = min(anchor.line, point.line);
                    let end_line = max(anchor.line, point.line);
                    self.anchor = Some(SelectionPoint::new(start_line, 0));
                    self.endpoint = Some(SelectionPoint::new(end_line, self.cols));
                }
            },
            SelectionType::Block => {
                self.endpoint = Some(point);
            },
        }
    }

    /// 完成选择
    pub fn finish(&mut self) {
        if self.state == SelectionState::Selecting {
            self.state = SelectionState::Selected;
        }
    }

    /// 清除选择
    pub fn clear(&mut self) {
        self.anchor = None;
        self.endpoint = None;
        self.state = SelectionState::None;
        self.selection_type = SelectionType::Character;
    }

    /// 检查是否有活动选择
    pub fn is_active(&self) -> bool {
        self.state != SelectionState::None && self.anchor.is_some() && self.endpoint.is_some()
    }

    /// 检查是否正在选择
    pub fn is_selecting(&self) -> bool {
        self.state == SelectionState::Selecting
    }

    /// 检查是否有已完成的选择
    pub fn has_selection(&self) -> bool {
        self.state == SelectionState::Selected && self.range().map_or(false, |r| !r.is_empty())
    }

    /// 获取选择范围
    pub fn range(&self) -> Option<SelectionRange> {
        match (self.anchor, self.endpoint) {
            (Some(anchor), Some(endpoint)) => Some(SelectionRange::new(anchor, endpoint)),
            _ => None,
        }
    }

    /// 获取选择类型
    pub fn selection_type(&self) -> SelectionType {
        self.selection_type
    }

    /// 获取选择状态
    pub fn state(&self) -> SelectionState {
        self.state
    }

    /// 获取锚点
    pub fn anchor(&self) -> Option<SelectionPoint> {
        self.anchor
    }

    /// 获取终点
    pub fn endpoint(&self) -> Option<SelectionPoint> {
        self.endpoint
    }

    /// 检查指定点是否在选择范围内
    pub fn contains(&self, point: &SelectionPoint) -> bool {
        self.range().map_or(false, |r| r.contains(point))
    }

    /// 检查指定行是否与选择范围相交
    pub fn intersects_line(&self, line: i32) -> bool {
        self.range().map_or(false, |r| r.intersects_line(line))
    }

    /// 获取指定行的选择列范围
    pub fn columns_for_line(&self, line: i32, max_col: i32) -> Option<(i32, i32)> {
        self.range().and_then(|r| r.columns_for_line(line, max_col))
    }
}

/// 单词边界检测器
///
/// 用于检测单词的边界，支持双击选择单词
pub struct WordBoundaryDetector;

impl WordBoundaryDetector {
    /// 检测单词边界
    ///
    /// 给定一行文本和点击位置，返回单词的起始和结束列
    pub fn detect(line_text: &str, col: usize) -> Option<(i32, i32)> {
        if line_text.is_empty() || col >= line_text.len() {
            return None;
        }

        let chars: Vec<char> = line_text.chars().collect();
        if col >= chars.len() {
            return None;
        }

        let clicked_char = chars[col];

        // 如果点击的是空白字符，不选择
        if clicked_char.is_whitespace() {
            return None;
        }

        // 确定字符类型
        let char_type = Self::char_type(clicked_char);

        // 向左查找单词起始
        let mut start = col;
        while start > 0 && Self::char_type(chars[start - 1]) == char_type {
            start -= 1;
        }

        // 向右查找单词结束
        let mut end = col;
        while end < chars.len() - 1 && Self::char_type(chars[end + 1]) == char_type {
            end += 1;
        }

        Some((start as i32, (end + 1) as i32))
    }

    /// 获取字符类型
    fn char_type(c: char) -> CharType {
        if c.is_alphanumeric() || c == '_' {
            CharType::Word
        } else if c.is_whitespace() {
            CharType::Whitespace
        } else {
            CharType::Symbol
        }
    }
}

/// 字符类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharType {
    /// 单词字符（字母、数字、下划线）
    Word,
    /// 空白字符
    Whitespace,
    /// 符号字符
    Symbol,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_point_ordering() {
        let p1 = SelectionPoint::new(0, 5);
        let p2 = SelectionPoint::new(0, 10);
        let p3 = SelectionPoint::new(1, 0);

        assert!(p1.is_before(&p2));
        assert!(p2.is_before(&p3));
        assert!(p1.is_before(&p3));
        assert!(!p2.is_before(&p1));
    }

    #[test]
    fn test_selection_range_new() {
        let p1 = SelectionPoint::new(0, 10);
        let p2 = SelectionPoint::new(0, 5);

        // 应该自动排序
        let range = SelectionRange::new(p1, p2);
        assert_eq!(range.start, p2);
        assert_eq!(range.end, p1);
    }

    #[test]
    fn test_selection_range_contains() {
        let range = SelectionRange::new(SelectionPoint::new(0, 5), SelectionPoint::new(2, 10));

        assert!(range.contains(&SelectionPoint::new(0, 5)));
        assert!(range.contains(&SelectionPoint::new(1, 0)));
        assert!(range.contains(&SelectionPoint::new(2, 10)));
        assert!(!range.contains(&SelectionPoint::new(0, 4)));
        assert!(!range.contains(&SelectionPoint::new(2, 11)));
    }

    #[test]
    fn test_selection_range_columns_for_line() {
        let range = SelectionRange::new(SelectionPoint::new(0, 5), SelectionPoint::new(2, 10));

        // 第一行：从col 5 到行尾
        assert_eq!(range.columns_for_line(0, 80), Some((5, 80)));

        // 中间行：整行
        assert_eq!(range.columns_for_line(1, 80), Some((0, 80)));

        // 最后一行：从行首到 col 10
        assert_eq!(range.columns_for_line(2, 80), Some((0, 10)));

        // 不在范围内的行
        assert_eq!(range.columns_for_line(3, 80), None);
    }

    #[test]
    fn test_selection_start_and_update() {
        let mut selection = Selection::new();

        selection.start(SelectionPoint::new(0, 5));
        assert!(selection.is_selecting());

        selection.update(SelectionPoint::new(2, 10));

        let range = selection.range().unwrap();
        assert_eq!(range.start, SelectionPoint::new(0, 5));
        assert_eq!(range.end, SelectionPoint::new(2, 10));
    }

    #[test]
    fn test_selection_finish_and_clear() {
        let mut selection = Selection::new();

        selection.start(SelectionPoint::new(0, 5));
        selection.update(SelectionPoint::new(2, 10));
        selection.finish();

        assert!(selection.has_selection());
        assert!(!selection.is_selecting());

        selection.clear();
        assert!(!selection.has_selection());
        assert!(!selection.is_active());
    }

    #[test]
    fn test_selection_line_mode() {
        let mut selection = Selection::new();
        selection.set_cols(80);

        selection.start_line(5);

        let range = selection.range().unwrap();
        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.col, 0);
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.col, 80);
    }

    #[test]
    fn test_word_boundary_detector() {
        let line = "hello world test_var123";

        // 点击 "hello" 中的 'e'
        let bounds = WordBoundaryDetector::detect(line, 1);
        assert_eq!(bounds, Some((0, 5)));

        // 点击 "world" 中的 'o'
        let bounds = WordBoundaryDetector::detect(line, 7);
        assert_eq!(bounds, Some((6, 11)));

        // 点击 "test_var123" 中的 '_'
        // 整个 "test_var123" 是一个单词（字母、数字、下划线都是单词字符）
        let bounds = WordBoundaryDetector::detect(line, 16);
        assert_eq!(bounds, Some((12, 23)));

        // 点击空格
        let bounds = WordBoundaryDetector::detect(line, 5);
        assert_eq!(bounds, None);
    }
}
