//! 日志系统初始化模块
//!
//! 提供 Zeterm 的日志配置和初始化功能。

// 允许暂时未使用但将来会用到的代码
#![allow(dead_code)]

use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// 日志配置
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// 日志级别
    pub level: Level,
    /// 是否显示目标模块
    pub with_target: bool,
    /// 是否显示文件名和行号
    pub with_file: bool,
    /// 是否显示线程 ID
    pub with_thread_ids: bool,
    /// 是否使用 ANSI 颜色
    pub with_ansi: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            with_target: true,
            with_file: false,
            with_thread_ids: false,
            with_ansi: true,
        }
    }
}

impl LogConfig {
    /// 创建开发环境配置
    pub fn development() -> Self {
        Self {
            level: Level::DEBUG,
            with_target: true,
            with_file: true,
            with_thread_ids: true,
            with_ansi: true,
        }
    }

    /// 创建生产环境配置
    pub fn production() -> Self {
        Self {
            level: Level::INFO,
            with_target: false,
            with_file: false,
            with_thread_ids: false,
            with_ansi: false,
        }
    }
}

/// 初始化日志系统
///
/// # Arguments
///
/// * `config` - 日志配置
///
/// # Example
///
/// ```ignore
/// use zeterm::logging::{init_logging, LogConfig};
///
/// init_logging(LogConfig::default());
/// ```
pub fn init_logging(config: LogConfig) {
    // 创建环境过滤器，允许通过 RUST_LOG 环境变量覆盖
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "zeterm={},zeterm_core={},zeterm_mock={},zeterm_ssh={},zeterm_storage={}",
            config.level, config.level, config.level, config.level, config.level,
        ))
    });

    // 创建格式化层
    let fmt_layer = fmt::layer()
        .with_target(config.with_target)
        .with_file(config.with_file)
        .with_line_number(config.with_file)
        .with_thread_ids(config.with_thread_ids)
        .with_ansi(config.with_ansi);

    // 初始化订阅者
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("Logging initialized with level: {:?}", config.level);
}

/// 使用默认配置初始化日志系统
pub fn init_default_logging() {
    init_logging(LogConfig::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert!(config.with_target);
        assert!(config.with_ansi);
    }

    #[test]
    fn test_log_config_development() {
        let config = LogConfig::development();
        assert_eq!(config.level, Level::DEBUG);
        assert!(config.with_file);
        assert!(config.with_thread_ids);
    }

    #[test]
    fn test_log_config_production() {
        let config = LogConfig::production();
        assert_eq!(config.level, Level::INFO);
        assert!(!config.with_file);
        assert!(!config.with_ansi);
    }
}
