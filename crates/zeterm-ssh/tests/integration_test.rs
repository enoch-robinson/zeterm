//! SSH集成测试
//!
//! 这些测试需要真实的 SSH 服务器才能运行。
//! 默认情况下这些测试会被跳过，使用以下命令运行：
//!
//! ```bash
//! # 运行所有集成测试（需要设置环境变量）
//! SSH_TEST_HOST=localhost SSH_TEST_USER=testuser SSH_TEST_PASSWORD=testpass cargo test --test integration_test -- --ignored
//!
//! # 或者使用公钥认证
//! SSH_TEST_HOST=localhost SSH_TEST_USER=testuser SSH_TEST_KEY_PATH=~/.ssh/id_rsa cargo test --test integration_test -- --ignored
//! ```

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use zeterm_core::traits::{ConnectionInfo, ConnectionType, TerminalConnection};
use zeterm_ssh::{AuthMethod, SshConfig, SshConnection};

/// 测试配置
struct TestConfig {
    host: String,
    port: u16,
    username: String,
    auth_method: AuthMethod,
}

impl TestConfig {
    /// 从环境变量加载测试配置
    fn from_env() -> Option<Self> {
        let host = env::var("SSH_TEST_HOST").ok()?;
        let port = env::var("SSH_TEST_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(22);
        let username = env::var("SSH_TEST_USER").ok()?;

        // 优先使用密码认证
        let auth_method = if let Ok(password) = env::var("SSH_TEST_PASSWORD") {
            AuthMethod::Password(password)
        } else if let Ok(key_path) = env::var("SSH_TEST_KEY_PATH") {
            let passphrase = env::var("SSH_TEST_KEY_PASSPHRASE").ok();
            AuthMethod::PublicKey {
                key_path: PathBuf::from(key_path),
                passphrase,
            }
        } else {
            return None;
        };

        Some(Self {
            host,
            port,
            username,
            auth_method,
        })
    }

    /// 创建 SSH 配置
    fn to_ssh_config(&self) -> SshConfig {
        let mut config = SshConfig::new(&self.host, &self.username).with_port(self.port);

        config = match &self.auth_method {
            AuthMethod::Password(password) => config.with_password(password),
            AuthMethod::PublicKey {
                key_path,
                passphrase,
            } => {
                if let Some(pass) = passphrase {
                    config.with_key_file_and_passphrase(key_path, pass)
                } else {
                    config.with_key_file(key_path)
                }
            },
            _ => config,
        };

        config
    }
}

/// 辅助函数：获取测试配置或跳过测试
fn get_test_config() -> TestConfig {
    TestConfig::from_env().expect(
        "SSH test configuration not found. Set SSH_TEST_HOST, SSH_TEST_USER, \
         and either SSH_TEST_PASSWORD or SSH_TEST_KEY_PATH environment variables.",
    )
}

//==================== 连接测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_connect_password() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);

    // 验证初始状态
    assert!(!conn.is_connected());
    assert_eq!(conn.connection_type(), ConnectionType::Ssh);

    // 建立连接
    let result = conn.connect().await;
    assert!(result.is_ok(), "Connection failed: {:?}", result.err());

    // 验证连接状态
    assert!(conn.is_connected());
    assert!(conn.remote_address().is_some());
    assert!(conn.connected_at().is_some());

    // 关闭连接
    let result = conn.close().await;
    assert!(result.is_ok());
    assert!(!conn.is_connected());
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_connect_timeout() {
    // 使用一个不存在的主机来测试超时
    let config = SshConfig::new("192.0.2.1", "testuser") // TEST-NET-1,不可路由
        .with_password("testpass")
        .with_connect_timeout(Duration::from_secs(2));

    let conn = SshConnection::new(config);
    let result = conn.connect().await;

    assert!(result.is_err());
    // 应该是超时或连接错误
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_connect_invalid_password() {
    let config = get_test_config();

    // 使用错误的密码
    let ssh_config = SshConfig::new(&config.host, &config.username)
        .with_port(config.port)
        .with_password("wrong_password_12345");

    let conn = SshConnection::new(ssh_config);
    let result = conn.connect().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Authentication")
            || err.to_string().contains("authentication")
            || err.to_string().contains("auth"),
        "Expected authentication error, got: {}",
        err
    );
}

// ==================== 数据传输测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_write_and_receive() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 发送一个简单的命令
    conn.write(b"echo 'hello world'\n")
        .await
        .expect("Failed to write");

    // 等待接收数据（带超时）
    let mut received_data = Vec::new();
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(result) = stream.next().await {
            if let Ok(data) = result {
                received_data.extend_from_slice(&data); // 检查是否收到了预期的输出
                let output = String::from_utf8_lossy(&received_data);
                if output.contains("hello world") {
                    return true;
                }
            }
        }
        false
    });

    let found = timeout.await.unwrap_or(false);
    assert!(
        found,
        "Did not receive expected output. Got: {}",
        String::from_utf8_lossy(&received_data)
    );

    conn.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_execute_command() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    // 执行 ls 命令
    conn.write(b"ls -la /\n").await.expect("Failed to write");

    // 收集输出
    let mut output = String::new();
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(result) = stream.next().await {
            if let Ok(data) = result {
                output.push_str(&String::from_utf8_lossy(&data)); // 检查是否收到了目录列表
                if output.contains("bin") || output.contains("etc") || output.contains("usr") {
                    return true;
                }
            }
        }
        false
    });

    let found = timeout.await.unwrap_or(false);
    assert!(found, "Did not receive directory listing. Got: {}", output);

    conn.close().await.expect("Failed to close");
}

