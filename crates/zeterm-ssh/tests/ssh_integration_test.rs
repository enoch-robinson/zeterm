//! SSH 集成测试
//!
//! 测试 SSH 连接功能的集成测试。
//!
//! ## 测试分类
//!
//! - **基础测试**: 不需要真实服务器，直接运行
//! - **集成测试**: 需要真实 SSH 服务器，默认被`#[ignore]` 标记
//!
//! ## 运行密码认证测试
//!
//! 设置环境变量后运行被忽略的测试：
//!
//! ```bash
//! SSH_TEST_HOST=your-server \
//! SSH_TEST_PORT=22 \
//! SSH_TEST_USER=your-user \
//! SSH_TEST_PASSWORD=your-pass \
//! cargo test -p zeterm-ssh --test ssh_integration_test -- --ignored
//! ```
//!
//! ## 运行公钥认证测试
//!
//! ```bash
//! SSH_TEST_HOST=your-server \
//! SSH_TEST_PORT=22 \
//! SSH_TEST_USER=your-user \
//! SSH_TEST_KEY_PATH=~/.ssh/id_rsa \
//! cargo test -p zeterm-ssh --test ssh_integration_test test_ssh_pubkey -- --ignored
//! ```
//!
//! 如果私钥有密码保护：
//!
//! ```bash
//! SSH_TEST_HOST=your-server \
//! SSH_TEST_USER=your-user \
//! SSH_TEST_KEY_PATH=~/.ssh/id_rsa \
//! SSH_TEST_KEY_PASSPHRASE=your-key-passphrase \
//! cargo test -p zeterm-ssh --test ssh_integration_test test_ssh_pubkey -- --ignored
//! ```
//!
//! ## 环境变量说明
//!
//! | 变量 | 说明 | 默认值 |
//! |------|------|--------|
//! | `SSH_TEST_HOST` | SSH 服务器地址 | localhost |
//! | `SSH_TEST_PORT` | SSH 端口 | 22 |
//! | `SSH_TEST_USER` | 用户名 | testuser |
//! | `SSH_TEST_PASSWORD` | 密码（密码认证） | testpass |
//! | `SSH_TEST_KEY_PATH` |私钥文件路径（公钥认证） | ~/.ssh/id_rsa |
//! | `SSH_TEST_KEY_PASSPHRASE` | 私钥密码（可选） | 无 |
//!
//! ## 运行所有测试
//!
//! ```bash
//! cargo test -p zeterm-ssh --test ssh_integration_test -- --include-ignored
//! ```

use std::env;
use std::time::Duration;

use futures::StreamExt;
use tokio::time::timeout;

use zeterm_core::traits::{ConnectionInfo, ConnectionType, TerminalConnection};
use zeterm_ssh::{AuthMethod, HostKeyVerification, SshConfig, SshConnection};

