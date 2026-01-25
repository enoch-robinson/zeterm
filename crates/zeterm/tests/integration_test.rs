//!集成测试: Mock → Alacritty 数据流
//!
//! 测试 MockConnection 到 TerminalState 的数据流转。

use std::time::Duration;

use futures::StreamExt;
use tokio::time::timeout;

use zeterm_core::traits::{ConnectionInfo, TerminalConnection};
use zeterm_mock::{MockConfig, MockConnection};

/// 测试 MockConnection 基本数据流
#[tokio::test]
async fn test_mock_connection_data_flow() {
    // 创建 MockConnection（禁用自动输出）
    let conn = MockConnection::new(MockConfig {
        auto_output_interval_ms: None,
        echo_input: false,
        ..Default::default()
    });

    // 连接
    conn.connect().await.expect("Failed to connect");
    assert!(conn.is_connected());

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 注入测试数据
    conn.inject_output(b"Hello, World!")
        .await
        .expect("Failed to inject");

    // 接收数据（带超时）
    let result = timeout(Duration::from_secs(1), stream.next()).await;
    assert!(result.is_ok(), "Timeout waiting for data");

    let data = result.unwrap().expect("Stream ended");
    assert!(data.is_ok(), "Data error");

    // 关闭连接
    conn.close().await.expect("Failed to close");
    assert!(!conn.is_connected());
}

/// 测试 MockConnection 回显功能
#[tokio::test]
async fn test_mock_connection_echo() {
    let conn = MockConnection::new(MockConfig {
        auto_output_interval_ms: None,
        echo_input: true,
        ..Default::default()
    });

    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    //跳过欢迎消息
    let _ = timeout(Duration::from_millis(100), stream.next()).await;

    // 写入数据
    conn.write(b"test input").await.expect("Failed to write");

    // 接收回显
    let result = timeout(Duration::from_secs(1), stream.next()).await;
    assert!(result.is_ok(), "Timeout waiting for echo");

    let data = result.unwrap().expect("Stream ended").expect("Data error");
    assert_eq!(data, b"test input");

    conn.close().await.expect("Failed to close");
}

/// 测试 MockConnection resize 功能
#[tokio::test]
async fn test_mock_connection_resize() {
    let conn = MockConnection::new(MockConfig {
        auto_output_interval_ms: None,
        ..Default::default()
    });

    conn.connect().await.expect("Failed to connect");

    // 调整大小
    conn.resize(40, 120).await.expect("Failed to resize");

    conn.close().await.expect("Failed to close");
}

/// 测试 ANSI 彩色文本注入
#[tokio::test]
async fn test_mock_connection_colored_text() {
    let conn = MockConnection::new(MockConfig {
        auto_output_interval_ms: None,
        echo_input: false,
        ..Default::default()
    });

    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    // 跳过欢迎消息
    let _ = timeout(Duration::from_millis(100), stream.next()).await;

    // 注入红色文本 (color code 31)
    conn.inject_colored_text("Red Text", 31)
        .await
        .expect("Failed to inject colored text");

    // 接收数据
    let result = timeout(Duration::from_secs(1), stream.next()).await;
    assert!(result.is_ok(), "Timeout waiting for colored text");

    let data = result.unwrap().expect("Stream ended").expect("Data error");
    let text = String::from_utf8_lossy(&data);

    // 验证包含 ANSI 转义序列
    assert!(text.contains("\x1b[31m"), "Missing color start");
    assert!(text.contains("Red Text"), "Missing text content");
    assert!(text.contains("\x1b[0m"), "Missing color reset");

    conn.close().await.expect("Failed to close");
}

// ============================================================================
// RenderConfig 和主题集成测试
// ============================================================================

/// 测试 RenderConfig 默认值
#[test]
fn test_render_config_default() {
    use zeterm::ui::terminal_view::{CursorStyle, RenderConfig};

    let config = RenderConfig::default();

    assert_eq!(config.font_size, 14.0);
    assert_eq!(config.line_height, 1.2);
    assert_eq!(config.cursor_style, CursorStyle::Block);
    assert!(config.cursor_blink);
    assert_eq!(config.scrollback_lines, 10000);
    assert!(config.show_scrollbar);
    assert!(config.theme.is_dark);
}

/// 测试 RenderConfig Builder 模式
#[test]
fn test_render_config_builder() {
    use zeterm::ui::terminal_view::{CursorStyle, RenderConfig, TerminalTheme};

    let config = RenderConfig::new()
        .with_font_size(16.0)
        .with_line_height(1.5)
        .with_cursor_style(CursorStyle::Beam)
        .with_theme(TerminalTheme::light());

    assert_eq!(config.font_size, 16.0);
    assert_eq!(config.line_height, 1.5);
    assert_eq!(config.cursor_style, CursorStyle::Beam);
    assert!(!config.theme.is_dark);
}

/// 测试字体大小限制
#[test]
fn test_render_config_font_size_limits() {
    use zeterm::ui::terminal_view::RenderConfig;

    // 测试最小值限制
    let config = RenderConfig::new().with_font_size(4.0);
    assert_eq!(config.font_size, 8.0); // 应该被限制到最小值 8.0

    // 测试最大值限制
    let config = RenderConfig::new().with_font_size(100.0);
    assert_eq!(config.font_size, 72.0); // 应该被限制到最大值 72.0

    // 测试正常值
    let config = RenderConfig::new().with_font_size(20.0);
    assert_eq!(config.font_size, 20.0);
}

