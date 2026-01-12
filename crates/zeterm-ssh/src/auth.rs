//! 认证模块
//!
//! 提供 SSH 认证的重试支持和多种认证策略。

use std::path::PathBuf;
use std::time::Duration;

use tracing::{debug, info, warn};
use zeterm_core::errors::AuthError;

/// 默认最大重试次数
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// 默认重试间隔（毫秒）
pub const DEFAULT_RETRY_DELAY_MS: u64 = 500;

/// 认证重试配置
#[derive(Debug, Clone)]
pub struct AuthRetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔
    pub retry_delay: Duration,
    /// 是否在重试前提示用户
    pub prompt_before_retry: bool,
    /// 是否允许回退到其他认证方式
    pub allow_fallback: bool,
}

impl Default for AuthRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay: Duration::from_millis(DEFAULT_RETRY_DELAY_MS),
            prompt_before_retry: true,
            allow_fallback: true,
        }
    }
}

impl AuthRetryConfig {
    /// 创建新的重试配置
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// 设置重试间隔
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// 设置是否在重试前提示用户
    pub fn with_prompt_before_retry(mut self, prompt: bool) -> Self {
        self.prompt_before_retry = prompt;
        self
    }

    /// 设置是否允许回退
    pub fn with_allow_fallback(mut self, allow: bool) -> Self {
        self.allow_fallback = allow;
        self
    }

    ///禁用重试
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }
}

/// 认证尝试结果
#[derive(Debug, Clone)]
pub enum AuthAttemptResult {
    /// 认证成功
    Success,
    /// 认证失败，可重试
    RetryableFailure {
        error: AuthError,
        attempt: u32,
        remaining: u32,
    },
    /// 认证失败，不可重试
    FinalFailure { error: AuthError, attempts: u32 },
    /// 用户取消
    Cancelled,
}

impl AuthAttemptResult {
    /// 是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// 是否可以重试
    pub fn can_retry(&self) -> bool {
        matches!(self, Self::RetryableFailure { .. })
    }

    /// 获取错误（如果有）
    pub fn error(&self) -> Option<&AuthError> {
        match self {
            Self::RetryableFailure { error, .. } | Self::FinalFailure { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// 认证方法类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethodType {
    /// 密码认证
    Password,
    /// 公钥认证
    PublicKey,
    /// SSH Agent 认证
    Agent,
    /// 键盘交互认证
    KeyboardInteractive,
}

impl std::fmt::Display for AuthMethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password => write!(f, "Password"),
            Self::PublicKey => write!(f, "Public Key"),
            Self::Agent => write!(f, "SSH Agent"),
            Self::KeyboardInteractive => write!(f, "Keyboard Interactive"),
        }
    }
}

/// 认证凭据
#[derive(Debug, Clone)]
pub enum AuthCredential {
    /// 密码
    Password(String),
    /// 私钥文件
    PrivateKey {
        path: PathBuf,
        passphrase: Option<String>,
    },
    /// SSH Agent
    Agent,
}

impl AuthCredential {
    /// 获取认证方法类型
    pub fn method_type(&self) -> AuthMethodType {
        match self {
            Self::Password(_) => AuthMethodType::Password,
            Self::PrivateKey { .. } => AuthMethodType::PublicKey,
            Self::Agent => AuthMethodType::Agent,
        }
    }
}

/// 认证状态
#[derive(Debug, Clone)]
pub struct AuthState {
    /// 当前尝试次数
    pub attempt: u32,
    /// 最大尝试次数
    pub max_attempts: u32,
    /// 当前使用的认证方法
    pub current_method: AuthMethodType,
    /// 已尝试的认证方法
    pub tried_methods: Vec<AuthMethodType>,
    /// 最后一次错误
    pub last_error: Option<AuthError>,
}

impl AuthState {
    /// 创建新的认证状态
    pub fn new(method: AuthMethodType, max_attempts: u32) -> Self {
        Self {
            attempt: 0,
            max_attempts,
            current_method: method,
            tried_methods: Vec::new(),
            last_error: None,
        }
    }

    /// 记录一次尝试
    pub fn record_attempt(&mut self, error: Option<AuthError>) {
        self.attempt += 1;
        self.last_error = error;
    }