/// 获取测试配置
///
/// 从环境变量读取 SSH 连接配置
fn get_test_config() -> SshConfig {
    let host = env::var("SSH_TEST_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = env::var("SSH_TEST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(22);
    let username = env::var("SSH_TEST_USER").unwrap_or_else(|_| "testuser".to_string());
    let password = env::var("SSH_TEST_PASSWORD").unwrap_or_else(|_| "testpass".to_string());

    SshConfig::new(host, username)
        .with_port(port)
        .with_password(password)
        .with_host_key_verification(HostKeyVerification::AutoAccept)
        .with_terminal_size(80, 24)
}

/// 获取公钥认证测试配置
///
/// 从环境变量读取 SSH 公钥认证配置
fn get_pubkey_test_config() -> SshConfig {
    let host = env::var("SSH_TEST_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = env::var("SSH_TEST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(22);
    let username = env::var("SSH_TEST_USER").unwrap_or_else(|_| "testuser".to_string());
    let key_path = env::var("SSH_TEST_KEY_PATH")
        .unwrap_or_else(|_| format!("{}/.ssh/id_rsa", env::var("HOME").unwrap_or_default()));
    let key_passphrase = env::var("SSH_TEST_KEY_PASSPHRASE").ok();

    let mut config = SshConfig::new(host, username)
        .with_port(port)
        .with_host_key_verification(HostKeyVerification::AutoAccept)
        .with_terminal_size(80, 24);

    // 根据是否有密码短语选择配置方法
    if let Some(passphrase) = key_passphrase {
        config = config.with_key_file_and_passphrase(&key_path, passphrase);
    } else {
        config = config.with_key_file(&key_path);
    }

    config
}

//==================== 基础测试 (不需要真实服务器) ====================

#[test]
fn test_ssh_config_creation() {
    let config = SshConfig::new("example.com", "testuser")
        .with_port(2222)
        .with_password("testpass")
        .with_terminal_size(120, 40);

    assert_eq!(config.host, "example.com");
    assert_eq!(config.username, "testuser");
    assert_eq!(config.port, 2222);
    assert_eq!(config.terminal_cols, 120);
    assert_eq!(config.terminal_rows, 40);
    assert!(matches!(config.auth_method, AuthMethod::Password(_)));
}

#[test]
fn test_ssh_config_validation() {
    //缺少主机
    let config = SshConfig::default();
    assert!(config.validate().is_err());

    // 缺少用户名
    let config = SshConfig {
        host: "example.com".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_err());

    // 缺少认证方式
    let config = SshConfig::new("example.com", "user");
    assert!(config.validate().is_err());

    // 完整配置
    let config = SshConfig::new("example.com", "user").with_password("pass");
    assert!(config.validate().is_ok());
}

#[test]
fn test_ssh_connection_info_before_connect() {
    let config = SshConfig::new("example.com", "user").with_password("pass");
    let conn = SshConnection::new(config);

    assert!(!conn.is_connected());
    assert_eq!(conn.connection_type(), ConnectionType::Ssh);
    assert_eq!(conn.remote_address(), None);
    assert!(conn.connected_at().is_none());
}

#[tokio::test]
async fn test_ssh_write_without_connect() {
    let config = SshConfig::new("example.com", "user").with_password("pass");
    let conn = SshConnection::new(config);

    // 未连接时写入应该失败
    let result = conn.write(b"test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_ssh_resize_without_connect() {
    let config = SshConfig::new("example.com", "user").with_password("pass");
    let conn = SshConnection::new(config);

    // 未连接时调整大小应该失败
    let result = conn.resize(24, 80).await;
    assert!(result.is_err());
}

// ==================== 连接测试 (需要真实服务器) ====================
// 这些测试默认被忽略，需要设置环境变量后用 --ignored 运行

#[tokio::test]
#[ignore = "需要真实 SSH 服务器，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_connection_to_real_server() {
    let config = get_test_config();
    let host = config.host.clone();
    let port = config.port;

    println!("Testing SSH connection to {}:{}", host, port);

    let conn = SshConnection::new(config);

    // 连接
    let connect_result = timeout(Duration::from_secs(10), conn.connect()).await;

    match connect_result {
        Ok(Ok(())) => {
            println!("✅ SSH connection successful!");
            assert!(conn.is_connected());
            assert!(conn.remote_address().is_some());
            assert!(conn.connected_at().is_some());

            // 关闭连接
            let close_result = conn.close().await;
            assert!(close_result.is_ok());
            assert!(!conn.is_connected());
            println!("✅ SSH connection closed successfully!");
        },
        Ok(Err(e)) => {
            panic!("❌ SSH connection failed: {}", e);
        },
        Err(_) => {
            panic!("❌ SSH connection timed out");
        },
    }
}

#[tokio::test]
#[ignore = "需要真实 SSH 服务器，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_send_command() {
    let config = get_test_config();
    let conn = SshConnection::new(config);

    // 连接
    conn.connect()
        .await
        .expect("Failed to connect to SSH server");

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 发送命令
    conn.write(b"echo hello\n")
        .await
        .expect("Failed to send command");

    // 等待响应
    let response = timeout(Duration::from_secs(5), stream.next()).await;

    match response {
        Ok(Some(Ok(data))) => {
            let text = String::from_utf8_lossy(&data);
            println!("✅ Received: {}", text);
        },
        Ok(Some(Err(e))) => {
            println!("⚠️ Error receiving data: {}", e);
        },
        Ok(None) => {
            println!("⚠️ Stream ended");
        },
        Err(_) => {
            println!("⚠️ Timeout waiting for response (this may be normal)");
        },
    }

    // 关闭连接
    conn.close().await.expect("Failed to close connection");
    println!("✅ Test completed");
}

#[tokio::test]
#[ignore = "需要真实 SSH 服务器，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_resize() {
    let config = get_test_config();
    let conn = SshConnection::new(config);

    // 连接
    conn.connect()
        .await
        .expect("Failed to connect to SSH server");

    // 调整终端大小
    conn.resize(40, 120)
        .await
        .expect("Failed to resize terminal");

    println!("✅ Terminal resized to 120x40");

    // 关闭连接
    conn.close().await.expect("Failed to close connection");
}

