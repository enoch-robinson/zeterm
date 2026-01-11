//! 颜色转换模块
//!
//! 将Alacritty 终端颜色转换为 GPUI 颜色。
//! 支持 16 色、256 色索引色和 24 位真彩色。

use gpui::Hsla;

/// 16 色基础调色板
///
/// 标准 ANSI 颜色定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl NamedColor {
    /// 从索引创建命名颜色
    pub fn from_index(idx: u8) -> Option<Self> {
        match idx {
            0 => Some(NamedColor::Black),
            1 => Some(NamedColor::Red),
            2 => Some(NamedColor::Green),
            3 => Some(NamedColor::Yellow),
            4 => Some(NamedColor::Blue),
            5 => Some(NamedColor::Magenta),
            6 => Some(NamedColor::Cyan),
            7 => Some(NamedColor::White),
            8 => Some(NamedColor::BrightBlack),
            9 => Some(NamedColor::BrightRed),
            10 => Some(NamedColor::BrightGreen),
            11 => Some(NamedColor::BrightYellow),
            12 => Some(NamedColor::BrightBlue),
            13 => Some(NamedColor::BrightMagenta),
            14 => Some(NamedColor::BrightCyan),
            15 => Some(NamedColor::BrightWhite),
            _ => None,
        }
    }
}

