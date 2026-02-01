//! SSH 连接模块
//!
//! 实现基于 russh 的 SSH 连接，实现 TerminalConnection trait。

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::BoxStream;
use parking_lot::Mutex;
use russh::Channel;
use russh::client::{self, Handle, Msg};
// 条件编译：平台特定的 stream 类型
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use zeterm_core::errors::ConnectionError;
use zeterm_core::traits::{ConnectionInfo, ConnectionType, TerminalConnection};

use crate::agent::{get_agent_socket_path, is_agent_available};
use crate::config::{AuthMethod, SshConfig};
use crate::handler::{DataReceiver, HostKeyConfirmCallback, SshHandler, create_data_channel};
use crate::keepalive::{KeepaliveCallback, KeepaliveConfig, KeepaliveEvent, KeepaliveManager};

/// 连接心跳回调
///
/// 处理心跳事件，在连接丢失时更新连接状态
struct ConnectionKeepaliveCallback {
    /// 主机名（用于日志）
    host: String,
    /// 连接状态标志
    connected: Arc<std::sync::atomic::AtomicBool>,
}

impl KeepaliveCallback for ConnectionKeepaliveCallback {
    fn on_event(&self, event: KeepaliveEvent) {
        match event {
            KeepaliveEvent::Started => {
                info!("Keepalive started for {}", self.host);
            },
            KeepaliveEvent::Sent { seq } => {
                debug!("Keepalive #{} sent to {}", seq, self.host);
            },
            KeepaliveEvent::Received { seq, rtt } => {
                debug!(
                    "Keepalive #{} received from {}, RTT: {:?}",
                    seq, self.host, rtt
                );
            },
            KeepaliveEvent::Timeout { missed_count } => {
                warn!(
                    "Keepalive timeout for {} (missed: {})",
                    self.host, missed_count
                );
            },
            KeepaliveEvent::ConnectionLost => {
                error!("Connection lost to {} (keepalive failed)", self.host); // 更新连接状态
                self.connected
                    .store(false, std::sync::atomic::Ordering::SeqCst);
            },
            KeepaliveEvent::Stopped => {
                info!("Keepalive stopped for {}", self.host);
            },
        }
    }
}

/// SSH 连接内部状态
struct SshConnectionInner {
    /// SSH 会话句柄
    session: Option<Handle<SshHandler>>,
    /// SSH 通道
    channel: Option<Channel<Msg>>,
    /// 数据接收器（只能取出一次）
    data_receiver: Option<DataReceiver>,
    /// 连接建立时间
    connected_at: Option<Instant>,
    /// 当前终端尺寸
    terminal_size: (u16, u16),
    /// 心跳管理器
    keepalive_manager: Option<KeepaliveManager>,
    /// 心跳任务句柄
    keepalive_handle: Option<JoinHandle<()>>,
}

/// SSH 连接
///
/// 实现 TerminalConnection trait，提供 SSH 连接功能。
pub struct SshConnection {
    /// 连接配置
    config: SshConfig,
    /// 内部状态
    inner: Arc<Mutex<SshConnectionInner>>,
    /// 连接状态
    connected: Arc<std::sync::atomic::AtomicBool>,
    /// 主机密钥确认回调
    host_key_confirm_callback: Option<HostKeyConfirmCallback>,
}

