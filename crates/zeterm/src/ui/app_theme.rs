//! 应用主题管理模块
//!
//! 管理应用程序的全局主题，包括：
//! - 深色/浅色模式切换
//! - 内置主题选择（Dracula, One Dark, Solarized 等）
//! - 主题持久化
//! - 与 gpui-component Theme 集成

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::ui::terminal_view::theme::{TerminalTheme, ThemeManager as TerminalThemeManager};

/// 主题模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// 浅色模式
    Light,
    /// 深色模式
    #[default]
    Dark,
    /// 跟随系统
    System,
}

impl ThemeMode {
    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
            ThemeMode::System => "System",
        }
    }

    /// 是否为深色模式
    pub fn is_dark(&self) -> bool {
        match self {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => detect_system_dark_mode(),
        }
    }
}

/// 检测系统是否使用深色模式
///
/// 支持以下平台：
/// - Windows: 读取 AppsUseLightTheme 注册表值
/// - macOS: 检查 AppleInterfaceStyle
/// - Linux: 检查 GTK 主题或 color-scheme portal
///
/// 如果检测失败，默认返回深色模式
pub fn detect_system_dark_mode() -> bool {
    #[cfg(target_os = "windows")]
    {
        detect_windows_dark_mode()
    }

    #[cfg(target_os = "macos")]
    {
        detect_macos_dark_mode()
    }

    #[cfg(target_os = "linux")]
    {
        detect_linux_dark_mode()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        true // 默认深色模式
    }
}

/// Windows 深色模式检测
#[cfg(target_os = "windows")]
fn detect_windows_dark_mode() -> bool {
    use std::process::Command;

    // 尝试通过 reg query 读取注册表
    let output = Command::new("reg")
        .args([
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // AppsUseLightTheme = 0 表示深色模式，= 1 表示浅色模式
            if stdout.contains("0x0") {
                tracing::debug!("Windows system theme detected: dark mode");
                true
            } else if stdout.contains("0x1") {
                tracing::debug!("Windows system theme detected: light mode");
                false
            } else {
                tracing::debug!("Windows theme detection: unable to parse, defaulting to dark");
                true
            }
        },
        Err(e) => {
            tracing::warn!("Failed to detect Windows theme: {}, defaulting to dark", e);
            true
        },
    }
}

/// macOS 深色模式检测
#[cfg(target_os = "macos")]
fn detect_macos_dark_mode() -> bool {
    use std::process::Command;

    // 使用 defaults read 检查 AppleInterfaceStyle
    let output = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().eq_ignore_ascii_case("dark") {
                tracing::debug!("macOS system theme detected: dark mode");
                true
            } else {
                tracing::debug!("macOS system theme detected: light mode");
                false
            }
        },
        Err(_) => {
            // 如果读取失败，通常意味着使用浅色模式（默认没有设置 AppleInterfaceStyle）
            tracing::debug!("macOS theme: AppleInterfaceStyle not set, assuming light mode");
            false
        },
    }
}

/// Linux 深色模式检测
#[cfg(target_os = "linux")]
fn detect_linux_dark_mode() -> bool {
    use std::process::Command;

    // 方法 1: 尝试使用 freedesktop portal (适用于 GNOME, KDE 等)
    let portal_result = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest=org.freedesktop.portal.Desktop",
            "--object-path=/org/freedesktop/portal/desktop",
            "--method=org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output();

    if let Ok(output) = portal_result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // color-scheme: 0 = 无偏好, 1 = 深色, 2 = 浅色
        if stdout.contains("uint32 1") {
            tracing::debug!("Linux system theme detected via portal: dark mode");
            return true;
        } else if stdout.contains("uint32 2") {
            tracing::debug!("Linux system theme detected via portal: light mode");
            return false;
        }
    }

    // 方法 2: 检查 GTK 主题名称
    if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
        let is_dark = gtk_theme.to_lowercase().contains("dark");
        tracing::debug!("Linux GTK_THEME={}, dark mode: {}", gtk_theme, is_dark);
        return is_dark;
    }

    // 方法 3: 尝试通过 gsettings 读取 GNOME 主题
    let gsettings_result = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output();

    if let Ok(output) = gsettings_result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("prefer-dark") {
            tracing::debug!("Linux system theme detected via gsettings: dark mode");
            return true;
        } else if stdout.contains("prefer-light") || stdout.contains("default") {
            tracing::debug!("Linux system theme detected via gsettings: light mode");
            return false;
        }
    }

    // 默认深色模式
    tracing::debug!("Linux theme detection: all methods failed, defaulting to dark");
    true
}

