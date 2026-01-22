//! SSH Handler模块
//!
//! 实现 russh 的 client::Handler trait，处理 SSH 连接事件。

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use russh::ChannelId;
use russh::client::{Handler, Session};
use russh::keys::{Algorithm, EcdsaCurve, HashAlg, PublicKey, PublicKeyBase64};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::HostKeyVerification;
use crate::known_hosts::{KeyType, KnownHostsStore, VerificationResult};

/// SSH 数据接收器类型
pub type DataSender = mpsc::UnboundedSender<Vec<u8>>;
pub type DataReceiver = mpsc::UnboundedReceiver<Vec<u8>>;

/// 主机密钥确认回调结果
/// - `None`: 用户拒绝，中止连接
/// - `Some(true)`: 用户接受，保存到 known_hosts
/// - `Some(false)`: 用户接受，但不保存（临时信任）
pub type HostKeyConfirmCallback = Arc<dyn Fn(&str, u16, &str, &str) -> Option<bool> + Send + Sync>;

/// 不安全模式标志
/// 当设置为 true 时，跳过所有主机密钥验证（仅用于测试环境）
type InsecureFlag = bool;

/// SSH Handler 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerState {
    /// 初始状态
    Initial,
    /// 已验证主机密钥
    HostKeyVerified,
    /// 已认证
    Authenticated,
    /// 会话已建立
    SessionEstablished,
    /// 已断开
    Disconnected,
}

/// SSH 事件处理器
///
/// 实现 russh 的 Handler trait，处理 SSH 连接的各种事件。
pub struct SshHandler {
    /// 数据发送通道（Option 允许在断开时关闭）
    data_sender: Option<DataSender>,
    /// 主机密钥验证策略
    pub host_key_verification: HostKeyVerification,
    /// 是否允许不安全连接（跳过验证）
    allow_insecure: InsecureFlag,
    /// 当前状态
    state: Arc<Mutex<HandlerState>>,
    /// 服务器主机名（用于主机密钥验证）
    server_host: String,
    /// 服务器端口
    server_port: u16,
    /// Known hosts 存储
    known_hosts_store: Arc<Mutex<Option<KnownHostsStore>>>,
    /// 主机密钥确认回调（用于首次连接时询问用户）
    host_key_confirm_callback: Option<HostKeyConfirmCallback>,
    /// 不安全模式警告（仅记录一次）
    insecure_warning_logged: Arc<Mutex<bool>>,
}

impl SshHandler {
    /// 创建新的 SSH Handler
    ///
    /// # 参数
    /// - `data_sender`: 数据发送通道
    /// - `host_key_verification`: 主机密钥验证策略
    /// - `server_host`: 服务器主机名
    /// - `server_port`: 服务器端口
    /// - `allow_insecure`: 是否允许不安全连接（跳过主机密钥验证，不推荐）
    pub fn new(
        data_sender: DataSender,
        host_key_verification: HostKeyVerification,
        server_host: String,
        server_port: u16,
        allow_insecure: InsecureFlag,
    ) -> Self {
        // 根据验证策略初始化 KnownHostsStore
        let known_hosts_store = match &host_key_verification {
            HostKeyVerification::KnownHostsFile(path) => {
                let mut store = KnownHostsStore::with_path(path);
                if let Err(e) = store.load() {
                    warn!("Failed to load known_hosts file: {}", e);
                }
                Some(store)
            },
            HostKeyVerification::Strict | HostKeyVerification::AskOnFirstConnect => {
                // 使用默认路径
                let mut store = KnownHostsStore::new();
                if let Err(e) = store.load() {
                    debug!("Failed to load default known_hosts: {}", e);
                }
                Some(store)
            },
            HostKeyVerification::AutoAccept => None,
        };

        Self {
            data_sender: Some(data_sender),
            host_key_verification,
            allow_insecure,
            state: Arc::new(Mutex::new(HandlerState::Initial)),
            server_host,
            server_port,
            known_hosts_store: Arc::new(Mutex::new(known_hosts_store)),
            host_key_confirm_callback: None,
            insecure_warning_logged: Arc::new(Mutex::new(false)),
        }
    }

