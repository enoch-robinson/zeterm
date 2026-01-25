//! 重连策略模块
//!
//! 提供 SSH 连接的自动重连功能，包括：
//! - 指数退避算法
//! - 可配置的重连策略
//! - 重连状态跟踪

use std::time::Duration;

use tracing::{debug, info, warn};

/// 默认最大重连次数
pub const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// 默认初始重连延迟（毫秒）
pub const DEFAULT_INITIAL_DELAY_MS: u64 = 1000;

/// 默认最大重连延迟（毫秒）
pub const DEFAULT_MAX_DELAY_MS: u64 = 30000;

/// 默认退避乘数
pub const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// 重连策略
///
/// 定义连接断开后的重连行为
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    /// 是否启用自动重连
    pub enabled: bool,
    /// 最大重连次数（0 表示无限重试）
    pub max_attempts: u32,
    /// 初始重连延迟
    pub initial_delay: Duration,
    /// 最大重连延迟
    pub max_delay: Duration,
    /// 退避乘数
    pub backoff_multiplier: f64,
    /// 是否添加随机抖动
    pub jitter: bool,
    /// 抖动因子（0.0 - 1.0）
    pub jitter_factor: f64,
    /// 连接成功后是否重置计数器
    pub reset_on_success: bool,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: DEFAULT_MAX_RECONNECT_ATTEMPTS,
            initial_delay: Duration::from_millis(DEFAULT_INITIAL_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter: true,
            jitter_factor: 0.1,
            reset_on_success: true,
        }
    }
}

impl ReconnectPolicy {
    /// 创建新的重连策略
    pub fn new() -> Self {
        Self::default()
    }

    /// 禁用自动重连
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// 创建激进的重连策略（快速重试）
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            max_attempts: 10,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.5,
            jitter: true,
            jitter_factor: 0.1,
            reset_on_success: true,
        }
    }

    /// 创建保守的重连策略（慢速重试）
    pub fn conservative() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            initial_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 3.0,
            jitter: true,
            jitter_factor: 0.2,
            reset_on_success: true,
        }
    }

    /// 创建无限重试策略
    pub fn infinite() -> Self {
        Self {
            enabled: true,
            max_attempts: 0, // 0 表示无限
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
            jitter_factor: 0.1,
            reset_on_success: true,
        }
    }

    /// 设置是否启用
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置最大重连次数
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }

    /// 设置初始延迟
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// 设置最大延迟
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// 设置退避乘数
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier.max(1.0);
        self
    }

    /// 设置是否添加抖动
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// 检查是否允许无限重试
    pub fn is_infinite(&self) -> bool {
        self.max_attempts == 0
    }
}

/// 指数退避计算器
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// 重连策略
    policy: ReconnectPolicy,
    /// 当前尝试次数
    attempt: u32,
    /// 当前延迟
    current_delay: Duration,
}

impl ExponentialBackoff {
    /// 创建新的退避计算器
    pub fn new(policy: ReconnectPolicy) -> Self {
        let initial_delay = policy.initial_delay;
        Self {
            policy,
            attempt: 0,
            current_delay: initial_delay,
        }
    }

    /// 使用默认策略创建
    pub fn with_defaults() -> Self {
        Self::new(ReconnectPolicy::default())
    }

    /// 获取当前尝试次数
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// 获取下一次重连延迟
    pub fn next_delay(&mut self) -> Option<Duration> {
        if !self.policy.enabled {
            return None;
        }

        // 检查是否超过最大尝试次数
        if !self.policy.is_infinite() && self.attempt >= self.policy.max_attempts {
            return None;
        }

        self.attempt += 1;

        // 计算延迟
        let delay = if self.attempt == 1 {
            self.policy.initial_delay
        } else {
            let new_delay = self.current_delay.mul_f64(self.policy.backoff_multiplier);
            new_delay.min(self.policy.max_delay)
        };

        // 添加抖动
        let final_delay = if self.policy.jitter {
            self.add_jitter(delay)
        } else {
            delay
        };

        self.current_delay = delay;

        debug!("Backoff: attempt {}, delay {:?}", self.attempt, final_delay);

        Some(final_delay)
    }

    /// 添加随机抖动
    fn add_jitter(&self, delay: Duration) -> Duration {
        // 简单的伪随机抖动（基于当前时间）
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let jitter_range = (delay.as_millis() as f64 * self.policy.jitter_factor) as u128;
        if jitter_range == 0 {
            return delay;
        }

        let jitter = (now % jitter_range) as i64 - (jitter_range / 2) as i64;
        let new_millis = (delay.as_millis() as i64 + jitter).max(0) as u64;

        Duration::from_millis(new_millis)
    }

