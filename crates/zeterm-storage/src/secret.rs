//! 敏感信息存储模块
//!
//! 提供跨平台的敏感信息（密码、密钥等）安全存储功能。
//!
//! # 支持的平台
//!
//! - **macOS**: Keychain Access
//! - **Windows**: Credential Manager
//! - **Linux**: Secret Service (GNOME Keyring / KWallet)
//!
//! # 示例
//!
//! ```no_run
//! use zeterm_storage::secret::{SecretStore, KeyringSecretStore};
//!
//! # fn example() -> anyhow::Result<()> {
//! let store = KeyringSecretStore::new();
//!
//! // 存储密码
//! store.set_password("my-server", "secret123")?;
//!
//! // 读取密码
//! let password = store.get_password("my-server")?;
//! assert_eq!(password, Some("secret123".to_string()));
//!
//! // 删除密码
//! store.delete_password("my-server")?;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, info, warn};
use zeterm_core::config::PasswordRef;

/// 服务名称（用于在系统密钥链中标识应用）
const SERVICE_NAME: &str = "zeterm";

/// 敏感信息存储 Trait
///
/// 定义敏感信息（密码、密钥密码等）的存储接口。
/// 不同平台可以有不同的实现（系统密钥链、加密文件等）。
pub trait SecretStore: Send + Sync {
    /// 存储密码
    ///
    /// # 参数
    /// - `key`: 密码标识符（如 "host:user@server:22"）
    /// - `password`: 要存储的密码
    fn set_password(&self, key: &str, password: &str) -> Result<()>;

    /// 获取密码
    ///
    /// # 参数
    /// - `key`: 密码标识符
    ///
    /// # 返回
    /// - `Some(password)`: 找到密码
    /// - `None`: 未找到密码
    fn get_password(&self, key: &str) -> Result<Option<String>>;

    /// 删除密码
    ///
    /// # 参数
    /// - `key`: 密码标识符
    ///
    /// # 返回
    /// - `Ok(true)`: 成功删除
    /// - `Ok(false)`: 密码不存在
    fn delete_password(&self, key: &str) -> Result<bool>;

    /// 检查密码是否存在
    fn has_password(&self, key: &str) -> Result<bool> {
        Ok(self.get_password(key)?.is_some())
    }

    /// 列出所有已存储的密钥名称
    ///
    /// 注意：某些后端可能不支持此操作，返回空列表
    fn list_keys(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// 清除所有存储的密码
    ///
    /// 警告：此操作不可逆！
    fn clear_all(&self) -> Result<()> {
        for key in self.list_keys()? {
            let _ = self.delete_password(&key);
        }
        Ok(())
    }
}

/// 密钥生成器
///
/// 用于生成标准化的密钥名称。
pub struct SecretKeyGenerator;

impl SecretKeyGenerator {
    /// 生成主机密码密钥
    ///
    /// 格式: `host:username@host:port`
    pub fn host_password(username: &str, host: &str, port: u16) -> String {
        format!("host:{}@{}:{}", username, host, port)
    }

    /// 生成私钥密码密钥
    ///
    /// 格式: `keyfile:path`
    pub fn key_passphrase(key_path: &str) -> String {
        format!("keyfile:{}", key_path)
    }

    /// 解析密钥类型
    pub fn parse_key_type(key: &str) -> SecretKeyType {
        if key.starts_with("host:") {
            SecretKeyType::HostPassword
        } else if key.starts_with("keyfile:") {
            SecretKeyType::KeyPassphrase
        } else {
            SecretKeyType::Custom
        }
    }
}

/// 密钥类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKeyType {
    /// 主机登录密码
    HostPassword,
    /// 私钥密码
    KeyPassphrase,
    /// 自定义密钥
    Custom,
}

/// 基于系统密钥链的 SecretStore 实现
///
/// 使用 `keyring` crate 提供跨平台支持：
/// - macOS: Keychain Access
/// - Windows: Credential Manager
/// - Linux: Secret Service (GNOME Keyring / KWallet)
#[derive(Debug)]
pub struct KeyringSecretStore {
    /// 服务名称
    service: String,
}

