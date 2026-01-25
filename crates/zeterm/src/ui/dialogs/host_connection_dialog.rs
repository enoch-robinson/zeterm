//! 主机连接对话框
//!
//! 用于新建和编辑主机配置的对话框组件。

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, Sizable, Size, button::Button};
use parking_lot::Mutex;
use tracing::{info, warn};
use zeterm_core::entities::{AuthConfig, HostConfig, HostId};

/// 对话框模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogMode {
    /// 新建主机
    Create,
    /// 编辑主机
    Edit,
}

/// 认证类型（用于 UI 选择）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// 密码认证
    Password,
    /// 公钥认证
    PublicKey,
    /// SSH Agent认证
    Agent,
}

impl AuthType {
    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            AuthType::Password => "Password",
            AuthType::PublicKey => "Public Key",
            AuthType::Agent => "SSH Agent",
        }
    }

    /// 从 AuthConfig 推断认证类型
    pub fn from_auth_config(auth: &AuthConfig) -> Self {
        match auth {
            AuthConfig::Password { .. } => AuthType::Password,
            AuthConfig::PublicKey { .. } => AuthType::PublicKey,
            AuthConfig::Agent => AuthType::Agent,
        }
    }
}

/// 表单数据
#[derive(Debug, Clone)]
struct FormData {
    /// 主机名称
    name: String,
    /// 主机地址
    host: String,
    /// SSH 端口
    port: String,
    /// 用户名
    username: String,
    /// 认证类型
    auth_type: AuthType,
    /// 密码引用（用于密码认证）
    password_ref: String,
    /// 私钥路径（用于公钥认证）
    key_path: String,
    /// 私钥密码引用（用于公钥认证，可选）
    passphrase_ref: String,
    /// 分组（可选）
    group: String,
    /// 描述（可选）
    description: String,
}

impl FormData {
    /// 创建空表单数据（用于新建）
    fn new() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: "22".to_string(),
            username: String::new(),
            auth_type: AuthType::Password,
            password_ref: String::new(),
            key_path: String::new(),
            passphrase_ref: String::new(),
            group: String::new(),
            description: String::new(),
        }
    }

    /// 从HostConfig 创建表单数据（用于编辑）
    fn from_host_config(config: &HostConfig) -> Self {
        let auth_type = AuthType::from_auth_config(&config.auth_config);
        let (password_ref, key_path, passphrase_ref) = match &config.auth_config {
            AuthConfig::Password { password_ref } => {
                (password_ref.clone(), String::new(), String::new())
            },
            AuthConfig::PublicKey {
                key_path,
                passphrase_ref,
            } => (
                String::new(),
                key_path.to_string_lossy().to_string(),
                passphrase_ref.clone().unwrap_or_default(),
            ),
            AuthConfig::Agent => (String::new(), String::new(), String::new()),
        };

        Self {
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port.to_string(),
            username: config.username.clone(),
            auth_type,
            password_ref,
            key_path,
            passphrase_ref,
            group: config.group.clone().unwrap_or_default(),
            description: config.description.clone().unwrap_or_default(),
        }
    }

    /// 转换为 HostConfig
    fn to_host_config(&self, original_id: Option<HostId>) -> Result<HostConfig, String> {
        // 验证端口
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .map_err(|_| "Invalid port number".to_string())?;

        // 构建认证配置
        let auth_config = match self.auth_type {
            AuthType::Password => {
                if self.password_ref.trim().is_empty() {
                    return Err("Password reference cannot be empty".to_string());
                }
                AuthConfig::password(self.password_ref.clone())
            },
            AuthType::PublicKey => {
                if self.key_path.trim().is_empty() {
                    return Err("Key path cannot be empty".to_string());
                }
                let passphrase = if self.passphrase_ref.trim().is_empty() {
                    None
                } else {
                    Some(self.passphrase_ref.clone())
                };
                AuthConfig::public_key(PathBuf::from(&self.key_path), passphrase)
            },
            AuthType::Agent => AuthConfig::agent(),
        };

        // 构建 HostConfig
        let mut config = HostConfig::new(
            self.name.clone(),
            self.host.clone(),
            self.username.clone(),
            auth_config,
        );
        config.id = original_id;
        config.port = port;

        if !self.group.trim().is_empty() {
            config.group = Some(self.group.clone());
        }

        if !self.description.trim().is_empty() {
            config.description = Some(self.description.clone());
        }

        Ok(config)
    }

    /// 验证表单数据
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Host name cannot be empty".to_string());
        }

        if self.host.trim().is_empty() {
            return Err("Host address cannot be empty".to_string());
        }

        if self.username.trim().is_empty() {
            return Err("Username cannot be empty".to_string());
        }

        // 验证端口
        if self.port.trim().parse::<u16>().is_err() {
            return Err("Invalid port number".to_string());
        }

        // 验证认证配置
        match self.auth_type {
            AuthType::Password => {
                if self.password_ref.trim().is_empty() {
                    return Err("Password reference cannot be empty".to_string());
                }
            },
            AuthType::PublicKey => {
                if self.key_path.trim().is_empty() {
                    return Err("Key path cannot be empty".to_string());
                }
            },
            AuthType::Agent => {},
        }

        Ok(())
    }
}