    /// 设置主机密钥确认回调
    pub fn with_host_key_confirm_callback(mut self, callback: HostKeyConfirmCallback) -> Self {
        self.host_key_confirm_callback = Some(callback);
        self
    }

    /// 设置主机密钥确认回调（可变引用版本）
    pub fn set_host_key_confirm_callback(&mut self, callback: HostKeyConfirmCallback) {
        self.host_key_confirm_callback = Some(callback);
    }

    /// 获取当前状态
    pub fn state(&self) -> HandlerState {
        *self.state.lock()
    }

    /// 设置状态
    pub fn set_state(&self, new_state: HandlerState) {
        let mut state = self.state.lock();
        debug!("SSH handler state: {:?} -> {:?}", *state, new_state);
        *state = new_state;
    }

    /// 关闭数据发送通道///
    /// 当连接断开时调用此方法，关闭 data_sender，
    /// 使receive_stream() 返回的流能够正常结束。
    fn close_data_sender(&mut self) {
        if let Some(sender) = self.data_sender.take() {
            drop(sender);
            info!("Data sender closed, receive stream will end");
        }
    }

    /// 从 PublicKey 提取密钥类型
    ///
    /// 使用 russh 官方 API 获取密钥算法类型，避免依赖不稳定的 Debug 格式。
    fn extract_key_type(public_key: &PublicKey) -> KeyType {
        match public_key.algorithm() {
            Algorithm::Ed25519 => KeyType::Ed25519,
            Algorithm::Rsa { .. } => KeyType::Rsa,
            Algorithm::Ecdsa { curve } => match curve {
                EcdsaCurve::NistP256 => KeyType::EcdsaSha2Nistp256,
                EcdsaCurve::NistP384 => KeyType::EcdsaSha2Nistp384,
                EcdsaCurve::NistP521 => KeyType::EcdsaSha2Nistp521,
            },
            Algorithm::Dsa => KeyType::Unknown("ssh-dss".to_string()),
            Algorithm::SkEcdsaSha2NistP256 => KeyType::EcdsaSha2Nistp256,
            Algorithm::SkEd25519 => KeyType::Ed25519,
            Algorithm::Other(name) => KeyType::Unknown(format!("{:?}", name)),
            // 处理未来可能添加的新变体
            _ => KeyType::Unknown("unknown".to_string()),
        }
    }

    /// 将 PublicKey 编码为字符串（用于存储和比较）
    ///
    /// 使用 base64 编码的公钥数据，与 OpenSSH known_hosts 格式兼容。
    /// 这比 Debug 格式更稳定，不会因库版本升级而改变。
    fn encode_public_key(public_key: &PublicKey) -> String {
        // 使用 PublicKeyBase64 trait 获取 base64 编码的公钥数据
        // 这与 OpenSSH 的 known_hosts 文件格式兼容
        public_key.public_key_base64()
    }

    /// 获取公钥的指纹（用于显示）
    ///
    /// 使用 russh 内置的 fingerprint 方法生成 SHA256 指纹，
    /// 与 OpenSSH 的 `ssh-keygen -l` 输出格式兼容。
    fn get_key_fingerprint(public_key: &PublicKey) -> String {
        // 使用 SHA256 算法生成指纹，与 OpenSSH 默认行为一致
        let fingerprint = public_key.fingerprint(HashAlg::Sha256);
        fingerprint.to_string()
    }

