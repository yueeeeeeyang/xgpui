//! 组件主题 token 和应用级皮肤状态。
//!
//! 该模块把基础 token 组织成组件默认主题，并通过 gpui 全局状态提供明暗皮肤切换能力。

use gpui::{App, Global, Hsla, Pixels};

use super::{color, radius, spacing};

/// xgpui 内置皮肤模式。
///
/// 当前只提供亮色和暗色两套基础 token；后续如果需要品牌主题或更多语义 token，
/// 应优先扩展这里的主题结构，而不是在组件内部硬编码颜色。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    /// 亮色皮肤，适合默认桌面窗口和浅色页面。
    #[default]
    Light,
    /// 暗色皮肤，适合深色窗口和低亮度工作区。
    Dark,
}

/// xgpui 存放在 gpui `App` 中的全局主题状态。
///
/// 该结构目前只保存皮肤模式。它被设计成全局状态，是为了让所有组件在同一帧读取同一套主题。
#[derive(Clone, Copy, Debug, Default)]
pub struct XgpuiTheme {
    /// 当前应用使用的皮肤模式。
    pub mode: ThemeMode,
}

impl XgpuiTheme {
    /// 使用指定皮肤模式创建主题状态。
    pub fn new(mode: ThemeMode) -> Self {
        Self { mode }
    }
}

impl Global for XgpuiTheme {}

/// `TextInput` 使用的默认主题 token。
///
/// 该结构只描述跨尺寸、跨状态共享的视觉基础值；尺寸相关值仍由组件尺寸枚举决定。
#[derive(Clone, Copy, Debug)]
pub struct TextInputTheme {
    /// 默认背景色。
    pub background: Hsla,
    /// filled 变体背景色。
    pub filled_background: Hsla,
    /// 默认边框色。
    pub border: Hsla,
    /// 正文颜色。
    pub text: Hsla,
    /// placeholder 颜色。
    pub placeholder: Hsla,
    /// 辅助文本颜色。
    pub helper: Hsla,
    /// 聚焦强调色。
    pub focus: Hsla,
    /// 错误状态颜色。
    pub danger: Hsla,
    /// 警告状态颜色。
    pub warning: Hsla,
    /// 成功状态颜色。
    pub success: Hsla,
    /// 选区背景色。
    pub selection: Hsla,
    /// 禁用态文本颜色。
    pub disabled_text: Hsla,
    /// 清除按钮图标颜色。
    pub clear_button_text: Hsla,
    /// 清除按钮 hover 背景色。
    pub clear_button_background: Hsla,
    /// Ghost 变体边框色。
    pub ghost_border: Hsla,
    /// 默认圆角。
    pub radius: Pixels,
    /// 输入框内部元素间距。
    pub gap: Pixels,
}

/// `Select` 使用的默认主题 token。
///
/// 该结构覆盖触发器、下拉面板、搜索区域、选项状态和清除按钮的基础颜色。
/// 尺寸相关规则仍由组件尺寸枚举决定，避免主题结构过度承担布局职责。
#[derive(Clone, Copy, Debug)]
pub struct SelectTheme {
    /// 默认触发器背景色。
    pub background: Hsla,
    /// filled 变体触发器背景色。
    pub filled_background: Hsla,
    /// 默认触发器边框色。
    pub border: Hsla,
    /// 正文颜色。
    pub text: Hsla,
    /// placeholder 颜色。
    pub placeholder: Hsla,
    /// 辅助文本颜色。
    pub helper: Hsla,
    /// 触发器图标颜色。
    pub icon: Hsla,
    /// 聚焦强调色。
    pub focus: Hsla,
    /// 搜索输入光标颜色。
    pub cursor: Hsla,
    /// 搜索输入选区背景色。
    pub selection: Hsla,
    /// 错误状态颜色。
    pub danger: Hsla,
    /// 警告状态颜色。
    pub warning: Hsla,
    /// 成功状态颜色。
    pub success: Hsla,
    /// 禁用态文本颜色。
    pub disabled_text: Hsla,
    /// Ghost 变体边框色。
    pub ghost_border: Hsla,
    /// 下拉面板背景色。
    pub popup_background: Hsla,
    /// 下拉面板边框色。
    pub popup_border: Hsla,
    /// 搜索区域背景色。
    pub search_background: Hsla,
    /// 搜索区域边框色。
    pub search_border: Hsla,
    /// 选项 hover 背景色。
    pub option_hover: Hsla,
    /// 键盘高亮选项背景色。
    pub option_highlighted: Hsla,
    /// 已选选项背景色。
    pub option_selected: Hsla,
    /// 已选选项文本颜色。
    pub option_selected_text: Hsla,
    /// 空状态文本颜色。
    pub empty_text: Hsla,
    /// 清除按钮图标颜色。
    pub clear_button_text: Hsla,
    /// 清除按钮 hover 背景色。
    pub clear_button_background: Hsla,
    /// 触发器圆角。
    pub radius: Pixels,
    /// 下拉面板圆角。
    pub popup_radius: Pixels,
    /// 触发器内部元素间距。
    pub gap: Pixels,
    /// 下拉面板相对触发器的垂直偏移。
    pub popup_offset: Pixels,
}

/// 确保应用已经拥有 xgpui 主题全局状态。
///
/// `install` 会调用该函数；如果调用方先手动设置过主题，这里不会覆盖已有选择。
pub fn ensure_theme(cx: &mut App) {
    if !cx.has_global::<XgpuiTheme>() {
        cx.set_global(XgpuiTheme::default());
    }
}

