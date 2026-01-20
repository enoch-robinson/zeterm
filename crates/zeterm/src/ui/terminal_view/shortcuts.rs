//! 快捷键系统模块
//!
//! 提供终端快捷键功能，包括：
//! - 快捷键定义和映射
//! - 默认快捷键配置
//! - 快捷键动作处理
//!
//! # 内置快捷键
//!
//! | 快捷键 | 动作 |
//! |--------|------|
//! | Ctrl+Shift+C | 复制选中内容 |
//! | Ctrl+Shift+V | 粘贴剪贴板 |
//! | Ctrl+Shift+T | 新建标签 |
//! | Ctrl+Shift+W | 关闭标签 |
//! | Ctrl+Tab | 切换标签 |
//! | Ctrl+Shift+F | 搜索 |
//! | Ctrl+Plus | 放大字体 |
//! | Ctrl+Minus | 缩小字体 |
//! | Ctrl+0 | 重置字体大小 |

use std::collections::HashMap;

/// 快捷键动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutAction {
    /// 复制选中内容
    Copy,
    /// 粘贴剪贴板
    Paste,
    /// 新建标签
    NewTab,
    /// 关闭标签
    CloseTab,
    /// 切换到下一个标签
    NextTab,
    /// 切换到上一个标签
    PrevTab,
    /// 打开搜索
    Search,
    /// 放大字体
    ZoomIn,
    /// 缩小字体
    ZoomOut,
    /// 重置字体大小
    ZoomReset,
    /// 清屏
    Clear,
    /// 滚动到顶部
    ScrollToTop,
    /// 滚动到底部
    ScrollToBottom,
    /// 向上滚动一页
    ScrollPageUp,
    /// 向下滚动一页
    ScrollPageDown,
    /// 全选
    SelectAll,
    /// 取消选择
    ClearSelection,
    /// 切换全屏
    ToggleFullscreen,
    /// 打开设置
    OpenSettings,
    /// 断开连接
    Disconnect,
    /// 重新连接
    Reconnect,
    //========== 分屏相关 ==========
    /// 水平分屏（左右）
    SplitHorizontal,
    /// 垂直分屏（上下）
    SplitVertical,
    /// 关闭当前面板
    ClosePane,
    /// 切换到下一个面板
    FocusNextPane,
    /// 切换到上一个面板
    FocusPrevPane,
    /// 切换侧边栏
    ToggleSidebar,
}

impl ShortcutAction {
    /// 获取动作的描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::Copy => "复制选中内容",
            Self::Paste => "粘贴剪贴板",
            Self::NewTab => "新建标签",
            Self::CloseTab => "关闭标签",
            Self::NextTab => "下一个标签",
            Self::PrevTab => "上一个标签",
            Self::Search => "搜索",
            Self::ZoomIn => "放大字体",
            Self::ZoomOut => "缩小字体",
            Self::ZoomReset => "重置字体大小",
            Self::Clear => "清屏",
            Self::ScrollToTop => "滚动到顶部",
            Self::ScrollToBottom => "滚动到底部",
            Self::ScrollPageUp => "向上翻页",
            Self::ScrollPageDown => "向下翻页",
            Self::SelectAll => "全选",
            Self::ClearSelection => "取消选择",
            Self::ToggleFullscreen => "切换全屏",
            Self::OpenSettings => "打开设置",
            Self::Disconnect => "断开连接",
            Self::Reconnect => "重新连接",
            //分屏相关
            Self::SplitHorizontal => "水平分屏",
            Self::SplitVertical => "垂直分屏",
            Self::ClosePane => "关闭面板",
            Self::FocusNextPane => "下一个面板",
            Self::FocusPrevPane => "上一个面板",
            Self::ToggleSidebar => "切换侧边栏",
        }
    }
}

/// 修饰键
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// Ctrl 键
    pub ctrl: bool,
    /// Alt 键
    pub alt: bool,
    /// Shift 键
    pub shift: bool,
    /// Meta/Super/Win 键
    pub meta: bool,
}

