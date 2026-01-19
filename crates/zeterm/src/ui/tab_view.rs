//! Tab视图组件
//!
//! 显示和管理终端标签页。

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, div, px,
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

impl TabView {
    ///渲染 Tab 栏
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 读取 Tab 列表并克隆（避免借用冲突）
        let tabs: Vec<_> = self
            .tab_manager
            .read(cx)
            .tabs()
            .iter()
            .map(|t| (*t).clone())
            .collect();

        let mut tab_bar = div()
            .id("tab-bar")
            .flex()
            .items_center()
            .h(px(40.0))
            .px_2()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary);

        // 渲染每个 Tab
        for tab in &tabs {
            tab_bar = tab_bar.child(self.render_tab_item(tab, cx));
        }

        tab_bar
    }

    ///渲染单个 Tab 项
    fn render_tab_item(
        &self,
        tab: &super::tab_manager::TabInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let is_active = tab.is_active;
        let tab_id = tab.id;

        div()
            .id(format!("tab-{}", tab_id))
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .mr_1()
            .rounded_t_md()
            .bg(if is_active {
                theme.background
            } else {
                theme.secondary
            })
            .border_t_2()
            .border_color(if is_active {
                gpui::rgb(0x3b82f6).into()
            } else {
                gpui::rgba(0x00000000)
            })
            .cursor_pointer()
            .hover(|style| {
                if !is_active {
                    style.bg(theme.border)
                } else {
                    style
                }
            })
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(tab.title.clone()),
            )
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
            .child(self.render_tab_bar(cx))
    }
}
