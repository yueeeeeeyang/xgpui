//! `Tree` 样式解析。
//!
//! Tree 的渲染层只消费这里返回的解析结果，从而让尺寸、状态、变体和明暗皮肤规则集中维护。
//! 组件内部不硬编码颜色，所有颜色都来自 `foundation::theme::TreeTheme`。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{TreeSize, TreeStatus, TreeVariant};

/// Tree 渲染所需的完整样式。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTreeStyle {
    /// 单行固定高度。虚拟列表依赖该值保证每个 item 高度一致。
    pub row_height: Pixels,
    /// 容器水平内边距。
    pub padding_x: Pixels,
    /// 容器垂直内边距。
    pub padding_y: Pixels,
    /// 节点缩进步长。
    pub indent: Pixels,
    /// 行内图标尺寸。
    pub icon_size: Pixels,
    /// 复选框尺寸。
    pub checkbox_size: Pixels,
    /// 文本字号。
    pub font_size: Pixels,
    /// 文本行高。
    pub line_height: Pixels,
    /// 容器圆角。
    pub radius: Pixels,
    /// 容器背景色。
    pub background: Hsla,
    /// 容器边框色。
    pub border: Hsla,
    /// 正文颜色。
    pub text: Hsla,
    /// 弱化文本和图标颜色。
    pub muted_text: Hsla,
    /// helper text 颜色。
    pub helper: Hsla,
    /// 行 hover 背景色。
    pub row_hover: Hsla,
    /// 键盘活动行背景色。
    pub row_active: Hsla,
    /// selected 行背景色。
    pub row_selected: Hsla,
    /// selected 行文本颜色。
    pub row_selected_text: Hsla,
    /// checkbox 边框颜色。
    pub checkbox_border: Hsla,
    /// checkbox 背景颜色。
    pub checkbox_background: Hsla,
    /// checkbox 选中背景颜色。
    pub checkbox_checked_background: Hsla,
    /// checkbox 选中图标颜色。
    pub checkbox_checked_text: Hsla,
    /// 空状态文本颜色。
    pub empty_text: Hsla,
    /// 禁用态文本颜色。
    pub disabled_text: Hsla,
    /// 禁用态透明度。
    pub opacity: f32,
}

/// 根据尺寸、变体、状态、焦点和禁用态解析 Tree 样式。
pub fn resolve_tree_style(
    tree_size: TreeSize,
    variant: TreeVariant,
    status: TreeStatus,
    focused: bool,
    disabled: bool,
    cx: &App,
) -> ResolvedTreeStyle {
    let theme = theme::tree_theme(cx);
    let (
        row_height,
        padding_x,
        padding_y,
        indent,
        icon_size,
        checkbox_size,
        font_size,
        line_height,
    ) = size_tokens(tree_size);
    let semantic_color = semantic_color(status, focused, theme);
    let (background, border) = variant_tokens(variant, semantic_color, theme);

    ResolvedTreeStyle {
        row_height,
        padding_x,
        padding_y,
        indent,
        icon_size,
        checkbox_size,
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
        muted_text: if disabled {
            theme.disabled_text
        } else {
            theme.muted_text
        },
        helper: if status == TreeStatus::Default {
            theme.helper
        } else {
            semantic_color
        },
        row_hover: theme.row_hover,
        row_active: theme.row_active,
        row_selected: theme.row_selected,
        row_selected_text: theme.row_selected_text,
        checkbox_border: theme.checkbox_border,
        checkbox_background: theme.checkbox_background,
        checkbox_checked_background: theme.checkbox_checked_background,
        checkbox_checked_text: theme.checkbox_checked_text,
        empty_text: theme.empty_text,
        disabled_text: theme.disabled_text,
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 解析尺寸相关 token。
fn size_tokens(
    tree_size: TreeSize,
) -> (
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
) {
    match tree_size {
        TreeSize::Small => (
            size::input_sm_height(),
            spacing::sm(),
            spacing::xs(),
            spacing::lg(),
            size::text_sm(),
            size::text_sm(),
            size::text_sm(),
            size::line_sm(),
        ),
        TreeSize::Medium => (
            size::input_md_height(),
            spacing::md(),
            spacing::xs(),
            spacing::lg() * 1.5,
            size::text_md(),
            size::text_md(),
            size::text_md(),
            size::line_md(),
        ),
        TreeSize::Large => (
            size::input_lg_height(),
            spacing::lg(),
            spacing::sm(),
            spacing::lg() * 1.5,
            size::text_lg(),
            size::text_lg(),
            size::text_lg(),
            size::line_lg(),
        ),
    }
}

/// 解析状态色。
fn semantic_color(status: TreeStatus, focused: bool, theme: theme::TreeTheme) -> Hsla {
    match status {
        TreeStatus::Default if focused => theme.focus,
        TreeStatus::Default => theme.border,
        TreeStatus::Error => theme.danger,
        TreeStatus::Warning => theme.warning,
        TreeStatus::Success => theme.success,
    }
}

/// 解析容器背景和边框。
fn variant_tokens(
    variant: TreeVariant,
    semantic_color: Hsla,
    theme: theme::TreeTheme,
) -> (Hsla, Hsla) {
    match variant {
        TreeVariant::Outlined => (theme.background, semantic_color),
        TreeVariant::Filled => (theme.filled_background, semantic_color),
        TreeVariant::Ghost => (theme.background, theme.ghost_border),
    }
}