impl SshConnection {
    /// 创建新的 SSH 连接（未连接状态）
    pub fn new(config: SshConfig) -> Self {
        let terminal_size = (config.terminal_cols, config.terminal_rows);

        Self {
            config,
            inner: Arc::new(Mutex::new(SshConnectionInner {
                session: None,
                channel: None,
                data_receiver: None,
                connected_at: None,
                terminal_size,
                keepalive_manager: None,
                keepalive_handle: None,
            })),
            connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            host_key_confirm_callback: None,
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

    /// 建立 SSH 连接
    pub async fn connect(&self) -> Result<(), ConnectionError> {
        // 验证配置
        self.config
            .validate()
            .map_err(|e| ConnectionError::Configuration(e.to_string()))?;

        info!("Connecting to {}...", self.config.address());

        // 创建数据通道
        let (data_sender, data_receiver) = create_data_channel();

        // 创建 SSH Handler
        let mut handler = SshHandler::new(
            data_sender,
            self.config.host_key_verification.clone(),
            self.config.host.clone(),
            self.config.port,
            self.config.allow_insecure,
        );

        // 设置主机密钥确认回调
        if let Some(ref callback) = self.host_key_confirm_callback {
            handler.set_host_key_confirm_callback(callback.clone());
        }

        // 配置 SSH 客户端
        let ssh_config = client::Config {
            inactivity_timeout: self.config.keepalive_interval,
            keepalive_interval: self.config.keepalive_interval,
            keepalive_max: 3,
            ..Default::default()
        };

        // 建立 TCP 连接并进行 SSH 握手
        let mut session = tokio::time::timeout(
            self.config.connect_timeout,
            client::connect(Arc::new(ssh_config), self.config.address(), handler),
        )
        .await
        .map_err(|_| ConnectionError::Timeout)?
        .map_err(|e| ConnectionError::Connection(e.to_string()))?;

        info!("SSH handshake completed, authenticating...");

        // 执行认证
        self.authenticate(&mut session).await?;

        info!("Authentication successful, opening channel...");

        // 打开会话通道
        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| ConnectionError::Connection(e.to_string()))?;

        // 请求 PTY
        channel
            .request_pty(
                false,
                &self.config.terminal_type,
                self.config.terminal_cols as u32,
                self.config.terminal_rows as u32,
                0,
                0,
                &[],
            )
            .await
            .map_err(|e| ConnectionError::Connection(format!("Failed to request PTY: {}", e)))?;

        // 请求 Shell
        channel
            .request_shell(false)
            .await
            .map_err(|e| ConnectionError::Connection(format!("Failed to request shell: {}", e)))?;

        info!("SSH session established on channel {:?}", channel.id());

        // 更新内部状态
        {
            let mut inner = self.inner.lock();
            inner.session = Some(session);
            inner.channel = Some(channel);
            inner.data_receiver = Some(data_receiver);
            inner.connected_at = Some(Instant::now());
        }

        self.connected
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // 启动心跳保活
        self.start_keepalive();

        Ok(())
    }

    /// 启动心跳保活
    fn start_keepalive(&self) {
        let keepalive_interval = match self.config.keepalive_interval {
            Some(interval) => interval,
            None => {
                info!("Keepalive is disabled in config");
                return;
            },
        };

        let keepalive_config = KeepaliveConfig::new()
            .with_interval(keepalive_interval)
            .with_timeout(std::time::Duration::from_secs(15))
            .with_max_missed(3);

        let mut manager = KeepaliveManager::new(keepalive_config);

        // 获取 session 的克隆用于心跳发送
        let inner = self.inner.clone();
        let connected = self.connected.clone();
        let host = self.config.host.clone();

        // 创建心跳发送函数
        // 注意：russh 的 Handle 不支持 Clone，所以我们只检查连接状态标志
        // russh 的 client::Config 中已经配置了 keepalive_interval，会自动发送心跳
        // 这里的 keepalive 主要用于检测连接状态并在超时时更新状态标志
        let send_keepalive = move || {
            let inner = inner.clone();
            let connected = connected.clone();
            let host = host.clone();

            async move {
                // 检查连接状态标志
                if !connected.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("Connection closed".into());
                }

                // 检查 session 是否存在
                let has_session = {
                    let guard = inner.lock();
                    guard.session.is_some() && guard.channel.is_some()
                };

                if has_session {
                    // russh 的 Config 中已经配置了 keepalive，会自动发送心跳
                    // 这里只是确认连接仍然活跃
                    debug!("Keepalive check passed for {}", host);
                    Ok(())
                } else {
                    Err("No active session".into())
                }
            }
        };

        // 创建事件回调
        let callback: Arc<dyn KeepaliveCallback> = Arc::new(ConnectionKeepaliveCallback {
            host: self.config.host.clone(),
            connected: self.connected.clone(),
        });

        // 启动心跳管理器
        let handle = manager.start(send_keepalive, Some(callback));

        // 保存管理器和句柄
        {
            let mut inner = self.inner.lock();
            inner.keepalive_manager = Some(manager);
            inner.keepalive_handle = handle;
        }

        info!("Keepalive started for {}", self.config.host);
    }

