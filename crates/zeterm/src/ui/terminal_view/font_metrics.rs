//! 字体度量模块
//!
//! 计算终端字体的单元格尺寸，用于终端渲染布局。

use gpui::{Pixels, px};
use gpui_component::PixelsExt;

/// 默认终端字体大小
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// 默认行高倍数
pub const DEFAULT_LINE_HEIGHT: f32 = 1.2;

/// 默认字符宽度比例 (相对于字体大小)
pub const DEFAULT_CHAR_WIDTH_RATIO: f32 = 0.6;

/// 最小字体大小
pub const MIN_FONT_SIZE: f32 = 8.0;

/// 最大字体大小
pub const MAX_FONT_SIZE: f32 = 72.0;

/// 字体度量信息
///
/// 包含终端渲染所需的所有字体尺寸信息
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// 单元格宽度
    pub cell_width: Pixels,
    /// 单元格高度 (行高)
    pub cell_height: Pixels,
    /// 字体大小
    pub font_size: Pixels,
    /// 基线偏移 (从单元格顶部到基线的距离)
    pub baseline_offset: Pixels,
    /// 下划线位置 (从基线向下的偏移)
    pub underline_position: Pixels,
    /// 下划线粗细
    pub underline_thickness: Pixels,
    /// 删除线位置 (从基线向上的偏移)
    pub strikethrough_position: Pixels,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self::from_font_size(DEFAULT_FONT_SIZE)
    }
}

impl FontMetrics {
    /// 从字体大小创建字体度量
    pub fn from_font_size(font_size: f32) -> Self {
        Self::from_font_size_and_line_height(font_size, DEFAULT_LINE_HEIGHT)
    }

    /// 从字体大小和行高创建字体度量
    pub fn from_font_size_and_line_height(font_size: f32, line_height: f32) -> Self {
        let font_size = clamp_font_size(font_size);
        let cell_width = calculate_cell_width(font_size);
        let cell_height = calculate_cell_height(font_size, line_height);
        let baseline_offset = calculate_baseline_offset(font_size, cell_height);
        let underline_position = calculate_underline_position(font_size);
        let underline_thickness = calculate_underline_thickness(font_size);
        let strikethrough_position = calculate_strikethrough_position(font_size);

        Self {
            cell_width: px(cell_width),
            cell_height: px(cell_height),
            font_size: px(font_size),
            baseline_offset: px(baseline_offset),
            underline_position: px(underline_position),
            underline_thickness: px(underline_thickness),
            strikethrough_position: px(strikethrough_position),
        }
    }

    /// 从实际测量的字符宽度创建字体度量
    pub fn from_measured_width(font_size: f32, measured_width: f32, line_height: f32) -> Self {
        let font_size = clamp_font_size(font_size);
        let cell_width = measured_width;
        let cell_height = calculate_cell_height(font_size, line_height);
        let baseline_offset = calculate_baseline_offset(font_size, cell_height);
        let underline_position = calculate_underline_position(font_size);
        let underline_thickness = calculate_underline_thickness(font_size);
        let strikethrough_position = calculate_strikethrough_position(font_size);

        Self {
            cell_width: px(cell_width),
            cell_height: px(cell_height),
            font_size: px(font_size),
            baseline_offset: px(baseline_offset),
            underline_position: px(underline_position),
            underline_thickness: px(underline_thickness),
            strikethrough_position: px(strikethrough_position),
        }
    }

    /// 计算给定列数和行数所需的尺寸
    pub fn calculate_size(&self, cols: usize, rows: usize) -> (Pixels, Pixels) {
        let width = px(self.cell_width.as_f32() * cols as f32);
        let height = px(self.cell_height.as_f32() * rows as f32);
        (width, height)
    }

