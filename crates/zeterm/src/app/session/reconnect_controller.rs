//! 重连控制器
//!
//! 负责管理连接断开后的自动重连逻辑，包括：
//! - 监听连接丢失事件
//! - 使用指数退避算法计算重连延迟
//! - 协调重连过程
//! - 重连成功后重启数据泵

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use zeterm_core::state::{ConnectionEvent, ConnectionState};
use zeterm_ssh::{
    ExponentialBackoff, ReconnectCallback, ReconnectEvent, ReconnectPolicy, ReconnectState,
    SshConfig,
};

/// 重连控制器配置
#[derive(Debug, Clone)]
pub struct ReconnectControllerConfig {
    /// 重连策略
    pub policy: ReconnectPolicy,
    /// 是否在首次连接失败时也进行重连
    pub reconnect_on_initial_failure: bool,
}

impl Default for ReconnectControllerConfig {
    fn default() -> Self {
        Self {
            policy: ReconnectPolicy::default(),
            reconnect_on_initial_failure: false,
        }
    }
}

/// 连接工厂 trait
///
/// 用于创建新的连接实例，因为 SSH 连接是一次性的，
/// 重连时需要创建新的连接对象。
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync {
    /// 创建新的 SSH 连接
    async fn create_connection(
        &self,
    ) -> Result<zeterm_ssh::SshConnection, zeterm_core::ConnectionError>;

    /// 获取连接配置（用于日志和显示）
    fn config(&self) -> &SshConfig;
}

/// SSH 连接工厂实现
pub struct SshConnectionFactory {
    config: SshConfig,
    host_key_callback: Option<zeterm_ssh::HostKeyConfirmCallback>,
}

impl SshConnectionFactory {
    /// 创建新的 SSH 连接工厂
    pub fn new(config: SshConfig) -> Self {
        Self {
            config,
            host_key_callback: None,
        }
    }

