//! 主机密钥确认对话框
//!
//! 当连接到未知 SSH 主机时，显示此对话框让用户确认是否信任该主机的密钥。

use std::sync::Arc;

use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, Sizable, Size, button::Button};
use parking_lot::Mutex;
use tracing::info;

/// 主机密钥信息
#[derive(Debug, Clone)]
pub struct HostKeyInfo {
    /// 主机名
    pub hostname: String,
    /// 端口
    pub port: u16,
    /// 密钥类型
    pub key_type: String,
    /// 密钥指纹
    pub fingerprint: String,
}

impl HostKeyInfo {
    /// 创建新的主机密钥信息
    pub fn new(
        hostname: impl Into<String>,
        port: u16,
        key_type: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            hostname: hostname.into(),
            port,
            key_type: key_type.into(),
            fingerprint: fingerprint.into(),
        }
    }

    /// 获取显示用的主机地址
    pub fn display_address(&self) -> String {
        if self.port == 22 {
            self.hostname.clone()
        } else {
            format!("{}:{}", self.hostname, self.port)
        }
    }
}

/// 用户响应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyResponse {
    /// 接受密钥
    Accept,
    /// 拒绝密钥
    Reject,
    /// 等待用户响应
    Pending,
}

impl Default for HostKeyResponse {
    fn default() -> Self {
        Self::Pending
    }
}

/// 主机密钥确认对话框
pub struct HostKeyDialog {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 主机密钥信息
    host_key_info: HostKeyInfo,
    /// 用户响应
    response: Arc<Mutex<HostKeyResponse>>,
    /// 是否记住选择
    remember_choice: Arc<Mutex<bool>>,
    /// 响应回调
    on_response: Option<Arc<dyn Fn(HostKeyResponse, bool) + Send + Sync>>,
}

impl HostKeyDialog {
    /// 创建新的对话框
    pub fn new(host_key_info: HostKeyInfo, cx: &mut Context<Self>) -> Self {
        info!(
            "Creating host key dialog for {}:{}",
            host_key_info.hostname, host_key_info.port
        );

        Self {
            focus_handle: cx.focus_handle(),
            host_key_info,
            response: Arc::new(Mutex::new(HostKeyResponse::Pending)),
            remember_choice: Arc::new(Mutex::new(true)),
            on_response: None,
        }
    }

    /// 设置响应回调
    pub fn with_on_response<F>(mut self, callback: F) -> Self
    where
        F: Fn(HostKeyResponse, bool) + Send + Sync + 'static,
    {
        self.on_response = Some(Arc::new(callback));
        self
    }

    /// 获取用户响应
    pub fn response(&self) -> HostKeyResponse {
        *self.response.lock()
    }

    /// 处理接受
    fn do_accept(&mut self, cx: &mut Context<Self>) {
        info!(
            "User accepted host key for {}",
            self.host_key_info.display_address()
        );
        *self.response.lock() = HostKeyResponse::Accept;
        let remember = *self.remember_choice.lock();

        if let Some(ref callback) = self.on_response {
            callback(HostKeyResponse::Accept, remember);
        }
        cx.notify();
    }

    /// 处理拒绝
    fn do_reject(&mut self, cx: &mut Context<Self>) {
        info!(
            "User rejected host key for {}",
            self.host_key_info.display_address()
        );
        *self.response.lock() = HostKeyResponse::Reject;
        let remember = *self.remember_choice.lock();

        if let Some(ref callback) = self.on_response {
            callback(HostKeyResponse::Reject, remember);
        }
        cx.notify();
    }

    /// 切换记住选择
    fn toggle_remember(&mut self, cx: &mut Context<Self>) {
        let mut remember = self.remember_choice.lock();
        *remember = !*remember;
        cx.notify();
    }
}

impl Focusable for HostKeyDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HostKeyDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let remember = *self.remember_choice.lock();

        // 对话框容器
        div()
            .id("host-key-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000080))
            .child(
                div()
                    .id("host-key-dialog")
                    .w(px(500.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // 标题
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_2xl().child("⚠"))
                            .child(
                                div()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(theme.foreground)
                                            .child("Unknown Host Key"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(format!(
                                                "The authenticity of host '{}' can't be established.",
                                                self.host_key_info.display_address()
                                            )),
                                    ),
                            ),
                    )
                    // 警告信息
                    .child(
                        div()
                            .p_3()
                            .bg(gpui::rgba(0xf59e0b20))
                            .border_1()
                            .border_color(gpui::rgb(0xf59e0b))
                            .rounded_md()
                            .child(
                                div().text_sm().text_color(theme.foreground).child(
                                    "This is the first time connecting to this host. Please verify the fingerprint.",
                                ),
                            ),
                    )
                    // 密钥信息
                    .child(
                        div()
                            .p_3()
                            .bg(theme.secondary)
                            .rounded_md()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.render_info_row("Host", &self.host_key_info.display_address(), cx))
                            .child(self.render_info_row("Key Type", &self.host_key_info.key_type, cx))
                            .child(self.render_info_row("Fingerprint", &self.host_key_info.fingerprint, cx)),
                    )
                    // 复选框
                    .child(self.render_checkbox(remember, cx))
                    // 按钮
                    .child(self.render_buttons(cx)),
            )
    }
}

impl HostKeyDialog {
    /// 渲染信息行
    fn render_info_row(
        &self,
        label: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .gap_2()
            .child(
                div()
                    .w(px(80.0))
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child(format!("{}:", label)),
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(value.to_string()),
            )
    }

    /// 渲染复选框
    fn render_checkbox(&self, checked: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let checkbox_bg = if checked {
            gpui::rgb(0x3b82f6).into()
        } else {
            theme.background
        };
        let check_mark = if checked { "✓" } else { "" };

        div()
            .id("remember-checkbox")
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.toggle_remember(cx);
            }))
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded_sm()
                    .bg(checkbox_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_xs().text_color(gpui::white()).child(check_mark)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child("Add this host to known_hosts"),
            )
    }

    /// 渲染按钮
    fn render_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("reject-btn")
                    .label("Reject")
                    .with_size(Size::Medium)
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.do_reject(cx);
                    })),
            )
            .child(
                Button::new("accept-btn")
                    .label("Accept")
                    .with_size(Size::Medium)
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.do_accept(cx);
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_key_info_new() {
        let info = HostKeyInfo::new("example.com", 22, "ssh-ed25519", "SHA256:abc123");
        assert_eq!(info.hostname, "example.com");
        assert_eq!(info.port, 22);
        assert_eq!(info.key_type, "ssh-ed25519");
        assert_eq!(info.fingerprint, "SHA256:abc123");
    }

    #[test]
    fn test_host_key_info_display_address() {
        let info1 = HostKeyInfo::new("example.com", 22, "ssh-rsa", "fp");
        assert_eq!(info1.display_address(), "example.com");

        let info2 = HostKeyInfo::new("example.com", 2222, "ssh-rsa", "fp");
        assert_eq!(info2.display_address(), "example.com:2222");
    }

    #[test]
    fn test_host_key_response_default() {
        let response = HostKeyResponse::default();
        assert_eq!(response, HostKeyResponse::Pending);
    }
}
