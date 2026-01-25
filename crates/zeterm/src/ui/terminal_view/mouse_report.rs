//! 鼠标报告模块
//!
//! 将鼠标事件编码为终端转义序列，发送给远端应用（如 vim、tmux、htop）。
//!
//! # 支持的模式
//!
//! - `MOUSE_REPORT_CLICK` (?1000h) - 报告点击事件
//! - `MOUSE_DRAG` (?1002h) - 报告拖拽事件
//! - `MOUSE_MOTION` (?1003h) - 报告所有鼠标移动
//! - `SGR_MOUSE` (?1006h) - 使用 SGR 编码格式
//!
//! # 编码格式
//!
//! 本模块优先使用 SGR 格式（现代终端标准）：
//! - 按下: `ESC [ < Cb ; Cx ; Cy M`
//! - 释放: `ESC [ < Cb ; Cx ; Cy m`
//!
//! 其中：
//! - Cb = 按钮码 (0=左键, 1=中键, 2=右键, 64=滚轮上, 65=滚轮下)
//! - Cx = 列号 (1-based)
//! - Cy = 行号 (1-based)
//!
//! # 示例
//!
//! ```ignore
//! use crate::ui::terminal_view::mouse_report::{MouseReporter, MouseReportEvent};
//!
//! let reporter = MouseReporter::new();
//!
//! // 编码左键按下事件
//! let event = MouseReportEvent::button_press(MouseReportButton::Left, 10, 5);
//! let bytes = reporter.encode_sgr(&event);
//! // 发送 bytes 到远端
//! ```

use super::mouse::{MouseButton, MouseEventType};

// ============================================================================
// 鼠标报告按钮
// ============================================================================

/// 鼠标报告按钮类型
///
/// 用于 SGR 编码的按钮码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReportButton {
    /// 左键 (button code: 0)
    Left,
    /// 中键 (button code: 1)
    Middle,
    /// 右键 (button code: 2)
    Right,
    /// 滚轮向上 (button code: 64)
    WheelUp,
    /// 滚轮向下 (button code: 65)
    WheelDown,
    /// 无按钮（用于移动事件）(button code: 35 for motion)
    None,
}

impl MouseReportButton {
    /// 获取 SGR 按钮码
    ///
    /// 返回用于 SGR 编码的基础按钮码
    pub fn sgr_code(&self) -> u8 {
        match self {
            MouseReportButton::Left => 0,
            MouseReportButton::Middle => 1,
            MouseReportButton::Right => 2,
            MouseReportButton::WheelUp => 64,
            MouseReportButton::WheelDown => 65,
            MouseReportButton::None => 35, // Motion with no button
        }
    }

    /// 从 MouseButton 转换
    pub fn from_mouse_button(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => MouseReportButton::Left,
            MouseButton::Middle => MouseReportButton::Middle,
            MouseButton::Right => MouseReportButton::Right,
        }
    }
}

// ============================================================================
// 鼠标报告动作
// ============================================================================

/// 鼠标报告动作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReportAction {
    /// 按下
    Press,
    /// 释放
    Release,
    /// 移动（按下状态）
    Drag,
    /// 移动（未按下状态）
    Motion,
    /// 滚轮
    Scroll,
}

impl MouseReportAction {
    /// 从 MouseEventType 转换
    pub fn from_event_type(event_type: MouseEventType) -> Self {
        match event_type {
            MouseEventType::Down => MouseReportAction::Press,
            MouseEventType::Up => MouseReportAction::Release,
            MouseEventType::Drag => MouseReportAction::Drag,
            MouseEventType::Move => MouseReportAction::Motion,
            MouseEventType::Scroll => MouseReportAction::Scroll,
        }
    }
}

// ============================================================================
// 修饰键
// ============================================================================

/// 鼠标事件修饰键
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseModifiers {
    /// Shift 键
    pub shift: bool,
    /// Alt/Meta 键
    pub alt: bool,
    /// Ctrl 键
    pub ctrl: bool,
}

impl MouseModifiers {
    /// 创建无修饰键的实例
    pub fn none() -> Self {
        Self::default()
    }

    /// 创建带 Shift 的修饰键
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// 创建带 Alt 的修饰键
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// 创建带 Ctrl 的修饰键
    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// 获取修饰键的 SGR 位掩码
    ///
    /// - Shift: +4
    /// - Alt: +8
    /// - Ctrl: +16
    pub fn sgr_mask(&self) -> u8 {
        let mut mask = 0u8;
        if self.shift {
            mask |= 4;
        }
        if self.alt {
            mask |= 8;
        }
        if self.ctrl {
            mask |= 16;
        }
        mask
    }

    /// 检查是否按下了 Shift 键（用于强制本地选择）
    pub fn has_shift(&self) -> bool {
        self.shift
    }
}