/// 测试字体大小调整方法
#[test]
fn test_render_config_font_size_adjustment() {
    use zeterm::ui::terminal_view::RenderConfig;

    let mut config = RenderConfig::new();
    let initial_size = config.font_size;

    // 增大字体
    config.increase_font_size();
    assert_eq!(config.font_size, initial_size + 1.0);

    // 减小字体
    config.decrease_font_size();
    assert_eq!(config.font_size, initial_size);

    // 重置字体大小
    config.font_size = 20.0;
    config.reset_font_size();
    assert_eq!(config.font_size, 14.0);
}

/// 测试主题颜色
#[test]
fn test_terminal_theme_colors() {
    use zeterm::ui::terminal_view::TerminalTheme;

    let dark = TerminalTheme::dark();
    let light = TerminalTheme::light();

    // 暗色主题背景应该较暗
    assert!(dark.background().r < 100);
    assert!(dark.background().g < 100);
    assert!(dark.background().b < 100);

    // 亮色主题背景应该较亮
    assert!(light.background().r > 200);
    assert!(light.background().g > 200);
    assert!(light.background().b > 200);

    // 验证光标颜色存在
    assert!(dark.cursor_color().r > 0 || dark.cursor_color().g > 0 || dark.cursor_color().b > 0);
}

/// 测试多种预设主题
#[test]
fn test_terminal_theme_presets() {
    use zeterm::ui::terminal_view::TerminalTheme;

    let themes = vec![
        TerminalTheme::dark(),
        TerminalTheme::light(),
        TerminalTheme::dracula(),
        TerminalTheme::one_dark(),
        TerminalTheme::solarized_dark(),
    ];

    for theme in themes {
        // 每个主题都应该有有效的名称
        assert!(!theme.name.is_empty());

        // 每个主题都应该有有效的颜色
        let bg = theme.background();
        let fg = theme.foreground();
        let cursor = theme.cursor_color();

        // 背景和前景应该不同（有对比度）
        assert!(
            bg.r != fg.r || bg.g != fg.g || bg.b != fg.b,
            "Theme {} should have different foreground and background",
            theme.name
        );

        // 光标颜色应该与背景有对比度（而不是简单检查非零，因为黑色光标在浅色背景上是有效的）
        let has_contrast = (cursor.r as i32 - bg.r as i32).abs() > 30
            || (cursor.g as i32 - bg.g as i32).abs() > 30
            || (cursor.b as i32 - bg.b as i32).abs() > 30;
        assert!(
            has_contrast,
            "Theme {} should have cursor with contrast against background",
            theme.name
        );
    }
}

/// 测试主题管理器
#[test]
fn test_theme_manager() {
    use zeterm::ui::terminal_view::ThemeManager;

    let mut manager = ThemeManager::new();

    // 默认应该是暗色主题
    assert!(manager.current().is_dark);

    // 切换到亮色主题
    manager.toggle_dark_light();
    assert!(!manager.current().is_dark);

    // 再次切换回暗色
    manager.toggle_dark_light();
    assert!(manager.current().is_dark);

    // 按名称切换
    assert!(manager.switch_by_name("Dracula"));
    assert_eq!(manager.current().name, "Dracula");

    // 无效名称应该返回 false
    assert!(!manager.switch_by_name("NonExistent"));
}

/// 测试 RGB 到 HSLA 转换
#[test]
fn test_rgb_to_hsla_conversion() {
    use zeterm::ui::terminal_view::{Rgb, rgb_to_hsla};

    // 测试纯红色
    let red = Rgb::new(255, 0, 0);
    let hsla = rgb_to_hsla(red);
    assert!((hsla.h - 0.0).abs() < 0.01 || (hsla.h - 1.0).abs() < 0.01); // 红色 hue = 0
    assert!((hsla.s - 1.0).abs() < 0.01); // 饱和度 = 1
    assert!((hsla.l - 0.5).abs() < 0.01); // 亮度 = 0.5

    // 测试纯绿色
    let green = Rgb::new(0, 255, 0);
    let hsla = rgb_to_hsla(green);
    assert!((hsla.h - 0.333).abs() < 0.02); // 绿色 hue≈ 0.33(120°/360°)

    // 测试纯蓝色
    let blue = Rgb::new(0, 0, 255);
    let hsla = rgb_to_hsla(blue);
    assert!((hsla.h - 0.666).abs() < 0.02); // 蓝色 hue ≈ 0.67 (240°/360°)

    // 测试白色
    let white = Rgb::new(255, 255, 255);
    let hsla = rgb_to_hsla(white);
    assert!((hsla.l - 1.0).abs() < 0.01); // 亮度 = 1

    // 测试黑色
    let black = Rgb::new(0, 0, 0);
    let hsla = rgb_to_hsla(black);
    assert!((hsla.l - 0.0).abs() < 0.01); // 亮度 = 0
}

/// 测试选择颜色配置
#[test]
fn test_selection_colors() {
    use zeterm::ui::terminal_view::TerminalTheme;

    let dark = TerminalTheme::dark();
    let light = TerminalTheme::light();

    //暗色主题选择背景应该较暗但可见
    let dark_sel = dark.selection_background();
    assert!(dark_sel.r > 0 || dark_sel.g > 0 || dark_sel.b > 0);

    // 亮色主题选择背景应该较亮
    let light_sel = light.selection_background();
    assert!(light_sel.r > 100 || light_sel.g > 100 || light_sel.b > 100);
}
