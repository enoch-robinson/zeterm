//! 终端主题模块
//!
//! 定义终端颜色主题结构，并与 gpui-component Theme 集成。
//! 支持从 TOML 文件加载自定义主题。

use std::path::Path;

use gpui::Hsla;
use serde::{Deserialize, Serialize};

use super::colors::{ColorPalette, Rgb, SerializablePalette};

/// 终端主题
///
/// 定义终端的完整颜色方案，包括基础颜色、UI 元素颜色等。
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    /// 主题名称
    pub name: String,
    /// 是否为暗色主题
    pub is_dark: bool,
    /// 颜色调色板
    pub palette: ColorPalette,
    /// 光标颜色
    pub cursor: CursorColors,
    /// 选择颜色
    pub selection: SelectionColors,
    /// UI 元素颜色
    pub ui: UiColors,
}

/// 光标颜色配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorColors {
    /// 光标颜色
    #[serde(default = "default_cursor_color")]
    pub color: Rgb,
    /// 光标下文字颜色
    #[serde(default = "default_cursor_text_color")]
    pub text_color: Rgb,
}

fn default_cursor_color() -> Rgb {
    Rgb::new(0xcc, 0xcc, 0xcc)
}

fn default_cursor_text_color() -> Rgb {
    Rgb::new(0x1e, 0x1e, 0x1e)
}

impl Default for CursorColors {
    fn default() -> Self {
        Self {
            color: default_cursor_color(),
            text_color: default_cursor_text_color(),
        }
    }
}

/// 选择颜色配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionColors {
    /// 选择背景色
    #[serde(default = "default_selection_background")]
    pub background: Rgb,
    /// 选择前景色（可选，None 表示使用原始颜色）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<Rgb>,
}

fn default_selection_background() -> Rgb {
    Rgb::new(0x26, 0x4f, 0x78)
}

impl Default for SelectionColors {
    fn default() -> Self {
        Self {
            background: default_selection_background(),
            foreground: None,
        }
    }
}

/// UI 元素颜色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiColors {
    /// 边框颜色
    #[serde(default = "default_ui_border")]
    pub border: Rgb,
    /// 滚动条颜色
    #[serde(default = "default_ui_scrollbar")]
    pub scrollbar: Rgb,
    /// 滚动条悬停颜色
    #[serde(default = "default_ui_scrollbar_hover")]
    pub scrollbar_hover: Rgb,
    /// 搜索匹配高亮颜色
    #[serde(default = "default_ui_search_match")]
    pub search_match: Rgb,
    /// 当前搜索匹配颜色
    #[serde(default = "default_ui_search_match_active")]
    pub search_match_active: Rgb,
}

fn default_ui_border() -> Rgb {
    Rgb::new(0x3c, 0x3c, 0x3c)
}

fn default_ui_scrollbar() -> Rgb {
    Rgb::new(0x4a, 0x4a, 0x4a)
}

fn default_ui_scrollbar_hover() -> Rgb {
    Rgb::new(0x5a, 0x5a, 0x5a)
}

fn default_ui_search_match() -> Rgb {
    Rgb::new(0x51, 0x50, 0x00)
}

fn default_ui_search_match_active() -> Rgb {
    Rgb::new(0x61, 0x5a, 0x00)
}

impl Default for UiColors {
    fn default() -> Self {
        Self {
            border: default_ui_border(),
            scrollbar: default_ui_scrollbar(),
            scrollbar_hover: default_ui_scrollbar_hover(),
            search_match: default_ui_search_match(),
            search_match_active: default_ui_search_match_active(),
        }
    }
}

/// 可序列化的主题文件格式
///
/// 用于 TOML 主题文件的结构，字段名更友好
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFile {
    /// 主题基本信息
    #[serde(default)]
    pub theme: ThemeInfo,
    /// 颜色配置
    #[serde(default)]
    pub colors: SerializablePalette,
    /// 光标配置
    #[serde(default)]
    pub cursor: CursorColors,
    /// 选择配置
    #[serde(default)]
    pub selection: SelectionColors,
    /// UI 配置
    #[serde(default)]
    pub ui: UiColors,
}

