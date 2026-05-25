//! 尺寸 token。
//!
//! 控件高度和字号集中定义，组件通过枚举映射到这些 token。

use gpui::{px, Pixels};

/// 小尺寸输入框高度。
pub fn input_sm_height() -> Pixels {
    px(28.0)
}

/// 中尺寸输入框高度。
pub fn input_md_height() -> Pixels {
    px(34.0)
}

/// 大尺寸输入框高度。
pub fn input_lg_height() -> Pixels {
    px(40.0)
}

/// 小尺寸文本字号。
pub fn text_sm() -> Pixels {
    px(13.0)
}

/// 中尺寸文本字号。
pub fn text_md() -> Pixels {
    px(14.0)
}

/// 大尺寸文本字号。
pub fn text_lg() -> Pixels {
    px(16.0)
}

/// 小尺寸文本行高。
pub fn line_sm() -> Pixels {
    px(18.0)
}

/// 中尺寸文本行高。
pub fn line_md() -> Pixels {
    px(20.0)
}

/// 大尺寸文本行高。
pub fn line_lg() -> Pixels {
    px(22.0)
}