impl KeyringSecretStore {
    /// 创建新的 KeyringSecretStore（使用默认服务名）
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.to_string(),
        }
    }

    /// 使用自定义服务名创建
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// 获取服务名称
    pub fn service(&self) -> &str {
        &self.service
    }

    /// 创建 keyring Entry
    fn entry(&self, key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, key).context("创建密钥链条目失败")
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringSecretStore {
    fn set_password(&self, key: &str, password: &str) -> Result<()> {
        debug!(key = %key, "Storing password in keyring");

        let entry = self.entry(key)?;
        entry
            .set_password(password)
            .with_context(|| format!("存储密码失败: {}", key))?;

        info!(key = %key, "Password stored successfully");
        Ok(())
    }

    fn get_password(&self, key: &str) -> Result<Option<String>> {
        debug!(key = %key, "Retrieving password from keyring");

        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(password) => {
                debug!(key = %key, "Password retrieved successfully");
                Ok(Some(password))
            },
            Err(keyring::Error::NoEntry) => {
                debug!(key = %key, "Password not found");
                Ok(None)
            },
            Err(e) => {
                warn!(key = %key, error = %e, "Failed to retrieve password");
                Err(e).with_context(|| format!("获取密码失败: {}", key))
            },
        }
    }

    fn delete_password(&self, key: &str) -> Result<bool> {
        debug!(key = %key, "Deleting password from keyring");

        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(()) => {
                info!(key = %key, "Password deleted successfully");
                Ok(true)
            },
            Err(keyring::Error::NoEntry) => {
                debug!(key = %key, "Password not found, nothing to delete");
                Ok(false)
            },
            Err(e) => {
                warn!(key = %key, error = %e, "Failed to delete password");
                Err(e).with_context(|| format!("删除密码失败: {}", key))
            },
        }
    }
}

/// 内存 SecretStore 实现
///
/// 仅用于测试，密码存储在内存中，程序退出后丢失。
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    /// 内存存储
    secrets: RwLock<HashMap<String, String>>,
}

impl MemorySecretStore {
    /// 创建新的内存存储
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
        }
    }

    /// 获取存储的密码数量
    pub fn len(&self) -> usize {
        self.secrets.read().unwrap().len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SecretStore for MemorySecretStore {
    fn set_password(&self, key: &str, password: &str) -> Result<()> {
        let mut secrets = self.secrets.write().unwrap();
        secrets.insert(key.to_string(), password.to_string());
        Ok(())
    }

    fn get_password(&self, key: &str) -> Result<Option<String>> {
        let secrets = self.secrets.read().unwrap();
        Ok(secrets.get(key).cloned())
    }

    fn delete_password(&self, key: &str) -> Result<bool> {
        let mut secrets = self.secrets.write().unwrap();
        Ok(secrets.remove(key).is_some())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        let secrets = self.secrets.read().unwrap();
        Ok(secrets.keys().cloned().collect())
    }

    fn clear_all(&self) -> Result<()> {
        let mut secrets = self.secrets.write().unwrap();
        secrets.clear();
        Ok(())
    }
}

/// 密码引用解析器
///
/// 解析 hosts.toml 中的密码引用格式，并从相应存储获取实际密码。
pub struct PasswordResolver<S: SecretStore> {
    /// 密钥存储
    store: S,
}

impl<S: SecretStore> PasswordResolver<S> {
    /// 创建新的解析器
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// 获取底层存储的引用
    pub fn store(&self) -> &S {
        &self.store
    }

    /// 获取底层存储的可变引用
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// 解析密码引用
    ///
    /// 支持的格式：
    /// - `keychain:key_name` - 从系统密钥链读取
    /// - `env:VAR_NAME` - 从环境变量读取
    /// - `plain:password` - 明文密码（不推荐）
    pub fn resolve(&self, password_ref: &str) -> Result<Option<String>> {
        if password_ref.is_empty() {
            return Ok(None);
        }

        if let Some(key) = password_ref.strip_prefix("keychain:") {
            debug!(key = %key, "Resolving password from keychain");
            self.store.get_password(key)
        } else if let Some(var) = password_ref.strip_prefix("env:") {
            debug!(var = %var, "Resolving password from environment variable");
            match std::env::var(var) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => {
                    debug!(var = %var, "Environment variable not set");
                    Ok(None)
                },
                Err(e) => {
                    bail!("读取环境变量 {} 失败: {}", var, e);
                },
            }
        } else if let Some(plain) = password_ref.strip_prefix("plain:") {
            warn!("Using plain text password (not recommended for production)");
            Ok(Some(plain.to_string()))
        } else {
            // 默认作为 keychain 引用处理
            debug!(key = %password_ref, "Resolving password from keychain (default)");
            self.store.get_password(password_ref)
        }
    }

    /// 存储密码并生成引用字符串
    ///
    /// # 参数
    /// - `key`: 密码标识符
    /// - `password`: 要存储的密码
    ///
    /// # 返回
    /// 返回可用于 hosts.toml 的密码引用字符串（如 "keychain:key"）
    pub fn store_password(&self, key: &str, password: &str) -> Result<String> {
        self.store.set_password(key, password)?;
        Ok(format!("keychain:{}", key))
    }

    /// 删除密码引用对应的密码
    pub fn delete_password(&self, password_ref: &str) -> Result<bool> {
        if let Some(key) = password_ref.strip_prefix("keychain:") {
            self.store.delete_password(key)
        } else if password_ref.starts_with("env:") || password_ref.starts_with("plain:") {
            // 环境变量和明文密码无法删除
            Ok(false)
        } else {
            // 默认作为 keychain 引用处理
            self.store.delete_password(password_ref)
        }
    }
}

