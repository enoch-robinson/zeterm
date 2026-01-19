    /// 渲染主内容区域（左侧主机列表 + 右侧终端/欢迎界面）
    fn render_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 提前克隆 theme 值以避免借用冲突
        let theme = cx.theme();
        let secondary = theme.secondary;
        let border = theme.border;
        let is_connected = self.coordinator.is_connected();

        // 获取当前活动 Tab 的终端视图
        let active_terminal_view = self
            .tab_manager
            .read(cx)
            .active_tab_id()
            .and_then(|tab_id| self.terminal_views.get(&tab_id).cloned());

        // 右侧主区域容器（包含 Tab 栏和终端/欢迎界面）
        let mut main_area = div().id("main-area").flex_1().flex().flex_col();

        // 渲染 Tab 栏（如果有 Tab）
        if self.tab_manager.read(cx).has_tabs() {
            main_area = main_area.child(self.tab_view.clone());
        }

        // 终端或欢迎界面
        let content_area = if is_connected {
            // 使用活动 Tab 的终端视图
            if let Some(terminal_view) = active_terminal_view {
                div()
                    .id("terminal-panel")
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("terminal-container")
                            .flex_1()
                            .w_full()
                            .child(terminal_view),
                    )
                    .into_any_element()
            } else {
                // 回退到旧的 terminal_view（兼容性）
                if self.terminal_view.is_none() {
                    let coordinator = self.coordinator.clone();
                    self.terminal_view = Some(cx.new(|cx| TerminalView::new(coordinator, cx)));
                    info!("TerminalView created (fallback)");
                }

                div()
                    .id("terminal-panel")
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("terminal-container")
                            .flex_1()
                            .w_full()
                            .child(self.terminal_view.clone().unwrap()),
                    )
                    .into_any_element()
            }
        } else {
            // 未连接时显示欢迎界面
            div()
                .id("welcome-panel")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(self.render_welcome())
                .into_any_element()
        };

        main_area = main_area.child(content_area);

        // 构建主内容区域（水平布局：左侧边栏 + 右侧主区域），添加键盘事件处理
        div()
            .id("content")
            .flex_1()
            .w_full()
            .flex()
            .flex_row()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                match event.keystroke.key.as_str() {
                    "ctrl-h" => {
                        // 水平分屏
                        this.split_horizontal(cx);
                    },
                    "ctrl-v" => {
                        // 垂直分屏
                        this.split_vertical(cx);
                    },
                    "ctrl-w" => {
                        // 关闭面板
                        this.close_pane(cx);
                    },
                    "ctrl-s" => {
                        // 切换侧边栏
                        this.toggle_sidebar(cx);
                    },
                    _ => {},
                }
            }))
            .child({
                // 左侧面板：主机列表
                if self.show_sidebar {
                    if let Some(ref host_list_view) = self.host_list_view {
                        div()
                            .id("sidebar")
                            .w(px(280.0))
                            .h_full()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(border)
                            .bg(secondary)
                            .child(host_list_view.clone())
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }
                } else {
                    div().into_any_element()
                }
            })
            .child(main_area)
            .child(self.split_view.clone())
    }
