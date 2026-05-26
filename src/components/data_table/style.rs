//! `DataTable` 样式解析。
//!
//! 本模块集中处理尺寸、变体、状态和明暗皮肤映射。渲染层只消费解析后的样式，
//! 避免在组件布局代码里散落颜色和间距常量。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{DataTableSize, DataTableStatus, DataTableVariant};

/// DataTable 渲染所需的样式快照。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedDataTableStyle {
    /// 表体行高。虚拟列表依赖固定行高。
    pub row_height: Pixels,
    /// 表头高度。
    pub header_height: Pixels,
    /// 分页栏高度。
    pub footer_height: Pixels,
    /// 单元格水平内边距。
    pub cell_padding_x: Pixels,
    /// 容器内边距。
    pub padding: Pixels,
    /// 元素间距。
    pub gap: Pixels,
    /// 选择列宽。
    pub selection_width: Pixels,
    /// 复选框尺寸。
    pub checkbox_size: Pixels,
    /// 图标尺寸。
    pub icon_size: Pixels,
    /// 字号。
    pub font_size: Pixels,
    /// 行高。
    pub line_height: Pixels,
    /// 圆角。
    pub radius: Pixels,
    /// 容器背景。
    pub background: Hsla,
    /// 容器边框。
    pub border: Hsla,
    /// 表头背景。
    pub header_background: Hsla,
    /// 表头文字。
    pub header_text: Hsla,
    /// 正文文字。
    pub text: Hsla,
    /// 弱化文字。
    pub muted_text: Hsla,
    /// helper text 颜色。
    pub helper: Hsla,
    /// 行 hover 背景。
    pub row_hover: Hsla,
    /// 当前 active 行背景。
    pub row_active: Hsla,
    /// selected 行背景。
    pub row_selected: Hsla,
    /// 行分割线。
    pub row_border: Hsla,
    /// 复选框边框。
    pub checkbox_border: Hsla,
    /// 复选框背景。
    pub checkbox_background: Hsla,
    /// 复选框选中背景。
    pub checkbox_checked_background: Hsla,
    /// 复选框选中图标。
    pub checkbox_checked_text: Hsla,
    /// 空状态文字。
    pub empty_text: Hsla,
    /// 禁用态文字。
    pub disabled_text: Hsla,
    /// 禁用透明度。
    pub opacity: f32,
}

/// 解析 DataTable 样式。
pub fn resolve_data_table_style(
    table_size: DataTableSize,
    variant: DataTableVariant,
    status: DataTableStatus,
    focused: bool,
    disabled: bool,
    cx: &App,
) -> ResolvedDataTableStyle {
    let theme = theme::data_table_theme(cx);
    let (
        row_height,
        header_height,
        footer_height,
        cell_padding_x,
        padding,
        gap,
        selection_width,
        checkbox_size,
        icon_size,
        font_size,
        line_height,
    ) = size_tokens(table_size);
    let semantic = semantic_color(status, focused, theme);
    let (background, border) = variant_tokens(variant, semantic, theme);

    ResolvedDataTableStyle {
        row_height,
        header_height,
        footer_height,
        cell_padding_x,
        padding,
        gap,
        selection_width,
        checkbox_size,
        icon_size,
        font_size,
        line_height,
        radius: theme.radius,
        background,
        border,
        header_background: theme.header_background,
        header_text: theme.header_text,
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
        helper: if status == DataTableStatus::Default {
            theme.helper
        } else {
            semantic
        },
        row_hover: theme.row_hover,
        row_active: theme.row_active,
        row_selected: theme.row_selected,
        row_border: theme.row_border,
        checkbox_border: theme.checkbox_border,
        checkbox_background: theme.checkbox_background,
        checkbox_checked_background: theme.checkbox_checked_background,
        checkbox_checked_text: theme.checkbox_checked_text,
        empty_text: theme.empty_text,
        disabled_text: theme.disabled_text,
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 解析尺寸 token。
#[allow(clippy::type_complexity)]
fn size_tokens(
    table_size: DataTableSize,
) -> (
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
    Pixels,
) {
    match table_size {
        DataTableSize::Small => (
            size::input_sm_height(),
            size::input_sm_height(),
            // footer 内包含一个小尺寸 Select；高度略高于普通行高，给紧凑表格也保留上下留白。
            size::input_sm_height() + spacing::sm(),
            spacing::sm(),
            spacing::sm(),
            spacing::xs(),
            gpui::px(36.0),
            size::text_sm(),
            size::text_sm(),
            size::text_sm(),
            size::line_sm(),
        ),
        DataTableSize::Medium => (
            size::input_md_height(),
            size::input_md_height(),
            size::input_md_height(),
            spacing::md(),
            spacing::md(),
            spacing::sm(),
            gpui::px(40.0),
            size::text_md(),
            size::text_md(),
            size::text_md(),
            size::line_md(),
        ),
        DataTableSize::Large => (
            size::input_lg_height(),
            size::input_lg_height(),
            size::input_lg_height(),
            spacing::lg(),
            spacing::lg(),
            spacing::sm(),
            gpui::px(44.0),
            size::text_lg(),
            size::text_lg(),
            size::text_lg(),
            size::line_lg(),
        ),
    }
}

/// 解析状态色。
fn semantic_color(status: DataTableStatus, focused: bool, theme: theme::DataTableTheme) -> Hsla {
    match status {
        DataTableStatus::Default if focused => theme.focus,
        DataTableStatus::Default => theme.border,
        DataTableStatus::Error => theme.danger,
        DataTableStatus::Warning => theme.warning,
        DataTableStatus::Success => theme.success,
    }
}

/// 解析容器背景和边框。
fn variant_tokens(
    variant: DataTableVariant,
    semantic: Hsla,
    theme: theme::DataTableTheme,
) -> (Hsla, Hsla) {
    match variant {
        DataTableVariant::Outlined => (theme.background, semantic),
        DataTableVariant::Filled => (theme.filled_background, semantic),
        DataTableVariant::Ghost => (theme.background, theme.ghost_border),
    }
}