    /// 切换到新的认证方法
    pub fn switch_method(&mut self, method: AuthMethodType) {
        self.tried_methods.push(self.current_method.clone());
        self.current_method = method;
        self.attempt = 0;
        self.last_error = None;
    }

    /// 检查是否还可以重试
    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_attempts
    }

    /// 获取剩余尝试次数
    pub fn remaining_attempts(&self) -> u32 {
        self.max_attempts.saturating_sub(self.attempt)
    }

    /// 检查某个方法是否已尝试过
    pub fn has_tried(&self, method: &AuthMethodType) -> bool {
        self.tried_methods.contains(method) || &self.current_method == method
    }
}

/// 密码提供者回调类型
pub type PasswordProvider = Box<dyn Fn(u32) -> Option<String> + Send + Sync>;

/// 密码重试器
pub struct PasswordRetrier {
    /// 重试配置
    config: AuthRetryConfig,
    /// 当前状态
    state: AuthState,
}

impl PasswordRetrier {
    /// 创建新的密码重试器
    pub fn new(config: AuthRetryConfig) -> Self {
        Self {
            state: AuthState::new(AuthMethodType::Password, config.max_retries + 1),
            config,
        }
    }

    /// 获取当前尝试次数
    pub fn current_attempt(&self) -> u32 {
        self.state.attempt
    }

    /// 获取剩余尝试次数
    pub fn remaining_attempts(&self) -> u32 {
        self.state.remaining_attempts()
    }

    /// 检查是否可以重试
    pub fn can_retry(&self) -> bool {
        self.state.can_retry()
    }

    /// 记录认证失败
    pub fn record_failure(&mut self, error: AuthError) -> AuthAttemptResult {
        self.state.record_attempt(Some(error.clone()));

        if error.is_retryable() && self.state.can_retry() {
            info!(
                "Authentication failed (attempt {}/{}), can retry: {}",
                self.state.attempt,
                self.state.max_attempts,
                error.user_message_en()
            );
            AuthAttemptResult::RetryableFailure {
                error,
                attempt: self.state.attempt,
                remaining: self.state.remaining_attempts(),
            }
        } else {
            warn!(
                "Authentication failed after {} attempts: {}",
                self.state.attempt,
                error.user_message_en()
            );
            AuthAttemptResult::FinalFailure {
                error,
                attempts: self.state.attempt,
            }
        }
    }

    /// 记录认证成功
    pub fn record_success(&mut self) -> AuthAttemptResult {
        self.state.record_attempt(None);
        info!(
            "Authentication successful on attempt {}",
            self.state.attempt
        );
        AuthAttemptResult::Success
    }

    /// 获取重试延迟
    pub fn retry_delay(&self) -> Duration {
        self.config.retry_delay
    }

    /// 是否需要在重试前提示用户
    pub fn should_prompt(&self) -> bool {
        self.config.prompt_before_retry
    }
}

/// 认证策略
#[derive(Debug, Clone)]
pub struct AuthStrategy {
    /// 认证方法优先级列表
    pub methods: Vec<AuthCredential>,
    /// 重试配置
    pub retry_config: AuthRetryConfig,
}

impl AuthStrategy {
    /// 创建仅密码认证策略
    pub fn password_only(password: impl Into<String>) -> Self {
        Self {
            methods: vec![AuthCredential::Password(password.into())],
            retry_config: AuthRetryConfig::default(),
        }
    }

    /// 创建仅公钥认证策略
    pub fn key_only(key_path: impl Into<PathBuf>, passphrase: Option<String>) -> Self {
        Self {
            methods: vec![AuthCredential::PrivateKey {
                path: key_path.into(),
                passphrase,
            }],
            retry_config: AuthRetryConfig::default(),
        }
    }

    /// 创建 Agent 优先策略
    pub fn agent_first() -> Self {
        Self {
            methods: vec![AuthCredential::Agent],
            retry_config: AuthRetryConfig::default(),
        }
    }

    /// 添加回退认证方法
    pub fn with_fallback(mut self, credential: AuthCredential) -> Self {
        self.methods.push(credential);
        self
    }

