//! 剪贴板操作模块
//!
//! 提供终端剪贴板功能，包括：
//! - 复制选中文本到剪贴板
//! - 从剪贴板粘贴文本
//! - 与GPUI 剪贴板 API 集成
//!
//! # 示例
//!
//! ```ignore
//! use gpui::ClipboardItem;
//!
//! // 复制文本
//! ClipboardManager::copy_text(cx, "Hello, World!");
//!
//! // 粘贴文本
//! if let Some(text) = ClipboardManager::paste_text(cx) {
//!     println!("Pasted: {}", text);
//! }
//! ```

use gpui::{App, ClipboardItem};
use tracing::debug;

/// 剪贴板管理器
///
/// 提供剪贴板操作的静态方法
pub struct ClipboardManager;

impl ClipboardManager {
    /// 复制文本到剪贴板
    ///
    /// # Arguments
    ///
    /// * `cx` - GPUI App 上下文引用
    /// * `text` - 要复制的文本
    ///
    /// # Returns
    ///
    /// 复制是否成功
    pub fn copy_text(cx: &mut App, text: &str) -> bool {
        if text.is_empty() {
            debug!("Clipboard: nothing to copy (empty text)");
            return false;
        }

        let item = ClipboardItem::new_string(text.to_string());
        cx.write_to_clipboard(item);
        debug!("Clipboard: copied {} characters", text.len());
        true
    }

    /// 从剪贴板粘贴文本
    ///
    /// # Arguments
    ///
    /// * `cx` - GPUI App 上下文引用
    ///
    /// # Returns
    ///
    /// 剪贴板中的文本，如果没有文本则返回 None
    pub fn paste_text(cx: &mut App) -> Option<String> {
        let item = cx.read_from_clipboard()?;

        // 尝试获取文本内容
        if let Some(text) = item.text() {
            if !text.is_empty() {
                debug!("Clipboard: pasted {} characters", text.len());
                return Some(text.to_string());
            }
        }

        debug!("Clipboard: no text content available");
        None
    }

    /// 检查剪贴板是否有文本内容
    ///
    /// # Arguments
    ///
    /// * `cx` - GPUI App 上下文引用
    ///
    /// # Returns
    ///
    ///剪贴板是否包含文本
    pub fn has_text(cx: &mut App) -> bool {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                return !text.is_empty();
            }
        }
        false
    }

    /// 清空剪贴板
    ///
    /// # Arguments
    ///
    /// * `cx` - GPUI App 上下文引用
    pub fn clear(cx: &mut App) {
        let item = ClipboardItem::new_string(String::new());
        cx.write_to_clipboard(item);
        debug!("Clipboard: cleared");
    }
}

/// 文本提取器
///
/// 从终端内容中提取选中的文本
pub struct TextExtractor;

impl TextExtractor {
    /// 从终端单元格中提取选中的文本
    ///
    /// # Arguments
    ///
    /// * `cells` - 终端单元格数据
    /// * `start_line` - 选择起始行
    /// * `start_col` - 选择起始列
    /// * `end_line` - 选择结束行
    /// * `end_col` - 选择结束列
    /// * `cols` - 终端列数
    ///
    /// # Returns
    ///
    /// 提取的文本
    pub fn extract_selection(
        cells: &[(i32, i32, char)], // (line, col, char)
        start_line: i32,
        start_col: i32,
        end_line: i32,
        end_col: i32,
        cols: i32,
    ) -> String {
        let mut result = String::new();
        let mut current_line = start_line;

        // 按行收集字符
        for line in start_line..=end_line {
            let line_start = if line == start_line { start_col } else { 0 };
            let line_end = if line == end_line { end_col } else { cols };

            // 收集该行的字符
            let mut line_chars: Vec<(i32, char)> = cells
                .iter()
                .filter(|(l, c, _)| *l == line && *c >= line_start && *c < line_end)
                .map(|(_, c, ch)| (*c, *ch))
                .collect();

            // 按列排序
            line_chars.sort_by_key(|(c, _)| *c);

            // 添加换行符（除了第一行）
            if line > current_line {
                result.push('\n');
                current_line = line;
            }

            // 添加字符
            for (_, ch) in line_chars {
                if ch != '\0' {
                    result.push(ch);
                }
            }
        }

        // 去除尾部空白
        result.trim_end().to_string()
    }