    /// 验证主机密钥
    fn verify_host_key(&self, server_public_key: &PublicKey) -> bool {
        let key_type = Self::extract_key_type(server_public_key);
        let key_data = Self::encode_public_key(server_public_key);
        let fingerprint = Self::get_key_fingerprint(server_public_key);

        // 不安全模式：跳过所有验证
        if self.allow_insecure {
            let mut warning_logged = self.insecure_warning_logged.lock();
            if !*warning_logged {
                error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                error!("@       WARNING: INSECURE MODE ENABLED!       @");
                error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                error!(
                    "Host key verification is DISABLED for {}:{}",
                    self.server_host, self.server_port
                );
                error!("This makes your connection vulnerable to");
                error!("man-in-the-middle attacks!");
                error!("Use this option ONLY in trusted test environments!");
                *warning_logged = true;
            }

            warn!(
                "Skipping host key verification for {}:{} (insecure mode)",
                self.server_host, self.server_port
            );
            return true;
        }

        info!(
            "Verifying host key for {}:{} - Type: {}, Fingerprint: {}",
            self.server_host, self.server_port, key_type, fingerprint
        );

        match &self.host_key_verification {
            HostKeyVerification::AutoAccept => {
                warn!(
                    "Auto-accepting host key for {}:{} (insecure)",
                    self.server_host, self.server_port
                );
                // 即使是自动接受，也保存到 known_hosts（如果有存储）
                self.save_host_key(&key_type, &key_data);
                true
            },

            HostKeyVerification::Strict => self.verify_strict(&key_type, &key_data, &fingerprint),

            HostKeyVerification::AskOnFirstConnect => {
                self.verify_ask_on_first_connect(&key_type, &key_data, &fingerprint)
            },

            HostKeyVerification::KnownHostsFile(_path) => {
                self.verify_with_known_hosts(&key_type, &key_data, &fingerprint)
            },
        }
    }

    /// 严格模式验证：密钥必须在 known_hosts 中且匹配
    fn verify_strict(&self, key_type: &KeyType, key_data: &str, fingerprint: &str) -> bool {
        let store_guard = self.known_hosts_store.lock();

        if let Some(store) = store_guard.as_ref() {
            match store.verify(&self.server_host, self.server_port, key_type, key_data) {
                VerificationResult::Match => {
                    info!("Host key verified (strict mode): {}", fingerprint);
                    true
                },
                VerificationResult::Unknown => {
                    error!(
                        "Host key verification failed (strict mode): unknown host {}:{}",
                        self.server_host, self.server_port
                    );
                    error!(
                        "Add the host key to known_hosts first, or use a different verification mode"
                    );
                    false
                },
                VerificationResult::Changed {
                    expected_type,
                    expected_key,
                } => {
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("@WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!    @");
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("IT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!");
                    error!(
                        "Someone could be eavesdropping on you right now (man-in-the-middle attack)!"
                    );
                    error!("Host: {}:{}", self.server_host, self.server_port);
                    error!("Expected key type: {}, Got: {}", expected_type, key_type);
                    error!(
                        "Expected key: {}...",
                        &expected_key[..expected_key.len().min(20)]
                    );
                    false
                },
                VerificationResult::Revoked => {
                    error!(
                        "Host key has been revoked for {}:{}",
                        self.server_host, self.server_port
                    );
                    false
                },
            }
        } else {
            error!("Strict mode requires known_hosts store, but none available");
            false
        }
    }

    /// 首次连接询问模式：未知主机时询问用户
    fn verify_ask_on_first_connect(
        &self,
        key_type: &KeyType,
        key_data: &str,
        fingerprint: &str,
    ) -> bool {
        let store_guard = self.known_hosts_store.lock();

        if let Some(store) = store_guard.as_ref() {
            match store.verify(&self.server_host, self.server_port, key_type, key_data) {
                VerificationResult::Match => {
                    info!("Host key verified: {}", fingerprint);
                    true
                },
                VerificationResult::Unknown => {
                    drop(store_guard); // 释放锁，因为回调可能需要时间

                    // 调用用户确认回调
                    let result = if let Some(ref callback) = self.host_key_confirm_callback {
                        info!(
                            "Unknown host {}:{}, asking user for confirmation",
                            self.server_host, self.server_port
                        );
                        callback(
                            &self.server_host,
                            self.server_port,
                            key_type.as_str(),
                            fingerprint,
                        )
                    } else {
                        // 没有回调时，默认接受并保存（与之前行为一致，但会记录警告）
                        warn!(
                            "No host key confirmation callback set, auto-accepting unknown host {}:{}",
                            self.server_host, self.server_port
                        );
                        warn!("This is insecure! Set a confirmation callback for production use.");
                        Some(true)
                    };

                    match result {
                        Some(save_to_known_hosts) => {
                            info!(
                                "User accepted host key for {}:{}",
                                self.server_host, self.server_port
                            );
                            if save_to_known_hosts {
                                self.save_host_key(key_type, key_data);
                            } else {
                                info!("User chose not to save host key to known_hosts");
                            }
                            true
                        },
                        None => {
                            info!(
                                "User rejected host key for {}:{}",
                                self.server_host, self.server_port
                            );
                            false
                        },
                    }
                },
                VerificationResult::Changed {
                    expected_type,
                    expected_key,
                } => {
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!    @");
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("Host: {}:{}", self.server_host, self.server_port);
                    error!(
                        "Expected: {} {}...",
                        expected_type,
                        &expected_key[..expected_key.len().min(20)]
                    );
                    error!("Got: {} {}", key_type, fingerprint);
                    false
                },
                VerificationResult::Revoked => {
                    error!(
                        "Host key has been revoked for {}:{}",
                        self.server_host, self.server_port
                    );
                    false
                },
            }
        } else {
            // 没有存储时，询问用户
            if let Some(ref callback) = self.host_key_confirm_callback {
                let result = callback(
                    &self.server_host,
                    self.server_port,
                    key_type.as_str(),
                    fingerprint,
                );
                // 没有 store，无法保存，只关心是否接受
                match result {
                    Some(_) => {
                        info!("User accepted host key (no known_hosts store available)");
                        true
                    },
                    None => {
                        info!("User rejected host key");
                        false
                    },
                }
            } else {
                warn!("No known_hosts store and no confirmation callback, auto-accepting");
                true
            }
        }
    }