/// 当前正在编辑的字段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditingField {
    Name,
    Host,
    Port,
    Username,
    PasswordRef,
    KeyPath,
    PassphraseRef,
    Group,
    Description,
}

/// 主机连接对话框
pub struct HostConnectionDialog {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 对话框模式
    mode: DialogMode,
    /// 原始主机配置（编辑模式时使用）
    original_config: Option<HostConfig>,
    /// 表单数据
    form_data: Arc<Mutex<FormData>>,
    /// 验证错误信息
    validation_error: Arc<Mutex<Option<String>>>,
    /// 保存回调
    on_save: Option<Arc<dyn Fn(HostConfig) + Send + Sync>>,
    /// 取消回调
    on_cancel: Option<Arc<dyn Fn() + Send + Sync>>,
    /// 当前正在编辑的字段
    editing_field: Option<EditingField>,
}

impl HostConnectionDialog {
    /// 创建新建主机对话框
    pub fn new_create(cx: &mut Context<Self>) -> Self {
        info!("Creating new host connection dialog (create mode)");

        Self {
            focus_handle: cx.focus_handle(),
            mode: DialogMode::Create,
            original_config: None,
            form_data: Arc::new(Mutex::new(FormData::new())),
            validation_error: Arc::new(Mutex::new(None)),
            on_save: None,
            on_cancel: None,
            editing_field: None,
        }
    }

    /// 创建编辑主机对话框
    pub fn new_edit(config: HostConfig, cx: &mut Context<Self>) -> Self {
        info!(
            "Creating new host connection dialog (edit mode) for host: {}",
            config.name
        );

        let form_data = FormData::from_host_config(&config);

        Self {
            focus_handle: cx.focus_handle(),
            mode: DialogMode::Edit,
            original_config: Some(config),
            form_data: Arc::new(Mutex::new(form_data)),
            validation_error: Arc::new(Mutex::new(None)),
            on_save: None,
            on_cancel: None,
            editing_field: None,
        }
    }

    /// 开始编辑指定字段
    fn start_editing(&mut self, field: EditingField, cx: &mut Context<Self>) {
        self.editing_field = Some(field);
        cx.notify();
    }

    /// 停止编辑
    fn stop_editing(&mut self, cx: &mut Context<Self>) {
        self.editing_field = None;
        cx.notify();
    }

    /// 处理键盘输入
    fn handle_key_input(&mut self, input: &str, cx: &mut Context<Self>) {
        if let Some(ref field) = self.editing_field {
            let mut form_data = self.form_data.lock();
            match field {
                EditingField::Name => form_data.name.push_str(input),
                EditingField::Host => form_data.host.push_str(input),
                EditingField::Port => form_data.port.push_str(input),
                EditingField::Username => form_data.username.push_str(input),
                EditingField::PasswordRef => form_data.password_ref.push_str(input),
                EditingField::KeyPath => form_data.key_path.push_str(input),
                EditingField::PassphraseRef => form_data.passphrase_ref.push_str(input),
                EditingField::Group => form_data.group.push_str(input),
                EditingField::Description => form_data.description.push_str(input),
            }
            drop(form_data);
            *self.validation_error.lock() = None;
            cx.notify();
        }
    }

