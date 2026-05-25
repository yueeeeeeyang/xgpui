//! `TextInput` 样式解析。
//!
//! 组件渲染层只消费这里返回的解析结果，从而避免尺寸、状态和变体规则散落在渲染代码中。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{TextInputSize, TextInputStatus, TextInputVariant};

/// 输入框渲染所需的完整样式。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTextInputStyle {
    /// 输入框整体高度。
    pub height: Pixels,
    /// 内容区域水平内边距。
    pub padding_x: Pixels,
    /// 前缀、文本区域、清除按钮和后缀之间的间距。
    pub gap: Pixels,
    /// 文本字号。
    pub font_size: Pixels,
    /// 文本行高。
    pub line_height: Pixels,
    /// 输入框圆角。
    pub radius: Pixels,
    /// 容器背景色。
    pub background: Hsla,
    /// 容器边框色。
    pub border: Hsla,
    /// 正文颜色。
    pub text: Hsla,
    /// placeholder 颜色。
    pub placeholder: Hsla,
    /// helper text 颜色。
    pub helper: Hsla,
    /// 文本选区背景色。
    pub selection: Hsla,
    /// 光标颜色。
    pub cursor: Hsla,
    /// IME marked text 下划线颜色。
    pub marked_underline: Hsla,
    /// 清除按钮文字颜色。
    pub clear_button_text: Hsla,
    /// 清除按钮 hover 背景色。
    pub clear_button_background: Hsla,
    /// 禁用态透明度。
    pub opacity: f32,
}

/// 根据尺寸、变体、状态和焦点解析输入框样式。
pub fn resolve_text_input_style(
    input_size: TextInputSize,
    variant: TextInputVariant,
    status: TextInputStatus,
    focused: bool,
    disabled: bool,
    cx: &App,
) -> ResolvedTextInputStyle {
    let theme = theme::text_input_theme(cx);
    let (height, padding_x, font_size, line_height) = size_tokens(input_size);
    let semantic_color = semantic_color(status, focused, theme);
    let (background, border) = variant_tokens(variant, semantic_color, theme);

    ResolvedTextInputStyle {
        height,
        padding_x,
        gap: theme.gap,
        font_size,
        line_height,
        radius: theme.radius,
        background,
        border,
        text: if disabled {
            theme.disabled_text
        } else {
            theme.text
        },
        placeholder: theme.placeholder,
        helper: if status == TextInputStatus::Default {
            theme.helper
        } else {
            semantic_color
        },
        selection: theme.selection,
        cursor: semantic_color,
        marked_underline: semantic_color,
        clear_button_text: theme.clear_button_text,
        clear_button_background: theme.clear_button_background,
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 解析尺寸相关 token。
fn size_tokens(input_size: TextInputSize) -> (Pixels, Pixels, Pixels, Pixels) {
    match input_size {
        TextInputSize::Small => (
            size::input_sm_height(),
            spacing::md(),
            size::text_sm(),
            size::line_sm(),
        ),
        TextInputSize::Medium => (
            size::input_md_height(),
            spacing::lg(),
            size::text_md(),
            size::line_md(),
        ),
        TextInputSize::Large => (
            size::input_lg_height(),
            spacing::lg(),
            size::text_lg(),
            size::line_lg(),
        ),
    }
}

/// 解析状态色。
fn semantic_color(status: TextInputStatus, focused: bool, theme: theme::TextInputTheme) -> Hsla {
    match status {
        TextInputStatus::Default if focused => theme.focus,
        TextInputStatus::Default => theme.border,
        TextInputStatus::Error => theme.danger,
        TextInputStatus::Warning => theme.warning,
        TextInputStatus::Success => theme.success,
    }
}

/// 解析变体背景和边框。
fn variant_tokens(
    variant: TextInputVariant,
    semantic_color: Hsla,
    theme: theme::TextInputTheme,
) -> (Hsla, Hsla) {
    match variant {
        TextInputVariant::Outlined => (theme.background, semantic_color),
        TextInputVariant::Filled => (theme.filled_background, semantic_color),
        TextInputVariant::Ghost => (theme.background, theme.ghost_border),
    }
}