/// 创建默认的 SecretStore
///
/// 在生产环境使用 KeyringSecretStore，
/// 在测试环境可以使用 MemorySecretStore。
pub fn default_secret_store() -> KeyringSecretStore {
    KeyringSecretStore::new()
}

/// 创建默认的 PasswordResolver
pub fn default_password_resolver() -> PasswordResolver<KeyringSecretStore> {
    PasswordResolver::new(default_secret_store())
}

/// Secret 助手
///
/// 提供便捷的 API 来解析 `PasswordRef` 并从 `SecretStore` 获取密码。
/// 这是连接 zeterm-core 的 PasswordRef 和 zeterm-storage 的 SecretStore 的桥梁。
pub struct SecretHelper<S: SecretStore> {
    store: S,
}

impl<S: SecretStore> SecretHelper<S> {
    /// 创建新的 SecretHelper
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// 获取底层存储
    pub fn store(&self) -> &S {
        &self.store
    }

    /// 获取底层存储的可变引用
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// 解析 PasswordRef 获取实际密码
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use zeterm_storage::secret::{SecretHelper, MemorySecretStore, SecretStore};
    /// use zeterm_core::config::PasswordRef;
    ///
    /// let store = MemorySecretStore::new();
    /// store.set_password("my-key", "secret").unwrap();
    ///
    /// let helper = SecretHelper::new(store);
    ///
    /// let password = helper.resolve_password_ref(&PasswordRef::Keychain("my-key".to_string())).unwrap();
    /// assert_eq!(password, Some("secret".to_string()));
    /// ```
    pub fn resolve_password_ref(&self, password_ref: &PasswordRef) -> Result<Option<String>> {
        match password_ref {
            PasswordRef::Keychain(key) => {
                debug!(key = %key, "Resolving password from keychain");
                self.store.get_password(key)
            },
            PasswordRef::Env(var) => {
                debug!(var = %var, "Resolving password from environment variable");
                match std::env::var(var) {
                    Ok(value) => Ok(Some(value)),
                    Err(std::env::VarError::NotPresent) => Ok(None),
                    Err(e) => bail!("读取环境变量 {} 失败: {}", var, e),
                }
            },
            PasswordRef::Plain(plain) => {
                warn!(
                    "⚠️  使用明文密码存在严重安全风险！\n\
                     明文密码会暴露在配置文件中，可能被其他程序读取。\n\
                     生产环境强烈建议使用 keychain 或环境变量"
                );
                Ok(Some(plain.clone()))
            },
            PasswordRef::None => Ok(None),
        }
    }

    /// 解析密码引用字符串获取实际密码
    ///
    /// 这是 `resolve_password_ref` 的便捷版本，直接接受字符串。
    pub fn resolve_password_str(&self, password_ref: &str) -> Result<Option<String>> {
        let parsed = PasswordRef::parse(password_ref);
        self.resolve_password_ref(&parsed)
    }

    /// 存储密码并返回 PasswordRef
    ///
    /// # 参数
    /// - `key`: 密码标识符
    /// - `password`: 要存储的密码
    ///
    /// # 返回
    /// 返回 `PasswordRef::Keychain(key)`
    pub fn store_password(&self, key: &str, password: &str) -> Result<PasswordRef> {
        self.store.set_password(key, password)?;
        Ok(PasswordRef::Keychain(key.to_string()))
    }

