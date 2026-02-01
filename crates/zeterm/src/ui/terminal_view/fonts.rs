//! 终端字体配置模块
//!
//! 提供终端渲染所需的等宽字体配置，包括：
//! - 字体族回退列表
//! - 跨平台字体支持
//! - 字体常量定义

use gpui::{Font, FontStyle, FontWeight, SharedString};

/// 等宽字体回退列表
///
/// 按优先级排序，第一个可用的字体将被使用
/// - Windows: Consolas, Courier New
/// - macOS: Menlo, Monaco
/// - Linux: DejaVu Sans Mono, Liberation Mono
/// - 跨平台: JetBrains Mono, Fira Code
pub const MONOSPACE_FONT_FAMILIES: &[&str] = &[
    "JetBrains Mono",   // 跨平台，推荐
    "Cascadia Code",    // Windows 11
    "Consolas",         // Windows
    "Menlo",            // macOS
    "Monaco",           // macOS
    "DejaVu Sans Mono", // Linux
    "Liberation Mono",  // Linux"Fira Code",         //跨平台
    "Source Code Pro",  // 跨平台
    "Courier New",      // 通用回退
    "monospace",        // 系统等宽字体
];

/// 创建终端字体
///
/// 使用等宽字体回退列表中的第一个字体族
pub fn terminal_font() -> Font {
    Font {
        family: SharedString::from(MONOSPACE_FONT_FAMILIES[0]),
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
        ..Default::default()
    }
}

/// 创建带样式的终端字体
pub fn terminal_font_with_style(bold: bool, italic: bool) -> Font {
    Font {
        family: SharedString::from(MONOSPACE_FONT_FAMILIES[0]),
        weight: if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..Default::default()
    }
}

/// 获取字体族名称
pub fn terminal_font_family() -> SharedString {
    SharedString::from(MONOSPACE_FONT_FAMILIES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_font() {
        let font = terminal_font();
        assert_eq!(font.family.as_ref(), "JetBrains Mono");
        assert_eq!(font.weight, FontWeight::NORMAL);
        assert_eq!(font.style, FontStyle::Normal);
    }

    #[test]
    fn test_terminal_font_bold() {
        let font = terminal_font_with_style(true, false);
        assert_eq!(font.weight, FontWeight::BOLD);
        assert_eq!(font.style, FontStyle::Normal);
    }

    #[test]
    fn test_terminal_font_italic() {
        let font = terminal_font_with_style(false, true);
        assert_eq!(font.weight, FontWeight::NORMAL);
        assert_eq!(font.style, FontStyle::Italic);
    }

    #[test]
    fn test_font_families_not_empty() {
        assert!(!MONOSPACE_FONT_FAMILIES.is_empty());
    }
}