    /// 使用 known_hosts 文件验证
    fn verify_with_known_hosts(
        &self,
        key_type: &KeyType,
        key_data: &str,
        fingerprint: &str,
    ) -> bool {
        let store_guard = self.known_hosts_store.lock();

        if let Some(store) = store_guard.as_ref() {
            match store.verify(&self.server_host, self.server_port, key_type, key_data) {
                VerificationResult::Match => {
                    info!("Host key verified from known_hosts: {}", fingerprint);
                    true
                },
                VerificationResult::Unknown => {
                    drop(store_guard);

                    // 首次连接，询问用户
                    let result = if let Some(ref callback) = self.host_key_confirm_callback {
                        info!(
                            "Host {}:{} not in known_hosts, asking user",
                            self.server_host, self.server_port
                        );
                        callback(
                            &self.server_host,
                            self.server_port,
                            key_type.as_str(),
                            fingerprint,
                        )
                    } else {
                        warn!(
                            "Host {}:{} not in known_hosts, no callback set, rejecting",
                            self.server_host, self.server_port
                        );
                        None
                    };

                    match result {
                        Some(save_to_known_hosts) => {
                            if save_to_known_hosts {
                                self.save_host_key(key_type, key_data);
                            } else {
                                info!("User chose not to save host key to known_hosts");
                            }
                            true
                        },
                        None => false,
                    }
                },
                VerificationResult::Changed {
                    expected_type,
                    expected_key,
                } => {
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!    @");
                    error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                    error!("Host: {}:{}", self.server_host, self.server_port);
                    error!(
                        "Expected: {} {}...",
                        expected_type,
                        &expected_key[..expected_key.len().min(20)]
                    );
                    error!("Got: {} {}", key_type, fingerprint);
                    error!("If this is expected, remove the old key from known_hosts");
                    false
                },
                VerificationResult::Revoked => {
                    error!(
                        "Host key has been revoked for {}:{}",
                        self.server_host, self.server_port
                    );
                    false
                },
            }
        } else {
            error!("KnownHostsFile mode but store not initialized");
            false
        }
    }

    /// 保存主机密钥到 known_hosts
    fn save_host_key(&self, key_type: &KeyType, key_data: &str) {
        let mut store_guard = self.known_hosts_store.lock();

        if let Some(store) = store_guard.as_mut() {
            store.add_or_update(
                &self.server_host,
                self.server_port,
                key_type.clone(),
                key_data.to_string(),
            );

            if let Err(e) = store.save() {
                warn!("Failed to save known_hosts: {}", e);
            } else {
                info!(
                    "Saved host key for {}:{} to known_hosts",
                    self.server_host, self.server_port
                );
            }
        }
    }

    /// 获取 known_hosts 存储路径
    pub fn known_hosts_path(&self) -> Option<PathBuf> {
        let store_guard = self.known_hosts_store.lock();
        store_guard.as_ref().map(|s| s.path().to_path_buf())
    }
}

impl Handler for SshHandler {
    type Error = russh::Error;