/// 内置主题名称
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinTheme {
    /// 默认深色主题
    #[default]
    Dark,
    /// 默认浅色主题
    Light,
    /// Dracula 主题
    Dracula,
    /// One Dark 主题
    OneDark,
    /// Solarized Dark 主题
    SolarizedDark,
    /// Solarized Light 主题
    SolarizedLight,
    /// Nord 主题
    Nord,
    /// Monokai 主题
    Monokai,
    /// Gruvbox Dark 主题
    GruvboxDark,
    /// Tokyo Night 主题
    TokyoNight,
}

impl BuiltinTheme {
    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            BuiltinTheme::Dark => "Dark",
            BuiltinTheme::Light => "Light",
            BuiltinTheme::Dracula => "Dracula",
            BuiltinTheme::OneDark => "One Dark",
            BuiltinTheme::SolarizedDark => "Solarized Dark",
            BuiltinTheme::SolarizedLight => "Solarized Light",
            BuiltinTheme::Nord => "Nord",
            BuiltinTheme::Monokai => "Monokai",
            BuiltinTheme::GruvboxDark => "Gruvbox Dark",
            BuiltinTheme::TokyoNight => "Tokyo Night",
        }
    }

    /// 是否为深色主题
    pub fn is_dark(&self) -> bool {
        match self {
            BuiltinTheme::Light | BuiltinTheme::SolarizedLight => false,
            _ => true,
        }
    }

    /// 获取所有内置主题
    pub fn all() -> &'static [BuiltinTheme] {
        &[
            BuiltinTheme::Dark,
            BuiltinTheme::Light,
            BuiltinTheme::Dracula,
            BuiltinTheme::OneDark,
            BuiltinTheme::SolarizedDark,
            BuiltinTheme::SolarizedLight,
            BuiltinTheme::Nord,
            BuiltinTheme::Monokai,
            BuiltinTheme::GruvboxDark,
            BuiltinTheme::TokyoNight,
        ]
    }

    /// 获取所有深色主题
    pub fn dark_themes() -> Vec<BuiltinTheme> {
        Self::all()
            .iter()
            .filter(|t| t.is_dark())
            .copied()
            .collect()
    }

    /// 获取所有浅色主题
    pub fn light_themes() -> Vec<BuiltinTheme> {
        Self::all()
            .iter()
            .filter(|t| !t.is_dark())
            .copied()
            .collect()
    }

    /// 转换为终端主题
    pub fn to_terminal_theme(&self) -> TerminalTheme {
        match self {
            BuiltinTheme::Dark => TerminalTheme::dark(),
            BuiltinTheme::Light => TerminalTheme::light(),
            BuiltinTheme::Dracula => TerminalTheme::dracula(),
            BuiltinTheme::OneDark => TerminalTheme::one_dark(),
            BuiltinTheme::SolarizedDark => TerminalTheme::solarized_dark(),
            BuiltinTheme::SolarizedLight => create_solarized_light(),
            BuiltinTheme::Nord => create_nord_theme(),
            BuiltinTheme::Monokai => create_monokai_theme(),
            BuiltinTheme::GruvboxDark => create_gruvbox_dark_theme(),
            BuiltinTheme::TokyoNight => create_tokyo_night_theme(),
        }
    }
}

/// 主题配置（持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// 主题模式
    #[serde(default)]
    pub mode: ThemeMode,
    /// 当前主题名称
    #[serde(default)]
    pub theme: BuiltinTheme,
    /// 自定义主题路径（可选）
    pub custom_theme_path: Option<PathBuf>,
    /// 终端透明度 (0.0 - 1.0)
    #[serde(default = "default_opacity")]
    pub terminal_opacity: f32,
    /// 是否使用系统字体
    #[serde(default)]
    pub use_system_font: bool,
}