    /// 从渲染内容中提取选中的文本（简化版本）
    ///
    ///这个方法直接处理字符串行
    pub fn extract_from_lines(
        lines: &[String],
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        if lines.is_empty() || start_line >= lines.len() {
            return String::new();
        }

        let mut result = String::new();
        let end_line = end_line.min(lines.len() - 1);

        for (i, line) in lines.iter().enumerate().skip(start_line) {
            if i > end_line {
                break;
            }

            let chars: Vec<char> = line.chars().collect();
            let line_start = if i == start_line { start_col } else { 0 };
            let line_end = if i == end_line {
                end_col.min(chars.len())
            } else {
                chars.len()
            };

            // 添加换行符（除了第一行）
            if i > start_line {
                result.push('\n');
            }

            // 添加字符
            if line_start < chars.len() {
                let selected: String = chars[line_start..line_end.min(chars.len())]
                    .iter()
                    .collect();
                result.push_str(&selected);
            }
        }

        result
    }
}

/// 粘贴处理器
///
/// 处理粘贴文本的转换和过滤
pub struct PasteProcessor;

impl PasteProcessor {
    /// 处理粘贴文本
    ///
    /// 将剪贴板文本转换为适合发送到终端的格式///
    /// # Arguments
    ///
    /// * `text` - 原始剪贴板文本
    /// * `bracketed_paste` - 是否启用括号粘贴模式
    ///
    /// # Returns
    ///
    /// 处理后的字节数据
    pub fn process(text: &str, bracketed_paste: bool) -> Vec<u8> {
        let mut result = Vec::new();

        if bracketed_paste {
            // 括号粘贴模式：添加开始和结束标记
            // ESC [ 200 ~ ... ESC [ 201 ~
            result.extend_from_slice(b"\x1b[200~");
            result.extend_from_slice(text.as_bytes());
            result.extend_from_slice(b"\x1b[201~");
        } else {
            // 普通模式：直接发送文本
            //将\r\n 和 \n 统一转换为 \r
            let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
            result.extend_from_slice(normalized.as_bytes());
        }

        result
    }

    /// 过滤控制字符
    ///
    /// 移除可能有害的控制字符
    pub fn filter_control_chars(text: &str) -> String {
        text.chars()
            .filter(|c| {
                // 保留可打印字符、空格、制表符、换行符
                c.is_ascii_graphic()
                    || *c == ' '
                    || *c == '\t'
                    || *c == '\n'
                    || *c == '\r'
                    || !c.is_ascii()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_extractor_from_lines() {
        let lines = vec![
            "Hello, World!".to_string(),
            "This is a test.".to_string(),
            "Third line here.".to_string(),
        ];

        // 选择第一行的部分
        let text = TextExtractor::extract_from_lines(&lines, 0, 0, 0, 5);
        assert_eq!(text, "Hello");

        // 选择跨行
        let text = TextExtractor::extract_from_lines(&lines, 0, 7, 1, 4);
        assert_eq!(text, "World!\nThis");

        // 选择整个内容
        let text = TextExtractor::extract_from_lines(&lines, 0, 0, 2, 16);
        assert_eq!(text, "Hello, World!\nThis is a test.\nThird line here.");
    }

    #[test]
    fn test_paste_processor_normal() {
        let text = "Hello\nWorld";
        let result = PasteProcessor::process(text, false);

        // \n 应该被转换为 \r
        assert_eq!(result, b"Hello\rWorld");
    }

    #[test]
    fn test_paste_processor_bracketed() {
        let text = "test";
        let result = PasteProcessor::process(text, true);

        // 应该有括号粘贴标记
        assert!(result.starts_with(b"\x1b[200~"));
        assert!(result.ends_with(b"\x1b[201~"));
    }

    #[test]
    fn test_paste_processor_filter() {
        let text = "Hello\x00World\x1bTest";
        let filtered = PasteProcessor::filter_control_chars(text);

        // 控制字符应该被移除
        assert_eq!(filtered, "HelloWorldTest");
    }
}