    /// 重置退避状态
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.current_delay = self.policy.initial_delay;
        debug!("Backoff reset");
    }

    /// 检查是否还可以重试
    pub fn can_retry(&self) -> bool {
        self.policy.enabled
            && (self.policy.is_infinite() || self.attempt < self.policy.max_attempts)
    }

    /// 获取剩余尝试次数
    pub fn remaining_attempts(&self) -> Option<u32> {
        if self.policy.is_infinite() {
            None
        } else {
            Some(self.policy.max_attempts.saturating_sub(self.attempt))
        }
    }

    /// 获取策略引用
    pub fn policy(&self) -> &ReconnectPolicy {
        &self.policy
    }
}

/// 重连状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconnectState {
    /// 空闲（未在重连）
    Idle,
    /// 等待重连
    Waiting {
        /// 当前尝试次数
        attempt: u32,
        /// 剩余等待时间
        remaining: Duration,
    },
    /// 正在重连
    Connecting {
        /// 当前尝试次数
        attempt: u32,
    },
    /// 重连成功
    Connected,
    /// 重连失败（达到最大次数）
    Failed {
        /// 总尝试次数
        total_attempts: u32,
    },
    /// 已禁用
    Disabled,
}

impl ReconnectState {
    /// 是否正在重连过程中
    pub fn is_reconnecting(&self) -> bool {
        matches!(self, Self::Waiting { .. } | Self::Connecting { .. })
    }

    /// 是否已完成（成功或失败）
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Connected | Self::Failed { .. } | Self::Disabled)
    }

    /// 获取用户友好的状态消息
    pub fn message(&self) -> String {
        match self {
            Self::Idle => "Not reconnecting".to_string(),
            Self::Waiting { attempt, remaining } => {
                format!(
                    "Waiting to reconnect (attempt {}, {:.1}s remaining)",
                    attempt,
                    remaining.as_secs_f64()
                )
            },
            Self::Connecting { attempt } => {
                format!("Reconnecting (attempt {})", attempt)
            },
            Self::Connected => "Reconnected successfully".to_string(),
            Self::Failed { total_attempts } => {
                format!("Reconnection failed after {} attempts", total_attempts)
            },
            Self::Disabled => "Auto-reconnect disabled".to_string(),
        }
    }

    /// 获取用户友好的状态消息（中文）
    pub fn message_cn(&self) -> String {
        match self {
            Self::Idle => "未在重连".to_string(),
            Self::Waiting { attempt, remaining } => {
                format!(
                    "等待重连中（第 {} 次尝试，还需 {:.1} 秒）",
                    attempt,
                    remaining.as_secs_f64()
                )
            },
            Self::Connecting { attempt } => {
                format!("正在重连（第 {} 次尝试）", attempt)
            },
            Self::Connected => "重连成功".to_string(),
            Self::Failed { total_attempts } => {
                format!("重连失败，共尝试 {} 次", total_attempts)
            },
            Self::Disabled => "自动重连已禁用".to_string(),
        }
    }
}

/// 重连事件
#[derive(Debug, Clone)]
pub enum ReconnectEvent {
    /// 开始重连
    Started,
    /// 等待中
    Waiting { delay: Duration, attempt: u32 },
    /// 尝试连接
    Attempting { attempt: u32 },
    /// 尝试失败
    AttemptFailed { attempt: u32, error: String },
    /// 重连成功
    Succeeded { total_attempts: u32 },
    /// 重连失败（放弃）
    GaveUp { total_attempts: u32 },
    /// 被取消
    Cancelled,
}

/// 重连事件回调
pub trait ReconnectCallback: Send + Sync {
    /// 处理重连事件
    fn on_event(&self, event: ReconnectEvent);
}

/// 日志重连回调
pub struct LoggingReconnectCallback;