#[tokio::test]
#[ignore = "需要真实 SSH 服务器，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_interactive_session() {
    let config = get_test_config();
    let conn = SshConnection::new(config);

    // 连接
    conn.connect()
        .await
        .expect("Failed to connect to SSH server");

    println!("✅ Connected to SSH server");

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 等待初始输出（shell prompt）
    let initial = timeout(Duration::from_secs(3), stream.next()).await;
    if let Ok(Some(Ok(data))) = initial {
        let text = String::from_utf8_lossy(&data);
        println!("Initial output: {}", text);
    }

    // 发送 pwd 命令
    conn.write(b"pwd\n").await.expect("Failed to send pwd");

    // 等待响应
    let response = timeout(Duration::from_secs(3), stream.next()).await;
    if let Ok(Some(Ok(data))) = response {
        let text = String::from_utf8_lossy(&data);
        println!("pwd output: {}", text);
    }

    // 发送 exit 命令
    conn.write(b"exit\n").await.expect("Failed to send exit");

    // 关闭连接
    conn.close().await.expect("Failed to close connection");
    println!("✅ Interactive session test completed");
}

// ==================== 公钥认证测试 ====================

#[tokio::test]
#[ignore = "需要真实 SSH 服务器和公钥配置，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_pubkey_authentication() {
    let config = get_pubkey_test_config();
    let host = config.host.clone();
    let port = config.port;

    println!("Testing SSH public key authentication to {}:{}", host, port);
    println!(
        "Key path: {:?}",
        match &config.auth_method {
            AuthMethod::PublicKey { key_path, .. } => key_path.display().to_string(),
            _ => "N/A".to_string(),
        }
    );

    let conn = SshConnection::new(config);

    match timeout(Duration::from_secs(15), conn.connect()).await {
        Ok(Ok(())) => {
            println!("✅ Public key authentication successful!");
            assert!(conn.is_connected());
            assert!(conn.remote_address().is_some());

            // 发送测试命令验证连接正常
            conn.write(b"echo 'pubkey auth works'\n")
                .await
                .expect("Failed to send command");

            // 等待响应
            let mut stream = conn.receive_stream();
            let response = timeout(Duration::from_secs(3), stream.next()).await;
            if let Ok(Some(Ok(data))) = response {
                let text = String::from_utf8_lossy(&data);
                println!("Response: {}", text);
            }

            conn.close().await.expect("Failed to close");
            println!("✅ Public key authentication test passed!");
        },
        Ok(Err(e)) => {
            panic!("❌ Public key authentication failed: {}", e);
        },
        Err(_) => {
            panic!("❌ Connection timed out");
        },
    }
}

#[tokio::test]
#[ignore = "需要真实 SSH 服务器和公钥配置，设置 SSH_TEST_* 环境变量后用 --ignored 运行"]
async fn test_ssh_pubkey_interactive_session() {
    let config = get_pubkey_test_config();
    let conn = SshConnection::new(config);

    // 连接
    conn.connect()
        .await
        .expect("Failed to connect with public key");

    println!("✅ Connected with public key authentication");

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 等待初始输出
    let _ = timeout(Duration::from_secs(2), stream.next()).await;

    // 测试 htop (如果安装了)
    println!("Testing: echo test");
    conn.write(b"echo 'Hello from pubkey auth'\n")
        .await
        .expect("Failed to send echo");

    let response = timeout(Duration::from_secs(3), stream.next()).await;
    if let Ok(Some(Ok(data))) = response {
        let text = String::from_utf8_lossy(&data);
        println!("Echo output: {}", text);
        assert!(text.contains("Hello") || text.contains("pubkey"));
    }

    // 关闭连接
    conn.close().await.expect("Failed to close connection");
    println!("✅ Public key interactive session test completed");
}

// ==================== 错误处理测试 ====================

#[tokio::test]
async fn test_ssh_connection_refused() {
    //尝试连接到一个不存在的端口
    let config = SshConfig::new("127.0.0.1", "user")
        .with_port(59999) // 不太可能被使用的端口
        .with_password("pass")
        .with_connect_timeout(Duration::from_secs(2));

    let conn = SshConnection::new(config);
    let result = conn.connect().await;

    // 应该失败
    assert!(result.is_err());
    println!("✅ Connection refused as expected: {:?}", result.err());
}

#[tokio::test]
async fn test_ssh_connection_timeout() {
    // 尝试连接到一个不可达的地址（会超时）
    let config = SshConfig::new("10.255.255.1", "user") // 不可路由的地址
        .with_password("pass")
        .with_connect_timeout(Duration::from_secs(2));

    let conn = SshConnection::new(config);
    let result = conn.connect().await;

    // 应该超时失败
    assert!(result.is_err());
    println!("✅ Connection timeout as expected: {:?}", result.err());
}
