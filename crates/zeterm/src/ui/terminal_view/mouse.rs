//! 鼠标事件处理模块
//!
//! 提供终端鼠标事件处理功能，包括：
//! - 鼠标事件类型定义
//! - 屏幕坐标到终端单元格坐标转换
//! - 点击检测（单击、双击、三击）
//! -鼠标状态跟踪

use std::time::{Duration, Instant};

use gpui::Pixels;

use super::selection::SelectionPoint;

/// 双击时间阈值（毫秒）
pub const DOUBLE_CLICK_THRESHOLD_MS: u64 = 500;

/// 三击时间阈值（毫秒）
pub const TRIPLE_CLICK_THRESHOLD_MS: u64 = 500;

/// 点击距离阈值（像素）
pub const CLICK_DISTANCE_THRESHOLD: f32 = 5.0;

/// 鼠标按钮
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// 左键
    Left,
    /// 中键
    Middle,
    /// 右键
    Right,
}

/// 鼠标事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventType {
    /// 鼠标按下
    Down,
    /// 鼠标释放
    Up,
    /// 鼠标移动
    Move,
    /// 鼠标拖动（按下状态下移动）
    Drag,
    /// 滚轮滚动
    Scroll,
}

/// 点击类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickType {
    /// 单击
    Single,
    /// 双击
    Double,
    /// 三击
    Triple,
}

impl Default for ClickType {
    fn default() -> Self {
        Self::Single
    }
}

/// 鼠标位置（屏幕像素坐标）
#[derive(Debug, Clone, Copy, Default)]
pub struct MousePosition {
    /// X坐标（像素）
    pub x: f32,
    /// Y 坐标（像素）
    pub y: f32,
}

impl MousePosition {
    /// 创建新的鼠标位置
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// 从GPUI Point 创建
    pub fn from_point(point: gpui::Point<Pixels>) -> Self {
        Self {
            x: f32::from(point.x),
            y: f32::from(point.y),
        }
    }

