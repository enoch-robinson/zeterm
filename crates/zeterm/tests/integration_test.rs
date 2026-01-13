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
