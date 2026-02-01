//! 窗口 Resize 模块
//!
//! 提供终端窗口尺寸调整功能，包括：
//! - 监听窗口尺寸变化
//! - 计算新的终端行列数
//! - 防抖处理（避免频繁 resize）
//! - 尺寸同步到远端
//!
//! # 示例
//!
//! ```ignore
//! let mut resize_handler = ResizeHandler::new(8.0, 16.0);
//!
//! // 处理尺寸变化
//! if let Some(new_size) = resize_handler.handle_resize(800.0, 600.0) {
//!     // 更新终端尺寸
//!     terminal.resize(new_size.rows, new_size.cols);
//! }
//! ```

use std::time::{Duration, Instant};

use tracing::debug;

/// 默认防抖延迟（毫秒）
pub const DEFAULT_DEBOUNCE_MS: u64 = 50;

/// 最小终端列数
pub const MIN_COLS: u16 = 2;

/// 最小终端行数
pub const MIN_ROWS: u16 = 1;

/// 终端尺寸
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalDimensions {
    /// 行数
    pub rows: u16,
    /// 列数
    pub cols: u16,
}

impl TerminalDimensions {
    /// 创建新的终端尺寸
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows: rows.max(MIN_ROWS),
            cols: cols.max(MIN_COLS),
        }
    }

    /// 默认尺寸 (24x80)
    pub fn default_size() -> Self {
        Self::new(24, 80)
    }

    /// 检查尺寸是否有效
    pub fn is_valid(&self) -> bool {
        self.rows >= MIN_ROWS && self.cols >= MIN_COLS
    }
}

impl Default for TerminalDimensions {
    fn default() -> Self {
        Self::default_size()
    }
}

/// 像素尺寸
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelSize {
    /// 宽度（像素）
    pub width: f32,
    /// 高度（像素）
    pub height: f32,
}

impl PixelSize {
    /// 创建新的像素尺寸
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// 检查尺寸是否有效
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}

impl Default for PixelSize {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// Resize 处理器
///
/// 负责处理窗口尺寸变化，计算新的终端行列数
#[derive(Debug, Clone)]
pub struct ResizeHandler {
    /// 单元格宽度（像素）
    cell_width: f32,
    /// 单元格高度（像素）
    cell_height: f32,
    /// 当前终端尺寸
    current_dimensions: TerminalDimensions,
    /// 当前像素尺寸
    current_pixel_size: PixelSize,
    /// 上次 resize 时间
    last_resize_time: Option<Instant>,
    /// 防抖延迟
    debounce_duration: Duration,
    /// 待处理的尺寸（防抖期间）
    pending_size: Option<PixelSize>,
}

impl ResizeHandler {
    /// 创建新的 Resize 处理器
    pub fn new(cell_width: f32, cell_height: f32) -> Self {
        Self {
            cell_width,
            cell_height,
            current_dimensions: TerminalDimensions::default(),
            current_pixel_size: PixelSize::default(),
            last_resize_time: None,
            debounce_duration: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            pending_size: None,
        }
    }

    /// 设置防抖延迟
    pub fn with_debounce(mut self, duration: Duration) -> Self {
        self.debounce_duration = duration;
        self
    }

    /// 更新单元格尺寸
    pub fn set_cell_size(&mut self, width: f32, height: f32) {
        self.cell_width = width;
        self.cell_height = height;
    }

    /// 获取单元格宽度
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// 获取单元格高度
    pub fn cell_height(&self) -> f32 {
        self.cell_height
    }

    /// 获取当前终端尺寸
    pub fn current_dimensions(&self) -> TerminalDimensions {
        self.current_dimensions
    }

    /// 获取当前像素尺寸
    pub fn current_pixel_size(&self) -> PixelSize {
        self.current_pixel_size
    }

    /// 计算终端尺寸
    ///
    /// 根据像素尺寸和单元格尺寸计算终端行列数
    pub fn calculate_dimensions(&self, width: f32, height: f32) -> TerminalDimensions {
        if self.cell_width <= 0.0 || self.cell_height <= 0.0 {
            return TerminalDimensions::default();
        }

        let cols = (width / self.cell_width).floor() as u16;
        let rows = (height / self.cell_height).floor() as u16;

        TerminalDimensions::new(rows, cols)
    }

