//! Tab 管理器
//!
//! 管理终端标签页的创建、切换和关闭。
//! 每个 Tab 拥有独立的 SplitManager，实现独立分屏布局。

use gpui::{Context, Entity, EventEmitter};
use std::collections::HashMap;
use uuid::Uuid;
use zeterm_core::entities::{HostConfig, SessionId};

use super::split_pane::SplitManager;

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

    /// 是否已修改（用于显示未保存指示器）
    pub is_modified: bool,

    /// Tab 图标（可选）
    pub icon: Option<String>,
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
            is_modified: false,
            icon: None,
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
            is_modified: false,
            icon: Some("🖥".to_string()),
        }
    }

    /// 创建本地终端的 Tab
    pub fn new_local(title: impl Into<String>) -> Self {
        Self {
            id: TabId::new(),
            title: title.into(),
            host_config: None,
            session_id: None,
            is_active: false,
            is_modified: false,
            icon: Some("💻".to_string()),
        }
    }

    /// 设置会话 ID
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// 设置图标
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置为活动状态
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    /// 设置为已修改状态
    pub fn set_modified(&mut self, modified: bool) {
        self.is_modified = modified;
    }
}

/// Tab 数据（包含 TabInfo 和独立的 SplitManager）
pub struct TabData {
    /// Tab 信息
    pub info: TabInfo,

    /// 独立的分屏管理器
    pub split_manager: Entity<SplitManager>,
}

impl TabData {
    /// 创建新的 Tab 数据
    pub fn new(info: TabInfo, split_manager: Entity<SplitManager>) -> Self {
        Self {
            info,
            split_manager,
        }
    }
}

/// Tab 管理器
pub struct TabManager {
    /// 所有 Tab（TabId -> TabData）
    tabs: HashMap<TabId, TabData>,

    /// Tab 顺序（用于显示）
    tab_order: Vec<TabId>,

    /// 当前活动的 Tab ID
    active_tab_id: Option<TabId>,

    /// 最大 Tab 数量（0 表示无限制）
    max_tabs: usize,
}