/// RGB 颜色
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// 创建新的 RGB 颜色
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// 从 u32 创建 (0xRRGGBB 格式)
    pub fn from_u32(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    /// 转换为 u32 (0xRRGGBB 格式)
    pub fn to_u32(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

/// 终端颜色
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalColor {
    /// 命名颜色 (16 色)
    Named(NamedColor),
    /// 索引颜色 (256 色)
    Indexed(u8),
    /// RGB 真彩色
    Rgb(Rgb),
}

///颜色调色板
///
/// 定义终端使用的颜色方案
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /// 16 色基础调色板
    pub base_colors: [Rgb; 16],
    /// 前景色
    pub foreground: Rgb,
    /// 背景色
    pub background: Rgb,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorPalette {
    /// 创建暗色主题调色板
    pub fn dark() -> Self {
        Self {
            base_colors: [
                Rgb::new(0x00, 0x00, 0x00), // Black
                Rgb::new(0xcc, 0x00, 0x00), // Red
                Rgb::new(0x00, 0xcc, 0x00), // Green
                Rgb::new(0xcc, 0xcc, 0x00), // Yellow
                Rgb::new(0x00, 0x00, 0xcc), // Blue
                Rgb::new(0xcc, 0x00, 0xcc), // Magenta
                Rgb::new(0x00, 0xcc, 0xcc), // Cyan
                Rgb::new(0xcc, 0xcc, 0xcc), // White
                Rgb::new(0x66, 0x66, 0x66), // Bright Black
                Rgb::new(0xff, 0x00, 0x00), // Bright Red
                Rgb::new(0x00, 0xff, 0x00), // Bright Green
                Rgb::new(0xff, 0xff, 0x00), // Bright Yellow
                Rgb::new(0x00, 0x00, 0xff), // Bright Blue
                Rgb::new(0xff, 0x00, 0xff), // Bright Magenta
                Rgb::new(0x00, 0xff, 0xff), // Bright Cyan
                Rgb::new(0xff, 0xff, 0xff), // Bright White
            ],
            foreground: Rgb::new(0xcc, 0xcc, 0xcc),
            background: Rgb::new(0x1e, 0x1e, 0x1e),
        }
    }

    /// 创建亮色主题调色板
    pub fn light() -> Self {
        Self {
            base_colors: [
                Rgb::new(0x00, 0x00, 0x00), // Black
                Rgb::new(0xaa, 0x00, 0x00), // Red
                Rgb::new(0x00, 0xaa, 0x00), // Green
                Rgb::new(0xaa, 0x55, 0x00), // Yellow (darker for light bg)
                Rgb::new(0x00, 0x00, 0xaa), // Blue
                Rgb::new(0xaa, 0x00, 0xaa), // Magenta
                Rgb::new(0x00, 0xaa, 0xaa), // Cyan
                Rgb::new(0xaa, 0xaa, 0xaa), // White
                Rgb::new(0x55, 0x55, 0x55), // Bright Black
                Rgb::new(0xff, 0x55, 0x55), // Bright Red
                Rgb::new(0x55, 0xff, 0x55), // Bright Green
                Rgb::new(0xff, 0xff, 0x55), // Bright Yellow
                Rgb::new(0x55, 0x55, 0xff), // Bright Blue
                Rgb::new(0xff, 0x55, 0xff), // Bright Magenta
                Rgb::new(0x55, 0xff, 0xff), // Bright Cyan
                Rgb::new(0xff, 0xff, 0xff), // Bright White
            ],
            foreground: Rgb::new(0x00, 0x00, 0x00),
            background: Rgb::new(0xff, 0xff, 0xff),
        }
    }
}

/// 将命名颜色转换为 RGB
pub fn named_color_to_rgb(color: NamedColor, palette: &ColorPalette) -> Rgb {
    let idx = match color {
        NamedColor::Black => 0,
        NamedColor::Red => 1,
        NamedColor::Green => 2,
        NamedColor::Yellow => 3,
        NamedColor::Blue => 4,
        NamedColor::Magenta => 5,
        NamedColor::Cyan => 6,
        NamedColor::White => 7,
        NamedColor::BrightBlack => 8,
        NamedColor::BrightRed => 9,
        NamedColor::BrightGreen => 10,
        NamedColor::BrightYellow => 11,
        NamedColor::BrightBlue => 12,
        NamedColor::BrightMagenta => 13,
        NamedColor::BrightCyan => 14,
        NamedColor::BrightWhite => 15,
    };
    palette.base_colors[idx]
}

/// 将索引颜色 (256色) 转换为 RGB
///
/// 256 色调色板结构:
/// - 0-15: 基础 16 色
/// - 16-231: 6x6x6 颜色立方体 (216 色)
/// - 232-255: 24 级灰度
pub fn indexed_color_to_rgb(idx: u8, palette: &ColorPalette) -> Rgb {
    if idx < 16 {
        // 基础 16 色
        palette.base_colors[idx as usize]
    } else if idx < 232 {
        // 216 色立方体 (6x6x6)
        let idx = idx - 16;
        let r = color_cube_component(idx / 36);
        let g = color_cube_component((idx / 6) % 6);
        let b = color_cube_component(idx % 6);
        Rgb::new(r, g, b)
    } else {
        // 24 级灰度
        let gray = grayscale_component(idx - 232);
        Rgb::new(gray, gray, gray)
    }
}

/// 计算颜色立方体分量值
///
/// 6x6x6 颜色立方体的每个分量值
fn color_cube_component(value: u8) -> u8 {
    if value == 0 { 0 } else { 55 + value * 40 }
}

/// 计算灰度分量值
///
/// 24 级灰度从 8 到 238
fn grayscale_component(idx: u8) -> u8 {
    8 + idx * 10
}

/// 将终端颜色转换为 RGB
pub fn terminal_color_to_rgb(color: TerminalColor, palette: &ColorPalette) -> Rgb {
    match color {
        TerminalColor::Named(named) => named_color_to_rgb(named, palette),
        TerminalColor::Indexed(idx) => indexed_color_to_rgb(idx, palette),
        TerminalColor::Rgb(rgb) => rgb,
    }
}

/// 将 RGB 转换为 GPUI Hsla
pub fn rgb_to_hsla(rgb: Rgb) -> Hsla {
    gpui::rgb(rgb.to_u32()).into()
}

/// 将终端颜色转换为 GPUI Hsla
pub fn terminal_color_to_hsla(color: TerminalColor, palette: &ColorPalette) -> Hsla {
    let rgb = terminal_color_to_rgb(color, palette);
    rgb_to_hsla(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    //==================== RGB 测试 ====================

    #[test]
    fn test_rgb_new() {
        let rgb = Rgb::new(255, 128, 64);
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 128);
        assert_eq!(rgb.b, 64);
    }

    #[test]
    fn test_rgb_from_u32() {
        let rgb = Rgb::from_u32(0xff8040);
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 128);
        assert_eq!(rgb.b, 64);
    }

    #[test]
    fn test_rgb_to_u32() {
        let rgb = Rgb::new(255, 128, 64);
        assert_eq!(rgb.to_u32(), 0xff8040);
    }

    #[test]
    fn test_rgb_roundtrip() {
        let original = 0xaabbcc;
        let rgb = Rgb::from_u32(original);
        assert_eq!(rgb.to_u32(), original);
    }

    // ==================== 命名颜色测试 ====================

    #[test]
    fn test_named_color_from_index() {
        assert_eq!(NamedColor::from_index(0), Some(NamedColor::Black));
        assert_eq!(NamedColor::from_index(1), Some(NamedColor::Red));
        assert_eq!(NamedColor::from_index(7), Some(NamedColor::White));
        assert_eq!(NamedColor::from_index(8), Some(NamedColor::BrightBlack));
        assert_eq!(NamedColor::from_index(15), Some(NamedColor::BrightWhite));
        assert_eq!(NamedColor::from_index(16), None);
    }

    #[test]
    fn test_named_color_to_rgb() {
        let palette = ColorPalette::dark();

        let black = named_color_to_rgb(NamedColor::Black, &palette);
        assert_eq!(black, Rgb::new(0x00, 0x00, 0x00));

        let red = named_color_to_rgb(NamedColor::Red, &palette);
        assert_eq!(red, Rgb::new(0xcc, 0x00, 0x00));

        let bright_white = named_color_to_rgb(NamedColor::BrightWhite, &palette);
        assert_eq!(bright_white, Rgb::new(0xff, 0xff, 0xff));
    }

    // ==================== 索引颜色测试 ====================

    #[test]
    fn test_indexed_color_base_16() {
        let palette = ColorPalette::dark();

        // 测试基础 16 色
        let black = indexed_color_to_rgb(0, &palette);
        assert_eq!(black, palette.base_colors[0]);

        let red = indexed_color_to_rgb(1, &palette);
        assert_eq!(red, palette.base_colors[1]);

        let bright_white = indexed_color_to_rgb(15, &palette);
        assert_eq!(bright_white, palette.base_colors[15]);
    }

    #[test]
    fn test_indexed_color_cube() {
        let palette = ColorPalette::dark();

        // 索引 16:颜色立方体起始 (0, 0, 0) - 但不是黑色
        let color_16 = indexed_color_to_rgb(16, &palette);
        assert_eq!(color_16, Rgb::new(0, 0, 0));

        // 索引 17: (0, 0, 1) -> (0, 0, 95)
        let color_17 = indexed_color_to_rgb(17, &palette);
        assert_eq!(color_17, Rgb::new(0, 0, 95));

        // 索引 21: (0, 0, 5) -> (0, 0, 255)
        let color_21 = indexed_color_to_rgb(21, &palette);
        assert_eq!(color_21, Rgb::new(0, 0, 255));

        // 索引 196: 纯红(5, 0, 0) -> (255, 0, 0)
        let color_196 = indexed_color_to_rgb(196, &palette);
        assert_eq!(color_196, Rgb::new(255, 0, 0));

        // 索引 231: 颜色立方体结束 (5, 5, 5) -> (255, 255, 255)
        let color_231 = indexed_color_to_rgb(231, &palette);
        assert_eq!(color_231, Rgb::new(255, 255, 255));
    }

    #[test]
    fn test_indexed_color_grayscale() {
        let palette = ColorPalette::dark();

        // 索引 232: 灰度起始 (最暗)
        let gray_232 = indexed_color_to_rgb(232, &palette);
        assert_eq!(gray_232, Rgb::new(8, 8, 8));

        // 索引 243: 中间灰度
        let gray_243 = indexed_color_to_rgb(243, &palette);
        let expected_gray = 8 + 11 * 10; // 118
        assert_eq!(
            gray_243,
            Rgb::new(expected_gray, expected_gray, expected_gray)
        );

        // 索引 255:灰度结束 (最亮)
        let gray_255 = indexed_color_to_rgb(255, &palette);
        let expected_gray = 8 + 23 * 10; // 238
        assert_eq!(
            gray_255,
            Rgb::new(expected_gray, expected_gray, expected_gray)
        );
    }

    // ==================== 颜色立方体分量测试 ====================

    #[test]
    fn test_color_cube_component() {
        assert_eq!(color_cube_component(0), 0);
        assert_eq!(color_cube_component(1), 95); // 55 + 40
        assert_eq!(color_cube_component(2), 135); // 55 + 80
        assert_eq!(color_cube_component(3), 175); // 55 + 120
        assert_eq!(color_cube_component(4), 215); // 55 + 160
        assert_eq!(color_cube_component(5), 255); // 55 + 200
    }

    #[test]
    fn test_grayscale_component() {
        assert_eq!(grayscale_component(0), 8);
        assert_eq!(grayscale_component(1), 18);
        assert_eq!(grayscale_component(23), 238);
    }

    // ==================== 终端颜色转换测试 ====================

    #[test]
    fn test_terminal_color_named() {
        let palette = ColorPalette::dark();
        let color = TerminalColor::Named(NamedColor::Red);
        let rgb = terminal_color_to_rgb(color, &palette);
        assert_eq!(rgb, Rgb::new(0xcc, 0x00, 0x00));
    }

    #[test]
    fn test_terminal_color_indexed() {
        let palette = ColorPalette::dark();
        let color = TerminalColor::Indexed(196);
        let rgb = terminal_color_to_rgb(color, &palette);
        assert_eq!(rgb, Rgb::new(255, 0, 0));
    }

    #[test]
    fn test_terminal_color_rgb() {
        let palette = ColorPalette::dark();
        let color = TerminalColor::Rgb(Rgb::new(128, 64, 32));
        let rgb = terminal_color_to_rgb(color, &palette);
        assert_eq!(rgb, Rgb::new(128, 64, 32));
    }

    // ==================== 调色板测试 ====================

    #[test]
    fn test_dark_palette() {
        let palette = ColorPalette::dark();
        assert_eq!(palette.base_colors[0], Rgb::new(0x00, 0x00, 0x00)); // Black
        assert_eq!(palette.foreground, Rgb::new(0xcc, 0xcc, 0xcc));
        assert_eq!(palette.background, Rgb::new(0x1e, 0x1e, 0x1e));
    }

    #[test]
    fn test_light_palette() {
        let palette = ColorPalette::light();
        assert_eq!(palette.foreground, Rgb::new(0x00, 0x00, 0x00));
        assert_eq!(palette.background, Rgb::new(0xff, 0xff, 0xff));
    }

    // ==================== HSLA 转换测试 ====================

    #[test]
    fn test_rgb_to_hsla() {
        let rgb = Rgb::new(255, 0, 0);
        let hsla = rgb_to_hsla(rgb);
        // 红色应该有 hue 接近 0
        assert!(hsla.h >= 0.0 && hsla.h <= 0.1 || hsla.h >= 0.9);
        assert!(hsla.s > 0.9); // 高饱和度
        assert!(hsla.l > 0.4 && hsla.l < 0.6); // 中等亮度
    }

    #[test]
    fn test_terminal_color_to_hsla() {
        let palette = ColorPalette::dark();
        let color = TerminalColor::Named(NamedColor::Green);
        let hsla = terminal_color_to_hsla(color, &palette);
        // 绿色应该有 hue 接近 0.33(120度)
        assert!(hsla.h > 0.2 && hsla.h < 0.5);
    }
}