    /// 处理尺寸变化（带防抖）
    ///
    /// 返回 Some(dimensions) 如果需要更新终端尺寸
    pub fn handle_resize(&mut self, width: f32, height: f32) -> Option<TerminalDimensions> {
        let new_pixel_size = PixelSize::new(width, height);

        // 检查像素尺寸是否有变化
        if self.is_same_pixel_size(&new_pixel_size) {
            return None;
        }

        let now = Instant::now();

        // 检查是否在防抖期间
        if let Some(last_time) = self.last_resize_time {
            if now.duration_since(last_time) < self.debounce_duration {
                // 在防抖期间，记录待处理的尺寸
                self.pending_size = Some(new_pixel_size);
                return None;
            }
        }

        // 计算新的终端尺寸
        let new_dimensions = self.calculate_dimensions(width, height);

        // 检查终端尺寸是否有变化
        if new_dimensions == self.current_dimensions {
            self.current_pixel_size = new_pixel_size;
            return None;
        }

        // 更新状态
        self.current_pixel_size = new_pixel_size;
        self.current_dimensions = new_dimensions;
        self.last_resize_time = Some(now);
        self.pending_size = None;

        debug!(
            "Terminal resized to {}x{} ({}x{} pixels)",
            new_dimensions.cols, new_dimensions.rows, width, height
        );

        Some(new_dimensions)
    }

    /// 处理尺寸变化（无防抖）
    ///
    /// 立即处理尺寸变化，不进行防抖
    pub fn handle_resize_immediate(
        &mut self,
        width: f32,
        height: f32,
    ) -> Option<TerminalDimensions> {
        let new_pixel_size = PixelSize::new(width, height);
        let new_dimensions = self.calculate_dimensions(width, height);

        // 检查终端尺寸是否有变化
        if new_dimensions == self.current_dimensions {
            self.current_pixel_size = new_pixel_size;
            return None;
        }

        // 更新状态
        self.current_pixel_size = new_pixel_size;
        self.current_dimensions = new_dimensions;
        self.last_resize_time = Some(Instant::now());
        self.pending_size = None;

        Some(new_dimensions)
    }

    /// 检查是否有待处理的 resize
    pub fn has_pending_resize(&self) -> bool {
        self.pending_size.is_some()
    }

    /// 处理待处理的 resize
    ///
    /// 如果防抖期已过且有待处理的尺寸，则处理它
    pub fn flush_pending(&mut self) -> Option<TerminalDimensions> {
        let pending = self.pending_size.take()?;

        // 检查防抖期是否已过
        if let Some(last_time) = self.last_resize_time {
            if Instant::now().duration_since(last_time) < self.debounce_duration {
                self.pending_size = Some(pending);
                return None;
            }
        }

        self.handle_resize_immediate(pending.width, pending.height)
    }

    /// 强制更新尺寸
    ///
    /// 忽略防抖，立即更新到指定尺寸
    pub fn force_resize(&mut self, dimensions: TerminalDimensions) {
        self.current_dimensions = dimensions;
        self.last_resize_time = Some(Instant::now());
        self.pending_size = None;
    }

    /// 检查像素尺寸是否相同（考虑浮点误差）
    fn is_same_pixel_size(&self, other: &PixelSize) -> bool {
        const EPSILON: f32 = 0.5;
        (self.current_pixel_size.width - other.width).abs() < EPSILON
            && (self.current_pixel_size.height - other.height).abs() < EPSILON
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_dimensions() {
        let dims = TerminalDimensions::new(24, 80);
        assert_eq!(dims.rows, 24);
        assert_eq!(dims.cols, 80);
        assert!(dims.is_valid());
    }

    #[test]
    fn test_terminal_dimensions_min() {
        let dims = TerminalDimensions::new(0, 0);
        assert_eq!(dims.rows, MIN_ROWS);
        assert_eq!(dims.cols, MIN_COLS);
    }

    #[test]
    fn test_resize_handler_calculate() {
        let handler = ResizeHandler::new(8.0, 16.0);

        let dims = handler.calculate_dimensions(640.0, 384.0);
        assert_eq!(dims.cols, 80);
        assert_eq!(dims.rows, 24);
    }

    #[test]
    fn test_resize_handler_immediate() {
        let mut handler = ResizeHandler::new(8.0, 16.0);

        // 第一次 resize - 使用不同于默认尺寸的值
        // 默认是 (24, 80)，所以使用 800x480 得到 (30, 100)
        let result = handler.handle_resize_immediate(800.0, 480.0);
        assert!(result.is_some());
        let dims = result.unwrap();
        assert_eq!(dims.cols, 100);
        assert_eq!(dims.rows, 30);

        // 相同尺寸不应触发更新
        let result = handler.handle_resize_immediate(800.0, 480.0);
        assert!(result.is_none());

        // 不同尺寸应触发更新
        let result = handler.handle_resize_immediate(640.0, 384.0);
        assert!(result.is_some());
        let dims = result.unwrap();
        assert_eq!(dims.cols, 80);
        assert_eq!(dims.rows, 24);
    }

    #[test]
    fn test_pixel_size() {
        let size = PixelSize::new(800.0, 600.0);
        assert!(size.is_valid());

        let invalid = PixelSize::new(0.0, 0.0);
        assert!(!invalid.is_valid());
    }
}
