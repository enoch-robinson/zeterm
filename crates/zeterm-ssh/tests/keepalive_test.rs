//! KeepaliveManager 集成测试
//!
//! 测试心跳管理器的各种功能，包括：
//! - 启动和停止
//! - 心跳发送和接收
//! - 超时检测
//! - 连接丢失检测

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;
use zeterm_ssh::{
    KeepaliveCallback, KeepaliveConfig, KeepaliveEvent, KeepaliveManager, LoggingKeepaliveCallback,
};

/// 测试用的事件收集器
#[allow(dead_code)]
struct EventCollector {
    events: Mutex<Vec<KeepaliveEvent>>,
}

impl EventCollector {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    #[allow(dead_code)]
    async fn events(&self) -> Vec<KeepaliveEvent> {
        self.events.lock().await.clone()
    }

    #[allow(dead_code)]
    async fn event_count(&self) -> usize {
        self.events.lock().await.len()
    }

    async fn has_event(&self, check: impl Fn(&KeepaliveEvent) -> bool) -> bool {
        self.events.lock().await.iter().any(check)
    }
}

impl KeepaliveCallback for EventCollector {
    fn on_event(&self, event: KeepaliveEvent) {
        // 使用 blocking lock 因为这是同步回调
        if let Ok(mut events) = self.events.try_lock() {
            events.push(event);
        }
    }
}

//==================== 配置测试 ====================

#[test]
fn test_keepalive_config_default() {
    let config = KeepaliveConfig::default();
    assert!(config.enabled);
    assert_eq!(config.interval, Duration::from_secs(30));
    assert_eq!(config.timeout, Duration::from_secs(15));
    assert_eq!(config.max_missed, 3);
}

#[test]
fn test_keepalive_config_disabled() {
    let config = KeepaliveConfig::disabled();
    assert!(!config.enabled);
}

#[test]
fn test_keepalive_config_builder() {
    let config = KeepaliveConfig::new()
        .with_enabled(true)
        .with_interval(Duration::from_secs(10))
        .with_timeout(Duration::from_secs(5))
        .with_max_missed(5);

    assert!(config.enabled);
    assert_eq!(config.interval, Duration::from_secs(10));
    assert_eq!(config.timeout, Duration::from_secs(5));
    assert_eq!(config.max_missed, 5);
}

// ==================== 管理器基本测试 ====================

#[test]
fn test_keepalive_manager_new() {
    let config = KeepaliveConfig::default();
    let manager = KeepaliveManager::new(config);

    assert!(!manager.is_running());
    assert!(manager.config().enabled);
}

#[test]
fn test_keepalive_manager_with_defaults() {
    let manager = KeepaliveManager::with_defaults();
    assert!(!manager.is_running());
}

// ==================== 启动测试 ====================

#[tokio::test]
async fn test_keepalive_start_disabled() {
    let config = KeepaliveConfig::disabled();
    let mut manager = KeepaliveManager::new(config);

    // 禁用时不应该启动
    let handle = manager.start(|| async { Ok(()) }, None);

    assert!(handle.is_none());
    assert!(!manager.is_running());
}

#[tokio::test]
async fn test_keepalive_start_enabled() {
    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(100))
        .with_timeout(Duration::from_millis(50));

    let mut manager = KeepaliveManager::new(config);

    let handle = manager.start(|| async { Ok(()) }, None);

    assert!(handle.is_some());
    assert!(manager.is_running());

    // 停止管理器
    manager.stop();

    // 等待任务结束
    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    assert!(!manager.is_running());
}

#[tokio::test]
async fn test_keepalive_double_start() {
    let config = KeepaliveConfig::new().with_interval(Duration::from_millis(100));

    let mut manager = KeepaliveManager::new(config);

    // 第一次启动
    let handle1 = manager.start(|| async { Ok(()) }, None);
    assert!(handle1.is_some());

    // 第二次启动应该返回 None（已经在运行）
    let handle2 = manager.start(|| async { Ok(()) }, None);
    assert!(handle2.is_none());

    manager.stop();
}

// ==================== 心跳发送测试 ====================

#[tokio::test]
async fn test_keepalive_sends_heartbeats() {
    let send_count = Arc::new(AtomicU32::new(0));
    let send_count_clone = send_count.clone();

    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(50))
        .with_timeout(Duration::from_millis(100));

    let mut manager = KeepaliveManager::new(config);

    let handle = manager.start(
        move || {
            let count = send_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        },
        None,
    );

    // 等待几个心跳周期
    tokio::time::sleep(Duration::from_millis(200)).await;

    manager.stop();

    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 应该发送了多个心跳
    let count = send_count.load(Ordering::SeqCst);
    assert!(count >= 2, "Expected at least 2 heartbeats, got {}", count);
}

// ==================== 事件回调测试 ====================

