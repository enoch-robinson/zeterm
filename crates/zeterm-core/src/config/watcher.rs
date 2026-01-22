//! 配置文件监视模块
//!
//! 提供配置文件热更新功能，当配置文件发生变化时自动通知应用程序。
//!
//! # 功能
//!
//! - 监视 config.toml 和 hosts.toml 文件变化
//! - 事件去抖动（debounce）防止重复通知
//! - 支持多个监听器
//! - 线程安全
//!
//! # 示例
//!
//! ```ignore
//! use zeterm_core::config::{ConfigWatcher, ConfigChangeEvent};
//!
//! // 创建配置监视器
//! let watcher = ConfigWatcher::new()?;
//!
//! // 注册回调
//! watcher.on_change(|event| {
//!     match event {
//!         ConfigChangeEvent::AppConfigChanged => {
//!             println!("Application config changed, reloading...");
//!         }
//!         ConfigChangeEvent::HostsConfigChanged => {
//!             println!("Hosts config changed, reloading...");
//!         }
//!     }
//! });
//!
//! // 启动监视
//! watcher.start()?;
//!
//! // 停止监视
//! watcher.stop();
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::{Mutex, RwLock};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::{default_config_path, default_hosts_path};

// ============================================================================
// 错误类型
// ============================================================================

/// 配置监视错误
#[derive(Debug, Error)]
pub enum ConfigWatcherError {
    /// 无法创建监视器
    #[error("Failed to create watcher: {0}")]
    CreateWatcher(String),

    /// 无法添加监视路径
    #[error("Failed to watch path {path}: {message}")]
    WatchPath { path: String, message: String },

    /// 无法获取配置路径
    #[error("Failed to get config path: {0}")]
    ConfigPath(String),

    /// 监视器已在运行
    #[error("Watcher is already running")]
    AlreadyRunning,

    /// 监视器未运行
    #[error("Watcher is not running")]
    NotRunning,

    /// 其他错误
    #[error("Config watcher error: {0}")]
    Other(String),
}

/// 配置监视结果类型
pub type ConfigWatcherResult<T> = Result<T, ConfigWatcherError>;

// ============================================================================
// 事件定义
// ============================================================================

/// 配置变更事件
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigChangeEvent {
    /// 应用配置 (config.toml) 发生变化
    AppConfigChanged,
    /// 主机配置 (hosts.toml) 发生变化
    HostsConfigChanged,
    /// 配置目录发生变化（可能有新文件）
    ConfigDirChanged,
    /// 配置文件被删除
    ConfigDeleted(ConfigFileType),
    /// 配置文件被创建
    ConfigCreated(ConfigFileType),
}

/// 配置文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigFileType {
    /// 应用配置
    AppConfig,
    /// 主机配置
    HostsConfig,
    /// 其他配置文件
    Other,
}

impl ConfigFileType {
    /// 从文件名获取配置类型
    pub fn from_filename(filename: &str) -> Self {
        match filename {
            "config.toml" => ConfigFileType::AppConfig,
            "hosts.toml" => ConfigFileType::HostsConfig,
            _ => ConfigFileType::Other,
        }
    }

    /// 获取文件名
    pub fn filename(&self) -> &'static str {
        match self {
            ConfigFileType::AppConfig => "config.toml",
            ConfigFileType::HostsConfig => "hosts.toml",
            ConfigFileType::Other => "unknown",
        }
    }
}

// ============================================================================
// 配置变更回调
// ============================================================================

/// 配置变更回调类型
pub type ConfigChangeCallback = Arc<dyn Fn(ConfigChangeEvent) + Send + Sync>;

// ============================================================================
// 配置监视器
// ============================================================================

/// 配置监视器配置
#[derive(Debug, Clone)]
pub struct ConfigWatcherConfig {
    /// 去抖动延迟（毫秒）
    pub debounce_ms: u64,
    /// 是否监视应用配置
    pub watch_app_config: bool,
    /// 是否监视主机配置
    pub watch_hosts_config: bool,
    /// 是否监视整个配置目录
    pub watch_config_dir: bool,
}

