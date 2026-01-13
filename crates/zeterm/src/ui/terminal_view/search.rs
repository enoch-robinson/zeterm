//! 终端搜索模块
//!
//! 提供终端内容搜索功能，包括：
//! - 文本搜索（支持正则表达式）
//! - 搜索匹配高亮
//! - 上一个/下一个匹配导航

use gpui::Hsla;

//============================================================================
// 搜索方向
// ============================================================================

/// 搜索方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// 向前搜索（从上到下）
    #[default]
    Forward,
    /// 向后搜索（从下到上）
    Backward,
}

// ============================================================================
// 搜索匹配
// ============================================================================

/// 搜索匹配结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// 匹配的起始行
    pub start_line: i32,
    /// 匹配的起始列
    pub start_col: i32,
    /// 匹配的结束行
    pub end_line: i32,
    /// 匹配的结束列
    pub end_col: i32,
}

impl SearchMatch {
    /// 创建新的搜索匹配
    pub fn new(start_line: i32, start_col: i32, end_line: i32, end_col: i32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// 检查匹配是否在指定行
    pub fn is_on_line(&self, line: i32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// 获取指定行的匹配范围
    pub fn get_line_range(&self, line: i32, max_col: i32) -> Option<(i32, i32)> {
        if !self.is_on_line(line) {
            return None;
        }

        let start = if line == self.start_line {
            self.start_col
        } else {
            0
        };

        let end = if line == self.end_line {
            self.end_col
        } else {
            max_col
        };

        Some((start, end))
    }
}

// ============================================================================
// 搜索配置
// ============================================================================

/// 搜索配置
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// 是否区分大小写
    pub case_sensitive: bool,
    /// 是否使用正则表达式
    pub use_regex: bool,
    /// 是否全词匹配
    pub whole_word: bool,
    /// 搜索匹配高亮颜色
    pub match_color: Hsla,
    /// 当前匹配高亮颜色
    pub current_match_color: Hsla,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            //黄色半透明背景
            match_color: Hsla {
                h: 60.0 / 360.0,
                s: 0.8,
                l: 0.5,
                a: 0.3,
            },
            //橙色半透明背景（当前匹配）
            current_match_color: Hsla {
                h: 30.0 / 360.0,
                s: 0.9,
                l: 0.5,
                a: 0.5,
            },
        }
    }
}

// ============================================================================
// 搜索状态
// ============================================================================

/// 搜索状态
#[derive(Debug, Clone)]
pub struct SearchState {
    /// 搜索词
    query: String,
    /// 搜索配置
    config: SearchConfig,
    /// 所有匹配结果
    matches: Vec<SearchMatch>,
    /// 当前匹配索引
    current_index: Option<usize>,
    /// 搜索是否激活
    active: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchState {
    /// 创建新的搜索状态
    pub fn new() -> Self {
        Self {
            query: String::new(),
            config: SearchConfig::default(),
            matches: Vec::new(),
            current_index: None,
            active: false,
        }
    }

    /// 使用配置创建搜索状态
    pub fn with_config(config: SearchConfig) -> Self {
        Self {
            query: String::new(),
            config,
            matches: Vec::new(),
            current_index: None,
            active: false,
        }
    }

    /// 获取搜索词
    pub fn query(&self) -> &str {
        &self.query
    }

    /// 设置搜索词
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    /// 获取搜索配置
    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    /// 获取可变搜索配置
    pub fn config_mut(&mut self) -> &mut SearchConfig {
        &mut self.config
    }

    /// 获取所有匹配结果
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// 获取匹配数量
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// 获取当前匹配索引
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// 获取当前匹配
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.current_index.and_then(|i| self.matches.get(i))
    }

    /// 检查搜索是否激活
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// 激活搜索
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// 停用搜索
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// 清除搜索结果
    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_index = None;
    }

    /// 设置匹配结果
    pub fn set_matches(&mut self, matches: Vec<SearchMatch>) {
        self.matches = matches;
        // 如果有匹配，选中第一个
        self.current_index = if self.matches.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// 跳转到下一个匹配
    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }

        let next = match self.current_index {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        };

        self.current_index = Some(next);
        self.matches.get(next)
    }

    /// 跳转到上一个匹配
    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }

        let prev = match self.current_index {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            },
            None => self.matches.len() - 1,
        };

        self.current_index = Some(prev);
        self.matches.get(prev)
    }

    /// 跳转到指定索引的匹配
    pub fn goto_match(&mut self, index: usize) -> Option<&SearchMatch> {
        if index < self.matches.len() {
            self.current_index = Some(index);
            self.matches.get(index)
        } else {
            None
        }
    }

    /// 检查指定匹配是否为当前匹配
    pub fn is_current_match(&self, index: usize) -> bool {
        self.current_index == Some(index)
    }

    /// 获取搜索状态摘要（如"3/10"）
    pub fn status_text(&self) -> String {
        if self.matches.is_empty() {
            if self.query.is_empty() {
                String::new()
            } else {
                "No matches".to_string()
            }
        } else {
            match self.current_index {
                Some(i) => format!("{}/{}", i + 1, self.matches.len()),
                None => format!("0/{}", self.matches.len()),
            }
        }
    }
}

