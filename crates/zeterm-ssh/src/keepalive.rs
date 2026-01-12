//! 心跳保活模块
//!
//! 提供 SSH 连接的心跳保活功能，包括：
//! - 定时发送 keepalive 请求
//! - 检测连接超时
//! - 自动触发重连

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tracing::{debug, info, warn};

/// 默认心跳间隔（秒）
pub const DEFAULT_KEEPALIVE_INTERVAL_SECS: u64 = 30;

/// 默认心跳超时（秒）
pub const DEFAULT_KEEPALIVE_TIMEOUT_SECS: u64 = 15;

/// 默认最大连续失败次数
pub const DEFAULT_MAX_MISSED_KEEPALIVES: u32 = 3;

/// 心跳配置
#[derive(Debug, Clone)]
pub struct KeepaliveConfig {
    /// 是否启用心跳
    pub enabled: bool,
    /// 心跳间隔
    pub interval: Duration,
    /// 心跳超时时间
    pub timeout: Duration,
    /// 最大连续失败次数
    pub max_missed: u32,
}

impl Default for KeepaliveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(DEFAULT_KEEPALIVE_INTERVAL_SECS),
            timeout: Duration::from_secs(DEFAULT_KEEPALIVE_TIMEOUT_SECS),
            max_missed: DEFAULT_MAX_MISSED_KEEPALIVES,
        }
    }
}

impl KeepaliveConfig {
    /// 创建新的心跳配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 禁用心跳
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// 设置是否启用
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置心跳间隔
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置最大失败次数
    pub fn with_max_missed(mut self, max: u32) -> Self {
        self.max_missed = max;
        self
    }
}

/// 心跳状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeepaliveState {
    /// 空闲（未启动）
    Idle,
    /// 运行中
    Running,
    /// 等待响应
    WaitingResponse,
    /// 已停止
    Stopped,
    /// 超时
    TimedOut { missed_count: u32 },
}

impl KeepaliveState {
    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running | Self::WaitingResponse)
    }

    /// 是否超时
    pub fn is_timed_out(&self) -> bool {
        matches!(self, Self::TimedOut { .. })
    }
}

/// 心跳统计信息
#[derive(Debug, Clone)]
pub struct KeepaliveStats {
    /// 发送的心跳数
    pub sent_count: u64,
    /// 收到的响应数
    pub received_count: u64,
    /// 连续失败次数
    pub missed_count: u32,
    /// 最后发送时间
    pub last_sent: Option<Instant>,
    /// 最后响应时间
    pub last_received: Option<Instant>,
    /// 平均往返时间
    pub avg_rtt: Option<Duration>,
}

impl Default for KeepaliveStats {
    fn default() -> Self {
        Self {
            sent_count: 0,
            received_count: 0,
            missed_count: 0,
            last_sent: None,
            last_received: None,
            avg_rtt: None,
        }
    }
}

impl KeepaliveStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录发送
    pub fn record_sent(&mut self) {
        self.sent_count += 1;
        self.last_sent = Some(Instant::now());
    }

    /// 记录响应
    pub fn record_received(&mut self) {
        self.received_count += 1;
        self.missed_count = 0;
        let now = Instant::now();

        // 计算 RTT
        if let Some(sent_time) = self.last_sent {
            let rtt = now.duration_since(sent_time);
            self.avg_rtt = Some(match self.avg_rtt {
                Some(avg) => {
                    Duration::from_nanos((avg.as_nanos() as u64 * 7 + rtt.as_nanos() as u64) / 8)
                },
                None => rtt,
            });
        }

        self.last_received = Some(now);
    }

    /// 记录超时
    pub fn record_timeout(&mut self) {
        self.missed_count += 1;
    }

    /// 重置统计
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 心跳事件
#[derive(Debug, Clone)]
pub enum KeepaliveEvent {
    /// 已启动
    Started,
    /// 发送心跳
    Sent { seq: u64 },
    /// 收到响应
    Received { seq: u64, rtt: Duration },
    /// 超时
    Timeout { missed_count: u32 },
    /// 连接丢失
    ConnectionLost,
    /// 已停止
    Stopped,
}

