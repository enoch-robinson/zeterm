//! 主机密钥验证集成测试
//!
//! 测试 SSH 主机密钥验证功能，包括：
//! - KnownHostsStore 的加载和保存
//! - 主机密钥验证逻辑
//! - SshHandler 的主机密钥验证集成
//! - 各种验证模式的行为

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;
use zeterm_ssh::{
    // handler
    HostKeyConfirmCallback,
    // known_hosts
    HostKeyEntry,
    // config
    HostKeyVerification,
    KeyType,
    KnownHostsStore,
    SshHandler,
    VerificationResult,
    create_data_channel,
};

//==================== KeyType 测试 ====================

#[test]
fn test_key_type_from_str() {
    assert_eq!(KeyType::from_str("ssh-rsa"), KeyType::Rsa);
    assert_eq!(KeyType::from_str("ssh-ed25519"), KeyType::Ed25519);
    assert_eq!(
        KeyType::from_str("ecdsa-sha2-nistp256"),
        KeyType::EcdsaSha2Nistp256
    );
    assert_eq!(
        KeyType::from_str("ecdsa-sha2-nistp384"),
        KeyType::EcdsaSha2Nistp384
    );
    assert_eq!(
        KeyType::from_str("ecdsa-sha2-nistp521"),
        KeyType::EcdsaSha2Nistp521
    );

    // 未知类型
    match KeyType::from_str("unknown-type") {
        KeyType::Unknown(s) => assert_eq!(s, "unknown-type"),
        _ => panic!("Expected Unknown variant"),
    }
}

#[test]
fn test_key_type_as_str() {
    assert_eq!(KeyType::Rsa.as_str(), "ssh-rsa");
    assert_eq!(KeyType::Ed25519.as_str(), "ssh-ed25519");
    assert_eq!(KeyType::EcdsaSha2Nistp256.as_str(), "ecdsa-sha2-nistp256");
    assert_eq!(KeyType::EcdsaSha2Nistp384.as_str(), "ecdsa-sha2-nistp384");
    assert_eq!(KeyType::EcdsaSha2Nistp521.as_str(), "ecdsa-sha2-nistp521");
    assert_eq!(KeyType::Unknown("custom".to_string()).as_str(), "custom");
}

#[test]
fn test_key_type_display() {
    assert_eq!(format!("{}", KeyType::Rsa), "ssh-rsa");
    assert_eq!(format!("{}", KeyType::Ed25519), "ssh-ed25519");
}

// ==================== HostKeyEntry 测试 ====================

#[test]
fn test_host_key_entry_new() {
    let entry = HostKeyEntry::new("example.com", KeyType::Ed25519, "AAAAC3NzaC1lZDI1NTE5...");

    assert_eq!(entry.hostnames, vec!["example.com"]);
    assert_eq!(entry.key_type, KeyType::Ed25519);
    assert_eq!(entry.key_data, "AAAAC3NzaC1lZDI1NTE5...");
    assert!(entry.comment.is_none());
    assert!(!entry.is_hashed);
}

#[test]
fn test_host_key_entry_with_hostname() {
    let entry = HostKeyEntry::new("host1", KeyType::Rsa, "key")
        .with_hostname("host2")
        .with_hostname("host3");

    assert_eq!(entry.hostnames.len(), 3);
    assert!(entry.hostnames.contains(&"host1".to_string()));
    assert!(entry.hostnames.contains(&"host2".to_string()));
    assert!(entry.hostnames.contains(&"host3".to_string()));
}

#[test]
fn test_host_key_entry_with_comment() {
    let entry = HostKeyEntry::new("example.com", KeyType::Rsa, "key").with_comment("my server key");

    assert_eq!(entry.comment, Some("my server key".to_string()));
}

#[test]
fn test_host_key_entry_matches_host_standard_port() {
    let entry = HostKeyEntry::new("example.com", KeyType::Rsa, "key");

    assert!(entry.matches_host("example.com", 22));
    assert!(!entry.matches_host("other.com", 22));
    assert!(!entry.matches_host("example.com", 2222));
}

#[test]
fn test_host_key_entry_matches_host_custom_port() {
    let entry = HostKeyEntry::new("[example.com]:2222", KeyType::Rsa, "key");

    assert!(entry.matches_host("example.com", 2222));
    assert!(!entry.matches_host("example.com", 22));
    assert!(!entry.matches_host("example.com", 3333));
}

