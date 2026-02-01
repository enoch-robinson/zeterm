//! 应用层模块
//!
//! 包含终端会话管理、协调器等应用层逻辑。

// 允许暂时未使用但将来会用到的代码
#![allow(dead_code)]

use std::sync::{Arc, OnceLock};
use zeterm_storage::{Database, KeyringSecretStore, SecretHelper};
#[cfg(target_os = "windows")]
use zeterm_storage::{SqliteSecretStore, default_secret_helper_windows};

pub mod runtime;
pub mod session;
pub mod terminal;

// ==================== 全局数据库单例 ====================

/// 全局数据库实例
///
/// 使用 `OnceLock` 保证只初始化一次，线程安全。
static GLOBAL_DATABASE: OnceLock<Arc<Database>> = OnceLock::new();

/// 初始化全局数据库
///
/// 只能调用一次，重复调用会 panic。
///
/// # Panics
///
/// 如果数据库已经初始化，会 panic。
///
/// # Example
///
/// ```ignore
/// let db = Database::with_default_path().await?;
/// app::init_global_database(Arc::new(db));
/// ```
pub fn init_global_database(db: Arc<Database>) {
    if GLOBAL_DATABASE.set(db).is_err() {
        panic!("Database already initialized");
    }
}

/// 获取全局数据库实例
///
/// 如果数据库未初始化（未调用 `init_global_database`），返回 `None`。
///
/// # Example
///
/// ```ignore
/// if let Some(db) = app::global_database() {
///     let hosts = db.list_all_hosts().await?;
/// }
/// ```
pub fn global_database() -> Option<Arc<Database>> {
    GLOBAL_DATABASE.get().cloned()
}

/// 获取平台特定的 SecretHelper
///
/// - Windows: 使用 SQLite 存储（避免 Credential Manager 线程隔离问题）
/// - 其他平台: 使用系统密钥链（Keyring）
#[cfg(target_os = "windows")]
pub fn global_secret_helper() -> Option<SecretHelper<SqliteSecretStore>> {
    global_database().map(|db| default_secret_helper_windows(db))
}

/// 获取平台特定的 SecretHelper（非 Windows）
#[cfg(not(target_os = "windows"))]
pub fn global_secret_helper() -> Option<SecretHelper<KeyringSecretStore>> {
    Some(SecretHelper::with_keyring())
}