impl ReconnectCallback for LoggingReconnectCallback {
    fn on_event(&self, event: ReconnectEvent) {
        match event {
            ReconnectEvent::Started => {
                info!("Starting reconnection process");
            },
            ReconnectEvent::Waiting { delay, attempt } => {
                info!("Waiting {:?} before reconnect attempt {}", delay, attempt);
            },
            ReconnectEvent::Attempting { attempt } => {
                info!("Reconnect attempt {}", attempt);
            },
            ReconnectEvent::AttemptFailed { attempt, error } => {
                warn!("Reconnect attempt {} failed: {}", attempt, error);
            },
            ReconnectEvent::Succeeded { total_attempts } => {
                info!("Reconnected successfully after {} attempts", total_attempts);
            },
            ReconnectEvent::GaveUp { total_attempts } => {
                warn!("Gave up reconnecting after {} attempts", total_attempts);
            },
            ReconnectEvent::Cancelled => {
                info!("Reconnection cancelled");
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_policy_default() {
        let policy = ReconnectPolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.max_attempts, DEFAULT_MAX_RECONNECT_ATTEMPTS);
        assert!(!policy.is_infinite());
    }

    #[test]
    fn test_reconnect_policy_disabled() {
        let policy = ReconnectPolicy::disabled();
        assert!(!policy.enabled);
    }

    #[test]
    fn test_reconnect_policy_infinite() {
        let policy = ReconnectPolicy::infinite();
        assert!(policy.is_infinite());
        assert_eq!(policy.max_attempts, 0);
    }

    #[test]
    fn test_reconnect_policy_builder() {
        let policy = ReconnectPolicy::new()
            .with_max_attempts(10)
            .with_initial_delay(Duration::from_secs(2))
            .with_max_delay(Duration::from_secs(120))
            .with_backoff_multiplier(3.0)
            .with_jitter(false);

        assert_eq!(policy.max_attempts, 10);
        assert_eq!(policy.initial_delay, Duration::from_secs(2));
        assert_eq!(policy.max_delay, Duration::from_secs(120));
        assert_eq!(policy.backoff_multiplier, 3.0);
        assert!(!policy.jitter);
    }

    #[test]
    fn test_exponential_backoff_basic() {
        let policy = ReconnectPolicy::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(100))
            .with_backoff_multiplier(2.0)
            .with_jitter(false);

        let mut backoff = ExponentialBackoff::new(policy);

        assert_eq!(backoff.attempt(), 0);
        assert!(backoff.can_retry());

        // 第一次
        let delay1 = backoff.next_delay().unwrap();
        assert_eq!(delay1, Duration::from_millis(100));
        assert_eq!(backoff.attempt(), 1);

        // 第二次
        let delay2 = backoff.next_delay().unwrap();
        assert_eq!(delay2, Duration::from_millis(200));
        assert_eq!(backoff.attempt(), 2);

        // 第三次
        let delay3 = backoff.next_delay().unwrap();
        assert_eq!(delay3, Duration::from_millis(400));
        assert_eq!(backoff.attempt(), 3);

        // 第四次应该返回 None
        assert!(backoff.next_delay().is_none());
        assert!(!backoff.can_retry());
    }

    #[test]
    fn test_exponential_backoff_max_delay() {
        let policy = ReconnectPolicy::new()
            .with_max_attempts(10)
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(500))
            .with_backoff_multiplier(2.0)
            .with_jitter(false);

        let mut backoff = ExponentialBackoff::new(policy);

        //跳过几次
        for _ in 0..5 {
            backoff.next_delay();
        }

        // 延迟应该被限制在 max_delay
        let delay = backoff.next_delay().unwrap();
        assert!(delay <= Duration::from_millis(500));
    }

    #[test]
    fn test_exponential_backoff_reset() {
        let policy = ReconnectPolicy::new()
            .with_max_attempts(3)
            .with_jitter(false);

        let mut backoff = ExponentialBackoff::new(policy);

        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.attempt(), 2);

        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert!(backoff.can_retry());
    }

    #[test]
    fn test_exponential_backoff_disabled() {
        let policy = ReconnectPolicy::disabled();
        let mut backoff = ExponentialBackoff::new(policy);

        assert!(backoff.next_delay().is_none());
        assert!(!backoff.can_retry());
    }

    #[test]
    fn test_exponential_backoff_remaining() {
        let policy = ReconnectPolicy::new().with_max_attempts(5);
        let mut backoff = ExponentialBackoff::new(policy);

        assert_eq!(backoff.remaining_attempts(), Some(5));

        backoff.next_delay();
        backoff.next_delay();

        assert_eq!(backoff.remaining_attempts(), Some(3));
    }

    #[test]
    fn test_exponential_backoff_infinite_remaining() {
        let policy = ReconnectPolicy::infinite();
        let backoff = ExponentialBackoff::new(policy);

        assert_eq!(backoff.remaining_attempts(), None);
    }

    #[test]
    fn test_reconnect_state_messages() {
        let state = ReconnectState::Waiting {
            attempt: 2,
            remaining: Duration::from_secs(5),
        };
        assert!(state.message().contains("attempt 2"));
        assert!(state.message_cn().contains("第 2 次"));

        let state = ReconnectState::Failed { total_attempts: 5 };
        assert!(state.message().contains("5 attempts"));
        assert!(state.message_cn().contains("5 次"));
    }

    #[test]
    fn test_reconnect_state_checks() {
        assert!(!ReconnectState::Idle.is_reconnecting());
        assert!(
            ReconnectState::Waiting {
                attempt: 1,
                remaining: Duration::from_secs(1)
            }
            .is_reconnecting()
        );
        assert!(ReconnectState::Connecting { attempt: 1 }.is_reconnecting());
        assert!(!ReconnectState::Connected.is_reconnecting());

        assert!(!ReconnectState::Idle.is_finished());
        assert!(ReconnectState::Connected.is_finished());
        assert!(ReconnectState::Failed { total_attempts: 3 }.is_finished());
        assert!(ReconnectState::Disabled.is_finished());
    }
}
