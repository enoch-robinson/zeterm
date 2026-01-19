//! 分屏视图组件
//!
//! 渲染和管理分屏布局的 UI 组件。

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;

use super::{Pane, PaneContent, PaneId, SplitDirection, SplitManager, SplitManagerEvent};

/// 分屏视图组件
pub struct SplitView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 分屏管理器
    split_manager: Entity<SplitManager>,
}

impl SplitView {
    /// 创建新的分屏视图
    pub fn new(split_manager: Entity<SplitManager>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            split_manager,
        }
    }

    /// 获取分屏管理器
    pub fn split_manager(&self) -> &Entity<SplitManager> {
        &self.split_manager
    }

    /// 渲染单个面板
    fn render_pane(
        &self,
        pane: &Pane,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let pane_id = pane.id;
        let split_manager = self.split_manager.clone();

        match &pane.content {
            PaneContent::Terminal { title, focused, .. } => {
                // 终端面板容器
                div()
                    .id(format!("pane-{}", pane_id))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .bg(theme.background)
                    .border_1()
                    .border_color(if *focused {
                        gpui::rgb(0x3b82f6).into()
                    } else {
                        theme.border
                    })
                    .relative()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                        // 点击切换焦点
                        split_manager.update(cx, |manager, cx| {
                            manager.focus_pane(pane_id, cx);
                        });
                    }))
                    .child(
                        // 面板标题栏
                        div()
                            .h(px(24.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .bg(theme.secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(title.clone()),
                            ),
                    )
                    .child(
                        // 终端内容区域（占位符，实际内容由 MainWindow 提供的 TerminalView 填充）
                        div()
                            .id(format!("pane-content-{}", pane_id))
                            .flex_1()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.muted_foreground)
                                    .child("Terminal View"),
                            ),
                    )
            }
            PaneContent::Split {
                direction,
                first,
                second,
                ratio,
            } => {
                // 分屏容器
                let is_horizontal = matches!(direction, SplitDirection::Horizontal);
                let first_ratio = *ratio;
                let second_ratio = 1.0 - first_ratio;

                div()
                    .id(format!("split-{}", pane_id))
                    .flex()
                    .flex_col()
                    .if_true(!is_horizontal, |div| div.flex_row())
                    .flex_1()
                    .w_full()
                    .h_full()
                    // 第一个面板
                    .child({
                        let first_child = self.render_pane(first, _window, cx);
                        div()
                            .flex_1()
                            .if_true(is_horizontal, |div| div.h(px(0.0)))
                            .if_true(!is_horizontal, |div| div.w(px(0.0)))
                            .when_some(if is_horizontal {
                                Some(first_ratio as f32)
                            } else {
                                None
                            }, |div, ratio| div.h(px(0.0)).flex_grow(ratio))
                            .when_some(if !is_horizontal {
                                Some(first_ratio as f32)
                            } else {
                                None
                            }, |div, ratio| div.w(px(0.0)).flex_grow(ratio))
                            .child(first_child)
                    })
                    // 分隔条
                    .child(self.render_separator(*direction, pane_id, cx))
                    // 第二个面板
                    .child({
                        let second_child = self.render_pane(second, _window, cx);
                        div()
                            .flex_1()
                            .when_some(if is_horizontal {
                                Some(second_ratio as f32)
                            } else {
                                None
                            }, |div, ratio| div.h(px(0.0)).flex_grow(ratio))
                            .when_some(if !is_horizontal {
                                Some(second_ratio as f32)
                            } else {
                                None
                            }, |div, ratio| div.w(px(0.0)).flex_grow(ratio))
                            .child(second_child)
                    })
            }
        }
    }

    /// 渲染分隔条
    fn render_separator(
        &self,
        direction: SplitDirection,
        split_id: PaneId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let split_manager = self.split_manager.clone();

        let (width, height, cursor_class) = match direction {
            SplitDirection::Horizontal => (px(1.0), px(0.0), "col-resize"),
            SplitDirection::Vertical => (px(0.0), px(1.0), "row-resize"),
        };

        div()
            .id(format!("separator-{}", split_id))
            .bg(theme.border)
            .when(width != px(0.0), |div| div.w(width))
            .when(height != px(0.0), |div| div.h(height))
            .hover(|style| style.bg(gpui::rgb(0x3b82f6).into()))
            .cursor_style(cursor_class)
            .z_index(1)
    }
}

impl Focusable for SplitView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SplitView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 获取根面板
        let root_pane = self.split_manager.read(cx).root().cloned();

        // 基础容器
        div()
            .id("split-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .when_some(root_pane, |div, pane| {
                div.child(self.render_pane(&pane, _window, cx))
            })
            .when(root_pane.is_none(), |div| {
                // 没有面板时显示提示
                div.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child("暂无终端，请从左侧主机列表选择主机连接"),
                        ),
                )
            })
    }
}