    /// 处理退格键
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(ref field) = self.editing_field {
            let mut form_data = self.form_data.lock();
            let target = match field {
                EditingField::Name => &mut form_data.name,
                EditingField::Host => &mut form_data.host,
                EditingField::Port => &mut form_data.port,
                EditingField::Username => &mut form_data.username,
                EditingField::PasswordRef => &mut form_data.password_ref,
                EditingField::KeyPath => &mut form_data.key_path,
                EditingField::PassphraseRef => &mut form_data.passphrase_ref,
                EditingField::Group => &mut form_data.group,
                EditingField::Description => &mut form_data.description,
            };
            target.pop();
            drop(form_data);
            cx.notify();
        }
    }

    /// 设置保存回调
    pub fn with_on_save<F>(mut self, callback: F) -> Self
    where
        F: Fn(HostConfig) + Send + Sync + 'static,
    {
        self.on_save = Some(Arc::new(callback));
        self
    }

    /// 设置取消回调
    pub fn with_on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_cancel = Some(Arc::new(callback));
        self
    }

    /// 获取对话框标题
    fn title(&self) -> &'static str {
        match self.mode {
            DialogMode::Create => "New Host Connection",
            DialogMode::Edit => "Edit Host Connection",
        }
    }

    /// 处理保存
    fn handle_save(&mut self, cx: &mut Context<Self>) {
        let form_data = self.form_data.lock().clone();

        // 验证表单
        if let Err(error) = form_data.validate() {
            warn!("Form validation failed: {}", error);
            *self.validation_error.lock() = Some(error);
            cx.notify();
            return;
        }

        // 转换为 HostConfig
        let original_id = self.original_config.as_ref().and_then(|c| c.id);
        match form_data.to_host_config(original_id) {
            Ok(config) => {
                info!("Host config created/updated: {}", config.name);
                *self.validation_error.lock() = None;

                if let Some(ref callback) = self.on_save {
                    callback(config);
                }
            },
            Err(error) => {
                warn!("Failed to create host config: {}", error);
                *self.validation_error.lock() = Some(error);
                cx.notify();
            },
        }
    }

    /// 处理取消
    fn handle_cancel(&mut self, _cx: &mut Context<Self>) {
        info!("Host connection dialog cancelled");

        if let Some(ref callback) = self.on_cancel {
            callback();
        }
    }

    /// 更新表单字段
    fn update_field<F>(&mut self, updater: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut FormData),
    {
        let mut form_data = self.form_data.lock();
        updater(&mut form_data);
        drop(form_data);

        // 清除验证错误
        *self.validation_error.lock() = None;
        cx.notify();
    }
}

impl Focusable for HostConnectionDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostConnectionDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let validation_error = self.validation_error.lock().clone();
        let has_focus = self.focus_handle.is_focused(_window);

        // 对话框容器
        div()
            .id("host-connection-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000080))
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            // ESC 键：取消编辑或关闭对话框
                            if this.editing_field.is_some() {
                                this.stop_editing(cx);
                            } else {
                                this.handle_cancel(cx);
                            }
                        },
                        "enter" => {
                            // Enter 键：保存表单
                            if this.editing_field.is_none() {
                                this.handle_save(cx);
                            }
                        },
                        "backspace" => {
                            // 退格键：删除字符
                            if this.editing_field.is_some() {
                                this.handle_backspace(cx);
                            }
                        },
                        "tab" => {
                            // Tab 键：切换焦点（暂不实现）
                            cx.notify();
                        },
                        _ => {
                            // 处理字符输入（非控制键）
                            if this.editing_field.is_some() {
                                let key = event.keystroke.key.as_str();
                                // 只处理可打印字符（单字符，不是控制键）
                                if key.len() == 1 && !matches!(key, "\u{1b}" | "\r" | "\n" | "\t") {
                                    let input = if event.keystroke.modifiers.shift
                                        && key.chars().all(|c| c.is_ascii_lowercase())
                                    {
                                        key.to_uppercase()
                                    } else {
                                        key.to_string()
                                    };
                                    this.handle_key_input(&input, cx);
                                }
                            }
                        },
                    }
                }),
            )
            .child(
                div()
                    .id("host-connection-dialog")
                    .track_focus(&self.focus_handle)
                    .w(px(600.0))
                    .max_h(px(700.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(if has_focus {
                        gpui::rgb(0x3b82f6).into()
                    } else {
                        theme.border
                    })
                    .rounded_lg()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    // 标题栏
                    .child(self.render_header(cx))
                    // 内容区域（可滚动）
                    .child({
                        let mut content = div().flex_1().p_4().flex().flex_col().gap_4();

                        // 验证错误提示
                        if let Some(error) = validation_error {
                            content = content.child(self.render_error_message(&error, cx));
                        }

                        // 表单字段
                        content.child(self.render_form_fields(cx))
                    })
                    // 底部按钮栏
                    .child(self.render_footer(cx)),
            )
    }
}