fn default_opacity() -> f32 {
    1.0
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            theme: BuiltinTheme::Dark,
            custom_theme_path: None,
            terminal_opacity: 1.0,
            use_system_font: false,
        }
    }
}

impl ThemeConfig {
    /// 从 TOML 字符串加载
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// 转换为 TOML 字符串
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// 从文件加载
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_toml(&content)?)
    }

    /// 保存到文件
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = self.to_toml()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// 主题变更事件
#[derive(Debug, Clone)]
pub enum ThemeEvent {
    /// 主题已变更
    ThemeChanged {
        old_theme: BuiltinTheme,
        new_theme: BuiltinTheme,
    },
    /// 模式已变更
    ModeChanged {
        old_mode: ThemeMode,
        new_mode: ThemeMode,
    },
    /// 透明度已变更
    OpacityChanged { opacity: f32 },
}

/// 主题变更回调
pub type ThemeChangeCallback = Box<dyn Fn(&ThemeEvent) + Send + Sync>;

/// 应用主题管理器
pub struct AppThemeManager {
    /// 主题配置
    config: ThemeConfig,
    /// 终端主题管理器
    terminal_theme_manager: TerminalThemeManager,
    /// 配置文件路径
    config_path: Option<PathBuf>,
    /// 变更回调列表
    callbacks: Vec<ThemeChangeCallback>,
}

impl std::fmt::Debug for AppThemeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppThemeManager")
            .field("config", &self.config)
            .field("terminal_theme_manager", &self.terminal_theme_manager)
            .field("config_path", &self.config_path)
            .field("callbacks_count", &self.callbacks.len())
            .finish()
    }
}

impl Default for AppThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AppThemeManager {
    /// 创建新的主题管理器
    pub fn new() -> Self {
        Self {
            config: ThemeConfig::default(),
            terminal_theme_manager: TerminalThemeManager::new(),
            config_path: None,
            callbacks: Vec::new(),
        }
    }

    /// 使用配置创建
    pub fn with_config(config: ThemeConfig) -> Self {
        let mut manager = Self::new();
        manager.config = config.clone();
        // 同步终端主题
        manager
            .terminal_theme_manager
            .set_current(config.theme.to_terminal_theme());
        manager
    }