    /// 设置重试配置
    pub fn with_retry_config(mut self, config: AuthRetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// 获取下一个认证方法
    pub fn next_method(&self, tried: &[AuthMethodType]) -> Option<&AuthCredential> {
        self.methods
            .iter()
            .find(|cred| !tried.contains(&cred.method_type()))
    }

    /// 检查是否有更多认证方法可用
    pub fn has_more_methods(&self, tried: &[AuthMethodType]) -> bool {
        self.next_method(tried).is_some()
    }
}

/// 认证进度回调
pub trait AuthProgressCallback: Send + Sync {
    /// 认证开始
    fn on_auth_start(&self, method: &AuthMethodType);

    /// 认证尝试
    fn on_auth_attempt(&self, attempt: u32, max_attempts: u32);

    /// 认证失败（可重试）
    fn on_auth_retry(&self, error: &AuthError, remaining: u32);

    /// 认证方法切换
    fn on_method_switch(&self, from: &AuthMethodType, to: &AuthMethodType);

    /// 认证成功
    fn on_auth_success(&self, method: &AuthMethodType, attempts: u32);

    /// 认证最终失败
    fn on_auth_final_failure(&self, error: &AuthError, total_attempts: u32);
}

/// 默认认证进度回调（仅日志）
pub struct LoggingAuthCallback;

impl AuthProgressCallback for LoggingAuthCallback {
    fn on_auth_start(&self, method: &AuthMethodType) {
        info!("Starting authentication with method: {}", method);
    }

    fn on_auth_attempt(&self, attempt: u32, max_attempts: u32) {
        debug!("Authentication attempt {}/{}", attempt, max_attempts);
    }

    fn on_auth_retry(&self, error: &AuthError, remaining: u32) {
        warn!(
            "Authentication failed: {}. {} attempts remaining.",
            error.user_message_en(),
            remaining
        );
    }

    fn on_method_switch(&self, from: &AuthMethodType, to: &AuthMethodType) {
        info!("Switching authentication method from {} to {}", from, to);
    }

    fn on_auth_success(&self, method: &AuthMethodType, attempts: u32) {
        info!(
            "Authentication successful with {} after {} attempt(s)",
            method, attempts
        );
    }