    /// 从像素尺寸计算可容纳的列数和行数
    pub fn calculate_grid_size(&self, width: Pixels, height: Pixels) -> (usize, usize) {
        let cols = (width.as_f32() / self.cell_width.as_f32()).floor() as usize;
        let rows = (height.as_f32() / self.cell_height.as_f32()).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    /// 计算单元格位置 (左上角坐标)
    pub fn cell_position(&self, col: usize, row: usize) -> (Pixels, Pixels) {
        let x = px(self.cell_width.as_f32() * col as f32);
        let y = px(self.cell_height.as_f32() * row as f32);
        (x, y)
    }

    /// 计算文本基线位置
    pub fn text_baseline(&self, row: usize) -> Pixels {
        let cell_top = self.cell_height.as_f32() * row as f32;
        px(cell_top + self.baseline_offset.as_f32())
    }
}

/// 限制字体大小在有效范围内
pub fn clamp_font_size(font_size: f32) -> f32 {
    font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

/// 计算单元格宽度
pub fn calculate_cell_width(font_size: f32) -> f32 {
    font_size * DEFAULT_CHAR_WIDTH_RATIO
}

/// 计算单元格高度
pub fn calculate_cell_height(font_size: f32, line_height: f32) -> f32 {
    font_size * line_height
}

/// 计算基线偏移
///
/// 基线通常位于单元格底部向上约 20% 的位置
pub fn calculate_baseline_offset(font_size: f32, cell_height: f32) -> f32 {
    // 基线位置 = 单元格高度 - 下降部分
    // 下降部分约为字体大小的 20%
    let descent = font_size * 0.2;
    cell_height - descent
}

/// 计算下划线位置
pub fn calculate_underline_position(font_size: f32) -> f32 {
    font_size * 0.1
}

/// 计算下划线粗细
pub fn calculate_underline_thickness(font_size: f32) -> f32 {
    (font_size * 0.07).max(1.0)
}

/// 计算删除线位置
pub fn calculate_strikethrough_position(font_size: f32) -> f32 {
    font_size * 0.35
}

#[cfg(test)]
mod tests {
    use super::*;

    //==================== 基础创建测试 ====================

    #[test]
    fn test_default_font_metrics() {
        let metrics = FontMetrics::default();
        assert_eq!(metrics.font_size.as_f32(), DEFAULT_FONT_SIZE);
    }

    #[test]
    fn test_from_font_size() {
        let metrics = FontMetrics::from_font_size(16.0);
        assert_eq!(metrics.font_size.as_f32(), 16.0);
    }

    #[test]
    fn test_from_font_size_and_line_height() {
        let metrics = FontMetrics::from_font_size_and_line_height(14.0, 1.5);
        assert_eq!(metrics.font_size.as_f32(), 14.0);
        assert_eq!(metrics.cell_height.as_f32(), 14.0 * 1.5);
    }

    // ==================== 字体大小限制测试 ====================

    #[test]
    fn test_clamp_font_size_normal() {
        assert_eq!(clamp_font_size(14.0), 14.0);
        assert_eq!(clamp_font_size(20.0), 20.0);
    }

    #[test]
    fn test_clamp_font_size_too_small() {
        assert_eq!(clamp_font_size(4.0), MIN_FONT_SIZE);
        assert_eq!(clamp_font_size(0.0), MIN_FONT_SIZE);
        assert_eq!(clamp_font_size(-10.0), MIN_FONT_SIZE);
    }

    #[test]
    fn test_clamp_font_size_too_large() {
        assert_eq!(clamp_font_size(100.0), MAX_FONT_SIZE);
        assert_eq!(clamp_font_size(1000.0), MAX_FONT_SIZE);
    }

    // ==================== 单元格尺寸计算测试 ====================

    #[test]
    fn test_calculate_cell_width() {
        let width = calculate_cell_width(14.0);
        assert_eq!(width, 14.0 * DEFAULT_CHAR_WIDTH_RATIO);
    }

    #[test]
    fn test_calculate_cell_height() {
        let height = calculate_cell_height(14.0, 1.2);
        assert_eq!(height, 14.0 * 1.2);
    }

    #[test]
    fn test_cell_dimensions_consistency() {
        let metrics = FontMetrics::from_font_size(14.0);

        // 单元格宽度应该小于高度 (等宽字体特性)
        assert!(metrics.cell_width.as_f32() < metrics.cell_height.as_f32());

        // 单元格宽度应该约为字体大小的 60%
        let expected_width = 14.0 * DEFAULT_CHAR_WIDTH_RATIO;
        assert!((metrics.cell_width.as_f32() - expected_width).abs() < 0.001);
    }

    // ==================== 基线计算测试 ====================

    #[test]
    fn test_calculate_baseline_offset() {
        let font_size = 14.0;
        let cell_height = font_size * DEFAULT_LINE_HEIGHT;
        let baseline = calculate_baseline_offset(font_size, cell_height);

        // 基线应该在单元格内
        assert!(baseline > 0.0);
        assert!(baseline < cell_height);
        // 基线应该接近单元格底部
        assert!(baseline > cell_height * 0.7);
    }

    #[test]
    fn test_text_baseline() {
        let metrics = FontMetrics::from_font_size(14.0);
        let baseline_row_0 = metrics.text_baseline(0);
        let baseline_row_1 = metrics.text_baseline(1);

        // 第二行的基线应该比第一行高一个单元格高度
        let diff = baseline_row_1.as_f32() - baseline_row_0.as_f32();
        assert!((diff - metrics.cell_height.as_f32()).abs() < 0.001);
    }

    // ==================== 下划线和删除线测试 ====================

    #[test]
    fn test_underline_position() {
        let pos = calculate_underline_position(14.0);
        assert!(pos > 0.0);
        assert!(pos < 14.0 * 0.5); // 应该在字体大小的一半以内
    }

    #[test]
    fn test_underline_thickness() {
        let thickness = calculate_underline_thickness(14.0);
        assert!(thickness >= 1.0); // 至少 1 像素
        assert!(thickness < 14.0 * 0.2); // 不应该太粗
    }

    #[test]
    fn test_strikethrough_position() {
        let pos = calculate_strikethrough_position(14.0);
        // 删除线应该在字体中间偏上
        assert!(pos > 14.0 * 0.2);
        assert!(pos < 14.0 * 0.5);
    }

    // ==================== 网格尺寸计算测试 ====================

    #[test]
    fn test_calculate_size() {
        let metrics = FontMetrics::from_font_size(14.0);
        let (width, height) = metrics.calculate_size(80, 24);

        assert_eq!(width.as_f32(), metrics.cell_width.as_f32() * 80.0);
        assert_eq!(height.as_f32(), metrics.cell_height.as_f32() * 24.0);
    }

    #[test]
    fn test_calculate_grid_size() {
        let metrics = FontMetrics::from_font_size(14.0);

        // 计算 80x24 终端所需的像素尺寸
        let (width, height) = metrics.calculate_size(80, 24);

        // 反向计算应该得到相同的列数和行数
        let (cols, rows) = metrics.calculate_grid_size(width, height);
        assert_eq!(cols, 80);
        assert_eq!(rows, 24);
    }

    #[test]
    fn test_calculate_grid_size_minimum() {
        let metrics = FontMetrics::from_font_size(14.0);

        // 即使空间很小，也应该至少有 1x1
        let (cols, rows) = metrics.calculate_grid_size(px(1.0), px(1.0));
        assert_eq!(cols, 1);
        assert_eq!(rows, 1);
    }

    #[test]
    fn test_calculate_grid_size_partial() {
        let metrics = FontMetrics::from_font_size(14.0);

        // 测试部分单元格的情况 (应该向下取整)
        let width = px(metrics.cell_width.as_f32() * 80.5);
        let height = px(metrics.cell_height.as_f32() * 24.9);

        let (cols, rows) = metrics.calculate_grid_size(width, height);
        assert_eq!(cols, 80);
        assert_eq!(rows, 24);
    }

    // ==================== 单元格位置测试 ====================

    #[test]
    fn test_cell_position_origin() {
        let metrics = FontMetrics::from_font_size(14.0);
        let (x, y) = metrics.cell_position(0, 0);

        assert_eq!(x.as_f32(), 0.0);
        assert_eq!(y.as_f32(), 0.0);
    }

    #[test]
    fn test_cell_position() {
        let metrics = FontMetrics::from_font_size(14.0);
        let (x, y) = metrics.cell_position(10, 5);

        assert_eq!(x.as_f32(), metrics.cell_width.as_f32() * 10.0);
        assert_eq!(y.as_f32(), metrics.cell_height.as_f32() * 5.0);
    }

    // ==================== 从测量宽度创建测试 ====================

    #[test]
    fn test_from_measured_width() {
        let measured_width = 9.5; // 假设实际测量的字符宽度
        let metrics = FontMetrics::from_measured_width(14.0, measured_width, 1.2);

        assert_eq!(metrics.cell_width.as_f32(), measured_width);
        assert_eq!(metrics.font_size.as_f32(), 14.0);
    }

    // ==================== 边界情况测试 ====================

    #[test]
    fn test_zero_grid_size() {
        let metrics = FontMetrics::from_font_size(14.0);
        let (width, height) = metrics.calculate_size(0, 0);

        assert_eq!(width.as_f32(), 0.0);
        assert_eq!(height.as_f32(), 0.0);
    }

    #[test]
    fn test_large_grid_size() {
        let metrics = FontMetrics::from_font_size(14.0);
        let (width, height) = metrics.calculate_size(1000, 500);

        // 确保没有溢出
        assert!(width.as_f32() > 0.0);
        assert!(height.as_f32() > 0.0);
        assert!(width.as_f32().is_finite());
        assert!(height.as_f32().is_finite());
    }
}