/// 主题基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeInfo {
    /// 主题名称
    #[serde(default = "default_theme_name")]
    pub name: String,
    /// 是否为暗色主题
    #[serde(default = "default_is_dark")]
    pub is_dark: bool,
}

fn default_theme_name() -> String {
    "Custom Theme".to_string()
}

fn default_is_dark() -> bool {
    true
}

impl Default for ThemeInfo {
    fn default() -> Self {
        Self {
            name: default_theme_name(),
            is_dark: default_is_dark(),
        }
    }
}

impl Default for ThemeFile {
    fn default() -> Self {
        Self {
            theme: ThemeInfo::default(),
            colors: SerializablePalette::default(),
            cursor: CursorColors::default(),
            selection: SelectionColors::default(),
            ui: UiColors::default(),
        }
    }
}

impl From<ThemeFile> for TerminalTheme {
    fn from(file: ThemeFile) -> Self {
        Self {
            name: file.theme.name,
            is_dark: file.theme.is_dark,
            palette: file.colors.into(),
            cursor: file.cursor,
            selection: file.selection,
            ui: file.ui,
        }
    }
}

impl From<&TerminalTheme> for ThemeFile {
    fn from(theme: &TerminalTheme) -> Self {
        Self {
            theme: ThemeInfo {
                name: theme.name.clone(),
                is_dark: theme.is_dark,
            },
            colors: SerializablePalette::from(&theme.palette),
            cursor: theme.cursor.clone(),
            selection: theme.selection.clone(),
            ui: theme.ui.clone(),
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl TerminalTheme {
    /// 从 TOML 字符串解析主题
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let theme_file: ThemeFile = toml::from_str(toml_str)?;
        Ok(theme_file.into())
    }

    /// 从 TOML 文件加载主题
    pub fn from_toml_file(path: &Path) -> Result<Self, ThemeLoadError> {
        let content = std::fs::read_to_string(path).map_err(ThemeLoadError::Io)?;
        Self::from_toml(&content).map_err(ThemeLoadError::Parse)
    }

    /// 导出为 TOML 字符串
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let theme_file = ThemeFile::from(self);
        toml::to_string_pretty(&theme_file)
    }

    /// 保存到 TOML 文件
    pub fn to_toml_file(&self, path: &Path) -> Result<(), ThemeLoadError> {
        let content = self.to_toml().map_err(|e| {
            ThemeLoadError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        std::fs::write(path, content).map_err(ThemeLoadError::Io)
    }

    /// 创建暗色主题
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),
            is_dark: true,
            palette: ColorPalette::dark(),
            cursor: CursorColors {
                color: Rgb::new(0xcc, 0xcc, 0xcc),
                text_color: Rgb::new(0x1e, 0x1e, 0x1e),
            },
            selection: SelectionColors {
                background: Rgb::new(0x26, 0x4f, 0x78),
                foreground: None,
            },
            ui: UiColors {
                border: Rgb::new(0x3c, 0x3c, 0x3c),
                scrollbar: Rgb::new(0x4a, 0x4a, 0x4a),
                scrollbar_hover: Rgb::new(0x5a, 0x5a, 0x5a),
                search_match: Rgb::new(0x51, 0x50, 0x00),
                search_match_active: Rgb::new(0x61, 0x5a, 0x00),
            },
        }
    }