impl Modifiers {
    /// 创建新的修饰键组合
    pub fn new(ctrl: bool, alt: bool, shift: bool, meta: bool) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            meta,
        }
    }

    /// 只有 Ctrl
    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    /// Ctrl + Shift
    pub fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            shift: true,
            ..Default::default()
        }
    }

    /// Ctrl + Alt
    pub fn ctrl_alt() -> Self {
        Self {
            ctrl: true,
            alt: true,
            ..Default::default()
        }
    }

    /// 只有 Shift
    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    /// 无修饰键
    pub fn none() -> Self {
        Self::default()
    }

    /// 从GPUI 修饰键创建
    pub fn from_gpui(mods: &gpui::Modifiers) -> Self {
        Self {
            ctrl: mods.control,
            alt: mods.alt,
            shift: mods.shift,
            meta: mods.platform,
        }
    }
}

/// 快捷键定义
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// 按键名称（小写）
    pub key: String,
    /// 修饰键
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// 创建新的快捷键
    pub fn new(key: impl Into<String>, modifiers: Modifiers) -> Self {
        Self {
            key: key.into().to_lowercase(),
            modifiers,
        }
    }

    /// 从按键和修饰键创建
    pub fn from_key(key: &str, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self::new(key, Modifiers::new(ctrl, alt, shift, false))
    }

    /// 格式化为可读字符串
    pub fn to_string_readable(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.meta {
            parts.push("Meta");
        }

        // 格式化按键名称
        let key_display = match self.key.as_str() {
            "+" | "=" => "+",
            "-" => "-",
            "0" => "0",
            _ => &self.key,
        };

        parts.push(key_display);
        parts.join("+")
    }
}