impl Default for ConfigWatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            watch_app_config: true,
            watch_hosts_config: true,
            watch_config_dir: false,
        }
    }
}

/// 配置监视器
pub struct ConfigWatcher {
    /// 配置
    config: ConfigWatcherConfig,
    /// 监视的路径
    watched_paths: RwLock<Vec<PathBuf>>,
    /// 变更回调列表
    callbacks: RwLock<Vec<ConfigChangeCallback>>,
    /// 是否正在运行
    running: Arc<AtomicBool>,
    /// 停止信号发送器
    stop_tx: Mutex<Option<mpsc::Sender<()>>>,
    /// 最后事件时间（用于去抖动）
    last_events: RwLock<std::collections::HashMap<PathBuf, Instant>>,
}

impl ConfigWatcher {
    /// 创建新的配置监视器（使用默认配置）
    pub fn new() -> ConfigWatcherResult<Self> {
        Self::with_config(ConfigWatcherConfig::default())
    }

    /// 创建新的配置监视器（指定配置）
    pub fn with_config(config: ConfigWatcherConfig) -> ConfigWatcherResult<Self> {
        Ok(Self {
            config,
            watched_paths: RwLock::new(Vec::new()),
            callbacks: RwLock::new(Vec::new()),
            running: Arc::new(AtomicBool::new(false)),
            stop_tx: Mutex::new(None),
            last_events: RwLock::new(std::collections::HashMap::new()),
        })
    }

    /// 注册配置变更回调
    pub fn on_change<F>(&self, callback: F)
    where
        F: Fn(ConfigChangeEvent) + Send + Sync + 'static,
    {
        self.callbacks.write().push(Arc::new(callback));
    }

    /// 添加自定义监视路径
    pub fn add_watch_path(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        if !self.watched_paths.read().contains(&path) {
            self.watched_paths.write().push(path);
        }
    }