    /// 创建亮色主题
    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
            is_dark: false,
            palette: ColorPalette::light(),
            cursor: CursorColors {
                color: Rgb::new(0x00, 0x00, 0x00),
                text_color: Rgb::new(0xff, 0xff, 0xff),
            },
            selection: SelectionColors {
                background: Rgb::new(0xad, 0xd6, 0xff),
                foreground: None,
            },
            ui: UiColors {
                border: Rgb::new(0xd0, 0xd0, 0xd0),
                scrollbar: Rgb::new(0xc0, 0xc0, 0xc0),
                scrollbar_hover: Rgb::new(0xa0, 0xa0, 0xa0),
                search_match: Rgb::new(0xff, 0xf0, 0x80),
                search_match_active: Rgb::new(0xff, 0xe0, 0x40),
            },
        }
    }

    /// 创建 Dracula 主题
    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            is_dark: true,
            palette: ColorPalette {
                base_colors: [
                    Rgb::new(0x21, 0x22, 0x2c), // Black
                    Rgb::new(0xff, 0x55, 0x55), // Red
                    Rgb::new(0x50, 0xfa, 0x7b), // Green
                    Rgb::new(0xf1, 0xfa, 0x8c), // Yellow
                    Rgb::new(0xbd, 0x93, 0xf9), // Blue
                    Rgb::new(0xff, 0x79, 0xc6), // Magenta
                    Rgb::new(0x8b, 0xe9, 0xfd), // Cyan
                    Rgb::new(0xf8, 0xf8, 0xf2), // White
                    Rgb::new(0x62, 0x72, 0xa4), // Bright Black
                    Rgb::new(0xff, 0x6e, 0x6e), // Bright Red
                    Rgb::new(0x69, 0xff, 0x94), // Bright Green
                    Rgb::new(0xff, 0xff, 0xa5), // Bright Yellow
                    Rgb::new(0xd6, 0xac, 0xff), // Bright Blue
                    Rgb::new(0xff, 0x92, 0xdf), // Bright Magenta
                    Rgb::new(0xa4, 0xff, 0xff), // Bright Cyan
                    Rgb::new(0xff, 0xff, 0xff), // Bright White
                ],
                foreground: Rgb::new(0xf8, 0xf8, 0xf2),
                background: Rgb::new(0x28, 0x2a, 0x36),
            },
            cursor: CursorColors {
                color: Rgb::new(0xf8, 0xf8, 0xf2),
                text_color: Rgb::new(0x28, 0x2a, 0x36),
            },
            selection: SelectionColors {
                background: Rgb::new(0x44, 0x47, 0x5a),
                foreground: None,
            },
            ui: UiColors {
                border: Rgb::new(0x44, 0x47, 0x5a),
                scrollbar: Rgb::new(0x44, 0x47, 0x5a),
                scrollbar_hover: Rgb::new(0x62, 0x72, 0xa4),
                search_match: Rgb::new(0xf1, 0xfa, 0x8c),
                search_match_active: Rgb::new(0xff, 0xb8, 0x6c),
            },
        }
    }

    /// 创建 One Dark 主题
    pub fn one_dark() -> Self {
        Self {
            name: "One Dark".to_string(),
            is_dark: true,
            palette: ColorPalette {
                base_colors: [
                    Rgb::new(0x28, 0x2c, 0x34), // Black
                    Rgb::new(0xe0, 0x6c, 0x75), // Red
                    Rgb::new(0x98, 0xc3, 0x79), // Green
                    Rgb::new(0xe5, 0xc0, 0x7b), // Yellow
                    Rgb::new(0x61, 0xaf, 0xef), // Blue
                    Rgb::new(0xc6, 0x78, 0xdd), // Magenta
                    Rgb::new(0x56, 0xb6, 0xc2), // Cyan
                    Rgb::new(0xab, 0xb2, 0xbf), // White
                    Rgb::new(0x54, 0x5b, 0x69), // Bright Black
                    Rgb::new(0xe0, 0x6c, 0x75), // Bright Red
                    Rgb::new(0x98, 0xc3, 0x79), // Bright Green
                    Rgb::new(0xe5, 0xc0, 0x7b), // Bright Yellow
                    Rgb::new(0x61, 0xaf, 0xef), // Bright Blue
                    Rgb::new(0xc6, 0x78, 0xdd), // Bright Magenta
                    Rgb::new(0x56, 0xb6, 0xc2), // Bright Cyan
                    Rgb::new(0xff, 0xff, 0xff), // Bright White
                ],
                foreground: Rgb::new(0xab, 0xb2, 0xbf),
                background: Rgb::new(0x28, 0x2c, 0x34),
            },
            cursor: CursorColors {
                color: Rgb::new(0x52, 0x8b, 0xff),
                text_color: Rgb::new(0x28, 0x2c, 0x34),
            },
            selection: SelectionColors {
                background: Rgb::new(0x3e, 0x44, 0x51),
                foreground: None,
            },
            ui: UiColors {
                border: Rgb::new(0x3e, 0x44, 0x51),
                scrollbar: Rgb::new(0x3e, 0x44, 0x51),
                scrollbar_hover: Rgb::new(0x54, 0x5b, 0x69),
                search_match: Rgb::new(0xe5, 0xc0, 0x7b),
                search_match_active: Rgb::new(0x61, 0xaf, 0xef),
            },
        }
    }

    /// 创建 Solarized Dark 主题
    pub fn solarized_dark() -> Self {
        Self {
            name: "Solarized Dark".to_string(),
            is_dark: true,
            palette: ColorPalette {
                base_colors: [
                    Rgb::new(0x07, 0x36, 0x42), // Black (base02)
                    Rgb::new(0xdc, 0x32, 0x2f), // Red
                    Rgb::new(0x85, 0x99, 0x00), // Green
                    Rgb::new(0xb5, 0x89, 0x00), // Yellow
                    Rgb::new(0x26, 0x8b, 0xd2), // Blue
                    Rgb::new(0xd3, 0x36, 0x82), // Magenta
                    Rgb::new(0x2a, 0xa1, 0x98), // Cyan
                    Rgb::new(0xee, 0xe8, 0xd5), // White (base2)
                    Rgb::new(0x00, 0x2b, 0x36), // Bright Black (base03)
                    Rgb::new(0xcb, 0x4b, 0x16), // Bright Red (orange)
                    Rgb::new(0x58, 0x6e, 0x75), // Bright Green (base01)
                    Rgb::new(0x65, 0x7b, 0x83), // Bright Yellow (base00)
                    Rgb::new(0x83, 0x94, 0x96), // Bright Blue (base0)
                    Rgb::new(0x6c, 0x71, 0xc4), // Bright Magenta (violet)
                    Rgb::new(0x93, 0xa1, 0xa1), // Bright Cyan (base1)
                    Rgb::new(0xfd, 0xf6, 0xe3), // Bright White (base3)
                ],
                foreground: Rgb::new(0x83, 0x94, 0x96),
                background: Rgb::new(0x00, 0x2b, 0x36),
            },
            cursor: CursorColors {
                color: Rgb::new(0x83, 0x94, 0x96),
                text_color: Rgb::new(0x00, 0x2b, 0x36),
            },
            selection: SelectionColors {
                background: Rgb::new(0x07, 0x36, 0x42),
                foreground: None,
            },
            ui: UiColors {
                border: Rgb::new(0x07, 0x36, 0x42),
                scrollbar: Rgb::new(0x07, 0x36, 0x42),
                scrollbar_hover: Rgb::new(0x58, 0x6e, 0x75),
                search_match: Rgb::new(0xb5, 0x89, 0x00),
                search_match_active: Rgb::new(0xcb, 0x4b, 0x16),
            },
        }
    }

    /// 获取前景色
    pub fn foreground(&self) -> Rgb {
        self.palette.foreground
    }

    /// 获取背景色
    pub fn background(&self) -> Rgb {
        self.palette.background
    }

    /// 获取光标颜色
    pub fn cursor_color(&self) -> Rgb {
        self.cursor.color
    }

    /// 获取选择背景色
    pub fn selection_background(&self) -> Rgb {
        self.selection.background
    }
}