/// 返回当前应用皮肤模式。
///
/// 如果调用方还没有执行 `install`，则回退为亮色，保证组件仍能安全渲染。
pub fn theme_mode(cx: &App) -> ThemeMode {
    cx.try_global::<XgpuiTheme>()
        .map(|theme| theme.mode)
        .unwrap_or_default()
}

/// 设置当前应用皮肤模式。
///
/// 该函数会刷新所有窗口，让已经渲染的组件在下一帧读取新的主题 token。
pub fn set_theme_mode(cx: &mut App, mode: ThemeMode) {
    let changed = if cx.has_global::<XgpuiTheme>() {
        let theme = cx.global_mut::<XgpuiTheme>();
        if theme.mode == mode {
            false
        } else {
            theme.mode = mode;
            true
        }
    } else {
        cx.set_global(XgpuiTheme::new(mode));
        true
    };

    if changed {
        cx.refresh_windows();
    }
}

/// 返回当前应用 `TextInput` 应使用的主题。
pub fn text_input_theme(cx: &App) -> TextInputTheme {
    match theme_mode(cx) {
        ThemeMode::Light => light_text_input_theme(),
        ThemeMode::Dark => dark_text_input_theme(),
    }
}

/// 返回当前应用 `Select` 应使用的主题。
pub fn select_theme(cx: &App) -> SelectTheme {
    match theme_mode(cx) {
        ThemeMode::Light => light_select_theme(),
        ThemeMode::Dark => dark_select_theme(),
    }
}

/// 返回亮色皮肤下的 `TextInput` 主题。
fn light_text_input_theme() -> TextInputTheme {
    TextInputTheme {
        background: color::neutral_0(),
        filled_background: color::neutral_50(),
        border: color::neutral_200(),
        text: color::neutral_900(),
        placeholder: color::neutral_400(),
        helper: color::neutral_500(),
        focus: color::primary_500(),
        danger: color::danger_500(),
        warning: color::warning_500(),
        success: color::success_500(),
        selection: color::selection(),
        disabled_text: color::neutral_500(),
        clear_button_text: color::neutral_500(),
        clear_button_background: color::clear_button_hover(),
        ghost_border: color::neutral_0(),
        radius: radius::md(),
        gap: spacing::sm(),
    }
}

/// 返回暗色皮肤下的 `TextInput` 主题。
fn dark_text_input_theme() -> TextInputTheme {
    TextInputTheme {
        background: color::dark_background(),
        filled_background: color::dark_filled_background(),
        border: color::dark_border(),
        text: color::dark_text(),
        placeholder: color::dark_placeholder(),
        helper: color::dark_helper(),
        focus: color::dark_primary(),
        danger: color::dark_danger(),
        warning: color::dark_warning(),
        success: color::dark_success(),
        selection: color::dark_selection(),
        disabled_text: color::dark_disabled_text(),
        clear_button_text: color::dark_helper(),
        clear_button_background: color::dark_clear_button_hover(),
        ghost_border: color::dark_muted_border(),
        radius: radius::md(),
        gap: spacing::sm(),
    }
}

/// 返回亮色皮肤下的 `Select` 主题。
fn light_select_theme() -> SelectTheme {
    SelectTheme {
        background: color::neutral_0(),
        filled_background: color::neutral_50(),
        border: color::neutral_200(),
        text: color::neutral_900(),
        placeholder: color::neutral_400(),
        helper: color::neutral_500(),
        icon: color::neutral_500(),
        focus: color::primary_500(),
        cursor: color::primary_500(),
        selection: color::selection(),
        danger: color::danger_500(),
        warning: color::warning_500(),
        success: color::success_500(),
        disabled_text: color::neutral_500(),
        ghost_border: color::neutral_0(),
        popup_background: color::neutral_0(),
        popup_border: color::neutral_200(),
        search_background: color::neutral_50(),
        search_border: color::neutral_200(),
        option_hover: color::neutral_100(),
        option_highlighted: color::primary_100(),
        option_selected: color::primary_50(),
        option_selected_text: color::neutral_900(),
        empty_text: color::neutral_500(),
        clear_button_text: color::neutral_500(),
        clear_button_background: color::clear_button_hover(),
        radius: radius::md(),
        popup_radius: radius::md(),
        gap: spacing::sm(),
        popup_offset: spacing::xs(),
    }
}

/// 返回暗色皮肤下的 `Select` 主题。
fn dark_select_theme() -> SelectTheme {
    SelectTheme {
        background: color::dark_background(),
        filled_background: color::dark_filled_background(),
        border: color::dark_border(),
        text: color::dark_text(),
        placeholder: color::dark_placeholder(),
        helper: color::dark_helper(),
        icon: color::dark_helper(),
        focus: color::dark_primary(),
        cursor: color::dark_primary(),
        selection: color::dark_selection(),
        danger: color::dark_danger(),
        warning: color::dark_warning(),
        success: color::dark_success(),
        disabled_text: color::dark_disabled_text(),
        ghost_border: color::dark_muted_border(),
        popup_background: color::dark_background(),
        popup_border: color::dark_border(),
        search_background: color::dark_filled_background(),
        search_border: color::dark_border(),
        option_hover: color::dark_option_hover(),
        option_highlighted: color::dark_option_highlighted(),
        option_selected: color::dark_option_selected(),
        option_selected_text: color::dark_text(),
        empty_text: color::dark_helper(),
        clear_button_text: color::dark_helper(),
        clear_button_background: color::dark_clear_button_hover(),
        radius: radius::md(),
        popup_radius: radius::md(),
        gap: spacing::sm(),
        popup_offset: spacing::xs(),
    }
}