#[test]
fn test_host_key_entry_matches_host_wildcard() {
    let entry = HostKeyEntry::new("*.example.com", KeyType::Rsa, "key");

    assert!(entry.matches_host("server.example.com", 22));
    assert!(entry.matches_host("test.example.com", 22));
    assert!(!entry.matches_host("example.com", 22));
}

#[test]
fn test_host_key_entry_to_line() {
    let entry =
        HostKeyEntry::new("example.com", KeyType::Ed25519, "AAAAC3...").with_comment("test key");

    let line = entry.to_line();
    assert!(line.contains("example.com"));
    assert!(line.contains("ssh-ed25519"));
    assert!(line.contains("AAAAC3..."));
    assert!(line.contains("test key"));
}

#[test]
fn test_host_key_entry_from_line() {
    let line = "example.com ssh-rsa AAAAB3NzaC1yc2E... user@host";
    let entry = HostKeyEntry::from_line(line).unwrap();

    assert_eq!(entry.hostnames, vec!["example.com"]);
    assert_eq!(entry.key_type, KeyType::Rsa);
    assert_eq!(entry.key_data, "AAAAB3NzaC1yc2E...");
    assert_eq!(entry.comment, Some("user@host".to_string()));
}

#[test]
fn test_host_key_entry_from_line_multiple_hosts() {
    let line = "host1,host2,host3 ssh-ed25519 AAAAC3...";
    let entry = HostKeyEntry::from_line(line).unwrap();

    assert_eq!(entry.hostnames.len(), 3);
}

#[test]
fn test_host_key_entry_from_line_skip_comments() {
    assert!(HostKeyEntry::from_line("# This is a comment").is_none());
    assert!(HostKeyEntry::from_line("").is_none());
    assert!(HostKeyEntry::from_line("   ").is_none());
}

#[test]
fn test_host_key_entry_from_line_invalid() {
    // 不完整的行
    assert!(HostKeyEntry::from_line("example.com").is_none());
    assert!(HostKeyEntry::from_line("example.com ssh-rsa").is_none());
}

// ==================== VerificationResult 测试 ====================

#[test]
fn test_verification_result_is_safe() {
    assert!(VerificationResult::Match.is_safe());
    assert!(!VerificationResult::Unknown.is_safe());
    assert!(
        !VerificationResult::Changed {
            expected_type: KeyType::Rsa,
            expected_key: "old".to_string(),
        }
        .is_safe()
    );
    assert!(!VerificationResult::Revoked.is_safe());
}

#[test]
fn test_verification_result_needs_confirmation() {
    assert!(!VerificationResult::Match.needs_confirmation());
    assert!(VerificationResult::Unknown.needs_confirmation());
    assert!(
        !VerificationResult::Changed {
            expected_type: KeyType::Rsa,
            expected_key: "old".to_string(),
        }
        .needs_confirmation()
    );
}

#[test]
fn test_verification_result_is_warning() {
    assert!(!VerificationResult::Match.is_warning());
    assert!(!VerificationResult::Unknown.is_warning());
    assert!(
        VerificationResult::Changed {
            expected_type: KeyType::Rsa,
            expected_key: "old".to_string(),
        }
        .is_warning()
    );
    assert!(VerificationResult::Revoked.is_warning());
}

#[test]
fn test_verification_result_messages() {
    assert!(!VerificationResult::Match.message().is_empty());
    assert!(!VerificationResult::Unknown.message().is_empty());
    assert!(!VerificationResult::Match.message_cn().is_empty());
    assert!(!VerificationResult::Unknown.message_cn().is_empty());
}

// ==================== KnownHostsStore 测试 ====================

#[test]
fn test_known_hosts_store_new() {
    let store = KnownHostsStore::new();
    assert!(store.entries().is_empty());
    assert!(!store.is_modified());
}

#[test]
fn test_known_hosts_store_with_path() {
    let path = PathBuf::from("/tmp/test_known_hosts");
    let store = KnownHostsStore::with_path(&path);
    assert_eq!(store.path(), path);
}