    /// 为主机生成密钥并存储密码
    ///
    /// # 参数
    /// - `username`: 用户名
    /// - `host`: 主机地址
    /// - `port`: SSH 端口
    /// - `password`: 要存储的密码
    ///
    /// # 返回
    /// 返回 `PasswordRef::Keychain(generated_key)`
    pub fn store_host_password(
        &self,
        username: &str,
        host: &str,
        port: u16,
        password: &str,
    ) -> Result<PasswordRef> {
        let key = SecretKeyGenerator::host_password(username, host, port);
        self.store_password(&key, password)
    }

    /// 为私钥文件存储密码
    ///
    /// # 参数
    /// - `key_path`: 私钥文件路径
    /// - `passphrase`: 私钥密码
    ///
    /// # 返回
    /// 返回 `PasswordRef::Keychain(generated_key)`
    pub fn store_key_passphrase(&self, key_path: &str, passphrase: &str) -> Result<PasswordRef> {
        let key = SecretKeyGenerator::key_passphrase(key_path);
        self.store_password(&key, passphrase)
    }

    /// 删除 PasswordRef 对应的密码
    ///
    /// 只有 `PasswordRef::Keychain` 类型的密码可以被删除。
    pub fn delete_password_ref(&self, password_ref: &PasswordRef) -> Result<bool> {
        match password_ref {
            PasswordRef::Keychain(key) => self.store.delete_password(key),
            _ => Ok(false),
        }
    }

    /// 检查 PasswordRef 对应的密码是否存在
    pub fn has_password_ref(&self, password_ref: &PasswordRef) -> Result<bool> {
        Ok(self.resolve_password_ref(password_ref)?.is_some())
    }
}

impl SecretHelper<KeyringSecretStore> {
    /// 创建使用系统密钥链的 SecretHelper
    pub fn with_keyring() -> Self {
        Self::new(KeyringSecretStore::new())
    }
}

impl SecretHelper<MemorySecretStore> {
    /// 创建使用内存存储的 SecretHelper（用于测试）
    pub fn with_memory() -> Self {
        Self::new(MemorySecretStore::new())
    }
}

impl Default for SecretHelper<KeyringSecretStore> {
    fn default() -> Self {
        Self::with_keyring()
    }
}