// ============================================================================
// 鼠标报告事件
// ============================================================================

/// 鼠标报告事件
///
/// 包含编码鼠标事件所需的所有信息
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseReportEvent {
    /// 按钮
    pub button: MouseReportButton,
    /// 动作
    pub action: MouseReportAction,
    /// 列号（1-based）
    pub col: u16,
    /// 行号（1-based）
    pub row: u16,
    /// 修饰键
    pub modifiers: MouseModifiers,
}

impl MouseReportEvent {
    /// 创建新的鼠标报告事件
    pub fn new(
        button: MouseReportButton,
        action: MouseReportAction,
        col: u16,
        row: u16,
        modifiers: MouseModifiers,
    ) -> Self {
        Self {
            button,
            action,
            // 确保坐标至少为 1（SGR 使用 1-based 坐标）
            col: col.max(1),
            row: row.max(1),
            modifiers,
        }
    }

    /// 创建按钮按下事件
    pub fn button_press(button: MouseReportButton, col: u16, row: u16) -> Self {
        Self::new(
            button,
            MouseReportAction::Press,
            col,
            row,
            MouseModifiers::none(),
        )
    }

    /// 创建按钮释放事件
    pub fn button_release(button: MouseReportButton, col: u16, row: u16) -> Self {
        Self::new(
            button,
            MouseReportAction::Release,
            col,
            row,
            MouseModifiers::none(),
        )
    }

    /// 创建拖拽事件
    pub fn drag(button: MouseReportButton, col: u16, row: u16) -> Self {
        Self::new(
            button,
            MouseReportAction::Drag,
            col,
            row,
            MouseModifiers::none(),
        )
    }

    /// 创建移动事件
    pub fn motion(col: u16, row: u16) -> Self {
        Self::new(
            MouseReportButton::None,
            MouseReportAction::Motion,
            col,
            row,
            MouseModifiers::none(),
        )
    }

    /// 创建滚轮事件
    pub fn scroll(up: bool, col: u16, row: u16) -> Self {
        let button = if up {
            MouseReportButton::WheelUp
        } else {
            MouseReportButton::WheelDown
        };
        Self::new(
            button,
            MouseReportAction::Scroll,
            col,
            row,
            MouseModifiers::none(),
        )
    }

    /// 添加修饰键
    pub fn with_modifiers(mut self, modifiers: MouseModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// 从 0-based 坐标创建（自动转换为 1-based）
    pub fn from_zero_based(
        button: MouseReportButton,
        action: MouseReportAction,
        col: i32,
        row: i32,
        modifiers: MouseModifiers,
    ) -> Self {
        Self::new(
            button,
            action,
            (col + 1).max(1) as u16,
            (row + 1).max(1) as u16,
            modifiers,
        )
    }
}

// ============================================================================
// 鼠标报告器
// ============================================================================

/// 鼠标报告器
///
/// 负责将鼠标事件编码为转义序列
#[derive(Debug, Clone, Default)]
pub struct MouseReporter {
    /// 当前按下的按钮（用于追踪拖拽状态）
    pressed_button: Option<MouseReportButton>,
}

impl MouseReporter {
    /// 创建新的鼠标报告器
    pub fn new() -> Self {
        Self::default()
    }

    /// 编码鼠标事件为 SGR 格式
    ///
    /// SGR 格式: `ESC [ < Cb ; Cx ; Cy M` (按下) 或 `ESC [ < Cb ; Cx ; Cy m` (释放)
    ///
    /// # 返回
    ///
    /// 编码后的字节序列
    pub fn encode_sgr(&self, event: &MouseReportEvent) -> Vec<u8> {
        let mut button_code = event.button.sgr_code();

        // 添加修饰键掩码
        button_code |= event.modifiers.sgr_mask();

        // 拖拽事件需要设置 bit 5 (32)
        if event.action == MouseReportAction::Drag {
            button_code |= 32;
        }

        // 移动事件（非拖拽）需要设置 bit 5 (32)
        if event.action == MouseReportAction::Motion {
            button_code |= 32;
        }

        // 确定结束字符：M = 按下/移动，m = 释放
        let end_char = match event.action {
            MouseReportAction::Release => 'm',
            _ => 'M',
        };

        // 构建序列: ESC [ < Cb ; Cx ; Cy M/m
        format!(
            "\x1b[<{};{};{}{}",
            button_code, event.col, event.row, end_char
        )
        .into_bytes()
    }