    /// 停止心跳保活
    fn stop_keepalive(&self) {
        let mut inner = self.inner.lock();

        // 停止心跳管理器
        if let Some(ref mut manager) = inner.keepalive_manager {
            manager.stop();
        }
        inner.keepalive_manager = None;

        // 取消心跳任务
        if let Some(handle) = inner.keepalive_handle.take() {
            handle.abort();
        }

        info!("Keepalive stopped");
    }

    /// 执行认证
    async fn authenticate(&self, session: &mut Handle<SshHandler>) -> Result<(), ConnectionError> {
        //尝试主认证方法
        let primary_result = self
            .try_auth_method(session, &self.config.auth_method)
            .await;

        if primary_result.is_ok() {
            return primary_result;
        }

        // 如果主方法失败且有回退方法，尝试回退
        if self.config.has_fallback() {
            let primary_error = primary_result.unwrap_err();
            warn!(
                "Primary authentication failed: {}, trying fallback methods",
                primary_error
            );

            for (i, fallback_method) in self.config.fallback_auth_methods.iter().enumerate() {
                info!(
                    "Trying fallback authentication method {}/{}",
                    i + 1,
                    self.config.fallback_auth_methods.len()
                );

                match self.try_auth_method(session, fallback_method).await {
                    Ok(()) => {
                        info!("Fallback authentication successful");
                        return Ok(());
                    },
                    Err(e) => {
                        debug!("Fallback method {} failed: {}", i + 1, e);
                        continue;
                    },
                }
            }

            // 所有方法都失败了
            Err(ConnectionError::Authentication(
                "All authentication methods failed".into(),
            ))
        } else {
            primary_result
        }
    }
    /// 尝试单个认证方法
    async fn try_auth_method(
        &self,
        session: &mut Handle<SshHandler>,
        method: &AuthMethod,
    ) -> Result<(), ConnectionError> {
        match method {
            AuthMethod::None => Err(ConnectionError::Authentication(
                "No authentication method configured".into(),
            )),
            AuthMethod::Password(password) => self.authenticate_password(session, password).await,
            AuthMethod::PublicKey {
                key_path,
                passphrase,
            } => {
                self.authenticate_publickey(session, key_path, passphrase.as_deref())
                    .await
            },
            AuthMethod::Agent => self.authenticate_agent(session).await,
            AuthMethod::KeyboardInteractive => {
                self.authenticate_keyboard_interactive(session).await
            },
        }
    }

    /// Keyboard Interactive 认证
    ///
    /// 支持基于提示的交互式认证，常用于：
    /// - 双因素认证 (2FA)
    /// - 一次性密码 (OTP)
    /// - 挑战-响应认证
    ///
    /// # 当前限制
    ///
    /// **警告**: 当前实现仅支持简单的单次密码提示场景。
    ///
    /// 不支持的功能：
    /// - 多轮交互式认证
    /// - 动态 2FA/TOTP 令牌输入
    /// - 自定义提示响应
    /// - 交互式问答
    ///
    /// 对于需要动态用户输入的场景（如 Google Authenticator），
    /// 请考虑使用其他认证方法（如公钥或 SSH Agent）。
    async fn authenticate_keyboard_interactive(
        &self,
        session: &mut Handle<SshHandler>,
    ) -> Result<(), ConnectionError> {
        info!(
            "尝试 keyboard-interactive 认证 (用户: {})",
            self.config.username
        );
        warn!("Keyboard-interactive 认证当前仅支持简单密码提示，不支持 2FA/TOTP 动态令牌");

        // 获取密码用于响应（如果配置了密码）
        let password = match &self.config.auth_method {
            AuthMethod::Password(pwd) => Some(pwd.clone()),
            _ => {
                // 检查回退方法中是否有密码
                self.config.fallback_auth_methods.iter().find_map(|m| {
                    if let AuthMethod::Password(pwd) = m {
                        Some(pwd.clone())
                    } else {
                        None
                    }
                })
            },
        };

        // 尝试使用 keyboard-interactive 认证
        // 对于简单的密码提示场景，我们提供密码作为响应
        match password {
            Some(pwd) => {
                debug!("使用预配置密码进行 keyboard-interactive 认证");
                // 使用密码进行 keyboard-interactive 认证
                // russh 0.56 的 API：authenticate_keyboard_interactive_start 开始认证
                // 然后通过 authenticate_keyboard_interactive_respond 响应提示

                // 首先尝试启动 keyboard-interactive 认证
                use russh::client::KeyboardInteractiveAuthResponse;

                let auth_start = session
                    .authenticate_keyboard_interactive_start(&self.config.username, None)
                    .await
                    .map_err(|e| ConnectionError::Authentication(e.to_string()))?;

                if matches!(auth_start, KeyboardInteractiveAuthResponse::Success) {
                    info!("Keyboard-interactive authentication successful (no prompts needed)");
                    return Ok(());
                }

                // 如果需要响应提示，提供密码
                let auth_result = session
                    .authenticate_keyboard_interactive_respond(vec![pwd])
                    .await
                    .map_err(|e| ConnectionError::Authentication(e.to_string()))?;

                if matches!(auth_result, KeyboardInteractiveAuthResponse::Success) {
                    info!("Keyboard-interactive authentication successful");
                    Ok(())
                } else {
                    Err(ConnectionError::Authentication(
                        "Keyboard-interactive authentication failed".into(),
                    ))
                }
            },
            None => {
                // 没有可用的密码，无法进行 keyboard-interactive 认证
                error!(
                    "Keyboard-interactive 认证失败: 未配置密码。\
                     提示: 对于 2FA/TOTP，请使用公钥认证或 SSH Agent"
                );
                Err(ConnectionError::Authentication(
                    "Keyboard-interactive 认证需要密码，但未提供。\
                     对于 2FA 场景，建议使用公钥认证"
                        .into(),
                ))
            },
        }
    }

