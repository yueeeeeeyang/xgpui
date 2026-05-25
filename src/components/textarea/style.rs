//! `Textarea` 样式解析。
//!
//! 组件渲染层只消费这里返回的解析结果，从而让尺寸、状态、变体和行数规则集中维护。
//! 颜色 token 复用 `TextInputTheme`，但公开枚举保持独立，避免单行和多行输入 API 强耦合。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{size, spacing, theme};

use super::props::{TextareaSize, TextareaStatus, TextareaVariant};

/// textarea 行数配置。
///
/// 行数规则需要同时参考默认行数、最小/最大行数和当前内容硬行数。
/// 将它们打包成独立结构，可以避免样式解析函数参数过长，也让后续扩展软换行预估时有清晰入口。
#[derive(Clone, Copy, Debug)]
pub struct TextareaRows {
    /// 默认可见行数。
    pub rows: usize,
    /// 最小可见行数。
    pub min_rows: Option<usize>,
    /// 最大可见行数。
    pub max_rows: Option<usize>,
    /// 当前内容的硬换行行数。
    pub content_rows: usize,
}

/// 多行输入渲染所需的完整样式。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedTextareaStyle {
    /// 输入框整体高度，包含上下内边距。
    pub height: Pixels,
    /// 可绘制文本视口高度，不包含上下内边距。
    pub viewport_height: Pixels,
    /// 内容区域水平内边距。
    pub padding_x: Pixels,
    /// 内容区域垂直内边距。
    pub padding_y: Pixels,
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
    /// 内部滚动条轨道颜色。
    pub scrollbar_track: Hsla,
    /// 内部滚动条滑块颜色。
    pub scrollbar_thumb: Hsla,
    /// 禁用态透明度。
    pub opacity: f32,
}

/// 根据尺寸、变体、状态、焦点、禁用态和内容行数解析 textarea 样式。
pub fn resolve_textarea_style(
    textarea_size: TextareaSize,
    variant: TextareaVariant,
    status: TextareaStatus,
    focused: bool,
    disabled: bool,
    rows: TextareaRows,
    cx: &App,
) -> ResolvedTextareaStyle {
    let theme = theme::text_input_theme(cx);
    let (padding_x, padding_y, font_size, line_height) = size_tokens(textarea_size);
    let semantic_color = semantic_color(status, focused, theme);
    let (background, border) = variant_tokens(variant, semantic_color, theme);
    let visible_rows = visible_rows(rows);
    let viewport_height = line_height * visible_rows as f32;

    ResolvedTextareaStyle {
        height: viewport_height + padding_y * 2.0,
        viewport_height,
        padding_x,
        padding_y,
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
        helper: if status == TextareaStatus::Default {
            theme.helper
        } else {
            semantic_color
        },
        selection: theme.selection,
        cursor: semantic_color,
        marked_underline: semantic_color,
        // 滚动条属于 textarea 的内部可视反馈，颜色使用输入框主题中的弱文本色派生，
        // 这样亮色和暗色皮肤都能保持足够低的视觉权重，不会压过输入内容和状态边框。
        scrollbar_track: theme.helper.opacity(0.16),
        scrollbar_thumb: theme.helper.opacity(0.56),
        opacity: if disabled { 0.58 } else { 1.0 },
    }
}

/// 解析尺寸相关 token。
fn size_tokens(textarea_size: TextareaSize) -> (Pixels, Pixels, Pixels, Pixels) {
    match textarea_size {
        TextareaSize::Small => (
            spacing::md(),
            spacing::sm(),
            size::text_sm(),
            size::line_sm(),
        ),
        TextareaSize::Medium => (
            spacing::lg(),
            spacing::md(),
            size::text_md(),
            size::line_md(),
        ),
        TextareaSize::Large => (
            spacing::lg(),
            spacing::md(),
            size::text_lg(),
            size::line_lg(),
        ),
    }
}

/// 解析当前可见行数。
///
/// `rows` 表达默认高度，`min_rows`/`max_rows` 是父组件的高度边界，`content_rows`
/// 让 textarea 在未达到最大行数前可以随硬换行自动增长。软换行高度在渲染阶段精确计算并通过内部滚动呈现，
/// 因为软换行必须依赖实际像素宽度，不能在样式层提前可靠得出。
fn visible_rows(rows: TextareaRows) -> usize {
    let min_rows = rows.min_rows.unwrap_or(1).max(1);
    let mut resolved = rows.rows.max(1).max(min_rows).max(rows.content_rows.max(1));
    if let Some(max_rows) = rows.max_rows {
        resolved = resolved.min(max_rows.max(1));
    }
    resolved
}

/// 解析状态色。
fn semantic_color(status: TextareaStatus, focused: bool, theme: theme::TextInputTheme) -> Hsla {
    match status {
        TextareaStatus::Default if focused => theme.focus,
        TextareaStatus::Default => theme.border,
        TextareaStatus::Error => theme.danger,
        TextareaStatus::Warning => theme.warning,
        TextareaStatus::Success => theme.success,
    }
}

/// 解析变体背景和边框。
fn variant_tokens(
    variant: TextareaVariant,
    semantic_color: Hsla,
    theme: theme::TextInputTheme,
) -> (Hsla, Hsla) {
    match variant {
        TextareaVariant::Outlined => (theme.background, semantic_color),
        TextareaVariant::Filled => (theme.filled_background, semantic_color),
        TextareaVariant::Ghost => (theme.background, theme.ghost_border),
    }
}