    /// 设置配置文件路径
    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    /// 从配置文件加载
    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.config_path {
            if path.exists() {
                self.config = ThemeConfig::load_from_file(path)?;
                self.terminal_theme_manager
                    .set_current(self.config.theme.to_terminal_theme());
                info!("Theme config loaded from {:?}", path);
            } else {
                debug!("Theme config file not found, using defaults");
            }
        }
        Ok(())
    }

    /// 保存配置到文件
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.config_path {
            self.config.save_to_file(path)?;
            info!("Theme config saved to {:?}", path);
        }
        Ok(())
    }

    /// 获取当前主题配置
    pub fn config(&self) -> &ThemeConfig {
        &self.config
    }

    /// 获取当前主题
    pub fn current_theme(&self) -> BuiltinTheme {
        self.config.theme
    }

    /// 获取当前终端主题
    pub fn terminal_theme(&self) -> &TerminalTheme {
        self.terminal_theme_manager.current()
    }

    /// 获取主题模式
    pub fn mode(&self) -> ThemeMode {
        self.config.mode
    }

    /// 是否为深色模式
    pub fn is_dark_mode(&self) -> bool {
        self.config.theme.is_dark()
    }

    /// 获取终端透明度
    pub fn terminal_opacity(&self) -> f32 {
        self.config.terminal_opacity
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: BuiltinTheme) {
        let old_theme = self.config.theme;
        if old_theme != theme {
            self.config.theme = theme;
            self.terminal_theme_manager
                .set_current(theme.to_terminal_theme());

            let event = ThemeEvent::ThemeChanged {
                old_theme,
                new_theme: theme,
            };
            self.notify_callbacks(&event);

            info!("Theme changed from {:?} to {:?}", old_theme, theme);

            // 自动保存
            if let Err(e) = self.save_config() {
                warn!("Failed to save theme config: {}", e);
            }
        }
    }

    /// 设置主题模式
    pub fn set_mode(&mut self, mode: ThemeMode) {
        let old_mode = self.config.mode;
        if old_mode != mode {
            self.config.mode = mode;

            // 根据模式自动选择主题
            match mode {
                ThemeMode::Light => {
                    if self.config.theme.is_dark() {
                        self.set_theme(BuiltinTheme::Light);
                    }
                },
                ThemeMode::Dark => {
                    if !self.config.theme.is_dark() {
                        self.set_theme(BuiltinTheme::Dark);
                    }
                },
                ThemeMode::System => {
                    // 检测系统主题并应用
                    let is_system_dark = detect_system_dark_mode();
                    info!(
                        "System theme detected: {}",
                        if is_system_dark { "dark" } else { "light" }
                    );

                    if is_system_dark && !self.config.theme.is_dark() {
                        self.set_theme(BuiltinTheme::Dark);
                    } else if !is_system_dark && self.config.theme.is_dark() {
                        self.set_theme(BuiltinTheme::Light);
                    }
                },
            }

            let event = ThemeEvent::ModeChanged {
                old_mode,
                new_mode: mode,
            };
            self.notify_callbacks(&event);

            info!("Theme mode changed from {:?} to {:?}", old_mode, mode);
        }
    }

    /// 设置终端透明度
    pub fn set_terminal_opacity(&mut self, opacity: f32) {
        let opacity = opacity.clamp(0.0, 1.0);
        if (self.config.terminal_opacity - opacity).abs() > 0.001 {
            self.config.terminal_opacity = opacity;

            let event = ThemeEvent::OpacityChanged { opacity };
            self.notify_callbacks(&event);

            if let Err(e) = self.save_config() {
                warn!("Failed to save theme config: {}", e);
            }
        }
    }

    /// 切换深色/浅色模式
    pub fn toggle_dark_light(&mut self) {
        if self.is_dark_mode() {
            self.set_mode(ThemeMode::Light);
        } else {
            self.set_mode(ThemeMode::Dark);
        }
    }

    /// 切换到下一个主题
    pub fn next_theme(&mut self) {
        let all = BuiltinTheme::all();
        let current_idx = all
            .iter()
            .position(|t| *t == self.config.theme)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % all.len();
        self.set_theme(all[next_idx]);
    }

    /// 切换到上一个主题
    pub fn prev_theme(&mut self) {
        let all = BuiltinTheme::all();
        let current_idx = all
            .iter()
            .position(|t| *t == self.config.theme)
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            all.len() - 1
        } else {
            current_idx - 1
        };
        self.set_theme(all[prev_idx]);
    }

    /// 获取所有可用主题
    pub fn available_themes(&self) -> &'static [BuiltinTheme] {
        BuiltinTheme::all()
    }

    /// 添加变更回调
    pub fn on_change(&mut self, callback: ThemeChangeCallback) {
        self.callbacks.push(callback);
    }

    /// 通知所有回调
    fn notify_callbacks(&self, event: &ThemeEvent) {
        for callback in &self.callbacks {
            callback(event);
        }
    }
}

// ============== 额外的内置主题定义 ==============

/// 创建 Solarized Light 主题
fn create_solarized_light() -> TerminalTheme {
    use crate::ui::terminal_view::colors::{ColorPalette, Rgb};
    use crate::ui::terminal_view::theme::{CursorColors, SelectionColors, UiColors};

    TerminalTheme {
        name: "Solarized Light".to_string(),
        is_dark: false,
        palette: ColorPalette {
            base_colors: [
                Rgb::new(0x07, 0x36, 0x42), // Black (base02)
                Rgb::new(0xdc, 0x32, 0x2f), // Red
                Rgb::new(0x85, 0x99, 0x00), // Green
                Rgb::new(0xb5, 0x89, 0x00), // Yellow
                Rgb::new(0x26, 0x8b, 0xd2), // Blue
                Rgb::new(0xd3, 0x36, 0x82), // Magenta
                Rgb::new(0x2a, 0xa1, 0x98), // Cyan
                Rgb::new(0xee, 0xe8, 0xd5), // White (base2)
                Rgb::new(0x00, 0x2b, 0x36), // Bright Black (base03)
                Rgb::new(0xcb, 0x4b, 0x16), // Bright Red (orange)
                Rgb::new(0x58, 0x6e, 0x75), // Bright Green (base01)
                Rgb::new(0x65, 0x7b, 0x83), // Bright Yellow (base00)
                Rgb::new(0x83, 0x94, 0x96), // Bright Blue (base0)
                Rgb::new(0x6c, 0x71, 0xc4), // Bright Magenta (violet)
                Rgb::new(0x93, 0xa1, 0xa1), // Bright Cyan (base1)
                Rgb::new(0xfd, 0xf6, 0xe3), // Bright White (base3)
            ],
            foreground: Rgb::new(0x65, 0x7b, 0x83),
            background: Rgb::new(0xfd, 0xf6, 0xe3),
        },
        cursor: CursorColors {
            color: Rgb::new(0x65, 0x7b, 0x83),
            text_color: Rgb::new(0xfd, 0xf6, 0xe3),
        },
        selection: SelectionColors {
            background: Rgb::new(0xee, 0xe8, 0xd5),
            foreground: None,
        },
        ui: UiColors {
            border: Rgb::new(0xee, 0xe8, 0xd5),
            scrollbar: Rgb::new(0x93, 0xa1, 0xa1),
            scrollbar_hover: Rgb::new(0x83, 0x94, 0x96),
            search_match: Rgb::new(0xb5, 0x89, 0x00),
            search_match_active: Rgb::new(0xcb, 0x4b, 0x16),
        },
    }
}