/// 主题加载错误
#[derive(Debug)]
pub enum ThemeLoadError {
    /// IO 错误
    Io(std::io::Error),
    /// TOML 解析错误
    Parse(toml::de::Error),
}

impl std::fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeLoadError::Io(e) => write!(f, "Failed to read theme file: {}", e),
            ThemeLoadError::Parse(e) => write!(f, "Failed to parse theme file: {}", e),
        }
    }
}

impl std::error::Error for ThemeLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ThemeLoadError::Io(e) => Some(e),
            ThemeLoadError::Parse(e) => Some(e),
        }
    }
}

/// 主题管理器
#[derive(Debug, Clone)]
pub struct ThemeManager {
    /// 当前主题
    current: TerminalTheme,
    /// 可用主题列表
    available: Vec<TerminalTheme>,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeManager {
    /// 创建新的主题管理器
    pub fn new() -> Self {
        Self {
            current: TerminalTheme::dark(),
            available: vec![
                TerminalTheme::dark(),
                TerminalTheme::light(),
                TerminalTheme::dracula(),
                TerminalTheme::one_dark(),
                TerminalTheme::solarized_dark(),
            ],
        }
    }

    /// 获取当前主题
    pub fn current(&self) -> &TerminalTheme {
        &self.current
    }

    /// 设置当前主题
    pub fn set_current(&mut self, theme: TerminalTheme) {
        self.current = theme;
    }

    /// 按名称切换主题
    pub fn switch_by_name(&mut self, name: &str) -> bool {
        if let Some(theme) = self.available.iter().find(|t| t.name == name) {
            self.current = theme.clone();
            true
        } else {
            false
        }
    }

    /// 切换到暗色/亮色主题
    pub fn toggle_dark_light(&mut self) {
        if self.current.is_dark {
            self.current = TerminalTheme::light();
        } else {
            self.current = TerminalTheme::dark();
        }
    }

    /// 获取可用主题列表
    pub fn available_themes(&self) -> &[TerminalTheme] {
        &self.available
    }

    /// 添加自定义主题
    pub fn add_theme(&mut self, theme: TerminalTheme) {
        self.available.push(theme);
    }
}

/// RGB 转HSLA
pub fn rgb_to_hsla(rgb: Rgb) -> Hsla {
    let r = rgb.r as f32 / 255.0;
    let g = rgb.g as f32 / 255.0;
    let b = rgb.b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) / 2.0;

