//! 圆角 token。
//!
//! 输入框、按钮等基础组件使用统一圆角，保证视觉语言一致。

use gpui::{px, Pixels};

/// 小型控件圆角。
pub fn sm() -> Pixels {
    px(4.0)
}

/// 默认控件圆角。
pub fn md() -> Pixels {
    px(6.0)
}

/// 胶囊形控件圆角，适合清除按钮等小型圆形目标。
pub fn full() -> Pixels {
    px(999.0)
}
