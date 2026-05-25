//! 图标字体和标准图标渲染工具。
//!
//! xgpui 使用成熟的 Lucide 图标集作为内置图标来源。该模块负责把 Lucide 字体安装进
//! gpui 文本系统，并提供统一的图标文本元素构造函数，避免组件各自手写 SVG 或路径。

use std::borrow::Cow;

use gpui::{div, font, App, Hsla, ParentElement, Pixels, SharedString, Styled};

/// xgpui 使用的 Lucide 字体族名。
///
/// 该名称来自 `lucide-icons` crate 内置字体元数据；渲染图标文本时必须使用这个字体族，
/// 否则私有区 Unicode 码位会回退成系统字体中的空白方块。
pub const LUCIDE_FONT_FAMILY: &str = "lucide";

/// 对外复用的 Lucide 图标枚举。
///
/// 通过 re-export 固定 xgpui 内部使用的图标库入口，组件代码无需直接依赖具体 crate 路径。
pub use lucide_icons::Icon as LucideIcon;

/// 把 Lucide 字体安装到 gpui 文本系统。
///
/// 该函数由 `xgpui::install(cx)` 调用。字体数据来自 `lucide-icons` crate 的内置 TTF，
/// 不依赖系统是否已经安装 Lucide 字体，保证图标在不同机器上呈现一致。
pub fn install_icon_fonts(cx: &mut App) {
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(lucide_icons::LUCIDE_FONT_BYTES)])
        .expect("xgpui should load bundled lucide icon font");
}

/// 创建一个 Lucide 图标元素。
///
/// 返回值是普通 gpui `Div`，内部用 Lucide 字体渲染指定图标的 Unicode 字符。
/// 调用方负责把该元素放进按钮或其他交互容器中，这样可以统一控制点击区域和 hover 样式。
pub fn lucide_icon(icon: LucideIcon, color: Hsla, size: Pixels) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(size)
        .font(font(LUCIDE_FONT_FAMILY))
        .text_size(size)
        .line_height(size)
        .text_color(color)
        .child(SharedString::from(char::from(icon).to_string()))
}
