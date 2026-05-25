//! `Select` 样式解析。
//!
//! 组件渲染层只消费这里返回的解析结果，避免尺寸、状态、变体和明暗皮肤规则散落在事件或布局代码中。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{SelectSize, SelectStatus, SelectVariant};

/// Select 渲染所需的完整样式。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedSelectStyle {
    /// 触发器整体高度。
    pub height: Pixels,
    /// 触发器水平内边距。
    pub padding_x: Pixels,
    /// 触发器内部文本、图标和清除按钮间距。
    pub gap: Pixels,
    /// 文本字号。
    pub font_size: Pixels,
    /// 文本行高。
    pub line_height: Pixels,
    /// 触发器圆角。
    pub radius: Pixels,
    /// 下拉面板圆角。
    pub popup_radius: Pixels,
    /// 下拉面板和触发器之间的垂直间距。
    pub popup_offset: Pixels,
    /// 单个选项高度。
    pub option_height: Pixels,
    /// 触发器背景色。
    pub background: Hsla,
    /// 触发器边框色。
    pub border: Hsla,
    /// 已选文本颜色。
    pub text: Hsla,
    /// 占位文本颜色。
    pub placeholder: Hsla,
    /// helper text 颜色。
    pub helper: Hsla,
    /// 图标颜色。
    pub icon: Hsla,
    /// 搜索输入光标颜色。
    pub cursor: Hsla,
    /// 搜索输入选区背景色。
    pub selection: Hsla,
    /// 下拉面板背景色。
    pub popup_background: Hsla,
    /// 下拉面板边框色。
    pub popup_border: Hsla,
    /// 选项 hover 背景色。
    pub option_hover: Hsla,
    /// 键盘高亮选项背景色。
    pub option_highlighted: Hsla,
    /// 已选选项背景色。
    pub option_selected: Hsla,
    /// 已选选项文本颜色。
    pub option_selected_text: Hsla,
    /// 禁用选项文本颜色。
    pub option_disabled_text: Hsla,
    /// 空结果文本颜色。
    pub empty_text: Hsla,
    /// 清除按钮文字颜色。
    pub clear_button_text: Hsla,
    /// 清除按钮 hover 背景色。
    pub clear_button_background: Hsla,
    /// 禁用态透明度。
    pub opacity: f32,
}

/// 根据尺寸、变体、状态和焦点解析 Select 样式。
pub fn resolve_select_style(
    select_size: SelectSize,
    variant: SelectVariant,
    status: SelectStatus,
    focused: bool,
    open: bool,
    disabled: bool,
    cx: &App,
) -> ResolvedSelectStyle {
    let theme = theme::select_theme(cx);
    let (height, padding_x, font_size, line_height, option_height) = size_tokens(select_size);
    let semantic_color = semantic_color(status, focused || open, theme);
    let (background, border) = variant_tokens(variant, semantic_color, theme);

    ResolvedSelectStyle {
        height,
        padding_x,
        gap: theme.gap,
        font_size,
        line_height,
        radius: theme.radius,
        popup_radius: theme.popup_radius,
        popup_offset: theme.popup_offset,
        option_height,
        background,
        border,
        text: if disabled {
            theme.disabled_text
        } else {
            theme.text
        },
        placeholder: theme.placeholder,
        helper: if status == SelectStatus::Default {
            theme.helper
        } else {
            semantic_color
        },
        icon: if disabled {
            theme.disabled_text
        } else {
            theme.icon
        },
        cursor: theme.cursor,
        selection: theme.selection,
        popup_background: theme.popup_background,
        popup_border: theme.popup_border,
        option_hover: theme.option_hover,
        option_highlighted: theme.option_highlighted,
        option_selected: theme.option_selected,
        option_selected_text: theme.option_selected_text,
        option_disabled_text: theme.disabled_text,
        empty_text: theme.empty_text,
        clear_button_text: theme.clear_button_text,
        clear_button_background: theme.clear_button_background,
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 解析尺寸相关 token。
fn size_tokens(select_size: SelectSize) -> (Pixels, Pixels, Pixels, Pixels, Pixels) {
    match select_size {
        SelectSize::Small => (
            size::input_sm_height(),
            spacing::md(),
            size::text_sm(),
            size::line_sm(),
            size::input_sm_height(),
        ),
        SelectSize::Medium => (
            size::input_md_height(),
            spacing::lg(),
            size::text_md(),
            size::line_md(),
            size::input_md_height(),
        ),
        SelectSize::Large => (
            size::input_lg_height(),
            spacing::lg(),
            size::text_lg(),
            size::line_lg(),
            size::input_lg_height(),
        ),
    }
}

/// 解析状态色。
fn semantic_color(status: SelectStatus, active: bool, theme: theme::SelectTheme) -> Hsla {
    match status {
        SelectStatus::Default if active => theme.focus,
        SelectStatus::Default => theme.border,
        SelectStatus::Error => theme.danger,
        SelectStatus::Warning => theme.warning,
        SelectStatus::Success => theme.success,
    }
}

/// 解析变体背景和边框。
fn variant_tokens(
    variant: SelectVariant,
    semantic_color: Hsla,
    theme: theme::SelectTheme,
) -> (Hsla, Hsla) {
    match variant {
        SelectVariant::Outlined => (theme.background, semantic_color),
        SelectVariant::Filled => (theme.filled_background, semantic_color),
        SelectVariant::Ghost => (theme.background, theme.ghost_border),
    }
}
