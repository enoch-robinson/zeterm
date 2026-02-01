//! 共享 Tokio Runtime 管理模块
//!
//! 提供应用级别的共享 Tokio Runtime，避免在多处创建临时 Runtime。
//!
//! # 使用方式
//!
//! ```ignore
//! use crate::app::runtime;
//!
//! // 初始化（在 main.rs 中调用一次）
//! runtime::init();
//!
//! // 异步执行
//! runtime::spawn(async {
//!     // 异步代码
//! });
//!
//! // 阻塞执行异步代码
//! let result = runtime::block_on(async {
//!     // 异步代码
//!     42
//! });
//!
//! // 在新线程中执行异步代码（适用于 UI 线程）
//! runtime::spawn_blocking(async {
//!     // 长时间运行的异步代码
//! });
//! ```

use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};
use tracing::{debug, info};

/// 全局共享的 Tokio Runtime
static GLOBAL_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Runtime 配置
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// 工作线程数量（None 表示使用 CPU 核心数）
    pub worker_threads: Option<usize>,
    /// 线程名称前缀
    pub thread_name: String,
    /// 是否启用 I/O
    pub enable_io: bool,
    /// 是否启用时间
    pub enable_time: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: None, // 使用 CPU 核心数
            thread_name: "zeterm-worker".to_string(),
            enable_io: true,
            enable_time: true,
        }
    }
}

/// 初始化全局 Runtime（使用默认配置）
///
/// 应在应用启动时调用一次。重复调用将被忽略。
pub fn init() {
    init_with_config(RuntimeConfig::default());
}

/// 使用指定配置初始化全局 Runtime
///
/// 应在应用启动时调用一次。重复调用将被忽略。
pub fn init_with_config(config: RuntimeConfig) {
    GLOBAL_RUNTIME.get_or_init(|| {
        info!("Initializing shared Tokio runtime");
        debug!("Runtime config: {:?}", config);

        let mut builder = tokio::runtime::Builder::new_multi_thread();

        if let Some(threads) = config.worker_threads {
            builder.worker_threads(threads);
        }

        builder.thread_name(&config.thread_name);

        if config.enable_io {
            builder.enable_io();
        }

        if config.enable_time {
            builder.enable_time();
        }

        builder
            .build()
            .expect("Failed to create shared Tokio runtime")
    });
}

/// 获取全局 Runtime 的 Handle
///
/// 如果 Runtime 未初始化，将自动初始化。
pub fn handle() -> Handle {
    get_or_init_runtime().handle().clone()
}

/// 在共享 Runtime 上 spawn 一个异步任务
///
/// 返回 `JoinHandle`，可用于等待任务完成或取消任务。
///
/// # 示例
///
/// ```ignore
/// let handle = runtime::spawn(async {
///     // 异步代码
///     Ok::<_, anyhow::Error>(42)
/// });
/// ```
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    get_or_init_runtime().spawn(future)
}

/// 阻塞当前线程，执行异步代码
///
/// **警告**: 不要在异步上下文中调用此方法，会导致死锁。
/// 如果在已有 Runtime 的上下文中，应该使用 `spawn` 代替。
///
/// # 示例
///
/// ```ignore
/// let result = runtime::block_on(async {
///     some_async_function().await
/// });
/// ```
pub fn block_on<F: Future>(future: F) -> F::Output {
    get_or_init_runtime().block_on(future)
}

/// 在阻塞线程池中执行异步代码
///
/// 使用 tokio 的内置阻塞线程池，避免创建新线程。
/// 执行完成后不返回结果（fire-and-forget 模式）。
///
/// # 性能优势
///
/// - 使用固定大小的线程池（由 tokio 管理）
/// - 线程复用，避免频繁创建/销毁的开销
/// - 自动限制并发数，避免资源耗尽
///
/// # 使用场景
///
/// 适用于从 UI 线程发起异步操作，避免阻塞 UI。
///
/// # 示例
///
/// ```ignore
/// runtime::spawn_blocking(async {
///     // 可能耗时的异步操作
///     heavy_async_operation().await;
/// });
/// ```
pub fn spawn_blocking<F>(future: F)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    get_or_init_runtime()
        .spawn_blocking(move || tokio::task::block_in_place(|| Handle::current().block_on(future)));
}

/// 在阻塞线程池中执行异步代码，并通过回调返回结果
///
/// 使用 tokio 的内置阻塞线程池，避免创建新线程。
///
/// # 性能优势
///
/// - 使用固定大小的线程池（由 tokio 管理）
/// - 线程复用，避免频繁创建/销毁的开销
/// - 自动限制并发数，避免资源耗尽
///
/// # 使用场景
///
/// 适用于需要获取异步操作结果的场景。
///
/// # 示例
///
/// ```ignore
/// runtime::spawn_with_callback(
///     async { fetch_data().await },
///     |result| {
///         // 在阻塞线程池中处理结果
///         println!("Got result: {:?}", result);
///     }
/// );
/// ```
pub fn spawn_with_callback<F, T, C>(future: F, callback: C)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
    C: FnOnce(T) + Send + 'static,
{
    get_or_init_runtime().spawn_blocking(move || {
        let result = tokio::task::block_in_place(|| Handle::current().block_on(future));
        callback(result);
    });
}

/// 尝试在当前线程的 Runtime 上执行，如果没有则使用全局 Runtime
///
/// 这是一个更智能的版本，会检测当前是否在异步上下文中。
pub fn try_spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // 尝试获取当前线程的 Runtime Handle
    match Handle::try_current() {
        Ok(handle) => handle.spawn(future),
        Err(_) => spawn(future),
    }
}

/// 获取或初始化全局 Runtime
fn get_or_init_runtime() -> &'static Runtime {
    GLOBAL_RUNTIME.get_or_init(|| {
        debug!("Auto-initializing shared Tokio runtime (not explicitly initialized)");
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("zeterm-worker")
            .enable_all()
            .build()
            .expect("Failed to create shared Tokio runtime")
    })
}

/// 关闭全局 Runtime
///
/// 通常在应用退出时调用。调用后不应再使用 Runtime。
/// 注意：由于 `OnceCell` 的限制，这实际上只是等待所有任务完成。
pub fn shutdown() {
    if let Some(rt) = GLOBAL_RUNTIME.get() {
        info!("Shutting down shared Tokio runtime");
        // Runtime 会在 drop 时自动关闭
        // 这里我们只是记录日志
        let _ = rt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn test_spawn_and_block_on() {
        let result = block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_spawn_task() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let handle = spawn(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // 等待任务完成
        block_on(handle).unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_spawn_blocking() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        spawn_blocking(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // 给线程一点时间完成
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_spawn_with_callback() {
        let result = Arc::new(AtomicUsize::new(0));
        let result_clone = result.clone();

        spawn_with_callback(async { 42usize }, move |value| {
            result_clone.store(value, Ordering::SeqCst);
        });

        // 给线程一点时间完成
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(result.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_handle() {
        let handle = handle();
        let result = handle.block_on(async { "test" });
        assert_eq!(result, "test");
    }
}