/// 创建默认的 SecretHelper（使用系统密钥链）
pub fn default_secret_helper() -> SecretHelper<KeyringSecretStore> {
    SecretHelper::with_keyring()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_secret_store_basic() {
        let store = MemorySecretStore::new();

        // 初始为空
        assert!(store.is_empty());

        // 存储密码
        store.set_password("test-key", "test-password").unwrap();
        assert_eq!(store.len(), 1);

        // 读取密码
        let password = store.get_password("test-key").unwrap();
        assert_eq!(password, Some("test-password".to_string()));

        // 检查存在性
        assert!(store.has_password("test-key").unwrap());
        assert!(!store.has_password("nonexistent").unwrap());

        // 删除密码
        assert!(store.delete_password("test-key").unwrap());
        assert!(!store.delete_password("test-key").unwrap()); // 再次删除返回 false
        assert!(store.is_empty());
    }

    #[test]
    fn test_memory_secret_store_list_keys() {
        let store = MemorySecretStore::new();

        store.set_password("key1", "pass1").unwrap();
        store.set_password("key2", "pass2").unwrap();
        store.set_password("key3", "pass3").unwrap();

        let mut keys = store.list_keys().unwrap();
        keys.sort();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn test_memory_secret_store_clear_all() {
        let store = MemorySecretStore::new();

        store.set_password("key1", "pass1").unwrap();
        store.set_password("key2", "pass2").unwrap();

        store.clear_all().unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn test_memory_secret_store_overwrite() {
        let store = MemorySecretStore::new();

        store.set_password("key", "password1").unwrap();
        assert_eq!(
            store.get_password("key").unwrap(),
            Some("password1".to_string())
        );

        store.set_password("key", "password2").unwrap();
        assert_eq!(
            store.get_password("key").unwrap(),
            Some("password2".to_string())
        );

        // 仍然只有一个键
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_secret_key_generator_host_password() {
        let key = SecretKeyGenerator::host_password("root", "192.168.1.100", 22);
        assert_eq!(key, "host:root@192.168.1.100:22");

        let key_type = SecretKeyGenerator::parse_key_type(&key);
        assert_eq!(key_type, SecretKeyType::HostPassword);
    }

    #[test]
    fn test_secret_key_generator_key_passphrase() {
        let key = SecretKeyGenerator::key_passphrase("/home/user/.ssh/id_rsa");
        assert_eq!(key, "keyfile:/home/user/.ssh/id_rsa");

        let key_type = SecretKeyGenerator::parse_key_type(&key);
        assert_eq!(key_type, SecretKeyType::KeyPassphrase);
    }

    #[test]
    fn test_secret_key_generator_custom() {
        let key_type = SecretKeyGenerator::parse_key_type("custom-key");
        assert_eq!(key_type, SecretKeyType::Custom);
    }

    #[test]
    fn test_password_resolver_keychain() {
        let store = MemorySecretStore::new();
        store.set_password("my-server", "secret123").unwrap();

        let resolver = PasswordResolver::new(store);

        // 带前缀的 keychain 引用
        let password = resolver.resolve("keychain:my-server").unwrap();
        assert_eq!(password, Some("secret123".to_string()));

        // 不带前缀（默认 keychain）
        let password = resolver.resolve("my-server").unwrap();
        assert_eq!(password, Some("secret123".to_string()));

        // 不存在的密钥
        let password = resolver.resolve("keychain:nonexistent").unwrap();
        assert_eq!(password, None);
    }

    #[test]
    fn test_password_resolver_env() {
        // SAFETY: This test runs in a single thread
        unsafe {
            std::env::set_var("TEST_ZETERM_SECRET", "env_password");
        }

        let store = MemorySecretStore::new();
        let resolver = PasswordResolver::new(store);

        let password = resolver.resolve("env:TEST_ZETERM_SECRET").unwrap();
        assert_eq!(password, Some("env_password".to_string()));

        let password = resolver.resolve("env:NONEXISTENT_VAR").unwrap();
        assert_eq!(password, None);

        // Cleanup
        unsafe {
            std::env::remove_var("TEST_ZETERM_SECRET");
        }
    }

    #[test]
    fn test_password_resolver_plain() {
        let store = MemorySecretStore::new();
        let resolver = PasswordResolver::new(store);

        let password = resolver.resolve("plain:my_plain_password").unwrap();
        assert_eq!(password, Some("my_plain_password".to_string()));
    }

    #[test]
    fn test_password_resolver_empty() {
        let store = MemorySecretStore::new();
        let resolver = PasswordResolver::new(store);

        let password = resolver.resolve("").unwrap();
        assert_eq!(password, None);
    }

    #[test]
    fn test_password_resolver_store_password() {
        let store = MemorySecretStore::new();
        let resolver = PasswordResolver::new(store);

        let ref_string = resolver.store_password("test-key", "test-pass").unwrap();
        assert_eq!(ref_string, "keychain:test-key");

        // 验证密码已存储
        let password = resolver.resolve(&ref_string).unwrap();
        assert_eq!(password, Some("test-pass".to_string()));
    }

    #[test]
    fn test_password_resolver_delete_password() {
        let store = MemorySecretStore::new();
        store.set_password("to-delete", "password").unwrap();

        let resolver = PasswordResolver::new(store);

        // 删除 keychain 密码
        assert!(resolver.delete_password("keychain:to-delete").unwrap());
        assert!(!resolver.delete_password("keychain:to-delete").unwrap());

        // 删除 env/plain 类型（应返回 false）
        assert!(!resolver.delete_password("env:VAR").unwrap());
        assert!(!resolver.delete_password("plain:pass").unwrap());
    }

    #[test]
    fn test_keyring_secret_store_new() {
        let store = KeyringSecretStore::new();
        assert_eq!(store.service(), SERVICE_NAME);

        let store = KeyringSecretStore::with_service("custom-service");
        assert_eq!(store.service(), "custom-service");
    }

    // Note: 实际的 keyring 操作测试需要系统密钥链支持
    // 以下测试仅在有密钥链支持的环境中运行
    #[test]
    #[ignore = "Requires system keychain support"]
    fn test_keyring_secret_store_integration() {
        let store = KeyringSecretStore::with_service("zeterm-test");

        // 存储
        store
            .set_password("integration-test-key", "integration-test-password")
            .unwrap();

        // 读取
        let password = store.get_password("integration-test-key").unwrap();
        assert_eq!(password, Some("integration-test-password".to_string()));

        // 删除
        assert!(store.delete_password("integration-test-key").unwrap());
        assert!(!store.has_password("integration-test-key").unwrap());
    }

    #[test]
    fn test_secret_helper_resolve_keychain() {
        let helper = SecretHelper::with_memory();
        helper.store().set_password("test-key", "secret").unwrap();

        let password = helper
            .resolve_password_ref(&PasswordRef::Keychain("test-key".to_string()))
            .unwrap();
        assert_eq!(password, Some("secret".to_string()));

        let password = helper
            .resolve_password_ref(&PasswordRef::Keychain("nonexistent".to_string()))
            .unwrap();
        assert_eq!(password, None);
    }

    #[test]
    fn test_secret_helper_resolve_env() {
        // SAFETY: Single-threaded test
        unsafe {
            std::env::set_var("TEST_SECRET_HELPER_VAR", "env_value");
        }

        let helper = SecretHelper::with_memory();

        let password = helper
            .resolve_password_ref(&PasswordRef::Env("TEST_SECRET_HELPER_VAR".to_string()))
            .unwrap();
        assert_eq!(password, Some("env_value".to_string()));

        unsafe {
            std::env::remove_var("TEST_SECRET_HELPER_VAR");
        }
    }

    #[test]
    fn test_secret_helper_resolve_plain() {
        let helper = SecretHelper::with_memory();

        let password = helper
            .resolve_password_ref(&PasswordRef::Plain("plain_password".to_string()))
            .unwrap();
        assert_eq!(password, Some("plain_password".to_string()));
    }

    #[test]
    fn test_secret_helper_resolve_none() {
        let helper = SecretHelper::with_memory();

        let password = helper.resolve_password_ref(&PasswordRef::None).unwrap();
        assert_eq!(password, None);
    }

    #[test]
    fn test_secret_helper_resolve_str() {
        let helper = SecretHelper::with_memory();
        helper.store().set_password("my-key", "my-secret").unwrap();

        // keychain: prefix
        let password = helper.resolve_password_str("keychain:my-key").unwrap();
        assert_eq!(password, Some("my-secret".to_string()));

        // no prefix (defaults to keychain)
        let password = helper.resolve_password_str("my-key").unwrap();
        assert_eq!(password, Some("my-secret".to_string()));

        // plain: prefix
        let password = helper.resolve_password_str("plain:direct").unwrap();
        assert_eq!(password, Some("direct".to_string()));
    }

    #[test]
    fn test_secret_helper_store_password() {
        let helper = SecretHelper::with_memory();

        let password_ref = helper.store_password("new-key", "new-password").unwrap();
        assert_eq!(password_ref, PasswordRef::Keychain("new-key".to_string()));

        // Verify stored
        let password = helper.resolve_password_ref(&password_ref).unwrap();
        assert_eq!(password, Some("new-password".to_string()));
    }

    #[test]
    fn test_secret_helper_store_host_password() {
        let helper = SecretHelper::with_memory();

        let password_ref = helper
            .store_host_password("root", "192.168.1.1", 22, "host-pass")
            .unwrap();

        assert!(
            matches!(&password_ref, PasswordRef::Keychain(k) if k.contains("root@192.168.1.1:22"))
        );

        let password = helper.resolve_password_ref(&password_ref).unwrap();
        assert_eq!(password, Some("host-pass".to_string()));
    }

    #[test]
    fn test_secret_helper_store_key_passphrase() {
        let helper = SecretHelper::with_memory();

        let password_ref = helper
            .store_key_passphrase("/home/user/.ssh/id_rsa", "key-pass")
            .unwrap();

        assert!(matches!(&password_ref, PasswordRef::Keychain(k) if k.contains("id_rsa")));

        let password = helper.resolve_password_ref(&password_ref).unwrap();
        assert_eq!(password, Some("key-pass".to_string()));
    }

    #[test]
    fn test_secret_helper_delete_password() {
        let helper = SecretHelper::with_memory();
        helper.store().set_password("to-delete", "pass").unwrap();

        let keychain_ref = PasswordRef::Keychain("to-delete".to_string());
        assert!(helper.has_password_ref(&keychain_ref).unwrap());

        assert!(helper.delete_password_ref(&keychain_ref).unwrap());
        assert!(!helper.has_password_ref(&keychain_ref).unwrap());

        // Non-keychain refs cannot be deleted
        assert!(
            !helper
                .delete_password_ref(&PasswordRef::Plain("x".to_string()))
                .unwrap()
        );
        assert!(
            !helper
                .delete_password_ref(&PasswordRef::Env("X".to_string()))
                .unwrap()
        );
        assert!(!helper.delete_password_ref(&PasswordRef::None).unwrap());
    }

    #[test]
    fn test_secret_helper_default() {
        let helper: SecretHelper<KeyringSecretStore> = SecretHelper::default();
        assert_eq!(helper.store().service(), SERVICE_NAME);
    }
}