/// 创建 Nord 主题
fn create_nord_theme() -> TerminalTheme {
    use crate::ui::terminal_view::colors::{ColorPalette, Rgb};
    use crate::ui::terminal_view::theme::{CursorColors, SelectionColors, UiColors};

    TerminalTheme {
        name: "Nord".to_string(),
        is_dark: true,
        palette: ColorPalette {
            base_colors: [
                Rgb::new(0x3b, 0x42, 0x52), // Black (nord1)
                Rgb::new(0xbf, 0x61, 0x6a), // Red (nord11)
                Rgb::new(0xa3, 0xbe, 0x8c), // Green (nord14)
                Rgb::new(0xeb, 0xcb, 0x8b), // Yellow (nord13)
                Rgb::new(0x81, 0xa1, 0xc1), // Blue (nord9)
                Rgb::new(0xb4, 0x8e, 0xad), // Magenta (nord15)
                Rgb::new(0x88, 0xc0, 0xd0), // Cyan (nord8)
                Rgb::new(0xe5, 0xe9, 0xf0), // White (nord5)
                Rgb::new(0x4c, 0x56, 0x6a), // Bright Black (nord3)
                Rgb::new(0xbf, 0x61, 0x6a), // Bright Red
                Rgb::new(0xa3, 0xbe, 0x8c), // Bright Green
                Rgb::new(0xeb, 0xcb, 0x8b), // Bright Yellow
                Rgb::new(0x81, 0xa1, 0xc1), // Bright Blue
                Rgb::new(0xb4, 0x8e, 0xad), // Bright Magenta
                Rgb::new(0x8f, 0xbc, 0xbb), // Bright Cyan (nord7)
                Rgb::new(0xec, 0xef, 0xf4), // Bright White (nord6)
            ],
            foreground: Rgb::new(0xd8, 0xde, 0xe9),
            background: Rgb::new(0x2e, 0x34, 0x40),
        },
        cursor: CursorColors {
            color: Rgb::new(0xd8, 0xde, 0xe9),
            text_color: Rgb::new(0x2e, 0x34, 0x40),
        },
        selection: SelectionColors {
            background: Rgb::new(0x43, 0x4c, 0x5e),
            foreground: None,
        },
        ui: UiColors {
            border: Rgb::new(0x43, 0x4c, 0x5e),
            scrollbar: Rgb::new(0x43, 0x4c, 0x5e),
            scrollbar_hover: Rgb::new(0x4c, 0x56, 0x6a),
            search_match: Rgb::new(0xeb, 0xcb, 0x8b),
            search_match_active: Rgb::new(0x88, 0xc0, 0xd0),
        },
    }
}

