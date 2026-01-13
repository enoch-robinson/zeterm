//! IME（输入法）模块
//!
//! 提供输入法支持功能，包括：
//! - 预编辑文本显示
//! - 光标位置计算
//! - 组合字符处理

use gpui::{Hsla, Pixels, Point};

//============================================================================
// IME 状态
// ============================================================================

/// IME 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImeState {
    /// 未激活
    #[default]
    Inactive,
    /// 正在输入（预编辑中）
    Composing,
    /// 已提交
    Committed,
}

// ============================================================================
// 预编辑文本
// ============================================================================

/// 预编辑文本
///
/// 表示输入法正在组合的文本，尚未提交到终端
#[derive(Debug, Clone, Default)]
pub struct PreeditText {
    /// 预编辑文本内容
    pub text: String,
    /// 光标在预编辑文本中的位置（字符索引）
    pub cursor_offset: usize,
    /// 选中范围（起始，结束）
    pub selection: Option<(usize, usize)>,
}

impl PreeditText {
    /// 创建新的预编辑文本
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor_offset = text.chars().count();
        Self {
            text,
            cursor_offset,
            selection: None,
        }
    }

    /// 创建带光标位置的预编辑文本
    pub fn with_cursor(text: impl Into<String>, cursor_offset: usize) -> Self {
        Self {
            text: text.into(),
            cursor_offset,
            selection: None,
        }
    }

    /// 创建带选中范围的预编辑文本
    pub fn with_selection(
        text: impl Into<String>,
        cursor_offset: usize,
        selection: (usize, usize),
    ) -> Self {
        Self {
            text: text.into(),
            cursor_offset,
            selection: Some(selection),
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 获取文本长度（字符数）
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// 获取光标前的文本
    pub fn text_before_cursor(&self) -> &str {
        let byte_offset = self
            .text
            .char_indices()
            .nth(self.cursor_offset)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        &self.text[..byte_offset]
    }

    /// 获取光标后的文本
    pub fn text_after_cursor(&self) -> &str {
        let byte_offset = self
            .text
            .char_indices()
            .nth(self.cursor_offset)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        &self.text[byte_offset..]
    }

    /// 清空预编辑文本
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_offset = 0;
        self.selection = None;
    }
}

// ============================================================================
// IME 配置
// ============================================================================

/// IME 配置
#[derive(Debug, Clone)]
pub struct ImeConfig {
    /// 是否启用 IME
    pub enabled: bool,
    /// 预编辑文本背景色
    pub preedit_background: Hsla,
    /// 预编辑文本前景色
    pub preedit_foreground: Hsla,
    /// 预编辑文本下划线颜色
    pub preedit_underline: Hsla,
    /// 选中部分背景色
    pub selection_background: Hsla,
}

impl Default for ImeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            //浅黄色背景
            preedit_background: Hsla {
                h: 60.0 / 360.0,
                s: 0.5,
                l: 0.9,
                a: 0.8,
            },
            // 黑色前景
            preedit_foreground: Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.1,
                a: 1.0,
            },
            //蓝色下划线
            preedit_underline: Hsla {
                h: 210.0 / 360.0,
                s: 0.8,
                l: 0.5,
                a: 1.0,
            },
            //蓝色选中背景
            selection_background: Hsla {
                h: 210.0 / 360.0,
                s: 0.5,
                l: 0.7,
                a: 0.5,
            },
        }
    }
}

// ============================================================================
// IME 上下文
// ============================================================================

/// IME 上下文
///
/// 管理 IME 的完整状态
#[derive(Debug, Clone, Default)]
pub struct ImeContext {
    /// IME 状态
    state: ImeState,
    /// 预编辑文本
    preedit: PreeditText,
    /// IME 配置
    config: ImeConfig,
    /// 光标位置（终端坐标）
    cursor_line: i32,
    cursor_col: i32,
}