    /// 获取所有监视路径
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.read().clone()
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 启动配置监视
    pub fn start(&self) -> ConfigWatcherResult<()> {
        if self.is_running() {
            return Err(ConfigWatcherError::AlreadyRunning);
        }

        // 收集要监视的路径
        let mut paths_to_watch = Vec::new();

        if self.config.watch_app_config {
            if let Ok(path) = default_config_path() {
                if path.exists() {
                    paths_to_watch.push(path);
                }
            }
        }

        if self.config.watch_hosts_config {
            if let Ok(path) = default_hosts_path() {
                if path.exists() {
                    paths_to_watch.push(path);
                }
            }
        }

        // 添加自定义路径
        for path in self.watched_paths.read().iter() {
            if path.exists() && !paths_to_watch.contains(path) {
                paths_to_watch.push(path.clone());
            }
        }

        if paths_to_watch.is_empty() {
            warn!("No config files to watch");
            return Ok(());
        }

        // 更新监视路径列表
        *self.watched_paths.write() = paths_to_watch.clone();

        // 创建停止通道
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        *self.stop_tx.lock() = Some(stop_tx);

        // 创建事件通道
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(100);

        // 克隆需要的数据
        let callbacks = self.callbacks.read().clone();
        let debounce_ms = self.config.debounce_ms;
        let last_events = Arc::new(RwLock::new(
            std::collections::HashMap::<PathBuf, Instant>::new(),
        ));

        // 创建 watcher
        let event_tx_clone = event_tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = event_tx_clone.blocking_send(event);
                }
            },
            Config::default(),
        )
        .map_err(|e| ConfigWatcherError::CreateWatcher(e.to_string()))?;

        // 添加监视路径
        for path in &paths_to_watch {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|e| ConfigWatcherError::WatchPath {
                    path: path.display().to_string(),
                    message: e.to_string(),
                })?;
            info!("Watching config file: {}", path.display());
        }

        self.running.store(true, Ordering::SeqCst);

        // 启动事件处理任务
        let running = Arc::clone(&self.running);
        let last_events_clone = last_events.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                // 保持 watcher 存活
                let _watcher = watcher;

                loop {
                    tokio::select! {
                        // 处理文件系统事件
                        Some(event) = event_rx.recv() => {
                            Self::handle_event(&event, &callbacks, debounce_ms, &last_events_clone);
                        }
                        // 处理停止信号
                        _ = stop_rx.recv() => {
                            info!("Config watcher stopping...");
                            break;
                        }
                    }
                }

                running.store(false, Ordering::SeqCst);
                info!("Config watcher stopped");
            });
        });

        info!(
            "Config watcher started, watching {} file(s)",
            paths_to_watch.len()
        );
        Ok(())
    }

    /// 停止配置监视
    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().take() {
            let _ = tx.blocking_send(());
        }
    }

    /// 处理文件系统事件
    fn handle_event(
        event: &Event,
        callbacks: &[ConfigChangeCallback],
        debounce_ms: u64,
        last_events: &RwLock<std::collections::HashMap<PathBuf, Instant>>,
    ) {
        // 检查事件类型
        let change_event = match &event.kind {
            EventKind::Modify(_) => {
                Self::path_to_change_event(&event.paths, ConfigChangeEvent::AppConfigChanged)
            },
            EventKind::Create(_) => Self::path_to_config_created(&event.paths),
            EventKind::Remove(_) => Self::path_to_config_deleted(&event.paths),
            _ => None,
        };

        if let Some(change_event) = change_event {
            // 去抖动检查
            if let Some(path) = event.paths.first() {
                let now = Instant::now();
                let should_notify = {
                    let last = last_events.read();
                    if let Some(last_time) = last.get(path) {
                        now.duration_since(*last_time) > Duration::from_millis(debounce_ms)
                    } else {
                        true
                    }
                };

                if should_notify {
                    last_events.write().insert(path.clone(), now);
                    debug!("Config change detected: {:?}", change_event);

                    // 通知所有回调
                    for callback in callbacks {
                        callback(change_event.clone());
                    }
                } else {
                    debug!("Config change debounced: {:?}", path);
                }
            }
        }
    }

    /// 从路径确定变更事件类型
    fn path_to_change_event(
        paths: &[PathBuf],
        _default: ConfigChangeEvent,
    ) -> Option<ConfigChangeEvent> {
        paths.first().and_then(|path| {
            path.file_name().and_then(|name| name.to_str()).map(|name| {
                match ConfigFileType::from_filename(name) {
                    ConfigFileType::AppConfig => ConfigChangeEvent::AppConfigChanged,
                    ConfigFileType::HostsConfig => ConfigChangeEvent::HostsConfigChanged,
                    ConfigFileType::Other => ConfigChangeEvent::ConfigDirChanged,
                }
            })
        })
    }

    /// 从路径确定文件创建事件
    fn path_to_config_created(paths: &[PathBuf]) -> Option<ConfigChangeEvent> {
        paths.first().and_then(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| ConfigChangeEvent::ConfigCreated(ConfigFileType::from_filename(name)))
        })
    }

    /// 从路径确定文件删除事件
    fn path_to_config_deleted(paths: &[PathBuf]) -> Option<ConfigChangeEvent> {
        paths.first().and_then(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| ConfigChangeEvent::ConfigDeleted(ConfigFileType::from_filename(name)))
        })
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// 全局配置监视器
// ============================================================================

/// 全局配置监视器（单例）
static GLOBAL_WATCHER: std::sync::OnceLock<Arc<ConfigWatcher>> = std::sync::OnceLock::new();