/// 快捷键管理器
#[derive(Debug, Clone)]
pub struct ShortcutManager {
    /// 快捷键映射
    shortcuts: HashMap<Shortcut, ShortcutAction>,
    /// 是否启用
    enabled: bool,
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutManager {
    /// 创建新的快捷键管理器（带默认快捷键）
    pub fn new() -> Self {
        let mut manager = Self {
            shortcuts: HashMap::new(),
            enabled: true,
        };
        manager.register_defaults();
        manager
    }

    /// 创建空的快捷键管理器
    pub fn empty() -> Self {
        Self {
            shortcuts: HashMap::new(),
            enabled: true,
        }
    }

    /// 注册默认快捷键
    fn register_defaults(&mut self) {
        // 复制粘贴
        self.register(
            Shortcut::new("c", Modifiers::ctrl_shift()),
            ShortcutAction::Copy,
        );
        self.register(
            Shortcut::new("v", Modifiers::ctrl_shift()),
            ShortcutAction::Paste,
        );

        // 标签管理
        self.register(
            Shortcut::new("t", Modifiers::ctrl_shift()),
            ShortcutAction::NewTab,
        );
        self.register(
            Shortcut::new("w", Modifiers::ctrl_shift()),
            ShortcutAction::CloseTab,
        );
        self.register(
            Shortcut::new("tab", Modifiers::ctrl()),
            ShortcutAction::NextTab,
        );
        self.register(
            Shortcut::new("tab", Modifiers::ctrl_shift()),
            ShortcutAction::PrevTab,
        );

        // 搜索
        self.register(
            Shortcut::new("f", Modifiers::ctrl_shift()),
            ShortcutAction::Search,
        );

        // 字体缩放
        self.register(
            Shortcut::new("=", Modifiers::ctrl()),
            ShortcutAction::ZoomIn,
        );
        self.register(
            Shortcut::new("+", Modifiers::ctrl()),
            ShortcutAction::ZoomIn,
        );
        self.register(
            Shortcut::new("-", Modifiers::ctrl()),
            ShortcutAction::ZoomOut,
        );
        self.register(
            Shortcut::new("0", Modifiers::ctrl()),
            ShortcutAction::ZoomReset,
        );

        // 滚动
        self.register(
            Shortcut::new("home", Modifiers::ctrl_shift()),
            ShortcutAction::ScrollToTop,
        );
        self.register(
            Shortcut::new("end", Modifiers::ctrl_shift()),
            ShortcutAction::ScrollToBottom,
        );
        self.register(
            Shortcut::new("pageup", Modifiers::shift()),
            ShortcutAction::ScrollPageUp,
        );
        self.register(
            Shortcut::new("pagedown", Modifiers::shift()),
            ShortcutAction::ScrollPageDown,
        );

        // 选择
        self.register(
            Shortcut::new("a", Modifiers::ctrl_shift()),
            ShortcutAction::SelectAll,
        );

        // 清屏
        self.register(
            Shortcut::new("l", Modifiers::ctrl_shift()),
            ShortcutAction::Clear,
        );

        // 分屏管理
        self.register(
            Shortcut::new("\\", Modifiers::ctrl()),
            ShortcutAction::SplitHorizontal,
        );
        self.register(
            Shortcut::new("|", Modifiers::ctrl_shift()),
            ShortcutAction::SplitHorizontal,
        );
        self.register(
            Shortcut::new("-", Modifiers::ctrl_shift()),
            ShortcutAction::SplitVertical,
        );
        self.register(
            Shortcut::new("_", Modifiers::ctrl_shift()),
            ShortcutAction::SplitVertical,
        );
        self.register(
            Shortcut::new("w", Modifiers::ctrl_alt()),
            ShortcutAction::ClosePane,
        );

        // 面板焦点切换
        self.register(
            Shortcut::new("]", Modifiers::ctrl()),
            ShortcutAction::FocusNextPane,
        );
        self.register(
            Shortcut::new("[", Modifiers::ctrl()),
            ShortcutAction::FocusPrevPane,
        );

        // 侧边栏
        self.register(
            Shortcut::new("b", Modifiers::ctrl()),
            ShortcutAction::ToggleSidebar,
        );
    }

    /// 注册快捷键
    pub fn register(&mut self, shortcut: Shortcut, action: ShortcutAction) {
        self.shortcuts.insert(shortcut, action);
    }

    /// 取消注册快捷键
    pub fn unregister(&mut self, shortcut: &Shortcut) {
        self.shortcuts.remove(shortcut);
    }

    /// 查找快捷键对应的动作
    pub fn find_action(&self, shortcut: &Shortcut) -> Option<ShortcutAction> {
        if !self.enabled {
            return None;
        }
        self.shortcuts.get(shortcut).copied()
    }

    /// 从按键事件查找动作
    pub fn find_action_from_key(
        &self,
        key: &str,
        modifiers: &gpui::Modifiers,
    ) -> Option<ShortcutAction> {
        let shortcut = Shortcut::new(key, Modifiers::from_gpui(modifiers));
        self.find_action(&shortcut)
    }

    /// 启用快捷键
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// 禁用快捷键
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 获取所有快捷键
    pub fn all_shortcuts(&self) -> impl Iterator<Item = (&Shortcut, &ShortcutAction)> {
        self.shortcuts.iter()
    }

    /// 获取指定动作的快捷键
    pub fn shortcut_for_action(&self, action: ShortcutAction) -> Option<&Shortcut> {
        self.shortcuts
            .iter()
            .find(|(_, a)| **a == action)
            .map(|(s, _)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_creation() {
        let shortcut = Shortcut::new("c", Modifiers::ctrl_shift());
        assert_eq!(shortcut.key, "c");
        assert!(shortcut.modifiers.ctrl);
        assert!(shortcut.modifiers.shift);
        assert!(!shortcut.modifiers.alt);
    }

    #[test]
    fn test_shortcut_to_string() {
        let shortcut = Shortcut::new("c", Modifiers::ctrl_shift());
        assert_eq!(shortcut.to_string_readable(), "Ctrl+Shift+c");
    }

    #[test]
    fn test_shortcut_manager_defaults() {
        let manager = ShortcutManager::new();

        // 测试复制快捷键
        let copy_shortcut = Shortcut::new("c", Modifiers::ctrl_shift());
        assert_eq!(
            manager.find_action(&copy_shortcut),
            Some(ShortcutAction::Copy)
        );

        // 测试粘贴快捷键
        let paste_shortcut = Shortcut::new("v", Modifiers::ctrl_shift());
        assert_eq!(
            manager.find_action(&paste_shortcut),
            Some(ShortcutAction::Paste)
        );
    }

    #[test]
    fn test_shortcut_manager_disable() {
        let mut manager = ShortcutManager::new();
        let shortcut = Shortcut::new("c", Modifiers::ctrl_shift());

        assert!(manager.find_action(&shortcut).is_some());

        manager.disable();
        assert!(manager.find_action(&shortcut).is_none());

        manager.enable();
        assert!(manager.find_action(&shortcut).is_some());
    }

    #[test]
    fn test_shortcut_action_description() {
        assert_eq!(ShortcutAction::Copy.description(), "复制选中内容");
        assert_eq!(ShortcutAction::Paste.description(), "粘贴剪贴板");
    }
}
