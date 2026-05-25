//! `Button` 样式解析。
//!
//! 组件渲染层只消费这里产出的解析结果，避免尺寸、变体、色调、禁用和聚焦规则散落在事件代码中。

use gpui::{App, Hsla, Pixels};

use crate::foundation::{color, size, spacing, theme};

use super::props::{ButtonSize, ButtonTone, ButtonVariant};

/// Button 渲染所需的完整样式。
#[derive(Clone, Copy, Debug)]
pub struct ResolvedButtonStyle {
    /// 按钮高度。
    pub height: Pixels,
    /// 按钮水平内边距。
    pub padding_x: Pixels,
    /// 非纯图标按钮的最小宽度。
    pub min_width: Pixels,
    /// 纯图标按钮的正方形边长。
    pub icon_only_size: Pixels,
    /// 文本和图标之间的间距。
    pub gap: Pixels,
    /// 图标尺寸。
    pub icon_size: Pixels,
    /// 文本字号。
    pub font_size: Pixels,
    /// 文本行高。
    pub line_height: Pixels,
    /// 按钮圆角。
    pub radius: Pixels,
    /// 默认背景色。
    pub background: Hsla,
    /// 默认边框色。
    pub border: Hsla,
    /// 默认文本和图标颜色。
    pub text: Hsla,
    /// hover 背景色。
    pub hover_background: Hsla,
    /// active 背景色。
    pub active_background: Hsla,
    /// 禁用或加载状态下的透明度。
    pub opacity: f32,
    /// 文本是否需要下划线。当前只给 Link 变体使用。
    pub underline: bool,
}

/// 根据按钮状态解析最终样式。
///
/// `focused` 只影响边框颜色，不改变按钮尺寸；这样按钮获得或失去焦点时不会造成布局跳动。
pub fn resolve_button_style(
    button_size: ButtonSize,
    variant: ButtonVariant,
    tone: ButtonTone,
    focused: bool,
    disabled: bool,
    loading: bool,
    cx: &App,
) -> ResolvedButtonStyle {
    resolve_button_style_with_theme(
        button_size,
        variant,
        tone,
        focused,
        disabled,
        loading,
        theme::button_theme(cx),
    )
}

/// 使用显式主题解析按钮样式。
///
/// 该函数把 `App` 依赖隔离在外层，方便单元测试直接传入构造好的主题 token，
/// 也让变体映射规则可以独立验证。
pub(crate) fn resolve_button_style_with_theme(
    button_size: ButtonSize,
    variant: ButtonVariant,
    tone: ButtonTone,
    focused: bool,
    disabled: bool,
    loading: bool,
    button_theme: theme::ButtonTheme,
) -> ResolvedButtonStyle {
    let size_tokens = size_tokens(button_size);
    let chrome = resolve_chrome(variant, tone, button_theme);
    let border = if focused && !disabled {
        button_theme.focus
    } else {
        chrome.border
    };

    ResolvedButtonStyle {
        height: size_tokens.height,
        padding_x: size_tokens.padding_x,
        min_width: size_tokens.min_width,
        icon_only_size: size_tokens.icon_only_size,
        gap: button_theme.gap,
        icon_size: size_tokens.icon_size,
        font_size: size_tokens.font_size,
        line_height: size_tokens.line_height,
        radius: button_theme.radius,
        background: if disabled {
            button_theme.disabled_background
        } else {
            chrome.background
        },
        border: if disabled {
            button_theme.disabled_border
        } else {
            border
        },
        text: if disabled {
            button_theme.disabled_text
        } else {
            chrome.text
        },
        hover_background: if disabled || loading {
            chrome.background
        } else {
            chrome.hover_background
        },
        active_background: if disabled || loading {
            chrome.background
        } else {
            chrome.active_background
        },
        opacity: if disabled {
            0.58
        } else if loading {
            0.82
        } else {
            1.0
        },
        underline: chrome.underline,
    }
}

/// 解析尺寸 token。
pub(crate) fn size_tokens(button_size: ButtonSize) -> ButtonSizeTokens {
    match button_size {
        ButtonSize::Small => ButtonSizeTokens {
            height: size::input_sm_height(),
            padding_x: spacing::md(),
            min_width: gpui::px(64.0),
            icon_only_size: size::input_sm_height(),
            icon_size: gpui::px(14.0),
            font_size: size::text_sm(),
            line_height: size::line_sm(),
        },
        ButtonSize::Medium => ButtonSizeTokens {
            height: size::input_md_height(),
            padding_x: spacing::lg(),
            min_width: gpui::px(80.0),
            icon_only_size: size::input_md_height(),
            icon_size: gpui::px(16.0),
            font_size: size::text_md(),
            line_height: size::line_md(),
        },
        ButtonSize::Large => ButtonSizeTokens {
            height: size::input_lg_height(),
            padding_x: spacing::lg(),
            min_width: gpui::px(96.0),
            icon_only_size: size::input_lg_height(),
            icon_size: gpui::px(18.0),
            font_size: size::text_lg(),
            line_height: size::line_lg(),
        },
    }
}

