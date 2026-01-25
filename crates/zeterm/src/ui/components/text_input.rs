//! 文本输入组件
//!
//! 提供简单的单行文本输入功能。

use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, div,
};
use gpui_component::ActiveTheme;
use parking_lot::Mutex;
use std::sync::Arc;

/// 文本输入组件
pub struct TextInput {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 输入的文本内容
    value: Arc<Mutex<String>>,

    /// 光标位置（字符索引）
    cursor_position: Arc<Mutex<usize>>,

    /// 占位符文本
    placeholder: String,

    /// 是否为密码输入
    is_password: bool,

    /// 是否只读
    is_readonly: bool,

    /// 输入变化回调
    on_change: Option<Arc<dyn Fn(String) + Send + Sync>>,

    /// 按下回车键的回调
    on_submit: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl TextInput {
    /// 创建新的文本输入组件
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            value: Arc::new(Mutex::new(String::new())),
            cursor_position: Arc::new(Mutex::new(0)),
            placeholder: String::new(),
            is_password: false,
            is_readonly: false,
            on_change: None,
            on_submit: None,
        }
    }

    /// 设置初始值
    pub fn with_value(self, value: impl Into<String>) -> Self {
        let value_str = value.into();
        let len = value_str.len();
        *self.value.lock() = value_str;
        *self.cursor_position.lock() = len;
        self
    }

    /// 设置占位符文本
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置为密码输入
    pub fn with_password(mut self, is_password: bool) -> Self {
        self.is_password = is_password;
        self
    }

    /// 设置为只读
    pub fn with_readonly(mut self, is_readonly: bool) -> Self {
        self.is_readonly = is_readonly;
        self
    }

    /// 设置输入变化回调
    pub fn with_on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(callback));
        self
    }

    /// 设置提交回调（按下回车键）
    pub fn with_on_submit<F>(mut self, callback: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        self.on_submit = Some(Arc::new(callback));
        self
    }

    /// 获取当前值
    pub fn value(&self) -> String {
        self.value.lock().clone()
    }

    /// 设置值
    pub fn set_value(&mut self, value: String, cx: &mut Context<Self>) {
        let len = value.len();
        *self.value.lock() = value.clone();
        *self.cursor_position.lock() = len;

        if let Some(ref callback) = self.on_change {
            callback(value);
        }

        cx.notify();
    }

    /// 插入字符
    fn insert_char(&mut self, c: char, cx: &mut Context<Self>) {
        if self.is_readonly {
            return;
        }

        let mut value = self.value.lock();
        let mut cursor_pos = self.cursor_position.lock();

        value.insert(*cursor_pos, c);
        *cursor_pos += 1;

        let new_value = value.clone();
        drop(value);
        drop(cursor_pos);

        if let Some(ref callback) = self.on_change {
            callback(new_value);
        }

        cx.notify();
    }

    /// 删除光标前的字符（退格）
    fn delete_backward(&mut self, cx: &mut Context<Self>) {
        if self.is_readonly {
            return;
        }

        let mut value = self.value.lock();
        let mut cursor_pos = self.cursor_position.lock();

        if *cursor_pos > 0 {
            value.remove(*cursor_pos - 1);
            *cursor_pos -= 1;
            let new_value = value.clone();
            drop(value);
            drop(cursor_pos);

            if let Some(ref callback) = self.on_change {
                callback(new_value);
            }

            cx.notify();
        }
    }

    /// 删除光标后的字符
    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.is_readonly {
            return;
        }

        let mut value = self.value.lock();
        let cursor_pos = *self.cursor_position.lock();

        if cursor_pos < value.len() {
            value.remove(cursor_pos);
            let new_value = value.clone();
            drop(value);

            if let Some(ref callback) = self.on_change {
                callback(new_value);
            }

            cx.notify();
        }
    }

    /// 移动光标到开始
    fn move_to_start(&mut self, cx: &mut Context<Self>) {
        *self.cursor_position.lock() = 0;
        cx.notify();
    }

    /// 移动光标到结束
    fn move_to_end(&mut self, cx: &mut Context<Self>) {
        let len = self.value.lock().len();
        *self.cursor_position.lock() = len;
        cx.notify();
    }

    /// 移动光标向左
    fn move_left(&mut self, cx: &mut Context<Self>) {
        let mut cursor_pos = self.cursor_position.lock();
        if *cursor_pos > 0 {
            *cursor_pos -= 1;
            cx.notify();
        }
    }

    /// 移动光标向右
    fn move_right(&mut self, cx: &mut Context<Self>) {
        let value_len = self.value.lock().len();
        let mut cursor_pos = self.cursor_position.lock();
        if *cursor_pos < value_len {
            *cursor_pos += 1;
            cx.notify();
        }
    }

    /// 处理提交（回车键）
    fn handle_submit(&mut self, _cx: &mut Context<Self>) {
        let value = self.value.lock().clone();
        if let Some(ref callback) = self.on_submit {
            callback(value);
        }
    }

    /// 获取显示的文本（密码模式下显示星号）
    fn display_text(&self) -> String {
        let value = self.value.lock();
        if self.is_password && !value.is_empty() {
            "•".repeat(value.len())
        } else if value.is_empty() {
            self.placeholder.clone()
        } else {
            value.clone()
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let display_text = self.display_text();
        let is_empty = self.value.lock().is_empty();
        let is_focused = self.focus_handle.is_focused(window);

        div()
            .id("text-input")
            .w_full()
            .px_3()
            .py_2()
            .bg(theme.secondary)
            .border_1()
            .border_color(if is_focused {
                gpui::rgb(0x3b82f6).into()
            } else {
                theme.border
            })
            .rounded_md()
            .text_sm()
            .text_color(if is_empty {
                theme.muted_foreground
            } else {
                theme.foreground
            })
            .cursor_text()
            .child(display_text)
    }
}