    /// 密码认证
    async fn authenticate_password(
        &self,
        session: &mut Handle<SshHandler>,
        password: &str,
    ) -> Result<(), ConnectionError> {
        debug!(
            "Attempting password authentication for user: {}",
            self.config.username
        );

        let auth_result = session
            .authenticate_password(&self.config.username, password)
            .await
            .map_err(|e| ConnectionError::Authentication(e.to_string()))?;

        if auth_result.success() {
            info!("Password authentication successful");
            Ok(())
        } else {
            Err(ConnectionError::Authentication(
                "Password authentication failed".into(),
            ))
        }
    }

    /// 公钥认证
    ///
    /// 支持的密钥类型：
    /// - RSA
    /// - Ed25519
    /// - ECDSA (NIST P-256, P-384, P-521) - 内置支持
    ///
    /// # 参数
    /// - `session`: SSH 会话句柄
    /// - `key_path`: 私钥文件路径
    /// - `passphrase`: 私钥密码（如果有）
    ///
    /// # 示例
    /// ```ignore
    /// use std::path::Path;
    /// # use zeterm_ssh::{SshConnection, SshHandler};
    /// # use russh::client::Handle;
    /// # use zeterm_core::ConnectionError;
    /// # async fn example(conn: &SshConnection, session: &mut Handle<SshHandler>) -> Result<(), ConnectionError> {
    /// conn.authenticate_publickey(session, Path::new("~/.ssh/id_rsa"), None).await?;
    /// conn.authenticate_publickey(session, Path::new("~/.ssh/id_ecdsa"), Some("passphrase")).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn authenticate_publickey(
        &self,
        session: &mut Handle<SshHandler>,
        key_path: &std::path::Path,
        passphrase: Option<&str>,
    ) -> Result<(), ConnectionError> {
        debug!(
            "Attempting public key authentication for user: {} with key: {:?}",
            self.config.username, key_path
        );

        // 加载私钥
        // russh 的 load_secret_key 函数会自动识别密钥类型（RSA、Ed25519、ECDSA 等）
        // ECDSA 支持是内置的（通过 p256/p384/p521 依赖）
        let key_pair = russh::keys::load_secret_key(key_path, passphrase)
            .map_err(|e| ConnectionError::Authentication(format!("Failed to load key: {}", e)))?;

        // 创建带哈希算法的私钥
        let key_with_hash = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), None);

        let auth_result = session
            .authenticate_publickey(&self.config.username, key_with_hash)
            .await
            .map_err(|e| ConnectionError::Authentication(e.to_string()))?;

        if auth_result.success() {
            info!("Public key authentication successful");
            Ok(())
        } else {
            Err(ConnectionError::Authentication(
                "Public key authentication failed".into(),
            ))
        }
    }

    /// SSH Agent 认证 - Unix 实现
    #[cfg(unix)]
    async fn authenticate_agent(
        &self,
        session: &mut Handle<SshHandler>,
    ) -> Result<(), ConnectionError> {
        debug!(
            "Attempting SSH Agent authentication for user: {}",
            self.config.username
        );

        // 检查 Agent 是否可用
        if !is_agent_available() {
            warn!("SSH Agent not available (SSH_AUTH_SOCK not set)");
            return Err(ConnectionError::Authentication(
                "SSH Agent not available: SSH_AUTH_SOCK environment variable not set".into(),
            ));
        }

        // 获取 Agent socket 路径
        let socket_path = get_agent_socket_path().map_err(|e| {
            ConnectionError::Authentication(format!("Failed to get agent socket: {}", e))
        })?;

        info!("Connecting to SSH Agent at: {}", socket_path);

        // Unix: 使用 UnixStream 连接
        let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
            error!("Failed to connect to SSH Agent: {}", e);
            ConnectionError::Authentication(format!("Failed to connect to SSH Agent: {}", e))
        })?;

        // 创建 Agent 客户端并执行认证
        let mut agent_client = russh::keys::agent::client::AgentClient::connect(stream);
        self.do_agent_auth(session, &mut agent_client).await
    }

    /// SSH Agent 认证 - Windows 实现
    #[cfg(windows)]
    async fn authenticate_agent(
        &self,
        session: &mut Handle<SshHandler>,
    ) -> Result<(), ConnectionError> {
        use tokio::net::windows::named_pipe::ClientOptions;

        debug!(
            "Attempting SSH Agent authentication for user: {}",
            self.config.username
        );

        // 检查 Agent 是否可用
        if !is_agent_available() {
            warn!("SSH Agent not available on Windows");
            return Err(ConnectionError::Authentication(
                "SSH Agent not available: OpenSSH Agent service may not be running".into(),
            ));
        }

        // 获取 Agent 命名管道路径
        let pipe_path = get_agent_socket_path().map_err(|e| {
            ConnectionError::Authentication(format!("Failed to get agent pipe path: {}", e))
        })?;

        info!("Connecting to SSH Agent at: {}", pipe_path);

        // Windows: 使用命名管道连接
        let stream = ClientOptions::new().open(&pipe_path).map_err(|e| {
            error!("Failed to connect to SSH Agent: {}", e);
            ConnectionError::Authentication(format!("Failed to connect to SSH Agent: {}", e))
        })?;

        // 创建 Agent 客户端并执行认证
        let mut agent_client = russh::keys::agent::client::AgentClient::connect(stream);
        self.do_agent_auth(session, &mut agent_client).await
    }

    /// 公共的 Agent 认证逻辑
    async fn do_agent_auth<S>(
        &self,
        session: &mut Handle<SshHandler>,
        agent_client: &mut russh::keys::agent::client::AgentClient<S>,
    ) -> Result<(), ConnectionError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        // 获取 Agent 中的密钥列表
        let identities = agent_client.request_identities().await.map_err(|e| {
            error!("Failed to list keys from SSH Agent: {}", e);
            ConnectionError::Authentication(format!("Failed to list keys from SSH Agent: {}", e))
        })?;

        if identities.is_empty() {
            warn!("No keys found in SSH Agent");
            return Err(ConnectionError::Authentication(
                "No keys available in SSH Agent".into(),
            ));
        }

        info!("Found {} key(s) in SSH Agent", identities.len());

        // 尝试使用每个密钥进行认证
        for (i, identity) in identities.iter().enumerate() {
            let key_comment = identity.comment();
            debug!("Trying key {}/{}: {}", i + 1, identities.len(), key_comment);

            // 使用 Agent 进行公钥认证
            let auth_result = session
                .authenticate_publickey_with(
                    &self.config.username,
                    identity.clone(),
                    None,
                    agent_client,
                )
                .await;

            match auth_result {
                Ok(result) if result.success() => {
                    info!(
                        "SSH Agent authentication successful with key: {}",
                        key_comment
                    );
                    return Ok(());
                },
                Ok(_) => {
                    debug!("Key {} rejected by server", key_comment);
                },
                Err(e) => {
                    debug!("Authentication error with key {}: {}", key_comment, e);
                },
            }
        }

        // 所有密钥都失败了
        Err(ConnectionError::Authentication(
            "SSH Agent authentication failed: no key accepted by server".into(),
        ))
    }

    /// 获取配置
    pub fn config(&self) -> &SshConfig {
        &self.config
    }
}