impl ImeContext {
    /// 创建新的 IME 上下文
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用配置创建 IME 上下文
    pub fn with_config(config: ImeConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    /// 获取 IME 状态
    pub fn state(&self) -> ImeState {
        self.state
    }

    /// 检查 IME 是否正在组合
    pub fn is_composing(&self) -> bool {
        self.state == ImeState::Composing
    }

    /// 获取预编辑文本
    pub fn preedit(&self) -> &PreeditText {
        &self.preedit
    }

    /// 获取配置
    pub fn config(&self) -> &ImeConfig {
        &self.config
    }

    /// 获取可变配置
    pub fn config_mut(&mut self) -> &mut ImeConfig {
        &mut self.config
    }

    /// 设置光标位置
    pub fn set_cursor_position(&mut self, line: i32, col: i32) {
        self.cursor_line = line;
        self.cursor_col = col;
    }

    /// 获取光标位置
    pub fn cursor_position(&self) -> (i32, i32) {
        (self.cursor_line, self.cursor_col)
    }

    /// 开始组合
    pub fn start_composing(&mut self, text: impl Into<String>) {
        self.state = ImeState::Composing;
        self.preedit = PreeditText::new(text);
    }

    /// 更新预编辑文本
    pub fn update_preedit(&mut self, text: impl Into<String>, cursor_offset: usize) {
        self.preedit = PreeditText::with_cursor(text, cursor_offset);
    }

    /// 更新预编辑文本（带选中范围）
    pub fn update_preedit_with_selection(
        &mut self,
        text: impl Into<String>,
        cursor_offset: usize,
        selection: (usize, usize),
    ) {
        self.preedit = PreeditText::with_selection(text, cursor_offset, selection);
    }

    /// 提交文本
    pub fn commit(&mut self) -> String {
        self.state = ImeState::Committed;
        let text = std::mem::take(&mut self.preedit.text);
        self.preedit.clear();
        text
    }

    /// 取消组合
    pub fn cancel(&mut self) {
        self.state = ImeState::Inactive;
        self.preedit.clear();
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.state = ImeState::Inactive;
        self.preedit.clear();
    }
}

// ============================================================================
// IME 布局
// ============================================================================

/// IME 预编辑文本布局
///
/// 用于渲染预编辑文本
#[derive(Debug, Clone)]
pub struct ImePreeditLayout {
    /// 预编辑文本
    pub text: String,
    /// 屏幕位置
    pub position: Point<Pixels>,
    /// 光标在预编辑文本中的X偏移
    pub cursor_x_offset: Pixels,
    /// 背景色
    pub background_color: Hsla,
    /// 前景色
    pub foreground_color: Hsla,
    /// 下划线颜色
    pub underline_color: Hsla,
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preedit_text_new() {
        let preedit = PreeditText::new("你好");
        assert_eq!(preedit.text, "你好");
        assert_eq!(preedit.cursor_offset, 2);
        assert!(preedit.selection.is_none());
    }

    #[test]
    fn test_preedit_text_with_cursor() {
        let preedit = PreeditText::with_cursor("hello", 3);
        assert_eq!(preedit.text, "hello");
        assert_eq!(preedit.cursor_offset, 3);
        assert_eq!(preedit.text_before_cursor(), "hel");
        assert_eq!(preedit.text_after_cursor(), "lo");
    }

    #[test]
    fn test_ime_context_composing() {
        let mut ctx = ImeContext::new();
        assert_eq!(ctx.state(), ImeState::Inactive);

        ctx.start_composing("ni");
        assert_eq!(ctx.state(), ImeState::Composing);
        assert!(ctx.is_composing());
        assert_eq!(ctx.preedit().text, "ni");

        ctx.update_preedit("你", 1);
        assert_eq!(ctx.preedit().text, "你");

        let committed = ctx.commit();
        assert_eq!(committed, "你");
        assert_eq!(ctx.state(), ImeState::Committed);
        assert!(ctx.preedit().is_empty());
    }

    #[test]
    fn test_ime_context_cancel() {
        let mut ctx = ImeContext::new();
        ctx.start_composing("test");
        assert!(ctx.is_composing());

        ctx.cancel();
        assert_eq!(ctx.state(), ImeState::Inactive);
        assert!(ctx.preedit().is_empty());
    }

    #[test]
    fn test_preedit_len() {
        let preedit = PreeditText::new("你好世界");
        assert_eq!(preedit.len(), 4);
        assert!(!preedit.is_empty());

        let empty = PreeditText::default();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