impl HostConnectionDialog {
    /// 渲染标题栏
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div().p_4().border_b_1().border_color(theme.border).child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.foreground)
                .child(self.title()),
        )
    }

    /// 渲染错误消息
    fn render_error_message(&self, error: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .p_3()
            .bg(gpui::rgba(0xef444420))
            .border_1()
            .border_color(gpui::rgb(0xef4444))
            .rounded_md()
            .flex()
            .items_center()
            .gap_2()
            .child(div().text_color(gpui::rgb(0xef4444)).child("⚠"))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(error.to_string()),
            )
    }

    /// 渲染底部按钮栏
    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .p_4()
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel-btn")
                    .label("Cancel")
                    .with_size(Size::Medium)
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.handle_cancel(cx);
                    })),
            )
            .child(
                Button::new("save-btn")
                    .label(match self.mode {
                        DialogMode::Create => "Create",
                        DialogMode::Edit => "Save",
                    })
                    .with_size(Size::Medium)
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.handle_save(cx);
                    })),
            )
    }

    /// 渲染表单字段
    fn render_form_fields(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form_data = self.form_data.lock();
        let name = form_data.name.clone();
        let host = form_data.host.clone();
        let port = form_data.port.clone();
        let username = form_data.username.clone();
        let auth_type = form_data.auth_type;
        let group = form_data.group.clone();
        let description = form_data.description.clone();
        drop(form_data);

        div()
            .flex()
            .flex_col()
            .gap_4()
            // 主机名称
            .child(self.render_text_field("Name", "name", &name, true, cx))
            // 主机地址和端口（同一行）
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(self.render_text_field("Host", "host", &host, true, cx)),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .child(self.render_text_field("Port", "port", &port, true, cx)),
                    ),
            )
            // 用户名
            .child(self.render_text_field("Username", "username", &username, true, cx))
            // 认证类型选择器
            .child(self.render_auth_type_selector(auth_type, cx))
            // 认证相关字段（根据类型动态显示）
            .child(self.render_auth_fields(auth_type, cx))
            // 分组（可选）
            .child(self.render_text_field("Group", "group", &group, false, cx))
            // 描述（可选）
            .child(self.render_textarea_field("Description", "description", &description, cx))
    }

    /// 渲染文本输入框
    fn render_text_field(
        &self,
        label: &str,
        field_id: &str,
        value: &str,
        required: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let value_clone = value.to_string();
        let field_id_str = field_id.to_string();
        let label_str = label.to_string();

        // 确定当前字段是否正在编辑
        let is_editing = match (self.editing_field, field_id) {
            (Some(EditingField::Name), "name") => true,
            (Some(EditingField::Host), "host") => true,
            (Some(EditingField::Port), "port") => true,
            (Some(EditingField::Username), "username") => true,
            (Some(EditingField::PasswordRef), "password_ref") => true,
            (Some(EditingField::KeyPath), "key_path") => true,
            (Some(EditingField::PassphraseRef), "passphrase_ref") => true,
            (Some(EditingField::Group), "group") => true,
            (Some(EditingField::Description), "description") => true,
            _ => false,
        };

        // 解析字段类型用于点击处理
        let field_type = match field_id {
            "name" => EditingField::Name,
            "host" => EditingField::Host,
            "port" => EditingField::Port,
            "username" => EditingField::Username,
            "password_ref" => EditingField::PasswordRef,
            "key_path" => EditingField::KeyPath,
            "passphrase_ref" => EditingField::PassphraseRef,
            "group" => EditingField::Group,
            "description" => EditingField::Description,
            _ => EditingField::Name,
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child({
                let label_div = div().flex().items_center().gap_1().child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(label_str),
                );

                if required {
                    label_div.child(div().text_xs().text_color(gpui::rgb(0xef4444)).child("*"))
                } else {
                    label_div
                }
            })
            .child(
                div()
                    .id(field_id.to_string())
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(if is_editing {
                        gpui::rgb(0x3b82f6).into()
                    } else {
                        theme.border
                    })
                    .rounded_md()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(if value_clone.is_empty() && !is_editing {
                        format!("{}...", label)
                    } else {
                        // 密码字段显示星号
                        if field_id == "password_ref" && !is_editing && !value_clone.is_empty() {
                            "•".repeat(value_clone.len().min(20))
                        } else {
                            value_clone.clone()
                        }
                    })
                    .cursor_text()
                    .hover(|style| style.border_color(gpui::rgb(0x3b82f6)))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        // 点击开始编辑该字段
                        this.start_editing(field_type, cx);
                        info!(
                            "Text field clicked: {} (editing: {:?})",
                            field_id_str, field_type
                        );
                    })),
            )
    }

    /// 渲染文本区域
    fn render_textarea_field(
        &self,
        label: &str,
        field_id: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let value_clone = value.to_string();
        let field_id_str = field_id.to_string();
        let label_str = label.to_string();

        // 确定当前字段是否正在编辑
        let is_editing = match (self.editing_field, field_id) {
            (Some(EditingField::Description), "description") => true,
            _ => false,
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(label_str),
            )
            .child(
                div()
                    .id(field_id.to_string())
                    .w_full()
                    .px_3()
                    .py_2()
                    .min_h(px(80.0))
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(if is_editing {
                        gpui::rgb(0x3b82f6).into()
                    } else {
                        theme.border
                    })
                    .rounded_md()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(if value_clone.is_empty() && !is_editing {
                        format!("{}...", label)
                    } else {
                        value_clone.clone()
                    })
                    .cursor_text()
                    .hover(|style| style.border_color(gpui::rgb(0x3b82f6)))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        // 点击开始编辑该字段
                        this.start_editing(EditingField::Description, cx);
                        info!("Textarea field clicked: {}", field_id_str);
                        cx.notify();
                    })),
            )
    }

    /// 渲染认证类型选择器
    fn render_auth_type_selector(
        &self,
        current_type: AuthType,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child("Authentication Type"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_auth_type_button(AuthType::Password, current_type, cx))
                    .child(self.render_auth_type_button(AuthType::PublicKey, current_type, cx))
                    .child(self.render_auth_type_button(AuthType::Agent, current_type, cx)),
            )
    }

    /// 渲染认证类型按钮
    fn render_auth_type_button(
        &self,
        auth_type: AuthType,
        current_type: AuthType,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let is_selected = auth_type == current_type;

        let bg_color = if is_selected {
            gpui::rgb(0x3b82f6).into()
        } else {
            theme.secondary
        };

        let text_color = if is_selected {
            gpui::white().into()
        } else {
            theme.foreground
        };

        div()
            .id(format!("auth-type-{:?}", auth_type))
            .px_4()
            .py_2()
            .bg(bg_color)
            .border_1()
            .border_color(if is_selected {
                gpui::rgb(0x3b82f6).into()
            } else {
                theme.border
            })
            .rounded_md()
            .text_sm()
            .text_color(text_color)
            .child(auth_type.display_name())
            .cursor_pointer()
            .hover(|style| {
                if !is_selected {
                    style.bg(theme.border)
                } else {
                    style
                }
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.update_field(
                    |form| {
                        form.auth_type = auth_type;
                    },
                    cx,
                );
            }))
    }

    /// 渲染认证相关字段
    fn render_auth_fields(&self, auth_type: AuthType, cx: &mut Context<Self>) -> impl IntoElement {
        let form_data = self.form_data.lock();
        let password_ref = form_data.password_ref.clone();
        let key_path = form_data.key_path.clone();
        let passphrase_ref = form_data.passphrase_ref.clone();
        drop(form_data);

        match auth_type {
            AuthType::Password => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(self.render_text_field(
                    "Password Reference",
                    "password_ref",
                    &password_ref,
                    true,
                    cx,
                ))
                .child(self.render_help_text(
                    "Use'keychain:key_name' or 'env:VAR_NAME' to reference password",
                    cx,
                )),
            AuthType::PublicKey => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(self.render_text_field("Private Key Path", "key_path", &key_path, true, cx))
                .child(self.render_text_field(
                    "Passphrase Reference (Optional)",
                    "passphrase_ref",
                    &passphrase_ref,
                    false,
                    cx,
                ))
                .child(self.render_help_text(
                    "Path to your SSH private key file (e.g., ~/.ssh/id_rsa)",
                    cx,
                )),
            AuthType::Agent => div()
                .p_3()
                .bg(gpui::rgba(0x3b82f620))
                .border_1()
                .border_color(gpui::rgb(0x3b82f6))
                .rounded_md()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child("SSH Agent authentication will use your system's SSH agent."),
                ),
        }
    }

    /// 渲染帮助文本
    fn render_help_text(&self, text: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(text.to_string())
    }
}