/// 按钮尺寸解析后的基础布局 token。
#[derive(Clone, Copy, Debug)]
pub(crate) struct ButtonSizeTokens {
    /// 按钮高度。
    pub height: Pixels,
    /// 按钮水平内边距。
    pub padding_x: Pixels,
    /// 普通按钮最小宽度。
    pub min_width: Pixels,
    /// 纯图标按钮边长。
    pub icon_only_size: Pixels,
    /// 图标尺寸。
    pub icon_size: Pixels,
    /// 文本字号。
    pub font_size: Pixels,
    /// 文本行高。
    pub line_height: Pixels,
}

/// 按钮变体和色调解析后的颜色规则。
#[derive(Clone, Copy, Debug)]
struct ButtonChrome {
    /// 默认背景色。
    background: Hsla,
    /// 默认边框色。
    border: Hsla,
    /// 文本和图标颜色。
    text: Hsla,
    /// hover 背景色。
    hover_background: Hsla,
    /// active 背景色。
    active_background: Hsla,
    /// 是否给文案加下划线。
    underline: bool,
}

/// 把变体和色调映射成具体颜色。
///
/// 危险色调在非 Primary 变体下只改变文字和 hover 背景，保持按钮层级与变体语义一致。
fn resolve_chrome(
    variant: ButtonVariant,
    tone: ButtonTone,
    button_theme: theme::ButtonTheme,
) -> ButtonChrome {
    let danger = tone == ButtonTone::Danger;
    let solid_background = if danger {
        button_theme.danger_background
    } else {
        button_theme.primary_background
    };
    let solid_hover = if danger {
        button_theme.danger_hover_background
    } else {
        button_theme.primary_hover_background
    };
    let solid_active = if danger {
        button_theme.danger_active_background
    } else {
        button_theme.primary_active_background
    };
    let solid_text = if danger {
        button_theme.danger_text
    } else {
        button_theme.primary_text
    };
    let subtle_text = if danger {
        button_theme.danger_background
    } else {
        button_theme.text
    };
    let ghost_hover = if danger {
        button_theme.danger_ghost_hover_background
    } else {
        button_theme.ghost_hover_background
    };
    let ghost_active = if danger {
        button_theme.danger_ghost_active_background
    } else {
        button_theme.ghost_active_background
    };

    match variant {
        ButtonVariant::Primary => ButtonChrome {
            background: solid_background,
            border: solid_background,
            text: solid_text,
            hover_background: solid_hover,
            active_background: solid_active,
            underline: false,
        },
        ButtonVariant::Secondary => ButtonChrome {
            background: if danger {
                ghost_hover
            } else {
                button_theme.secondary_background
            },
            border: color::transparent(),
            text: subtle_text,
            hover_background: if danger {
                ghost_active
            } else {
                button_theme.secondary_hover_background
            },
            active_background: if danger {
                ghost_active
            } else {
                button_theme.secondary_active_background
            },
            underline: false,
        },
        ButtonVariant::Outline => ButtonChrome {
            background: color::transparent(),
            border: if danger {
                button_theme.danger_background
            } else {
                button_theme.border
            },
            text: subtle_text,
            hover_background: ghost_hover,
            active_background: ghost_active,
            underline: false,
        },
        ButtonVariant::Ghost => ButtonChrome {
            background: color::transparent(),
            border: color::transparent(),
            text: subtle_text,
            hover_background: ghost_hover,
            active_background: ghost_active,
            underline: false,
        },
        ButtonVariant::Link => ButtonChrome {
            background: color::transparent(),
            border: color::transparent(),
            text: subtle_text,
            hover_background: color::transparent(),
            active_background: ghost_hover,
            underline: true,
        },
    }
}

/// 判断按钮当前是否允许触发业务点击。
///
/// 该规则由鼠标和键盘共用，避免两个交互入口出现不一致。
pub(crate) fn can_trigger(disabled: bool, loading: bool) -> bool {
    !disabled && !loading
}
