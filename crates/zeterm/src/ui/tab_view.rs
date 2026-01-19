//! Tab视图组件
//!
//! 显示和管理终端标签页。

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, div,
};
use gpui_component::ActiveTheme;

use super::tab_manager::{TabId, TabManager, TabManagerEvent};

/// Tab 视图组件
pub struct TabView {
    /// 焦点句柄
    focus_handle: FocusHandle,

    /// Tab 管理器
    tab_manager: Entity<TabManager>,
}

impl TabView {
    /// 创建新的 Tab 视图
    pub fn new(tab_manager: Entity<TabManager>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_manager,
        }
    }

    /// 获取 Tab 管理器
    pub fn tab_manager(&self) -> &Entity<TabManager> {
        &self.tab_manager
    }
}

impl Focusable for TabView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 基础容器
        div()
            .id("tab-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
    }
}
