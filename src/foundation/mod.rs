//! 基础设计 token 模块。
//!
//! 这里放置跨组件共享的颜色、尺寸、圆角、间距和主题 token。
//! 组件应优先依赖这些基础能力，而不是在组件内部散落硬编码样式。

/// 颜色 token。
pub mod color;

/// 圆角 token。
pub mod radius;

/// 尺寸 token。
pub mod size;

/// 间距 token。
pub mod spacing;

/// 组件主题 token。
pub mod theme;

/// 图标字体安装和标准图标渲染工具。
pub mod icon;