/// 心跳事件回调
pub trait KeepaliveCallback: Send + Sync {
    /// 处理心跳事件
    fn on_event(&self, event: KeepaliveEvent);
}

/// 日志心跳回调
pub struct LoggingKeepaliveCallback;

impl KeepaliveCallback for LoggingKeepaliveCallback {
    fn on_event(&self, event: KeepaliveEvent) {
        match event {
            KeepaliveEvent::Started => {
                info!("Keepalive started");
            },
            KeepaliveEvent::Sent { seq } => {
                debug!("Keepalive sent: seq={}", seq);
            },
            KeepaliveEvent::Received { seq, rtt } => {
                debug!("Keepalive received: seq={}, rtt={:?}", seq, rtt);
            },
            KeepaliveEvent::Timeout { missed_count } => {
                warn!("Keepalive timeout: missed_count={}", missed_count);
            },
            KeepaliveEvent::ConnectionLost => {
                warn!("Connection lost (keepalive failed)");
            },
            KeepaliveEvent::Stopped => {
                info!("Keepalive stopped");
            },
        }
    }
}

/// 心跳管理器
pub struct KeepaliveManager {
    /// 配置
    config: KeepaliveConfig,
    /// 是否正在运行
    running: Arc<AtomicBool>,
    /// 停止信号发送端
    stop_tx: Option<watch::Sender<bool>>,
}

impl KeepaliveManager {
    /// 创建新的心跳管理器
    pub fn new(config: KeepaliveConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stop_tx: None,
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(KeepaliveConfig::default())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 获取配置
    pub fn config(&self) -> &KeepaliveConfig {
        &self.config
    }

    /// 停止心跳
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(true);
        }
        self.running.store(false, Ordering::SeqCst);
        info!("Keepalive manager stopped");
    }
}

impl Drop for KeepaliveManager {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keepalive_config_default() {
        let config = KeepaliveConfig::default();
        assert!(config.enabled);
        assert_eq!(
            config.interval,
            Duration::from_secs(DEFAULT_KEEPALIVE_INTERVAL_SECS)
        );
    }

    #[test]
    fn test_keepalive_config_disabled() {
        let config = KeepaliveConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_keepalive_config_builder() {
        let config = KeepaliveConfig::new()
            .with_interval(Duration::from_secs(60))
            .with_timeout(Duration::from_secs(30))
            .with_max_missed(5);

        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_missed, 5);
    }

    #[test]
    fn test_keepalive_stats() {
        let mut stats = KeepaliveStats::new();

        stats.record_sent();
        assert_eq!(stats.sent_count, 1);
        assert!(stats.last_sent.is_some());

        stats.record_received();
        assert_eq!(stats.received_count, 1);
        assert_eq!(stats.missed_count, 0);

        stats.record_timeout();
        assert_eq!(stats.missed_count, 1);

        stats.record_timeout();
        assert_eq!(stats.missed_count, 2);

        stats.record_received();
        assert_eq!(stats.missed_count, 0);
    }

    #[test]
    fn test_keepalive_state() {
        assert!(!KeepaliveState::Idle.is_running());
        assert!(KeepaliveState::Running.is_running());
        assert!(KeepaliveState::WaitingResponse.is_running());
        assert!(!KeepaliveState::Stopped.is_running());

        assert!(!KeepaliveState::Running.is_timed_out());
        assert!(KeepaliveState::TimedOut { missed_count: 3 }.is_timed_out());
    }

    #[test]
    fn test_keepalive_manager() {
        let manager = KeepaliveManager::with_defaults();
        assert!(!manager.is_running());
        assert!(manager.config().enabled);
    }
}