/// 创建 Monokai 主题
fn create_monokai_theme() -> TerminalTheme {
    use crate::ui::terminal_view::colors::{ColorPalette, Rgb};
    use crate::ui::terminal_view::theme::{CursorColors, SelectionColors, UiColors};

    TerminalTheme {
        name: "Monokai".to_string(),
        is_dark: true,
        palette: ColorPalette {
            base_colors: [
                Rgb::new(0x27, 0x28, 0x22), // Black
                Rgb::new(0xf9, 0x26, 0x72), // Red
                Rgb::new(0xa6, 0xe2, 0x2e), // Green
                Rgb::new(0xf4, 0xbf, 0x75), // Yellow
                Rgb::new(0x66, 0xd9, 0xef), // Blue
                Rgb::new(0xae, 0x81, 0xff), // Magenta
                Rgb::new(0xa1, 0xef, 0xe4), // Cyan
                Rgb::new(0xf8, 0xf8, 0xf2), // White
                Rgb::new(0x75, 0x71, 0x5e), // Bright Black
                Rgb::new(0xf9, 0x26, 0x72), // Bright Red
                Rgb::new(0xa6, 0xe2, 0x2e), // Bright Green
                Rgb::new(0xf4, 0xbf, 0x75), // Bright Yellow
                Rgb::new(0x66, 0xd9, 0xef), // Bright Blue
                Rgb::new(0xae, 0x81, 0xff), // Bright Magenta
                Rgb::new(0xa1, 0xef, 0xe4), // Bright Cyan
                Rgb::new(0xf9, 0xf8, 0xf5), // Bright White
            ],
            foreground: Rgb::new(0xf8, 0xf8, 0xf2),
            background: Rgb::new(0x27, 0x28, 0x22),
        },
        cursor: CursorColors {
            color: Rgb::new(0xf8, 0xf8, 0xf0),
            text_color: Rgb::new(0x27, 0x28, 0x22),
        },
        selection: SelectionColors {
            background: Rgb::new(0x49, 0x48, 0x3e),
            foreground: None,
        },
        ui: UiColors {
            border: Rgb::new(0x49, 0x48, 0x3e),
            scrollbar: Rgb::new(0x49, 0x48, 0x3e),
            scrollbar_hover: Rgb::new(0x75, 0x71, 0x5e),
            search_match: Rgb::new(0xf4, 0xbf, 0x75),
            search_match_active: Rgb::new(0xf9, 0x26, 0x72),
        },
    }
}

/// 创建 Gruvbox Dark 主题
fn create_gruvbox_dark_theme() -> TerminalTheme {
    use crate::ui::terminal_view::colors::{ColorPalette, Rgb};
    use crate::ui::terminal_view::theme::{CursorColors, SelectionColors, UiColors};

    TerminalTheme {
        name: "Gruvbox Dark".to_string(),
        is_dark: true,
        palette: ColorPalette {
            base_colors: [
                Rgb::new(0x28, 0x28, 0x28), // Black (bg0)
                Rgb::new(0xcc, 0x24, 0x1d), // Red
                Rgb::new(0x98, 0x97, 0x1a), // Green
                Rgb::new(0xd7, 0x99, 0x21), // Yellow
                Rgb::new(0x45, 0x85, 0x88), // Blue
                Rgb::new(0xb1, 0x62, 0x86), // Magenta
                Rgb::new(0x68, 0x9d, 0x6a), // Cyan
                Rgb::new(0xa8, 0x99, 0x84), // White (fg4)
                Rgb::new(0x92, 0x83, 0x74), // Bright Black (bg4)
                Rgb::new(0xfb, 0x49, 0x34), // Bright Red
                Rgb::new(0xb8, 0xbb, 0x26), // Bright Green
                Rgb::new(0xfa, 0xbd, 0x2f), // Bright Yellow
                Rgb::new(0x83, 0xa5, 0x98), // Bright Blue
                Rgb::new(0xd3, 0x86, 0x9b), // Bright Magenta
                Rgb::new(0x8e, 0xc0, 0x7c), // Bright Cyan
                Rgb::new(0xeb, 0xdb, 0xb2), // Bright White (fg1)
            ],
            foreground: Rgb::new(0xeb, 0xdb, 0xb2),
            background: Rgb::new(0x28, 0x28, 0x28),
        },
        cursor: CursorColors {
            color: Rgb::new(0xeb, 0xdb, 0xb2),
            text_color: Rgb::new(0x28, 0x28, 0x28),
        },
        selection: SelectionColors {
            background: Rgb::new(0x50, 0x49, 0x45),
            foreground: None,
        },
        ui: UiColors {
            border: Rgb::new(0x50, 0x49, 0x45),
            scrollbar: Rgb::new(0x50, 0x49, 0x45),
            scrollbar_hover: Rgb::new(0x66, 0x5c, 0x54),
            search_match: Rgb::new(0xfa, 0xbd, 0x2f),
            search_match_active: Rgb::new(0xfe, 0x80, 0x19),
        },
    }
}