    fn on_auth_final_failure(&self, error: &AuthError, total_attempts: u32) {
        warn!(
            "Authentication failed after {} attempts: {}",
            total_attempts,
            error.user_message_en()
        );
    }
}

/// 将russh 认证错误转换为 AuthError
pub fn convert_auth_error(error: &str) -> AuthError {
    let error_lower = error.to_lowercase();

    if error_lower.contains("password")
        || error_lower.contains("authentication failed")
        || error_lower.contains("permission denied")
    {
        AuthError::InvalidPassword
    } else if error_lower.contains("key") && error_lower.contains("invalid") {
        AuthError::InvalidPrivateKey
    } else if error_lower.contains("passphrase") || error_lower.contains("decrypt") {
        AuthError::InvalidKeyPassphrase
    } else if error_lower.contains("not found") || error_lower.contains("no such file") {
        AuthError::KeyFileNotFound(error.to_string())
    } else if error_lower.contains("host key") {
        AuthError::HostKeyVerificationFailed
    } else if error_lower.contains("timeout") {
        AuthError::Timeout
    } else if error_lower.contains("cancelled") || error_lower.contains("canceled") {
        AuthError::Cancelled
    } else if error_lower.contains("not supported") || error_lower.contains("unsupported") {
        AuthError::UnsupportedMethod
    } else {
        // 默认为密码错误（最常见的情况）
        AuthError::InvalidPassword
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_retry_config_default() {
        let config = AuthRetryConfig::default();
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert!(config.prompt_before_retry);
        assert!(config.allow_fallback);
    }

    #[test]
    fn test_auth_retry_config_no_retry() {
        let config = AuthRetryConfig::no_retry();
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_auth_state_new() {
        let state = AuthState::new(AuthMethodType::Password, 3);
        assert_eq!(state.attempt, 0);
        assert_eq!(state.max_attempts, 3);
        assert!(state.can_retry());
        assert_eq!(state.remaining_attempts(), 3);
    }

    #[test]
    fn test_auth_state_record_attempt() {
        let mut state = AuthState::new(AuthMethodType::Password, 3);

        state.record_attempt(Some(AuthError::InvalidPassword));
        assert_eq!(state.attempt, 1);
        assert!(state.can_retry());
        assert_eq!(state.remaining_attempts(), 2);

        state.record_attempt(Some(AuthError::InvalidPassword));
        state.record_attempt(Some(AuthError::InvalidPassword));
        assert!(!state.can_retry());
        assert_eq!(state.remaining_attempts(), 0);
    }

    #[test]
    fn test_auth_state_switch_method() {
        let mut state = AuthState::new(AuthMethodType::Password, 3);
        state.record_attempt(Some(AuthError::InvalidPassword));

        state.switch_method(AuthMethodType::PublicKey);
        assert_eq!(state.current_method, AuthMethodType::PublicKey);
        assert_eq!(state.attempt, 0);
        assert!(state.has_tried(&AuthMethodType::Password));
        assert!(!state.has_tried(&AuthMethodType::Agent));
    }

    #[test]
    fn test_password_retrier() {
        let config = AuthRetryConfig::new(2);
        let mut retrier = PasswordRetrier::new(config);

        assert_eq!(retrier.current_attempt(), 0);
        assert_eq!(retrier.remaining_attempts(), 3); // max_retries + 1

        // 第一次失败
        let result = retrier.record_failure(AuthError::InvalidPassword);
        assert!(result.can_retry());

        // 第二次失败
        let result = retrier.record_failure(AuthError::InvalidPassword);
        assert!(result.can_retry());

        // 第三次失败
        let result = retrier.record_failure(AuthError::InvalidPassword);
        assert!(!result.can_retry());
    }

    #[test]
    fn test_password_retrier_success() {
        let config = AuthRetryConfig::new(2);
        let mut retrier = PasswordRetrier::new(config);

        retrier.record_failure(AuthError::InvalidPassword);
        let result = retrier.record_success();
        assert!(result.is_success());
    }

    #[test]
    fn test_auth_strategy_password_only() {
        let strategy = AuthStrategy::password_only("secret");
        assert_eq!(strategy.methods.len(), 1);
        assert!(matches!(&strategy.methods[0], AuthCredential::Password(_)));
    }

    #[test]
    fn test_auth_strategy_with_fallback() {
        let strategy = AuthStrategy::password_only("secret").with_fallback(AuthCredential::Agent);

        assert_eq!(strategy.methods.len(), 2);

        let tried = vec![AuthMethodType::Password];
        let next = strategy.next_method(&tried);
        assert!(next.is_some());
        assert!(matches!(next.unwrap(), AuthCredential::Agent));
    }

    #[test]
    fn test_convert_auth_error() {
        assert!(matches!(
            convert_auth_error("Authentication failed"),
            AuthError::InvalidPassword
        ));
        assert!(matches!(
            convert_auth_error("Invalid key format"),
            AuthError::InvalidPrivateKey
        ));
        assert!(matches!(
            convert_auth_error("Bad passphrase"),
            AuthError::InvalidKeyPassphrase
        ));
        assert!(matches!(
            convert_auth_error("Connection timeout"),
            AuthError::Timeout
        ));
    }

    #[test]
    fn test_auth_attempt_result() {
        let success = AuthAttemptResult::Success;
        assert!(success.is_success());
        assert!(!success.can_retry());
        assert!(success.error().is_none());

        let retryable = AuthAttemptResult::RetryableFailure {
            error: AuthError::InvalidPassword,
            attempt: 1,
            remaining: 2,
        };
        assert!(!retryable.is_success());
        assert!(retryable.can_retry());
        assert!(retryable.error().is_some());

        let final_fail = AuthAttemptResult::FinalFailure {
            error: AuthError::InvalidPassword,
            attempts: 3,
        };
        assert!(!final_fail.is_success());
        assert!(!final_fail.can_retry());
    }

    #[test]
    fn test_auth_credential_method_type() {
        let password = AuthCredential::Password("test".into());
        assert_eq!(password.method_type(), AuthMethodType::Password);

        let key = AuthCredential::PrivateKey {
            path: PathBuf::from("/path/to/key"),
            passphrase: None,
        };
        assert_eq!(key.method_type(), AuthMethodType::PublicKey);

        let agent = AuthCredential::Agent;
        assert_eq!(agent.method_type(), AuthMethodType::Agent);
    }
}
