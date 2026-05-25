//! 颜色 token。
//!
//! 这些函数返回 `gpui::Hsla`，用于在组件样式中保持统一的基础色板。

use gpui::{rgb, rgba, Hsla};

/// 完全透明颜色，用于无背景或无边框但仍需要占位样式值的场景。
pub fn transparent() -> Hsla {
    rgba(0x0000_0000).into()
}

/// 页面或输入框的基础白色背景。
pub fn neutral_0() -> Hsla {
    rgb(0xffffff).into()
}

/// 浅灰背景，适合 filled 输入框或弱强调区域。
pub fn neutral_50() -> Hsla {
    rgb(0xf8fafc).into()
}

/// 浅层 hover 背景，适合下拉选项或列表项。
pub fn neutral_100() -> Hsla {
    rgb(0xf1f5f9).into()
}

/// 默认边框颜色，避免输入框边界过重。
pub fn neutral_200() -> Hsla {
    rgb(0xe2e8f0).into()
}

/// 禁用态边框或浅层分隔线颜色。
pub fn neutral_300() -> Hsla {
    rgb(0xcbd5e1).into()
}

/// placeholder 与弱辅助文本颜色。
pub fn neutral_400() -> Hsla {
    rgb(0x94a3b8).into()
}

/// 次级正文颜色，用于 helper text 和图标。
pub fn neutral_500() -> Hsla {
    rgb(0x64748b).into()
}

/// 主正文颜色。
pub fn neutral_900() -> Hsla {
    rgb(0x0f172a).into()
}

/// 主色，用于聚焦边框和光标。
pub fn primary_500() -> Hsla {
    rgb(0x2563eb).into()
}

/// 主色深一阶，用于主按钮 hover 背景。
pub fn primary_600() -> Hsla {
    rgb(0x1d4ed8).into()
}

/// 主色更深一阶，用于主按钮按下背景。
pub fn primary_700() -> Hsla {
    rgb(0x1e40af).into()
}

/// 主色浅背景，用于已选或高亮选项的弱强调底色。
pub fn primary_50() -> Hsla {
    rgb(0xeff6ff).into()
}

/// 主色次浅背景，用于键盘高亮选项。
pub fn primary_100() -> Hsla {
    rgb(0xdbeafe).into()
}

/// 错误色，用于错误状态边框和错误提示。
pub fn danger_500() -> Hsla {
    rgb(0xdc2626).into()
}

/// 错误色深一阶，用于危险主按钮 hover 背景。
pub fn danger_600() -> Hsla {
    rgb(0xb91c1c).into()
}

/// 错误色更深一阶，用于危险主按钮按下背景。
pub fn danger_700() -> Hsla {
    rgb(0x991b1b).into()
}

/// 危险色浅背景，用于危险幽灵按钮 hover 或弱强调背景。
pub fn danger_50() -> Hsla {
    rgb(0xfef2f2).into()
}

/// 警告色，用于警告状态边框和提示。
pub fn warning_500() -> Hsla {
    rgb(0xd97706).into()
}

/// 成功色，用于校验通过状态边框和提示。
pub fn success_500() -> Hsla {
    rgb(0x16a34a).into()
}

/// 文本选区背景色，透明度较低以避免盖住文字。
pub fn selection() -> Hsla {
    rgba(0x2563_eb33).into()
}

/// 清除按钮 hover 背景色。
pub fn clear_button_hover() -> Hsla {
    rgba(0x0f17_2a14).into()
}

/// 暗色皮肤的页面和输入框基础背景。
pub fn dark_background() -> Hsla {
    rgb(0x0f172a).into()
}

/// 暗色皮肤的 filled 输入框背景。
pub fn dark_filled_background() -> Hsla {
    rgb(0x1e293b).into()
}

/// 暗色皮肤的默认边框颜色。
pub fn dark_border() -> Hsla {
    rgb(0x334155).into()
}

/// 暗色皮肤的弱化边框颜色。
pub fn dark_muted_border() -> Hsla {
    rgb(0x1e293b).into()
}

/// 暗色皮肤的主文本颜色。
pub fn dark_text() -> Hsla {
    rgb(0xe2e8f0).into()
}

/// 暗色皮肤的 placeholder 颜色。
pub fn dark_placeholder() -> Hsla {
    rgb(0x64748b).into()
}

/// 暗色皮肤的辅助文本和图标颜色。
pub fn dark_helper() -> Hsla {
    rgb(0x94a3b8).into()
}

/// 暗色皮肤的禁用态文本颜色。
pub fn dark_disabled_text() -> Hsla {
    rgb(0x64748b).into()
}

/// 暗色皮肤的聚焦强调色。
pub fn dark_primary() -> Hsla {
    rgb(0x60a5fa).into()
}

/// 暗色皮肤主按钮背景色。
pub fn dark_primary_button() -> Hsla {
    rgb(0x2563eb).into()
}

/// 暗色皮肤主按钮 hover 背景色。
pub fn dark_primary_button_hover() -> Hsla {
    rgb(0x3b82f6).into()
}

/// 暗色皮肤主按钮按下背景色。
pub fn dark_primary_button_active() -> Hsla {
    rgb(0x1d4ed8).into()
}

/// 暗色皮肤的错误状态颜色。
pub fn dark_danger() -> Hsla {
    rgb(0xf87171).into()
}

/// 暗色皮肤危险主按钮背景色。
pub fn dark_danger_button() -> Hsla {
    rgb(0xdc2626).into()
}

/// 暗色皮肤危险主按钮 hover 背景色。
pub fn dark_danger_button_hover() -> Hsla {
    rgb(0xef4444).into()
}

/// 暗色皮肤危险主按钮按下背景色。
pub fn dark_danger_button_active() -> Hsla {
    rgb(0xb91c1c).into()
}

/// 暗色皮肤的警告状态颜色。
pub fn dark_warning() -> Hsla {
    rgb(0xfbbf24).into()
}

/// 暗色皮肤的成功状态颜色。
pub fn dark_success() -> Hsla {
    rgb(0x4ade80).into()
}

/// 暗色皮肤的文本选区背景色。
pub fn dark_selection() -> Hsla {
    rgba(0x60a5_fa40).into()
}

/// 暗色皮肤的清除按钮 hover 背景色。
pub fn dark_clear_button_hover() -> Hsla {
    rgba(0xe2e8_f029).into()
}

/// 暗色皮肤的下拉选项 hover 背景色。
pub fn dark_option_hover() -> Hsla {
    rgba(0xe2e8_f014).into()
}

/// 暗色皮肤的键盘高亮选项背景色。
pub fn dark_option_highlighted() -> Hsla {
    rgb(0x172554).into()
}

/// 暗色皮肤的已选选项背景色。
pub fn dark_option_selected() -> Hsla {
    rgb(0x1e3a8a).into()
}