#[tokio::test]
async fn test_keepalive_events_started_stopped() {
    let collector = Arc::new(EventCollector::new());

    let config = KeepaliveConfig::new().with_interval(Duration::from_millis(100));

    let mut manager = KeepaliveManager::new(config);

    let handle = manager.start(|| async { Ok(()) }, Some(collector.clone()));

    // 等待启动事件
    tokio::time::sleep(Duration::from_millis(50)).await;

    manager.stop();

    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 等待停止事件
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 检查事件
    let has_started = collector
        .has_event(|e| matches!(e, KeepaliveEvent::Started))
        .await;
    let has_stopped = collector
        .has_event(|e| matches!(e, KeepaliveEvent::Stopped))
        .await;

    assert!(has_started, "Should have Started event");
    assert!(has_stopped, "Should have Stopped event");
}

#[tokio::test]
async fn test_keepalive_events_sent_received() {
    let collector = Arc::new(EventCollector::new());

    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(50))
        .with_timeout(Duration::from_millis(100));

    let mut manager = KeepaliveManager::new(config);

    let handle = manager.start(|| async { Ok(()) }, Some(collector.clone()));

    // 等待几个心跳
    tokio::time::sleep(Duration::from_millis(200)).await;

    manager.stop();

    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 检查发送和接收事件
    let has_sent = collector
        .has_event(|e| matches!(e, KeepaliveEvent::Sent { .. }))
        .await;
    let has_received = collector
        .has_event(|e| matches!(e, KeepaliveEvent::Received { .. }))
        .await;

    assert!(has_sent, "Should have Sent events");
    assert!(has_received, "Should have Received events");
}

// ==================== 超时测试 ====================

#[tokio::test]
async fn test_keepalive_timeout_detection() {
    let collector = Arc::new(EventCollector::new());

    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(50))
        .with_timeout(Duration::from_millis(10))
        .with_max_missed(5); // 设置高一点，避免连接丢失

    let mut manager = KeepaliveManager::new(config);

    // 模拟超时：心跳函数延迟超过 timeout
    let handle = manager.start(
        || async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        },
        Some(collector.clone()),
    );

    // 等待超时发生
    tokio::time::sleep(Duration::from_millis(200)).await;

    manager.stop();

    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 检查超时事件
    let has_timeout = collector
        .has_event(|e| matches!(e, KeepaliveEvent::Timeout { .. }))
        .await;

    assert!(has_timeout, "Should have Timeout events");
}

// ==================== 连接丢失测试 ====================

#[tokio::test]
async fn test_keepalive_connection_lost() {
    let collector = Arc::new(EventCollector::new());

    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(30))
        .with_timeout(Duration::from_millis(10))
        .with_max_missed(2); // 2次失败后触发连接丢失

    let mut manager = KeepaliveManager::new(config);

    // 模拟持续失败
    let handle = manager.start(
        || async { Err("Simulated failure".into()) },
        Some(collector.clone()),
    );

    // 等待连接丢失
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 任务应该已经自动停止
    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 检查连接丢失事件
    let has_connection_lost = collector
        .has_event(|e| matches!(e, KeepaliveEvent::ConnectionLost))
        .await;

    assert!(has_connection_lost, "Should have ConnectionLost event");
    assert!(!manager.is_running(), "Manager should have stopped");
}

// ==================== 错误恢复测试 ====================

#[tokio::test]
async fn test_keepalive_recovers_from_single_failure() {
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    let config = KeepaliveConfig::new()
        .with_interval(Duration::from_millis(30))
        .with_timeout(Duration::from_millis(50))
        .with_max_missed(3);

    let mut manager = KeepaliveManager::new(config);

    // 第一次调用失败，后续成功
    let handle = manager.start(
        move || {
            let count = call_count_clone.clone();
            async move {
                let n = count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err("First call fails".into())
                } else {
                    Ok(())
                }
            }
        },
        None,
    );

    // 等待多个心跳周期
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 管理器应该仍在运行（因为恢复了）
    assert!(
        manager.is_running(),
        "Manager should still be running after recovery"
    );

    manager.stop();

    if let Some(h) = handle {
        let _ = tokio::time::timeout(Duration::from_secs(1), h).await;
    }

    // 应该有多次调用
    let count = call_count.load(Ordering::SeqCst);
    assert!(count > 2, "Expected multiple calls, got {}", count);
}

// ==================== 日志回调测试 ====================

#[test]
fn test_logging_callback() {
    let callback = LoggingKeepaliveCallback;

    // 这些调用不应该 panic
    callback.on_event(KeepaliveEvent::Started);
    callback.on_event(KeepaliveEvent::Sent { seq: 1 });
    callback.on_event(KeepaliveEvent::Received {
        seq: 1,
        rtt: Duration::from_millis(10),
    });
    callback.on_event(KeepaliveEvent::Timeout { missed_count: 1 });
    callback.on_event(KeepaliveEvent::ConnectionLost);
    callback.on_event(KeepaliveEvent::Stopped);
}

// ==================== Drop 测试 ====================

#[tokio::test]
async fn test_keepalive_manager_drop() {
    let config = KeepaliveConfig::new().with_interval(Duration::from_millis(50));

    let handle = {
        let mut manager = KeepaliveManager::new(config);
        let h = manager.start(|| async { Ok(()) }, None);
        assert!(manager.is_running());
        h // manager 在这里被 drop
    };

    // 等待任务结束
    if let Some(h) = handle {
        let result = tokio::time::timeout(Duration::from_secs(2), h).await;
        assert!(result.is_ok(), "Task should complete after manager drop");
    }
}