#[async_trait]
impl TerminalConnection for SshConnection {
    async fn write(&self, bytes: &[u8]) -> Result<(), ConnectionError> {
        // 取出 channel，释放锁
        let channel = {
            let mut inner = self.inner.lock();
            inner.channel.take().ok_or(ConnectionError::NotConnected)?
        };

        // 发送数据到通道
        let result = channel.data(bytes).await;

        // 放回 channel
        {
            let mut inner = self.inner.lock();
            inner.channel = Some(channel);
        }

        result.map_err(|e| ConnectionError::Io(e.to_string()))?;
        Ok(())
    }

    async fn resize(&self, rows: u16, cols: u16) -> Result<(), ConnectionError> {
        // 取出 channel，释放锁
        let channel = {
            let mut inner = self.inner.lock();
            inner.channel.take().ok_or(ConnectionError::NotConnected)?
        };

        // 发送窗口大小变更
        let result = channel.window_change(cols as u32, rows as u32, 0, 0).await;

        // 放回 channel 并更新终端尺寸
        {
            let mut inner = self.inner.lock();
            inner.channel = Some(channel);
            inner.terminal_size = (cols, rows);
        }

        result.map_err(|e| ConnectionError::Io(e.to_string()))?;
        debug!("Terminal resized to {}x{}", cols, rows);

        Ok(())
    }