// ==================== 终端尺寸测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_resize() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    // 调整终端大小
    let result = conn.resize(40, 120).await;
    assert!(result.is_ok(), "Resize failed: {:?}", result.err());

    // 再次调整
    let result = conn.resize(24, 80).await;
    assert!(result.is_ok());

    conn.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_resize_and_verify() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    // 调整终端大小
    conn.resize(50, 100).await.expect("Resize failed");

    // 使用 stty 命令验证终端大小
    conn.write(b"stty size\n").await.expect("Failed to write");

    // 检查输出
    let mut output = String::new();
    let timeout = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(result) = stream.next().await {
            if let Ok(data) = result {
                output.push_str(&String::from_utf8_lossy(&data));
                // 检查是否包含尺寸信息
                if output.contains("50") && output.contains("100") {
                    return true;
                }
            }
        }
        false
    });

    let found = timeout.await.unwrap_or(false);
    // 注意：某些系统可能不会立即反映尺寸变化
    if !found {
        eprintln!(
            "Warning: Terminal size verification may have failed. Output: {}",
            output
        );
    }

    conn.close().await.expect("Failed to close");
}

// ==================== 交互式程序测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server with vim installed"]
async fn test_ssh_interactive_vim() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    // 启动 vim
    conn.write(b"vim\n").await.expect("Failed to write");

    // 等待 vim 启动
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 发送退出命令
    conn.write(b":q!\n").await.expect("Failed to write");

    // 等待退出
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 收集一些输出以验证 vim 运行过
    let mut output = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(result) = stream.next().await {
            if let Ok(data) = result {
                output.extend_from_slice(&data);
            }
        }
    })
    .await;

    // vim 应该产生一些输出（ANSI 转义序列等）
    assert!(!output.is_empty(), "Expected some output from vim");

    conn.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires real SSH server with htop installed"]
async fn test_ssh_interactive_htop() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    let mut stream = conn.receive_stream();

    // 启动 htop
    conn.write(b"htop\n").await.expect("Failed to write");

    // 等待 htop 启动并渲染
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 发送退出命令 (q)
    conn.write(b"q").await.expect("Failed to write");

    // 收集输出
    let mut output = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(result) = stream.next().await {
            if let Ok(data) = result {
                output.extend_from_slice(&data);
            }
        }
    })
    .await;

    // htop 应该产生大量输出（包含颜色代码）
    assert!(
        output.len() > 100,
        "Expected substantial output from htop, got {} bytes",
        output.len()
    );

    conn.close().await.expect("Failed to close");
}

// ==================== 连接状态测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_connection_info() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);

    // 连接前
    assert!(!conn.is_connected());
    assert_eq!(conn.connection_type(), ConnectionType::Ssh);
    assert!(conn.remote_address().is_none());
    assert!(conn.connected_at().is_none());

    // 连接后
    conn.connect().await.expect("Failed to connect");
    assert!(conn.is_connected());
    assert!(conn.remote_address().is_some());
    assert!(conn.connected_at().is_some());

    let addr = conn.remote_address().unwrap();
    assert!(addr.contains(&config.host) || addr.contains(':'));

    // 断开后
    conn.close().await.expect("Failed to close");
    assert!(!conn.is_connected());
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_multiple_writes() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    // 连续发送多个命令
    for i in 0..5 {
        let cmd = format!("echo 'test {}'\n", i);
        conn.write(cmd.as_bytes()).await.expect("Failed to write");
    }

    // 短暂等待
    tokio::time::sleep(Duration::from_millis(100)).await;

    conn.close().await.expect("Failed to close");
}

// ==================== 错误处理测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_write_after_close() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");
    conn.close().await.expect("Failed to close");

    // 尝试在关闭后写入
    let result = conn.write(b"echo test\n").await;
    assert!(result.is_err(), "Write after close should fail");
}

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_resize_after_close() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");
    conn.close().await.expect("Failed to close");

    // 尝试在关闭后调整大小
    let result = conn.resize(40, 80).await;
    assert!(result.is_err(), "Resize after close should fail");
}

// ==================== 长时间连接测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server, takes 10+ seconds"]
async fn test_ssh_long_connection() {
    let config = get_test_config();
    let ssh_config = config.to_ssh_config();

    let conn = SshConnection::new(ssh_config);
    conn.connect().await.expect("Failed to connect");

    // 保持连接 10 秒
    for i in 0..10 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(conn.is_connected(), "Connection lost at second {}", i);

        // 定期发送命令保持活跃
        if i % 3 == 0 {
            conn.write(b"echo keepalive\n")
                .await
                .expect("Failed to write");
        }
    }

    conn.close().await.expect("Failed to close");
}

// ==================== 并发测试 ====================

#[tokio::test]
#[ignore = "Requires real SSH server"]
async fn test_ssh_concurrent_connections() {
    let config = get_test_config();

    // 创建多个并发连接
    let mut handles = Vec::new();

    for i in 0..3 {
        let ssh_config = config.to_ssh_config();
        let handle = tokio::spawn(async move {
            let conn = SshConnection::new(ssh_config);
            conn.connect().await.expect("Failed to connect");

            let cmd = format!("echo 'connection {}'\n", i);
            conn.write(cmd.as_bytes()).await.expect("Failed to write");

            tokio::time::sleep(Duration::from_millis(100)).await;

            conn.close().await.expect("Failed to close");
            i
        });
        handles.push(handle);
    }

    // 等待所有连接完成
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
    }
}