/// 获取全局配置监视器
pub fn global_config_watcher() -> ConfigWatcherResult<Arc<ConfigWatcher>> {
    // 使用稳定的 get_or_init，在内部处理错误
    let watcher = GLOBAL_WATCHER.get_or_init(|| {
        match ConfigWatcher::new() {
            Ok(w) => Arc::new(w),
            Err(e) => {
                // 如果创建失败，记录错误并返回一个默认的监视器
                warn!("Failed to create config watcher: {}", e);
                Arc::new(
                    ConfigWatcher::with_config(ConfigWatcherConfig::default())
                        .expect("Failed to create default config watcher"),
                )
            },
        }
    });
    Ok(Arc::clone(watcher))
}

/// 启动全局配置监视
pub fn start_global_config_watcher() -> ConfigWatcherResult<()> {
    let watcher = global_config_watcher()?;
    watcher.start()
}

/// 停止全局配置监视
pub fn stop_global_config_watcher() {
    if let Some(watcher) = GLOBAL_WATCHER.get() {
        watcher.stop();
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_file_type_from_filename() {
        assert_eq!(
            ConfigFileType::from_filename("config.toml"),
            ConfigFileType::AppConfig
        );
        assert_eq!(
            ConfigFileType::from_filename("hosts.toml"),
            ConfigFileType::HostsConfig
        );
        assert_eq!(
            ConfigFileType::from_filename("other.toml"),
            ConfigFileType::Other
        );
    }

    #[test]
    fn test_config_file_type_filename() {
        assert_eq!(ConfigFileType::AppConfig.filename(), "config.toml");
        assert_eq!(ConfigFileType::HostsConfig.filename(), "hosts.toml");
        assert_eq!(ConfigFileType::Other.filename(), "unknown");
    }

    #[test]
    fn test_config_change_event_equality() {
        assert_eq!(
            ConfigChangeEvent::AppConfigChanged,
            ConfigChangeEvent::AppConfigChanged
        );
        assert_ne!(
            ConfigChangeEvent::AppConfigChanged,
            ConfigChangeEvent::HostsConfigChanged
        );
    }

    #[test]
    fn test_config_watcher_config_default() {
        let config = ConfigWatcherConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert!(config.watch_app_config);
        assert!(config.watch_hosts_config);
        assert!(!config.watch_config_dir);
    }

    #[test]
    fn test_config_watcher_creation() {
        let watcher = ConfigWatcher::new();
        assert!(watcher.is_ok());

        let watcher = watcher.unwrap();
        assert!(!watcher.is_running());
    }

    #[test]
    fn test_config_watcher_add_path() {
        let watcher = ConfigWatcher::new().unwrap();
        let path = PathBuf::from("/tmp/test.toml");

        watcher.add_watch_path(&path);

        let paths = watcher.watched_paths();
        assert!(paths.contains(&path));
    }

    #[test]
    fn test_config_watcher_callback_registration() {
        let watcher = ConfigWatcher::new().unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        watcher.on_change(move |_| {
            called_clone.store(true, Ordering::SeqCst);
        });

        // 回调已注册但未被调用（因为没有事件）
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_config_watcher_double_start() {
        let watcher = ConfigWatcher::new().unwrap();

        // 如果没有配置文件存在，start 会成功但不监视任何文件
        let _ = watcher.start();

        // 再次启动应该返回错误
        if watcher.is_running() {
            let result = watcher.start();
            assert!(matches!(result, Err(ConfigWatcherError::AlreadyRunning)));
        }
    }

    #[test]
    fn test_config_change_event_debug() {
        let event = ConfigChangeEvent::AppConfigChanged;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AppConfigChanged"));

        let event2 = ConfigChangeEvent::ConfigDeleted(ConfigFileType::HostsConfig);
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("ConfigDeleted"));
        assert!(debug_str2.contains("HostsConfig"));
    }

    #[test]
    fn test_config_watcher_error_display() {
        let err = ConfigWatcherError::AlreadyRunning;
        assert_eq!(err.to_string(), "Watcher is already running");

        let err2 = ConfigWatcherError::WatchPath {
            path: "/test".to_string(),
            message: "not found".to_string(),
        };
        assert!(err2.to_string().contains("/test"));
        assert!(err2.to_string().contains("not found"));
    }
}