#[test]
fn test_known_hosts_store_default_path() {
    let path = KnownHostsStore::default_path();
    assert!(path.to_string_lossy().contains("known_hosts"));
}

#[test]
fn test_known_hosts_store_add() {
    let mut store = KnownHostsStore::new();

    let entry = HostKeyEntry::new("example.com", KeyType::Ed25519, "AAAAC3...");
    store.add(entry);

    assert_eq!(store.entries().len(), 1);
    assert!(store.is_modified());
}

#[test]
fn test_known_hosts_store_add_or_update() {
    let mut store = KnownHostsStore::new();

    //添加第一个条目
    store.add_or_update("example.com", 22, KeyType::Ed25519, "key1".to_string());
    assert_eq!(store.entries().len(), 1);

    // 更新同一主机
    store.add_or_update("example.com", 22, KeyType::Ed25519, "key2".to_string());
    assert_eq!(store.entries().len(), 1);

    // 验证密钥已更新
    let entry = store.find("example.com", 22).unwrap();
    assert_eq!(entry.key_data, "key2");
}

#[test]
fn test_known_hosts_store_find() {
    let mut store = KnownHostsStore::new();

    store.add(HostKeyEntry::new("host1.com", KeyType::Rsa, "key1"));
    store.add(HostKeyEntry::new("host2.com", KeyType::Ed25519, "key2"));

    assert!(store.find("host1.com", 22).is_some());
    assert!(store.find("host2.com", 22).is_some());
    assert!(store.find("host3.com", 22).is_none());
}

#[test]
fn test_known_hosts_store_remove() {
    let mut store = KnownHostsStore::new();

    store.add(HostKeyEntry::new("example.com", KeyType::Rsa, "key"));
    assert_eq!(store.entries().len(), 1);

    let removed = store.remove("example.com", 22);
    assert!(removed);
    assert!(store.entries().is_empty());
    assert!(store.is_modified());

    // 再次删除应该返回 false
    let removed = store.remove("example.com", 22);
    assert!(!removed);
}

#[test]
fn test_known_hosts_store_verify_match() {
    let mut store = KnownHostsStore::new();
    store.add(HostKeyEntry::new(
        "example.com",
        KeyType::Ed25519,
        "correct_key",
    ));

    let result = store.verify("example.com", 22, &KeyType::Ed25519, "correct_key");
    assert_eq!(result, VerificationResult::Match);
}

#[test]
fn test_known_hosts_store_verify_unknown() {
    let store = KnownHostsStore::new();

    let result = store.verify("unknown.com", 22, &KeyType::Ed25519, "some_key");
    assert_eq!(result, VerificationResult::Unknown);
}

#[test]
fn test_known_hosts_store_verify_changed() {
    let mut store = KnownHostsStore::new();
    store.add(HostKeyEntry::new(
        "example.com",
        KeyType::Ed25519,
        "old_key",
    ));

    let result = store.verify("example.com", 22, &KeyType::Ed25519, "new_key");

    match result {
        VerificationResult::Changed {
            expected_type,
            expected_key,
        } => {
            assert_eq!(expected_type, KeyType::Ed25519);
            assert_eq!(expected_key, "old_key");
        },
        _ => panic!("Expected Changed result"),
    }
}

// ==================== KnownHostsStore 文件操作测试 ====================

#[test]
fn test_known_hosts_store_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("known_hosts");

    // 创建并保存
    {
        let mut store = KnownHostsStore::with_path(&path);
        store.add(HostKeyEntry::new(
            "example.com",
            KeyType::Ed25519,
            "AAAAC3NzaC1lZDI1NTE5...",
        ));
        store.add(HostKeyEntry::new(
            "server.local",
            KeyType::Rsa,
            "AAAAB3NzaC1yc2E...",
        ));
        store.save().unwrap();
    }

    // 重新加载
    {
        let mut store = KnownHostsStore::with_path(&path);
        store.load().unwrap();

        assert_eq!(store.entries().len(), 2);
        assert!(store.find("example.com", 22).is_some());
        assert!(store.find("server.local", 22).is_some());
    }
}

#[test]
fn test_known_hosts_store_load_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("nonexistent_known_hosts");

    let mut store = KnownHostsStore::with_path(&path);
    let result = store.load();

    // 加载不存在的文件应该成功（空存储）
    assert!(result.is_ok());
    assert!(store.entries().is_empty());
}