    /// 编码鼠标事件为 X10 格式（兼容旧终端）
    ///
    /// X10 格式: `ESC [ M Cb Cx Cy`
    ///
    /// 注意：X10 格式坐标限制为 223（因为 +32 后不能超过 255）
    ///
    /// # 返回
    ///
    /// 编码后的字节序列，如果坐标超出范围则返回空
    pub fn encode_x10(&self, event: &MouseReportEvent) -> Vec<u8> {
        // X10 格式坐标限制
        if event.col > 223 || event.row > 223 {
            return Vec::new();
        }

        let mut button_code = event.button.sgr_code();

        // 添加修饰键掩码
        button_code |= event.modifiers.sgr_mask();

        // 拖拽/移动事件需要设置 bit 5 (32)
        if event.action == MouseReportAction::Drag || event.action == MouseReportAction::Motion {
            button_code |= 32;
        }

        // 释放事件在 X10 中使用按钮码 3
        if event.action == MouseReportAction::Release {
            button_code = 3 | event.modifiers.sgr_mask();
        }

        // X10 格式需要 +32 偏移
        let cb = button_code + 32;
        let cx = (event.col as u8) + 32;
        let cy = (event.row as u8) + 32;

        vec![0x1b, b'[', b'M', cb, cx, cy]
    }

    /// 记录按钮按下
    pub fn button_pressed(&mut self, button: MouseReportButton) {
        self.pressed_button = Some(button);
    }

    /// 记录按钮释放
    pub fn button_released(&mut self) {
        self.pressed_button = None;
    }

    /// 获取当前按下的按钮
    pub fn pressed_button(&self) -> Option<MouseReportButton> {
        self.pressed_button
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.pressed_button = None;
    }
}

// ============================================================================
// 鼠标模式检测
// ============================================================================

/// 鼠标模式标志
///
/// 对应 Alacritty 的 TermMode 鼠标相关标志
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseModeFlags {
    /// 报告点击 (?1000h)
    pub report_click: bool,
    /// 报告拖拽 (?1002h)
    pub report_drag: bool,
    /// 报告所有移动 (?1003h)
    pub report_motion: bool,
    /// 使用 SGR 编码 (?1006h)
    pub sgr_mode: bool,
}

impl MouseModeFlags {
    /// 创建空的模式标志
    pub fn none() -> Self {
        Self::default()
    }

    /// 检查是否启用了任何鼠标报告模式
    pub fn any_enabled(&self) -> bool {
        self.report_click || self.report_drag || self.report_motion
    }

    /// 检查是否应该报告点击事件
    pub fn should_report_click(&self) -> bool {
        self.report_click || self.report_drag || self.report_motion
    }

    /// 检查是否应该报告拖拽事件
    pub fn should_report_drag(&self) -> bool {
        self.report_drag || self.report_motion
    }

    /// 检查是否应该报告移动事件（非拖拽）
    pub fn should_report_motion(&self) -> bool {
        self.report_motion
    }