    /// 计算与另一个位置的距离
    pub fn distance_to(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 终端单元格坐标
#[derive(Debug, Clone, Copy, Default)]
pub struct CellPosition {
    /// 行号（0-based）
    pub line: i32,
    /// 列号（0-based）
    pub col: i32,
}

impl CellPosition {
    /// 创建新的单元格位置
    pub fn new(line: i32, col: i32) -> Self {
        Self { line, col }
    }

    /// 转换为 SelectionPoint
    pub fn to_selection_point(&self) -> SelectionPoint {
        SelectionPoint::new(self.line, self.col)
    }
}

/// 坐标转换器
///
/// 负责将屏幕像素坐标转换为终端单元格坐标
#[derive(Debug, Clone, Copy)]
pub struct CoordinateConverter {
    /// 终端区域原点 X（像素）
    pub origin_x: f32,
    /// 终端区域原点 Y（像素）
    pub origin_y: f32,
    /// 单元格宽度（像素）
    pub cell_width: f32,
    /// 单元格高度（像素）
    pub cell_height: f32,
    /// 终端列数
    pub cols: i32,
    /// 终端行数
    pub rows: i32,
    /// 滚动偏移量（行数）
    pub scroll_offset: i32,
}

impl CoordinateConverter {
    /// 创建新的坐标转换器
    pub fn new(
        origin_x: f32,
        origin_y: f32,
        cell_width: f32,
        cell_height: f32,
        cols: i32,
        rows: i32,
    ) -> Self {
        Self {
            origin_x,
            origin_y,
            cell_width,
            cell_height,
            cols,
            rows,
            scroll_offset: 0,
        }
    }

    /// 设置滚动偏移量
    pub fn with_scroll_offset(mut self, offset: i32) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// 将屏幕坐标转换为单元格坐标
    pub fn screen_to_cell(&self, pos: MousePosition) -> CellPosition {
        let relative_x = pos.x - self.origin_x;
        let relative_y = pos.y - self.origin_y;

        let col = (relative_x / self.cell_width).floor() as i32;
        let line = (relative_y / self.cell_height).floor() as i32;

        // 限制在有效范围内
        let col = col.clamp(0, self.cols - 1);
        let line = line.clamp(0, self.rows - 1);

        // 考虑滚动偏移
        let adjusted_line = line - self.scroll_offset;

        CellPosition::new(adjusted_line, col)
    }

    /// 将单元格坐标转换为屏幕坐标（单元格左上角）
    pub fn cell_to_screen(&self, cell: CellPosition) -> MousePosition {
        let adjusted_line = cell.line + self.scroll_offset;
        let x = self.origin_x + (cell.col as f32) * self.cell_width;
        let y = self.origin_y + (adjusted_line as f32) * self.cell_height;
        MousePosition::new(x, y)
    }

    /// 检查屏幕坐标是否在终端区域内
    pub fn is_in_bounds(&self, pos: MousePosition) -> bool {
        let max_x = self.origin_x + (self.cols as f32) * self.cell_width;
        let max_y = self.origin_y + (self.rows as f32) * self.cell_height;

        pos.x >= self.origin_x && pos.x < max_x && pos.y >= self.origin_y && pos.y < max_y
    }
}

/// 点击检测器
///
/// 用于检测单击、双击、三击
#[derive(Debug, Clone)]
pub struct ClickDetector {
    /// 上次点击时间
    last_click_time: Option<Instant>,
    /// 上上次点击时间（用于三击检测）
    prev_click_time: Option<Instant>,
    /// 上次点击位置
    last_click_pos: Option<MousePosition>,
    /// 当前点击次数
    click_count: u32,
}

impl Default for ClickDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickDetector {
    /// 创建新的点击检测器
    pub fn new() -> Self {
        Self {
            last_click_time: None,
            prev_click_time: None,
            last_click_pos: None,
            click_count: 0,
        }
    }

    /// 记录点击并返回点击类型
    pub fn record_click(&mut self, pos: MousePosition) -> ClickType {
        let now = Instant::now();

        // 检查是否在时间和距离阈值内
        let is_continuation = self.is_continuation_click(now, pos);

        if is_continuation {
            self.click_count += 1;
            if self.click_count > 3 {
                self.click_count = 1;
            }
        } else {
            self.click_count = 1;
        }

        // 更新状态
        self.prev_click_time = self.last_click_time;
        self.last_click_time = Some(now);
        self.last_click_pos = Some(pos);

        match self.click_count {
            1 => ClickType::Single,
            2 => ClickType::Double,
            _ => ClickType::Triple,
        }
    }

    /// 检查是否是连续点击
    fn is_continuation_click(&self, now: Instant, pos: MousePosition) -> bool {
        if let (Some(last_time), Some(last_pos)) = (self.last_click_time, self.last_click_pos) {
            let time_diff = now.duration_since(last_time);
            let threshold = if self.click_count >= 2 {
                Duration::from_millis(TRIPLE_CLICK_THRESHOLD_MS)
            } else {
                Duration::from_millis(DOUBLE_CLICK_THRESHOLD_MS)
            };

            time_diff < threshold && pos.distance_to(&last_pos) < CLICK_DISTANCE_THRESHOLD
        } else {
            false
        }
    }

    /// 重置点击状态
    pub fn reset(&mut self) {
        self.last_click_time = None;
        self.prev_click_time = None;
        self.last_click_pos = None;
        self.click_count = 0;
    }

    /// 获取当前点击次数
    pub fn click_count(&self) -> u32 {
        self.click_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_position_distance() {
        let p1 = MousePosition::new(0.0, 0.0);
        let p2 = MousePosition::new(3.0, 4.0);

        assert!((p1.distance_to(&p2) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_coordinate_converter_screen_to_cell() {
        let converter = CoordinateConverter::new(10.0, 10.0, 8.0, 16.0, 80, 24);

        // 原点位置
        let cell = converter.screen_to_cell(MousePosition::new(10.0, 10.0));
        assert_eq!(cell.line, 0);
        assert_eq!(cell.col, 0);

        // 第二个单元格
        let cell = converter.screen_to_cell(MousePosition::new(18.0, 26.0));
        assert_eq!(cell.line, 1);
        assert_eq!(cell.col, 1);
    }

    #[test]
    fn test_coordinate_converter_bounds_check() {
        let converter = CoordinateConverter::new(10.0, 10.0, 8.0, 16.0, 80, 24);

        assert!(converter.is_in_bounds(MousePosition::new(15.0, 15.0)));
        assert!(!converter.is_in_bounds(MousePosition::new(5.0, 5.0)));
        assert!(!converter.is_in_bounds(MousePosition::new(1000.0, 1000.0)));
    }

    #[test]
    fn test_click_detector_single_click() {
        let mut detector = ClickDetector::new();
        let pos = MousePosition::new(100.0, 100.0);

        let click_type = detector.record_click(pos);
        assert_eq!(click_type, ClickType::Single);
    }
}