// ============================================================================
// 搜索执行器
// ============================================================================

/// 在终端行内容中搜索
///
/// #参数
/// - `lines`: 终端行内容，每行是一个字符串
/// - `query`: 搜索词
/// - `config`: 搜索配置
///
/// # 返回
/// 所有匹配结果的列表
pub fn search_in_lines(lines: &[String], query: &str, config: &SearchConfig) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // 准备搜索模式
    let search_query = if config.case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    for (line_idx, line) in lines.iter().enumerate() {
        let search_line = if config.case_sensitive {
            line.clone()
        } else {
            line.to_lowercase()
        };

        // 简单文本搜索
        let mut start = 0;
        while let Some(pos) = search_line[start..].find(&search_query) {
            let abs_pos = start + pos;
            let end_pos = abs_pos + search_query.len() - 1;

            // 全词匹配检查
            if config.whole_word {
                let is_word_start = abs_pos == 0
                    || !line
                        .chars()
                        .nth(abs_pos - 1)
                        .map_or(false, |c| c.is_alphanumeric());
                let is_word_end = end_pos >= line.len() - 1
                    || !line
                        .chars()
                        .nth(end_pos + 1)
                        .map_or(false, |c| c.is_alphanumeric());

                if !is_word_start || !is_word_end {
                    start = abs_pos + 1;
                    continue;
                }
            }

            matches.push(SearchMatch::new(
                line_idx as i32,
                abs_pos as i32,
                line_idx as i32,
                end_pos as i32,
            ));

            start = abs_pos + 1;
        }
    }

    matches
}

/// 单元格数据（用于搜索）
pub struct CellData {
    pub line: i32,
    pub col: i32,
    pub c: char,
}

/// 从终端内容提取行文本
///
/// 将终端单元格数据转换为字符串行列表
pub fn extract_lines_from_cells(cells: &[CellData], cols: usize) -> Vec<String> {
    use std::collections::BTreeMap;

    // 按行收集字符
    let mut line_chars: BTreeMap<i32, Vec<(i32, char)>> = BTreeMap::new();

    for cell in cells {
        // 跳过空字符
        if cell.c == '\0' {
            continue;
        }

        line_chars
            .entry(cell.line)
            .or_insert_with(Vec::new)
            .push((cell.col, cell.c));
    }

    // 转换为字符串
    let mut lines = Vec::new();
    let mut current_line = 0i32;

    for (line_idx, chars) in line_chars {
        // 填充空行
        while current_line < line_idx {
            lines.push(String::new());
            current_line += 1;
        }

        // 构建行字符串
        let mut line_str = vec![' '; cols];
        for (col, c) in chars {
            if (col as usize) < cols {
                line_str[col as usize] = c;
            }
        }

        // 去除尾部空格
        let line: String = line_str.into_iter().collect();
        lines.push(line.trim_end().to_string());
        current_line += 1;
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_basic() {
        let lines = vec![
            "Hello World".to_string(),
            "hello world".to_string(),
            "HELLO WORLD".to_string(),
        ];

        let config = SearchConfig::default();
        let matches = search_in_lines(&lines, "hello", &config);

        // 不区分大小写，应该匹配所有三行
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_case_sensitive() {
        let lines = vec![
            "Hello World".to_string(),
            "hello world".to_string(),
            "HELLO WORLD".to_string(),
        ];

        let mut config = SearchConfig::default();
        config.case_sensitive = true;
        let matches = search_in_lines(&lines, "hello", &config);

        // 区分大小写，只匹配第二行
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_line, 1);
    }

    #[test]
    fn test_search_multiple_matches_per_line() {
        let lines = vec!["hello hello hello".to_string()];

        let config = SearchConfig::default();
        let matches = search_in_lines(&lines, "hello", &config);

        // 一行中有三个匹配
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_state_navigation() {
        let mut state = SearchState::new();
        state.set_matches(vec![
            SearchMatch::new(0, 0, 0, 4),
            SearchMatch::new(1, 0, 1, 4),
            SearchMatch::new(2, 0, 2, 4),
        ]);

        assert_eq!(state.current_index(), Some(0));

        state.next_match();
        assert_eq!(state.current_index(), Some(1));

        state.next_match();
        assert_eq!(state.current_index(), Some(2));

        // 循环到开头
        state.next_match();
        assert_eq!(state.current_index(), Some(0));

        // 向后循环
        state.prev_match();
        assert_eq!(state.current_index(), Some(2));
    }

    #[test]
    fn test_search_status_text() {
        let mut state = SearchState::new();
        assert_eq!(state.status_text(), "");

        state.set_query("test");
        assert_eq!(state.status_text(), "No matches");

        state.set_matches(vec![
            SearchMatch::new(0, 0, 0, 3),
            SearchMatch::new(1, 0, 1, 3),
        ]);
        assert_eq!(state.status_text(), "1/2");

        state.next_match();
        assert_eq!(state.status_text(), "2/2");
    }
}