    /// 检查是否使用 SGR 编码
    pub fn use_sgr(&self) -> bool {
        self.sgr_mode
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 检查事件是否应该被报告
///
/// 根据鼠标模式和事件类型判断是否需要发送给远端
pub fn should_report_event(
    mode: &MouseModeFlags,
    event: &MouseReportEvent,
    shift_pressed: bool,
) -> bool {
    // Shift 键强制使用本地选择，不报告给远端
    if shift_pressed {
        return false;
    }

    // 检查是否启用了任何鼠标模式
    if !mode.any_enabled() {
        return false;
    }

    // 根据事件类型判断
    match event.action {
        MouseReportAction::Press | MouseReportAction::Release | MouseReportAction::Scroll => {
            mode.should_report_click()
        },
        MouseReportAction::Drag => mode.should_report_drag(),
        MouseReportAction::Motion => mode.should_report_motion(),
    }
}

/// 将 0-based 单元格坐标转换为 1-based SGR 坐标
///
/// # Arguments
///
/// * `col` - 0-based 列号
/// * `row` - 0-based 行号
/// * `max_col` - 最大列数（用于 clamp）
/// * `max_row` - 最大行数（用于 clamp）
///
/// # Returns
///
/// (1-based 列号, 1-based 行号)
pub fn to_sgr_coords(col: i32, row: i32, max_col: i32, max_row: i32) -> (u16, u16) {
    let col = col.clamp(0, max_col - 1) + 1;
    let row = row.clamp(0, max_row - 1) + 1;
    (col as u16, row as u16)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sgr_button_codes() {
        assert_eq!(MouseReportButton::Left.sgr_code(), 0);
        assert_eq!(MouseReportButton::Middle.sgr_code(), 1);
        assert_eq!(MouseReportButton::Right.sgr_code(), 2);
        assert_eq!(MouseReportButton::WheelUp.sgr_code(), 64);
        assert_eq!(MouseReportButton::WheelDown.sgr_code(), 65);
    }

    #[test]
    fn test_modifier_mask() {
        let mods = MouseModifiers::none();
        assert_eq!(mods.sgr_mask(), 0);

        let mods = MouseModifiers::none().with_shift();
        assert_eq!(mods.sgr_mask(), 4);

        let mods = MouseModifiers::none().with_alt();
        assert_eq!(mods.sgr_mask(), 8);

        let mods = MouseModifiers::none().with_ctrl();
        assert_eq!(mods.sgr_mask(), 16);

        let mods = MouseModifiers::none().with_shift().with_ctrl();
        assert_eq!(mods.sgr_mask(), 20);
    }

    #[test]
    fn test_sgr_encoding_press() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::button_press(MouseReportButton::Left, 10, 5);
        let bytes = reporter.encode_sgr(&event);
        let expected = "\x1b[<0;10;5M".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_sgr_encoding_release() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::button_release(MouseReportButton::Left, 10, 5);
        let bytes = reporter.encode_sgr(&event);
        let expected = "\x1b[<0;10;5m".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_sgr_encoding_right_click() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::button_press(MouseReportButton::Right, 20, 15);
        let bytes = reporter.encode_sgr(&event);
        let expected = "\x1b[<2;20;15M".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_sgr_encoding_scroll() {
        let reporter = MouseReporter::new();

        let event = MouseReportEvent::scroll(true, 10, 5);
        let bytes = reporter.encode_sgr(&event);
        let expected = "\x1b[<64;10;5M".as_bytes();
        assert_eq!(bytes, expected);

        let event = MouseReportEvent::scroll(false, 10, 5);
        let bytes = reporter.encode_sgr(&event);
        let expected = "\x1b[<65;10;5M".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_sgr_encoding_drag() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::drag(MouseReportButton::Left, 10, 5);
        let bytes = reporter.encode_sgr(&event);
        // button 0 + drag bit 32 = 32
        let expected = "\x1b[<32;10;5M".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_sgr_encoding_with_modifiers() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::button_press(MouseReportButton::Left, 10, 5)
            .with_modifiers(MouseModifiers::none().with_ctrl());
        let bytes = reporter.encode_sgr(&event);
        // button 0 + ctrl 16 = 16
        let expected = "\x1b[<16;10;5M".as_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_x10_encoding_press() {
        let reporter = MouseReporter::new();
        let event = MouseReportEvent::button_press(MouseReportButton::Left, 10, 5);
        let bytes = reporter.encode_x10(&event);
        // ESC [ M (0+32) (10+32) (5+32) = ESC [ M 32 42 37
        assert_eq!(bytes, vec![0x1b, b'[', b'M', 32, 42, 37]);
    }

    #[test]
    fn test_x10_encoding_overflow() {
        let reporter = MouseReporter::new();
        // 坐标超出 X10 范围
        let event = MouseReportEvent::button_press(MouseReportButton::Left, 300, 5);
        let bytes = reporter.encode_x10(&event);
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_mouse_mode_flags() {
        let mode = MouseModeFlags::none();
        assert!(!mode.any_enabled());
        assert!(!mode.should_report_click());

        let mode = MouseModeFlags {
            report_click: true,
            ..Default::default()
        };
        assert!(mode.any_enabled());
        assert!(mode.should_report_click());
        assert!(!mode.should_report_drag());
        assert!(!mode.should_report_motion());
    }

    #[test]
    fn test_should_report_event() {
        let mode = MouseModeFlags {
            report_click: true,
            sgr_mode: true,
            ..Default::default()
        };

        let event = MouseReportEvent::button_press(MouseReportButton::Left, 10, 5);

        // 正常情况应该报告
        assert!(should_report_event(&mode, &event, false));

        // Shift 按下时不报告
        assert!(!should_report_event(&mode, &event, true));

        // 无模式时不报告
        let no_mode = MouseModeFlags::none();
        assert!(!should_report_event(&no_mode, &event, false));
    }

    #[test]
    fn test_to_sgr_coords() {
        // 正常坐标
        assert_eq!(to_sgr_coords(0, 0, 80, 24), (1, 1));
        assert_eq!(to_sgr_coords(10, 5, 80, 24), (11, 6));

        // 边界 clamp
        assert_eq!(to_sgr_coords(-5, -3, 80, 24), (1, 1));
        assert_eq!(to_sgr_coords(100, 50, 80, 24), (80, 24));
    }

    #[test]
    fn test_from_zero_based() {
        let event = MouseReportEvent::from_zero_based(
            MouseReportButton::Left,
            MouseReportAction::Press,
            9, // 0-based col
            4, // 0-based row
            MouseModifiers::none(),
        );
        assert_eq!(event.col, 10); // 1-based
        assert_eq!(event.row, 5); // 1-based
    }

    #[test]
    fn test_reporter_state() {
        let mut reporter = MouseReporter::new();

        assert!(reporter.pressed_button().is_none());

        reporter.button_pressed(MouseReportButton::Left);
        assert_eq!(reporter.pressed_button(), Some(MouseReportButton::Left));

        reporter.button_released();
        assert!(reporter.pressed_button().is_none());
    }
}