    /// 设置主机密钥确认回调
    pub fn with_host_key_callback(mut self, callback: zeterm_ssh::HostKeyConfirmCallback) -> Self {
        self.host_key_callback = Some(callback);
        self
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SshConnectionFactory {
    async fn create_connection(
        &self,
    ) -> Result<zeterm_ssh::SshConnection, zeterm_core::ConnectionError> {
        let mut connection = zeterm_ssh::SshConnection::new(self.config.clone());

        if let Some(ref callback) = self.host_key_callback {
            connection = connection.with_host_key_confirm_callback(callback.clone());
        }

        Ok(connection)
    }

    fn config(&self) -> &SshConfig {
        &self.config
    }
}

/// 重连控制器事件
#[derive(Debug, Clone)]
pub enum ReconnectControllerEvent {
    /// 开始重连
    Started,
    /// 等待重连（包含延迟和尝试次数）
    Waiting { delay: Duration, attempt: u32 },
    /// 正在尝试连接
    Attempting { attempt: u32 },
    /// 尝试失败
    AttemptFailed { attempt: u32, error: String },
    /// 重连成功
    Succeeded { total_attempts: u32 },
    /// 放弃重连
    GaveUp { total_attempts: u32 },
    /// 重连被取消
    Cancelled,
    /// 状态更新
    StateChanged(ReconnectState),
}

/// 重连控制器
///
/// 管理连接断开后的自动重连逻辑。
pub struct ReconnectController {
    /// 配置
    config: ReconnectControllerConfig,
    /// 连接工厂
    factory: Option<Arc<dyn ConnectionFactory>>,
    /// 退避计算器
    backoff: RwLock<ExponentialBackoff>,
    /// 当前状态
    state: RwLock<ReconnectState>,
    /// 是否正在重连
    reconnecting: AtomicBool,
    /// 取消标志
    cancelled: AtomicBool,
    /// 事件发送器
    event_tx: RwLock<Option<mpsc::UnboundedSender<ReconnectControllerEvent>>>,
    /// 重连回调列表
    callbacks: RwLock<Vec<Arc<dyn ReconnectCallback>>>,
}

impl ReconnectController {
    /// 创建新的重连控制器
    pub fn new(config: ReconnectControllerConfig) -> Self {
        let backoff = ExponentialBackoff::new(config.policy.clone());

        Self {
            config,
            factory: None,
            backoff: RwLock::new(backoff),
            state: RwLock::new(ReconnectState::Idle),
            reconnecting: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            event_tx: RwLock::new(None),
            callbacks: RwLock::new(Vec::new()),
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(ReconnectControllerConfig::default())
    }

    /// 设置连接工厂
    pub fn set_factory(&mut self, factory: Arc<dyn ConnectionFactory>) {
        self.factory = Some(factory);
    }

    /// 获取连接工厂
    pub fn factory(&self) -> Option<Arc<dyn ConnectionFactory>> {
        self.factory.clone()
    }

    /// 添加重连回调
    pub fn add_callback(&self, callback: Arc<dyn ReconnectCallback>) {
        self.callbacks.write().push(callback);
    }

    /// 订阅重连事件
    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<ReconnectControllerEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.event_tx.write() = Some(tx);
        rx
    }

    /// 获取当前状态
    pub fn state(&self) -> ReconnectState {
        self.state.read().clone()
    }

    /// 是否正在重连
    pub fn is_reconnecting(&self) -> bool {
        self.reconnecting.load(Ordering::SeqCst)
    }

    /// 是否启用重连
    pub fn is_enabled(&self) -> bool {
        self.config.policy.enabled
    }

    /// 获取策略配置
    pub fn policy(&self) -> &ReconnectPolicy {
        &self.config.policy
    }

    /// 取消重连
    pub fn cancel(&self) {
        if self.is_reconnecting() {
            info!("Cancelling reconnection");
            self.cancelled.store(true, Ordering::SeqCst);
            self.set_state(ReconnectState::Idle);
            self.emit_event(ReconnectControllerEvent::Cancelled);
            self.notify_callbacks(ReconnectEvent::Cancelled);
        }
    }

    /// 重置重连状态
    pub fn reset(&self) {
        self.backoff.write().reset();
        self.cancelled.store(false, Ordering::SeqCst);
        self.reconnecting.store(false, Ordering::SeqCst);
        self.set_state(ReconnectState::Idle);
    }

    /// 处理连接状态变化
    ///
    /// 当连接状态变化时调用，检测是否需要启动重连。
    pub fn on_connection_state_changed(
        &self,
        old_state: &ConnectionState,
        new_state: &ConnectionState,
    ) {
        match (old_state, new_state) {
            // 从已连接状态变为断开（非用户主动），触发重连
            (ConnectionState::Connected { .. }, ConnectionState::Disconnected { reason }) => {
                use zeterm_core::state::DisconnectReason;

                // 用户主动断开不触发重连
                if matches!(reason, DisconnectReason::UserInitiated) {
                    debug!("User initiated disconnect, not reconnecting");
                    return;
                }

                info!(
                    "Connection lost (reason: {:?}), checking reconnect policy",
                    reason
                );

                if self.config.policy.enabled {
                    self.trigger_reconnect();
                }
            },
            // 重连状态
            (_, ConnectionState::Reconnecting { attempt }) => {
                self.set_state(ReconnectState::Connecting { attempt: *attempt });
            },
            // 重连成功
            (ConnectionState::Reconnecting { .. }, ConnectionState::Connected { .. }) => {
                let attempt = self.backoff.read().attempt();
                self.on_reconnect_success(attempt);
            },
            _ => {},
        }
    }

    /// 触发重连
    fn trigger_reconnect(&self) {
        if !self.config.policy.enabled {
            warn!("Reconnect triggered but policy is disabled");
            return;
        }

        if self.is_reconnecting() {
            debug!("Already reconnecting, ignoring trigger");
            return;
        }

        info!("Triggering reconnection");
        self.reconnecting.store(true, Ordering::SeqCst);
        self.cancelled.store(false, Ordering::SeqCst);
        self.set_state(ReconnectState::Waiting {
            attempt: 1,
            remaining: self.backoff.read().policy().initial_delay,
        });
        self.emit_event(ReconnectControllerEvent::Started);
        self.notify_callbacks(ReconnectEvent::Started);
    }

    /// 获取下一次重连延迟
    ///
    /// 返回 None 表示已达到最大尝试次数
    pub fn next_delay(&self) -> Option<Duration> {
        if self.cancelled.load(Ordering::SeqCst) {
            return None;
        }

        self.backoff.write().next_delay()
    }

    /// 开始重连尝试
    ///
    /// 返回是否应该继续尝试
    pub fn begin_attempt(&self) -> bool {
        if self.cancelled.load(Ordering::SeqCst) {
            return false;
        }

        let attempt = self.backoff.read().attempt();
        info!("Beginning reconnect attempt {}", attempt);

        self.set_state(ReconnectState::Connecting { attempt });
        self.emit_event(ReconnectControllerEvent::Attempting { attempt });
        self.notify_callbacks(ReconnectEvent::Attempting { attempt });

        true
    }

    /// 报告重连尝试失败
    pub fn on_attempt_failed(&self, error: &str) {
        let attempt = self.backoff.read().attempt();
        warn!("Reconnect attempt {} failed: {}", attempt, error);

        self.emit_event(ReconnectControllerEvent::AttemptFailed {
            attempt,
            error: error.to_string(),
        });
        self.notify_callbacks(ReconnectEvent::AttemptFailed {
            attempt,
            error: error.to_string(),
        });

        // 检查是否还能重试
        if !self.backoff.read().can_retry() {
            self.on_gave_up();
        } else if let Some(delay) = self.next_delay() {
            self.set_state(ReconnectState::Waiting {
                attempt: attempt + 1,
                remaining: delay,
            });
            self.emit_event(ReconnectControllerEvent::Waiting {
                delay,
                attempt: attempt + 1,
            });
            self.notify_callbacks(ReconnectEvent::Waiting {
                delay,
                attempt: attempt + 1,
            });
        }
    }

    /// 报告重连成功
    fn on_reconnect_success(&self, total_attempts: u32) {
        info!("Reconnection successful after {} attempts", total_attempts);

        self.reconnecting.store(false, Ordering::SeqCst);
        self.set_state(ReconnectState::Connected);

        // 根据策略决定是否重置退避
        if self.config.policy.reset_on_success {
            self.backoff.write().reset();
        }

        self.emit_event(ReconnectControllerEvent::Succeeded { total_attempts });
        self.notify_callbacks(ReconnectEvent::Succeeded { total_attempts });
    }

    /// 放弃重连
    fn on_gave_up(&self) {
        let total_attempts = self.backoff.read().attempt();
        error!("Gave up reconnecting after {} attempts", total_attempts);

        self.reconnecting.store(false, Ordering::SeqCst);
        self.set_state(ReconnectState::Failed { total_attempts });

        self.emit_event(ReconnectControllerEvent::GaveUp { total_attempts });
        self.notify_callbacks(ReconnectEvent::GaveUp { total_attempts });
    }

    /// 设置状态
    fn set_state(&self, state: ReconnectState) {
        *self.state.write() = state.clone();
        self.emit_event(ReconnectControllerEvent::StateChanged(state));
    }

    /// 发送事件
    fn emit_event(&self, event: ReconnectControllerEvent) {
        if let Some(ref tx) = *self.event_tx.read() {
            let _ = tx.send(event);
        }
    }

    /// 通知回调
    fn notify_callbacks(&self, event: ReconnectEvent) {
        for callback in self.callbacks.read().iter() {
            callback.on_event(&event);
        }
    }

    /// 可否重试
    pub fn can_retry(&self) -> bool {
        self.backoff.read().can_retry()
    }

    /// 获取剩余尝试次数
    pub fn remaining_attempts(&self) -> Option<u32> {
        self.backoff.read().remaining_attempts()
    }

    /// 获取当前尝试次数
    pub fn current_attempt(&self) -> u32 {
        self.backoff.read().attempt()
    }
}

impl Default for ReconnectController {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl std::fmt::Debug for ReconnectController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReconnectController")
            .field("config", &self.config)
            .field("state", &self.state.read())
            .field("reconnecting", &self.reconnecting.load(Ordering::SeqCst))
            .field("cancelled", &self.cancelled.load(Ordering::SeqCst))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_controller_new() {
        let controller = ReconnectController::with_defaults();
        assert!(!controller.is_reconnecting());
        assert!(controller.is_enabled());
        assert_eq!(controller.state(), ReconnectState::Idle);
    }

    #[test]
    fn test_reconnect_controller_disabled() {
        let config = ReconnectControllerConfig {
            policy: ReconnectPolicy::disabled(),
            reconnect_on_initial_failure: false,
        };
        let controller = ReconnectController::new(config);
        assert!(!controller.is_enabled());
    }

    #[test]
    fn test_reconnect_controller_cancel() {
        let controller = ReconnectController::with_defaults();
        controller.reconnecting.store(true, Ordering::SeqCst);
        controller.cancel();
        assert!(!controller.is_reconnecting());
    }

    #[test]
    fn test_reconnect_controller_reset() {
        let controller = ReconnectController::with_defaults();
        controller.reconnecting.store(true, Ordering::SeqCst);
        controller.cancelled.store(true, Ordering::SeqCst);
        controller.reset();
        assert!(!controller.is_reconnecting());
        assert!(!controller.cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn test_reconnect_controller_next_delay() {
        let controller = ReconnectController::with_defaults();

        // 首次延迟应该是初始延迟
        let delay1 = controller.next_delay();
        assert!(delay1.is_some());

        // 后续延迟应该增加
        let delay2 = controller.next_delay();
        assert!(delay2.is_some());
    }

    #[test]
    fn test_reconnect_controller_cancelled_no_delay() {
        let controller = ReconnectController::with_defaults();
        controller.cancelled.store(true, Ordering::SeqCst);

        assert!(controller.next_delay().is_none());
    }
}