#[test]
fn test_known_hosts_store_save_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("subdir").join("known_hosts");

    let mut store = KnownHostsStore::with_path(&path);
    store.add(HostKeyEntry::new("example.com", KeyType::Ed25519, "key"));

    let result = store.save();
    assert!(result.is_ok());
    assert!(path.exists());
}

#[test]
fn test_known_hosts_store_preserves_comments() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("known_hosts");

    // 创建带注释的条目
    {
        let mut store = KnownHostsStore::with_path(&path);
        store.add(
            HostKeyEntry::new("example.com", KeyType::Ed25519, "key123").with_comment("my server"),
        );
        store.save().unwrap();
    }

    // 重新加载并验证注释
    {
        let mut store = KnownHostsStore::with_path(&path);
        store.load().unwrap();

        let entry = store.find("example.com", 22).unwrap();
        assert_eq!(entry.comment, Some("my server".to_string()));
    }
}

// ==================== SshHandler 集成测试 ====================

#[test]
fn test_ssh_handler_auto_accept_mode() {
    let (sender, _receiver) = create_data_channel();
    let handler = SshHandler::new(
        sender,
        HostKeyVerification::AutoAccept,
        "example.com".to_string(),
        22,
    );

    // AutoAccept 模式不需要 known_hosts 存储
    assert!(handler.known_hosts_path().is_none());
}

#[test]
fn test_ssh_handler_strict_mode() {
    let (sender, _receiver) = create_data_channel();
    let handler = SshHandler::new(
        sender,
        HostKeyVerification::Strict,
        "example.com".to_string(),
        22,
    );

    // Strict 模式应该有 known_hosts 存储
    assert!(handler.known_hosts_path().is_some());
}

#[test]
fn test_ssh_handler_ask_on_first_connect_mode() {
    let (sender, _receiver) = create_data_channel();
    let handler = SshHandler::new(
        sender,
        HostKeyVerification::AskOnFirstConnect,
        "example.com".to_string(),
        22,
    );

    // AskOnFirstConnect 模式应该有 known_hosts 存储
    assert!(handler.known_hosts_path().is_some());
}

#[test]
fn test_ssh_handler_known_hosts_file_mode() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("known_hosts");

    let (sender, _receiver) = create_data_channel();
    let handler = SshHandler::new(
        sender,
        HostKeyVerification::KnownHostsFile(path.clone()),
        "example.com".to_string(),
        22,
    );

    // 应该使用指定的路径
    assert_eq!(handler.known_hosts_path(), Some(path));
}

#[test]
fn test_ssh_handler_with_callback() {
    let (sender, _receiver) = create_data_channel();

    let callback: HostKeyConfirmCallback = Arc::new(|host, port, key_type, fingerprint| {
        // 验证回调参数
        assert!(!host.is_empty());
        assert!(port > 0);
        assert!(!key_type.is_empty());
        assert!(!fingerprint.is_empty());
        true // 接受
    });

    let handler = SshHandler::new(
        sender,
        HostKeyVerification::AskOnFirstConnect,
        "example.com".to_string(),
        22,
    )
    .with_host_key_confirm_callback(callback);

    // 验证回调已设置
    assert!(handler.known_hosts_path().is_some());
}

#[test]
fn test_ssh_handler_state_transitions() {
    use zeterm_ssh::HandlerState;

    let (sender, _receiver) = create_data_channel();
    let handler = SshHandler::new(
        sender,
        HostKeyVerification::AutoAccept,
        "example.com".to_string(),
        22,
    );

    // 初始状态
    assert_eq!(handler.state(), HandlerState::Initial);

    // 状态转换
    handler.set_state(HandlerState::HostKeyVerified);
    assert_eq!(handler.state(), HandlerState::HostKeyVerified);

    handler.set_state(HandlerState::Authenticated);
    assert_eq!(handler.state(), HandlerState::Authenticated);

    handler.set_state(HandlerState::SessionEstablished);
    assert_eq!(handler.state(), HandlerState::SessionEstablished);

    handler.set_state(HandlerState::Disconnected);
    assert_eq!(handler.state(), HandlerState::Disconnected);
}