    fn receive_stream(&self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>> {
        let mut inner = self.inner.lock();

        if let Some(receiver) = inner.data_receiver.take() {
            // 将receiver 转换为 Stream
            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);
            Box::pin(stream.map(Ok))
        } else {
            // 已经被取走了，返回空流
            warn!("receive_stream() called multiple times, returning empty stream");
            Box::pin(futures::stream::empty())
        }
    }

    async fn close(&self) -> Result<(), ConnectionError> {
        info!("Closing SSH connection...");

        // 先停止心跳
        self.stop_keepalive();

        // 取出 channel 和 session，释放锁
        let (channel, session) = {
            let mut inner = self.inner.lock();
            (inner.channel.take(), inner.session.take())
        };

        // 发送 EOF
        if let Some(channel) = channel {
            if let Err(e) = channel.eof().await {
                warn!("Failed to send EOF: {}", e);
            }
        }

        // 断开连接
        if let Some(session) = session {
            if let Err(e) = session
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "User requested disconnect",
                    "",
                )
                .await
            {
                warn!("Failed to disconnect: {}", e);
            }
        }

        // 清理剩余状态
        {
            let mut inner = self.inner.lock();
            inner.data_receiver = None;
        }

        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);

        info!("SSH connection closed");
        Ok(())
    }
}

impl ConnectionInfo for SshConnection {
    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn connection_type(&self) -> ConnectionType {
        ConnectionType::Ssh
    }

    fn remote_address(&self) -> Option<String> {
        if self.is_connected() {
            Some(self.config.address())
        } else {
            None
        }
    }

    fn connected_at(&self) -> Option<Instant> {
        self.inner.lock().connected_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_connection_new() {
        let config = SshConfig::new("example.com", "user").with_password("secret");
        let conn = SshConnection::new(config);

        assert!(!conn.is_connected());
        assert_eq!(conn.connection_type(), ConnectionType::Ssh);
        assert_eq!(conn.remote_address(), None);
    }

    #[test]
    fn test_ssh_connection_config() {
        let config = SshConfig::new("example.com", "user")
            .with_port(2222)
            .with_password("secret");
        let conn = SshConnection::new(config);

        assert_eq!(conn.config().host, "example.com");
        assert_eq!(conn.config().port, 2222);
        assert_eq!(conn.config().username, "user");
    }
}
