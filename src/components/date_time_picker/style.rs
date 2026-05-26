//! `DateTimePicker` 样式解析。
//!
//! 本模块集中把尺寸、变体、状态和明暗主题 token 转成渲染层可直接使用的样式快照。
//! 日期网格和时间列都依赖固定尺寸，避免弹层打开后因内容变化产生布局跳动。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{DateTimePickerSize, DateTimePickerStatus};

/// DateTimePicker 渲染样式快照。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedDateTimePickerStyle {
    /// 输入触发器高度。
    pub height: Pixels,
    /// 字号。
    pub font_size: Pixels,
    /// 行高。
    pub line_height: Pixels,
    /// 弹层内边距。
    pub popup_padding: Pixels,
    /// 元素间距。
    pub gap: Pixels,
    /// 图标尺寸。
    pub icon_size: Pixels,
    /// 日历单元尺寸。
    pub cell_size: Pixels,
    /// 时间项高度。
    pub time_item_height: Pixels,
    /// 时间列宽度。
    pub time_column_width: Pixels,
    /// 时间列之间的间距。
    pub time_column_gap: Pixels,
    /// 弹层圆角。
    pub popup_radius: Pixels,
    /// 弹层偏移。
    pub popup_offset: Pixels,
    /// 正文颜色。
    pub text: Hsla,
    /// 弱化文字颜色。
    pub muted_text: Hsla,
    /// helper text 颜色。
    pub helper: Hsla,
    /// 弹层背景。
    pub popup_background: Hsla,
    /// 弹层边框。
    pub popup_border: Hsla,
    /// 单元 hover 背景。
    pub cell_hover: Hsla,
    /// 当前日或活动项背景。
    pub cell_active: Hsla,
    /// 已选单元背景。
    pub cell_selected: Hsla,
    /// 已选单元文字。
    pub cell_selected_text: Hsla,
    /// 范围中间段背景。
    pub range_background: Hsla,
    /// 禁用文字。
    pub disabled_text: Hsla,
    /// 清除按钮 hover 背景。
    pub clear_button_background: Hsla,
    /// 禁用透明度。
    pub opacity: f32,
}

/// 解析 DateTimePicker 样式。
///
/// 视觉变体已经在内部 `TextInput` 触发器中解析；弹层本身使用同一套浮层 token，
/// 因此这里仅接收会影响弹层和 helper text 的状态输入。
pub fn resolve_date_time_picker_style(
    picker_size: DateTimePickerSize,
    status: DateTimePickerStatus,
    focused: bool,
    open: bool,
    disabled: bool,
    has_parse_error: bool,
    cx: &App,
) -> ResolvedDateTimePickerStyle {
    let theme = theme::date_time_picker_theme(cx);
    let (height, font_size, line_height, popup_padding, icon_size, cell_size) =
        size_tokens(picker_size);
    let semantic = semantic_color(status, focused || open, has_parse_error, theme);

    ResolvedDateTimePickerStyle {
        height,
        font_size,
        line_height,
        popup_padding,
        gap: theme.gap,
        icon_size,
        cell_size,
        time_item_height: height,
        time_column_width: gpui::px(52.0),
        time_column_gap: gpui::px(6.0),
        popup_radius: theme.popup_radius,
        popup_offset: theme.popup_offset,
        text: if disabled {
            theme.disabled_text
        } else {
            theme.text
        },
        muted_text: if disabled {
            theme.disabled_text
        } else {
            theme.muted_text
        },
        helper: if has_parse_error {
            theme.danger
        } else if status == DateTimePickerStatus::Default {
            theme.helper
        } else {
            semantic
        },
        popup_background: theme.popup_background,
        popup_border: theme.popup_border,
        cell_hover: theme.cell_hover,
        cell_active: theme.cell_active,
        cell_selected: theme.cell_selected,
        cell_selected_text: theme.cell_selected_text,
        range_background: theme.range_background,
        disabled_text: theme.disabled_text,
        clear_button_background: theme.clear_button_background,
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 尺寸 token。
fn size_tokens(
    picker_size: DateTimePickerSize,
) -> (Pixels, Pixels, Pixels, Pixels, Pixels, Pixels) {
    match picker_size {
        DateTimePickerSize::Small => (
            size::input_sm_height(),
            size::text_sm(),
            size::line_sm(),
            spacing::sm(),
            size::text_sm(),
            gpui::px(28.0),
        ),
        DateTimePickerSize::Medium => (
            size::input_md_height(),
            size::text_md(),
            size::line_md(),
            spacing::md(),
            size::text_md(),
            gpui::px(32.0),
        ),
        DateTimePickerSize::Large => (
            size::input_lg_height(),
            size::text_lg(),
            size::line_lg(),
            spacing::lg(),
            size::text_lg(),
            gpui::px(36.0),
        ),
    }
}

/// 解析语义边框色。
fn semantic_color(
    status: DateTimePickerStatus,
    active: bool,
    has_parse_error: bool,
    theme: theme::DateTimePickerTheme,
) -> Hsla {
    if has_parse_error {
        return theme.danger;
    }
    match status {
        DateTimePickerStatus::Default if active => theme.focus,
        DateTimePickerStatus::Default => theme.border,
        DateTimePickerStatus::Error => theme.danger,
        DateTimePickerStatus::Warning => theme.warning,
        DateTimePickerStatus::Success => theme.success,
    }
}
