//! Tab 管理器
//!
//! 管理终端标签页的创建、切换和关闭。

use gpui::{Context, EventEmitter};
use std::collections::HashMap;
use uuid::Uuid;
use zeterm_core::entities::{HostConfig, SessionId};

/// Tab ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(Uuid);

impl TabId {
    /// 创建新的 Tab ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tab 信息
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Tab ID
    pub id: TabId,

    /// Tab 标题
    pub title: String,

    /// 主机配置（如果是 SSH 连接）
    pub host_config: Option<HostConfig>,

    /// 会话 ID（如果已连接）
    pub session_id: Option<SessionId>,

    /// 是否为活动 Tab
    pub is_active: bool,
}

impl TabInfo {
    /// 创建新的 Tab
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: TabId::new(),
            title: title.into(),
            host_config: None,
            session_id: None,
            is_active: false,
        }
    }

    /// 创建 SSH 连接的 Tab
    pub fn new_ssh(host_config: HostConfig) -> Self {
        let title = format!("{}@{}", host_config.username, host_config.host);
        Self {
            id: TabId::new(),
            title,
            host_config: Some(host_config),
            session_id: None,
            is_active: false,
        }
    }

    /// 设置会话 ID
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Tab 管理器
pub struct TabManager {
    /// 所有 Tab
    tabs: HashMap<TabId, TabInfo>,

    /// Tab顺序（用于显示）
    tab_order: Vec<TabId>,

    /// 当前活动的 Tab ID
    active_tab_id: Option<TabId>,
}

impl TabManager {
    /// 创建新的 Tab 管理器
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
        }
    }

    /// 添加新 Tab
    pub fn add_tab(&mut self, mut tab: TabInfo, cx: &mut Context<Self>) -> TabId {
        let tab_id = tab.id;

        // 如果是第一个 Tab，自动激活
        if self.tabs.is_empty() {
            tab.is_active = true;
            self.active_tab_id = Some(tab_id);
        }

        self.tabs.insert(tab_id, tab.clone());
        self.tab_order.push(tab_id);

        tracing::info!("Added tab: {} ({})", tab.title, tab_id);
        cx.emit(TabManagerEvent::TabAdded(tab));
        cx.notify();

        tab_id
    }

    /// 关闭 Tab
    pub fn close_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) -> bool {
        if let Some(tab) = self.tabs.remove(&tab_id) {
            self.tab_order.retain(|id| *id != tab_id);

            // 如果关闭的是活动 Tab，激活下一个
            if self.active_tab_id == Some(tab_id) {
                self.active_tab_id = self.tab_order.first().copied();

                if let Some(next_id) = self.active_tab_id {
                    if let Some(next_tab) = self.tabs.get_mut(&next_id) {
                        next_tab.is_active = true;
                    }
                }
            }

            tracing::info!("Closed tab: {} ({})", tab.title, tab_id);
            cx.emit(TabManagerEvent::TabClosed(tab_id));
            cx.notify();

            true
        } else {
            false
        }
    }

    /// 切换到指定 Tab
    pub fn switch_to_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) -> bool {
        if !self.tabs.contains_key(&tab_id) {
            return false;
        }

        // 取消当前活动 Tab
        if let Some(current_id) = self.active_tab_id {
            if let Some(current_tab) = self.tabs.get_mut(&current_id) {
                current_tab.is_active = false;
            }
        }

        // 激活新 Tab
        if let Some(new_tab) = self.tabs.get_mut(&tab_id) {
            new_tab.is_active = true;
            self.active_tab_id = Some(tab_id);

            tracing::info!("Switched to tab: {} ({})", new_tab.title, tab_id);
            cx.emit(TabManagerEvent::TabSwitched(tab_id));
            cx.notify();

            true
        } else {
            false
        }
    }

    /// 获取所有 Tab（按顺序）
    pub fn tabs(&self) -> Vec<&TabInfo> {
        self.tab_order
            .iter()
            .filter_map(|id| self.tabs.get(id))
            .collect()
    }

    /// 获取活动 Tab
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.active_tab_id.and_then(|id| self.tabs.get(&id))
    }

    /// 获取活动 Tab ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// 获取 Tab 数量
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 是否有 Tab
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Tab 管理器事件
#[derive(Clone, Debug)]
pub enum TabManagerEvent {
    /// Tab 已添加
    TabAdded(TabInfo),
    /// Tab 已关闭
    TabClosed(TabId),

    /// Tab 已切换
    TabSwitched(TabId),
}

impl EventEmitter<TabManagerEvent> for TabManager {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_tab_id_creation() {
        let id1 = TabId::new();
        let id2 = TabId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tab_info_creation() {
        let tab = TabInfo::new("Test Tab");
        assert_eq!(tab.title, "Test Tab");
        assert!(tab.host_config.is_none());
        assert!(tab.session_id.is_none());
        assert!(!tab.is_active);
    }
}