    /// 检查服务器公钥
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        info!(
            "Checking server key for {}:{}",
            self.server_host, self.server_port
        );

        let accepted = self.verify_host_key(server_public_key);

        if accepted {
            self.set_state(HandlerState::HostKeyVerified);
            info!("Server key accepted");
        } else {
            error!("Server key rejected");
        }

        Ok(accepted)
    }

    /// 处理通道数据
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Received {} bytes on channel {:?}", data.len(), channel);

        if let Some(ref sender) = self.data_sender {
            if let Err(e) = sender.send(data.to_vec()) {
                error!("Failed to send data to receiver: {}", e);
            }
        }

        Ok(())
    }

    /// 处理扩展数据（stderr）
    async fn extended_data(
        &mut self,
        channel: ChannelId,
        ext: u32,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(
            "Received {} bytes of extended data (ext={}) on channel {:?}",
            data.len(),
            ext,
            channel
        );

        // 将stderr 数据也发送到主数据流
        if let Some(ref sender) = self.data_sender {
            if let Err(e) = sender.send(data.to_vec()) {
                error!("Failed to send extended data to receiver: {}", e);
            }
        }

        Ok(())
    }

    /// 处理 EOF
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Channel {:?} received EOF", channel);
        // 关闭数据发送通道，使 receive_stream 能够正常结束
        self.close_data_sender();
        Ok(())
    }

    /// 处理通道关闭
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Channel {:?} closed", channel);
        self.set_state(HandlerState::Disconnected);
        // 确保数据发送通道关闭
        self.close_data_sender();
        Ok(())
    }

    /// 处理通道打开确认
    async fn channel_open_confirmation(
        &mut self,
        channel: ChannelId,
        max_packet_size: u32,
        window_size: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Channel {:?} opened: max_packet_size={}, window_size={}",
            channel, max_packet_size, window_size
        );
        self.set_state(HandlerState::SessionEstablished);
        Ok(())
    }

    /// 处理通道打开失败
    async fn channel_open_failure(
        &mut self,
        channel: ChannelId,
        reason: russh::ChannelOpenFailure,
        description: &str,
        _language: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        error!(
            "Channel {:?} open failed: {:?} - {}",
            channel, reason, description
        );
        Ok(())
    }
}

/// 创建数据通道
pub fn create_data_channel() -> (DataSender, DataReceiver) {
    mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_state_initial() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AutoAccept,
            "localhost".to_string(),
            22,
            false,
        );
        assert_eq!(handler.state(), HandlerState::Initial);
    }

    #[test]
    fn test_handler_state_transition() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AutoAccept,
            "localhost".to_string(),
            22,
            false,
        );

        handler.set_state(HandlerState::HostKeyVerified);
        assert_eq!(handler.state(), HandlerState::HostKeyVerified);

        handler.set_state(HandlerState::Authenticated);
        assert_eq!(handler.state(), HandlerState::Authenticated);
    }

    #[test]
    fn test_create_data_channel() {
        let (sender, mut receiver) = create_data_channel();

        // 发送数据
        sender.send(vec![1, 2, 3]).unwrap();

        // 接收数据
        let data = receiver.try_recv().unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }

    #[test]
    fn test_handler_with_known_hosts() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AskOnFirstConnect,
            "example.com".to_string(),
            22,
            false,
        );
        // 验证 known_hosts 存储已初始化
        let store_guard = handler.known_hosts_store.lock();
        assert!(store_guard.is_some());
    }

    #[test]
    fn test_handler_auto_accept_no_store() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AutoAccept,
            "example.com".to_string(),
            22,
            false,
        );

        // AutoAccept 模式不需要 known_hosts 存储
        let store_guard = handler.known_hosts_store.lock();
        assert!(store_guard.is_none());
    }

    #[test]
    fn test_handler_with_callback() {
        let (sender, _receiver) = create_data_channel();
        let callback: HostKeyConfirmCallback = Arc::new(|_host, _port, _key_type, _fingerprint| {
            Some(true) // 总是接受并保存
        });

        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AskOnFirstConnect,
            "example.com".to_string(),
            22,
            false,
        )
        .with_host_key_confirm_callback(callback);

        assert!(handler.host_key_confirm_callback.is_some());
    }
}