/// 创建 Tokyo Night 主题
fn create_tokyo_night_theme() -> TerminalTheme {
    use crate::ui::terminal_view::colors::{ColorPalette, Rgb};
    use crate::ui::terminal_view::theme::{CursorColors, SelectionColors, UiColors};

    TerminalTheme {
        name: "Tokyo Night".to_string(),
        is_dark: true,
        palette: ColorPalette {
            base_colors: [
                Rgb::new(0x1a, 0x1b, 0x26), // Black
                Rgb::new(0xf7, 0x76, 0x8e), // Red
                Rgb::new(0x9e, 0xce, 0x6a), // Green
                Rgb::new(0xe0, 0xaf, 0x68), // Yellow
                Rgb::new(0x7a, 0xa2, 0xf7), // Blue
                Rgb::new(0xbb, 0x9a, 0xf7), // Magenta
                Rgb::new(0x7d, 0xcf, 0xff), // Cyan
                Rgb::new(0xc0, 0xca, 0xf5), // White
                Rgb::new(0x41, 0x4d, 0x68), // Bright Black
                Rgb::new(0xf7, 0x76, 0x8e), // Bright Red
                Rgb::new(0x9e, 0xce, 0x6a), // Bright Green
                Rgb::new(0xe0, 0xaf, 0x68), // Bright Yellow
                Rgb::new(0x7a, 0xa2, 0xf7), // Bright Blue
                Rgb::new(0xbb, 0x9a, 0xf7), // Bright Magenta
                Rgb::new(0x7d, 0xcf, 0xff), // Bright Cyan
                Rgb::new(0xc0, 0xca, 0xf5), // Bright White
            ],
            foreground: Rgb::new(0xc0, 0xca, 0xf5),
            background: Rgb::new(0x1a, 0x1b, 0x26),
        },
        cursor: CursorColors {
            color: Rgb::new(0xc0, 0xca, 0xf5),
            text_color: Rgb::new(0x1a, 0x1b, 0x26),
        },
        selection: SelectionColors {
            background: Rgb::new(0x28, 0x2d, 0x3e),
            foreground: None,
        },
        ui: UiColors {
            border: Rgb::new(0x28, 0x2d, 0x3e),
            scrollbar: Rgb::new(0x28, 0x2d, 0x3e),
            scrollbar_hover: Rgb::new(0x41, 0x4d, 0x68),
            search_match: Rgb::new(0xe0, 0xaf, 0x68),
            search_match_active: Rgb::new(0xff, 0x9e, 0x64),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_mode_display() {
        assert_eq!(ThemeMode::Dark.display_name(), "Dark");
        assert_eq!(ThemeMode::Light.display_name(), "Light");
        assert_eq!(ThemeMode::System.display_name(), "System");
    }

    #[test]
    fn test_theme_mode_is_dark() {
        assert!(ThemeMode::Dark.is_dark());
        assert!(!ThemeMode::Light.is_dark());
    }

    #[test]
    fn test_builtin_theme_display() {
        assert_eq!(BuiltinTheme::Dark.display_name(), "Dark");
        assert_eq!(BuiltinTheme::Dracula.display_name(), "Dracula");
        assert_eq!(BuiltinTheme::OneDark.display_name(), "One Dark");
        assert_eq!(BuiltinTheme::TokyoNight.display_name(), "Tokyo Night");
    }

    #[test]
    fn test_builtin_theme_is_dark() {
        assert!(BuiltinTheme::Dark.is_dark());
        assert!(BuiltinTheme::Dracula.is_dark());
        assert!(BuiltinTheme::Nord.is_dark());
        assert!(!BuiltinTheme::Light.is_dark());
        assert!(!BuiltinTheme::SolarizedLight.is_dark());
    }

    #[test]
    fn test_builtin_theme_all() {
        let all = BuiltinTheme::all();
        assert!(!all.is_empty());
        assert!(all.contains(&BuiltinTheme::Dark));
        assert!(all.contains(&BuiltinTheme::Light));
        assert!(all.contains(&BuiltinTheme::Dracula));
    }

    #[test]
    fn test_builtin_theme_to_terminal_theme() {
        let dark = BuiltinTheme::Dark.to_terminal_theme();
        assert!(dark.is_dark);
        assert_eq!(dark.name, "Dark");

        let light = BuiltinTheme::Light.to_terminal_theme();
        assert!(!light.is_dark);

        let dracula = BuiltinTheme::Dracula.to_terminal_theme();
        assert_eq!(dracula.name, "Dracula");

        let tokyo = BuiltinTheme::TokyoNight.to_terminal_theme();
        assert_eq!(tokyo.name, "Tokyo Night");
    }

    #[test]
    fn test_theme_config_default() {
        let config = ThemeConfig::default();
        assert_eq!(config.mode, ThemeMode::Dark);
        assert_eq!(config.theme, BuiltinTheme::Dark);
        assert_eq!(config.terminal_opacity, 1.0);
    }

    #[test]
    fn test_theme_config_toml_roundtrip() {
        let config = ThemeConfig {
            mode: ThemeMode::Dark,
            theme: BuiltinTheme::Dracula,
            custom_theme_path: None,
            terminal_opacity: 0.95,
            use_system_font: true,
        };

        let toml_str = config.to_toml().unwrap();
        let parsed = ThemeConfig::from_toml(&toml_str).unwrap();

        assert_eq!(parsed.mode, config.mode);
        assert_eq!(parsed.theme, config.theme);
        assert!((parsed.terminal_opacity - config.terminal_opacity).abs() < 0.001);
        assert_eq!(parsed.use_system_font, config.use_system_font);
    }

    #[test]
    fn test_app_theme_manager_new() {
        let manager = AppThemeManager::new();
        assert_eq!(manager.current_theme(), BuiltinTheme::Dark);
        assert!(manager.is_dark_mode());
    }

    #[test]
    fn test_app_theme_manager_set_theme() {
        let mut manager = AppThemeManager::new();

        manager.set_theme(BuiltinTheme::Dracula);
        assert_eq!(manager.current_theme(), BuiltinTheme::Dracula);
        assert_eq!(manager.terminal_theme().name, "Dracula");

        manager.set_theme(BuiltinTheme::Light);
        assert_eq!(manager.current_theme(), BuiltinTheme::Light);
        assert!(!manager.is_dark_mode());
    }

    #[test]
    fn test_app_theme_manager_toggle() {
        let mut manager = AppThemeManager::new();
        assert!(manager.is_dark_mode());

        manager.toggle_dark_light();
        assert!(!manager.is_dark_mode());

        manager.toggle_dark_light();
        assert!(manager.is_dark_mode());
    }

    #[test]
    fn test_app_theme_manager_next_prev() {
        let mut manager = AppThemeManager::new();
        let initial = manager.current_theme();

        manager.next_theme();
        assert_ne!(manager.current_theme(), initial);

        manager.prev_theme();
        assert_eq!(manager.current_theme(), initial);
    }

    #[test]
    fn test_app_theme_manager_opacity() {
        let mut manager = AppThemeManager::new();
        assert_eq!(manager.terminal_opacity(), 1.0);

        manager.set_terminal_opacity(0.8);
        assert!((manager.terminal_opacity() - 0.8).abs() < 0.001);

        // Test clamping
        manager.set_terminal_opacity(1.5);
        assert_eq!(manager.terminal_opacity(), 1.0);

        manager.set_terminal_opacity(-0.5);
        assert_eq!(manager.terminal_opacity(), 0.0);
    }

    #[test]
    fn test_dark_light_theme_lists() {
        let dark = BuiltinTheme::dark_themes();
        let light = BuiltinTheme::light_themes();

        assert!(dark.iter().all(|t| t.is_dark()));
        assert!(light.iter().all(|t| !t.is_dark()));
        assert!(!dark.is_empty());
        assert!(!light.is_empty());
    }
}