impl TabManager {
    /// 创建新的 Tab 管理器
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
            max_tabs: 0, // 无限制
        }
    }

    /// 创建带最大 Tab 数量限制的管理器
    pub fn with_max_tabs(max_tabs: usize) -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
            max_tabs,
        }
    }

    /// 添加新 Tab（带独立的 SplitManager）
    pub fn add_tab(
        &mut self,
        mut tab_info: TabInfo,
        split_manager: Entity<SplitManager>,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        // 检查是否达到最大 Tab 数量
        if self.max_tabs > 0 && self.tabs.len() >= self.max_tabs {
            tracing::warn!("Cannot add tab: max tabs limit reached ({})", self.max_tabs);
            return None;
        }

        let tab_id = tab_info.id;

        // 如果是第一个 Tab，自动激活
        if self.tabs.is_empty() {
            tab_info.is_active = true;
            self.active_tab_id = Some(tab_id);
        }

        let tab_data = TabData::new(tab_info.clone(), split_manager);

        self.tabs.insert(tab_id, tab_data);
        self.tab_order.push(tab_id);

        tracing::info!("Added tab: {} ({})", tab_info.title, tab_id);
        cx.emit(TabManagerEvent::TabAdded(tab_id, tab_info));
        cx.notify();

        Some(tab_id)
    }

    /// 关闭 Tab
    pub fn close_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) -> bool {
        if let Some(tab_data) = self.tabs.remove(&tab_id) {
            self.tab_order.retain(|id| *id != tab_id);

            // 如果关闭的是活动 Tab，激活相邻的 Tab
            if self.active_tab_id == Some(tab_id) {
                self.active_tab_id = self.find_adjacent_tab(tab_id);

                if let Some(next_id) = self.active_tab_id {
                    if let Some(next_tab) = self.tabs.get_mut(&next_id) {
                        next_tab.info.is_active = true;
                        cx.emit(TabManagerEvent::TabSwitched(next_id));
                    }
                }
            }

            tracing::info!("Closed tab: {} ({})", tab_data.info.title, tab_id);
            cx.emit(TabManagerEvent::TabClosed(tab_id));
            cx.notify();

            true
        } else {
            false
        }
    }

    /// 关闭所有 Tab
    pub fn close_all_tabs(&mut self, cx: &mut Context<Self>) {
        let tab_ids: Vec<TabId> = self.tab_order.clone();
        for tab_id in tab_ids {
            self.close_tab(tab_id, cx);
        }
    }

    /// 关闭除指定 Tab 外的所有 Tab
    pub fn close_other_tabs(&mut self, keep_tab_id: TabId, cx: &mut Context<Self>) {
        let tab_ids: Vec<TabId> = self
            .tab_order
            .iter()
            .filter(|&&id| id != keep_tab_id)
            .copied()
            .collect();

        for tab_id in tab_ids {
            self.close_tab(tab_id, cx);
        }
    }

    /// 关闭右侧所有 Tab
    pub fn close_tabs_to_right(&mut self, tab_id: TabId, cx: &mut Context<Self>) {
        if let Some(index) = self.tab_order.iter().position(|&id| id == tab_id) {
            let tab_ids: Vec<TabId> = self.tab_order[index + 1..].to_vec();
            for id in tab_ids {
                self.close_tab(id, cx);
            }
        }
    }

    /// 关闭左侧所有 Tab
    pub fn close_tabs_to_left(&mut self, tab_id: TabId, cx: &mut Context<Self>) {
        if let Some(index) = self.tab_order.iter().position(|&id| id == tab_id) {
            let tab_ids: Vec<TabId> = self.tab_order[..index].to_vec();
            for id in tab_ids {
                self.close_tab(id, cx);
            }
        }
    }

    /// 查找相邻的 Tab（优先右侧，其次左侧）
    fn find_adjacent_tab(&self, closed_tab_id: TabId) -> Option<TabId> {
        if let Some(index) = self.tab_order.iter().position(|&id| id == closed_tab_id) {
            // 优先选择右侧的 Tab
            if index + 1 < self.tab_order.len() {
                return Some(self.tab_order[index + 1]);
            }
            // 其次选择左侧的 Tab
            if index > 0 {
                return Some(self.tab_order[index - 1]);
            }
        }
        // 回退到第一个 Tab
        self.tab_order.first().copied()
    }

    /// 切换到指定 Tab
    pub fn switch_to_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) -> bool {
        if !self.tabs.contains_key(&tab_id) {
            return false;
        }

        // 如果已经是活动 Tab，不需要切换
        if self.active_tab_id == Some(tab_id) {
            return true;
        }

        // 取消当前活动 Tab
        if let Some(current_id) = self.active_tab_id {
            if let Some(current_tab) = self.tabs.get_mut(&current_id) {
                current_tab.info.is_active = false;
            }
        }

        // 激活新 Tab
        if let Some(new_tab) = self.tabs.get_mut(&tab_id) {
            new_tab.info.is_active = true;
            self.active_tab_id = Some(tab_id);

            tracing::info!("Switched to tab: {} ({})", new_tab.info.title, tab_id);
            cx.emit(TabManagerEvent::TabSwitched(tab_id));
            cx.notify();

            true
        } else {
            false
        }
    }

    /// 切换到下一个 Tab
    pub fn switch_to_next_tab(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(current_id) = self.active_tab_id {
            if let Some(index) = self.tab_order.iter().position(|&id| id == current_id) {
                let next_index = (index + 1) % self.tab_order.len();
                let next_id = self.tab_order[next_index];
                return self.switch_to_tab(next_id, cx);
            }
        }
        false
    }

    /// 切换到上一个 Tab
    pub fn switch_to_prev_tab(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(current_id) = self.active_tab_id {
            if let Some(index) = self.tab_order.iter().position(|&id| id == current_id) {
                let prev_index = if index == 0 {
                    self.tab_order.len() - 1
                } else {
                    index - 1
                };
                let prev_id = self.tab_order[prev_index];
                return self.switch_to_tab(prev_id, cx);
            }
        }
        false
    }

    /// 切换到指定索引的 Tab（0-based）
    pub fn switch_to_tab_at_index(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if index < self.tab_order.len() {
            let tab_id = self.tab_order[index];
            self.switch_to_tab(tab_id, cx)
        } else {
            false
        }
    }

    /// 移动 Tab 到新位置
    pub fn move_tab(&mut self, tab_id: TabId, new_index: usize, cx: &mut Context<Self>) -> bool {
        if let Some(current_index) = self.tab_order.iter().position(|&id| id == tab_id) {
            let new_index = new_index.min(self.tab_order.len() - 1);
            if current_index != new_index {
                self.tab_order.remove(current_index);
                self.tab_order.insert(new_index, tab_id);

                tracing::info!(
                    "Moved tab {} from {} to {}",
                    tab_id,
                    current_index,
                    new_index
                );
                cx.emit(TabManagerEvent::TabMoved(tab_id, new_index));
                cx.notify();

                return true;
            }
        }
        false
    }

    /// 更新 Tab 标题
    pub fn update_tab_title(
        &mut self,
        tab_id: TabId,
        title: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(tab_data) = self.tabs.get_mut(&tab_id) {
            tab_data.info.title = title.into();
            cx.emit(TabManagerEvent::TabUpdated(tab_id));
            cx.notify();
            true
        } else {
            false
        }
    }

    /// 设置 Tab 的已修改状态
    pub fn set_tab_modified(
        &mut self,
        tab_id: TabId,
        modified: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(tab_data) = self.tabs.get_mut(&tab_id) {
            tab_data.info.is_modified = modified;
            cx.emit(TabManagerEvent::TabUpdated(tab_id));
            cx.notify();
            true
        } else {
            false
        }
    }

    /// 获取所有 Tab 信息（按顺序）
    pub fn tabs(&self) -> Vec<&TabInfo> {
        self.tab_order
            .iter()
            .filter_map(|id| self.tabs.get(id).map(|d| &d.info))
            .collect()
    }

    /// 获取所有 Tab 数据（按顺序）
    pub fn tab_data_list(&self) -> Vec<&TabData> {
        self.tab_order
            .iter()
            .filter_map(|id| self.tabs.get(id))
            .collect()
    }

    /// 获取指定 Tab 信息
    pub fn get_tab(&self, tab_id: TabId) -> Option<&TabInfo> {
        self.tabs.get(&tab_id).map(|d| &d.info)
    }

    /// 获取指定 Tab 数据
    pub fn get_tab_data(&self, tab_id: TabId) -> Option<&TabData> {
        self.tabs.get(&tab_id)
    }

    /// 获取指定 Tab 的 SplitManager
    pub fn get_split_manager(&self, tab_id: TabId) -> Option<&Entity<SplitManager>> {
        self.tabs.get(&tab_id).map(|d| &d.split_manager)
    }

    /// 获取活动 Tab 信息
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.active_tab_id
            .and_then(|id| self.tabs.get(&id).map(|d| &d.info))
    }

    /// 获取活动 Tab 数据
    pub fn active_tab_data(&self) -> Option<&TabData> {
        self.active_tab_id.and_then(|id| self.tabs.get(&id))
    }

    /// 获取活动 Tab 的 SplitManager
    pub fn active_split_manager(&self) -> Option<&Entity<SplitManager>> {
        self.active_tab_id
            .and_then(|id| self.tabs.get(&id).map(|d| &d.split_manager))
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

    /// 获取 Tab 的索引
    pub fn get_tab_index(&self, tab_id: TabId) -> Option<usize> {
        self.tab_order.iter().position(|&id| id == tab_id)
    }

    /// 检查 Tab 是否存在
    pub fn contains_tab(&self, tab_id: TabId) -> bool {
        self.tabs.contains_key(&tab_id)
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
    /// Tab 已添加（包含 Tab ID 和 Info）
    TabAdded(TabId, TabInfo),

    /// Tab 已关闭
    TabClosed(TabId),

    /// Tab 已切换（新的活动 Tab ID）
    TabSwitched(TabId),

    /// Tab 已更新（标题或状态变化）
    TabUpdated(TabId),

    /// Tab 已移动（Tab ID 和新索引）
    TabMoved(TabId, usize),
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
    fn test_tab_id_default() {
        let id1 = TabId::default();
        let id2 = TabId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tab_info_creation() {
        let tab = TabInfo::new("Test Tab");
        assert_eq!(tab.title, "Test Tab");
        assert!(tab.host_config.is_none());
        assert!(tab.session_id.is_none());
        assert!(!tab.is_active);
        assert!(!tab.is_modified);
    }

    #[test]
    fn test_tab_info_ssh() {
        let host_config = HostConfig::new(
            "Test Host".to_string(),
            "192.168.1.1".to_string(),
            "root".to_string(),
            zeterm_core::entities::AuthConfig::Password {
                password_ref: "plain:test".to_string(),
            },
        );

        let tab = TabInfo::new_ssh(host_config.clone());
        assert_eq!(tab.title, "root@192.168.1.1");
        assert!(tab.host_config.is_some());
        assert_eq!(tab.icon, Some("🖥".to_string()));
    }

    #[test]
    fn test_tab_info_local() {
        let tab = TabInfo::new_local("Local Terminal");
        assert_eq!(tab.title, "Local Terminal");
        assert!(tab.host_config.is_none());
        assert_eq!(tab.icon, Some("💻".to_string()));
    }

    #[test]
    fn test_tab_info_with_icon() {
        let tab = TabInfo::new("Test").with_icon("📁");
        assert_eq!(tab.icon, Some("📁".to_string()));
    }

    #[test]
    fn test_tab_info_set_active() {
        let mut tab = TabInfo::new("Test");
        assert!(!tab.is_active);
        tab.set_active(true);
        assert!(tab.is_active);
    }

    #[test]
    fn test_tab_info_set_modified() {
        let mut tab = TabInfo::new("Test");
        assert!(!tab.is_modified);
        tab.set_modified(true);
        assert!(tab.is_modified);
    }

    #[test]
    fn test_tab_manager_creation() {
        let manager = TabManager::new();
        assert_eq!(manager.tab_count(), 0);
        assert!(!manager.has_tabs());
        assert!(manager.active_tab_id().is_none());
    }

    #[test]
    fn test_tab_manager_with_max_tabs() {
        let manager = TabManager::with_max_tabs(5);
        assert_eq!(manager.max_tabs, 5);
    }
}