    if delta == 0.0 {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a: 1.0,
        };
    }

    let s = if l < 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    let h = if max == r {
        ((g - b) / delta) % 6.0
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };

    let h = (h * 60.0) / 360.0;
    let h = if h < 0.0 { h + 1.0 } else { h };

    Hsla { h, s, l, a: 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_theme_dark() {
        let theme = TerminalTheme::dark();
        assert!(theme.is_dark);
        assert_eq!(theme.name, "Dark");
    }

    #[test]
    fn test_terminal_theme_light() {
        let theme = TerminalTheme::light();
        assert!(!theme.is_dark);
        assert_eq!(theme.name, "Light");
    }

    #[test]
    fn test_terminal_theme_dracula() {
        let theme = TerminalTheme::dracula();
        assert!(theme.is_dark);
        assert_eq!(theme.name, "Dracula");
    }

    #[test]
    fn test_terminal_theme_one_dark() {
        let theme = TerminalTheme::one_dark();
        assert!(theme.is_dark);
        assert_eq!(theme.name, "One Dark");
    }

    #[test]
    fn test_theme_manager_new() {
        let manager = ThemeManager::new();
        assert!(manager.current().is_dark);
        assert!(!manager.available_themes().is_empty());
    }

    #[test]
    fn test_theme_manager_switch_by_name() {
        let mut manager = ThemeManager::new();
        assert!(manager.switch_by_name("Light"));
        assert!(!manager.current().is_dark);
        assert!(manager.switch_by_name("Dracula"));
        assert_eq!(manager.current().name, "Dracula");
    }

    #[test]
    fn test_theme_manager_toggle() {
        let mut manager = ThemeManager::new();
        assert!(manager.current().is_dark);
        manager.toggle_dark_light();
        assert!(!manager.current().is_dark);
        manager.toggle_dark_light();
        assert!(manager.current().is_dark);
    }

    #[test]
    fn test_rgb_to_hsla() {
        //测试纯红色
        let red = Rgb::new(255, 0, 0);
        let hsla = rgb_to_hsla(red);
        assert!((hsla.h - 0.0).abs() < 0.01);
        assert!((hsla.s - 1.0).abs() < 0.01);
        assert!((hsla.l - 0.5).abs() < 0.01);

        // 测试白色
        let white = Rgb::new(255, 255, 255);
        let hsla = rgb_to_hsla(white);
        assert!((hsla.l - 1.0).abs() < 0.01);

        // 测试黑色
        let black = Rgb::new(0, 0, 0);
        let hsla = rgb_to_hsla(black);
        assert!((hsla.l - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_theme_accessors() {
        let theme = TerminalTheme::dark();
        assert_eq!(theme.foreground(), theme.palette.foreground);
        assert_eq!(theme.background(), theme.palette.background);
        assert_eq!(theme.cursor_color(), theme.cursor.color);
        assert_eq!(theme.selection_background(), theme.selection.background);
    }

    #[test]
    fn test_theme_from_toml() {
        let toml_str = r##"
[theme]
name = "My Theme"
is_dark = true

[colors]
foreground = "#f8f8f2"
background = "#282a36"

[colors.palette]
black = "#21222c"
red = "#ff5555"
green = "#50fa7b"
yellow = "#f1fa8c"
blue = "#bd93f9"
magenta = "#ff79c6"
cyan = "#8be9fd"
white = "#f8f8f2"
bright_black = "#6272a4"
bright_red = "#ff6e6e"
bright_green = "#69ff94"
bright_yellow = "#ffffa5"
bright_blue = "#d6acff"
bright_magenta = "#ff92df"
bright_cyan = "#a4ffff"
bright_white = "#ffffff"

[cursor]
color = "#f8f8f2"
text_color = "#282a36"

[selection]
background = "#44475a"

[ui]
border = "#44475a"
scrollbar = "#44475a"
scrollbar_hover = "#6272a4"
search_match = "#f1fa8c"
search_match_active = "#ffb86c"
"##;

        let theme = TerminalTheme::from_toml(toml_str).unwrap();
        assert_eq!(theme.name, "My Theme");
        assert!(theme.is_dark);
        assert_eq!(theme.palette.foreground, Rgb::new(0xf8, 0xf8, 0xf2));
        assert_eq!(theme.palette.background, Rgb::new(0x28, 0x2a, 0x36));
        assert_eq!(theme.cursor.color, Rgb::new(0xf8, 0xf8, 0xf2));
    }

    #[test]
    fn test_theme_from_toml_minimal() {
        // 测试最小配置（使用所有默认值）
        let toml_str = r##"
[theme]
name = "Minimal"
"##;

        let theme = TerminalTheme::from_toml(toml_str).unwrap();
        assert_eq!(theme.name, "Minimal");
        assert!(theme.is_dark); // 默认值
    }

    #[test]
    fn test_theme_to_toml_roundtrip() {
        let original = TerminalTheme::dracula();
        let toml_str = original.to_toml().unwrap();
        let parsed = TerminalTheme::from_toml(&toml_str).unwrap();

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.is_dark, original.is_dark);
        assert_eq!(parsed.palette.foreground, original.palette.foreground);
        assert_eq!(parsed.palette.background, original.palette.background);
        assert_eq!(parsed.cursor.color, original.cursor.color);
    }

    #[test]
    fn test_theme_file_default() {
        let file = ThemeFile::default();
        assert_eq!(file.theme.name, "Custom Theme");
        assert!(file.theme.is_dark);
    }
}
