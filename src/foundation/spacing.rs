//! 间距 token。
//!
//! 通过统一间距函数避免不同组件出现不可控的 padding 和 gap 差异。

use gpui::{px, Pixels};

/// 极小间距。
pub fn xs() -> Pixels {
    px(4.0)
}

/// 小间距。
pub fn sm() -> Pixels {
    px(6.0)
}

/// 默认间距。
pub fn md() -> Pixels {
    px(8.0)
}

/// 大间距。
pub fn lg() -> Pixels {
    px(12.0)
}
