//! `Button` 的公开参数类型。
//!
//! 本文件只定义按钮组件的公共 API、视觉枚举和回调类型，不包含渲染细节。
//! 这样可以让调用方依赖的类型边界稳定，也便于后续扩展状态或样式时保持职责清晰。

use gpui::SharedString;

use crate::foundation::icon::LucideIcon;

/// 按钮点击回调。
///
/// 回调只表达“按钮已经被用户有效触发”这一业务语义，不暴露底层鼠标或键盘事件。
/// 组件会在 `disabled` 或 `loading` 时拦截触发，避免调用方重复判断不可交互状态。
pub type ButtonClickHandler = Box<dyn FnMut()>;

/// Button 尺寸。
///
/// 尺寸会同时影响按钮高度、水平内边距、字号、行高、图标尺寸和纯图标按钮边长。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonSize {
    /// 小尺寸，适合工具栏、表格行内操作或密集设置面板。
    Small,
    /// 默认尺寸，适合表单操作和常规页面按钮。
    Medium,
    /// 大尺寸，适合更强调的主要操作区域。
    Large,
}

impl Default for ButtonSize {
    /// 返回默认按钮尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// Button 视觉变体。
///
/// 变体只影响按钮背景、边框、阴影和文字颜色，不改变点击、键盘或焦点行为。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonVariant {
    /// 主按钮，使用高强调填充背景，适合页面主操作。
    Primary,
    /// 次级按钮，使用中性填充背景，适合同层级的普通操作。
    Secondary,
    /// 描边按钮，使用透明背景和可见边框，适合弱化但仍需边界的操作。
    Outline,
    /// 幽灵按钮，默认无明显边界，hover 时展示弱背景，适合工具型操作。
    Ghost,
    /// 链接按钮，强调文本操作语义，默认不占用明显按钮背景。
    Link,
}

impl Default for ButtonVariant {
    /// 返回默认按钮视觉变体。
    fn default() -> Self {
        Self::Primary
    }
}

/// Button 语义色调。
///
/// 色调描述按钮操作的业务语义。当前第一版只区分默认操作和危险操作，
/// 后续如果需要成功、警告等色调，应继续从主题 token 扩展，而不是在组件里写死颜色。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonTone {
    /// 默认操作色调。
    Default,
    /// 危险操作色调，例如删除、移除或不可逆提交。
    Danger,
}

impl Default for ButtonTone {
    /// 返回默认按钮色调。
    fn default() -> Self {
        Self::Default
    }
}

/// `Button` 创建参数。
///
/// Button 第一版提供常见按钮能力：变体、色调、尺寸、禁用、加载、块级宽度、前后 Lucide 图标、
/// 纯图标按钮和点击回调。组件使用 Entity 形式创建，便于后续在内部维护更复杂的交互状态。
pub struct ButtonProps {
    /// 按钮文案。纯图标按钮不会渲染该文案，但会把它作为 tooltip 默认内容。
    pub label: SharedString,
    /// 视觉变体。
    pub variant: ButtonVariant,
    /// 语义色调。
    pub tone: ButtonTone,
    /// 按钮尺寸。
    pub size: ButtonSize,
    /// 禁用状态。禁用后不可聚焦、不可点击，也不会触发回调。
    pub disabled: bool,
    /// 加载状态。加载时保持按钮可见但禁止触发，用圆圈 loading 动画提示操作处理中。
    pub loading: bool,
    /// 是否占满父容器宽度。
    pub block: bool,
    /// 是否渲染为纯图标按钮。纯图标按钮会使用正方形尺寸并隐藏 label 文本。
    pub icon_only: bool,
    /// 前置 Lucide 图标。`loading = true` 时会被 loading 圆圈动画临时替代。
    pub leading_icon: Option<LucideIcon>,
    /// 后置 Lucide 图标。纯图标按钮只会使用第一个可用图标，避免一个小按钮内出现多个图标。
    pub trailing_icon: Option<LucideIcon>,
    /// hover tooltip 文案。未设置且 `icon_only = true` 时会回退使用 `label`。
    pub tooltip: Option<SharedString>,
    /// 有效点击回调。
    pub on_click: Option<ButtonClickHandler>,
}

impl Default for ButtonProps {
    /// 返回默认按钮参数。
    fn default() -> Self {
        Self {
            label: SharedString::from("按钮"),
            variant: ButtonVariant::default(),
            tone: ButtonTone::default(),
            size: ButtonSize::default(),
            disabled: false,
            loading: false,
            block: false,
            icon_only: false,
            leading_icon: None,
            trailing_icon: None,
            tooltip: None,
            on_click: None,
        }
    }
}

impl ButtonProps {
    /// 设置按钮文案。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// 设置按钮视觉变体。
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置按钮语义色调。
    pub fn tone(mut self, tone: ButtonTone) -> Self {
        self.tone = tone;
        self
    }

    /// 设置按钮尺寸。
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置加载状态。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// 设置是否占满父容器宽度。
    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }

    /// 设置是否使用纯图标按钮布局。
    pub fn icon_only(mut self, icon_only: bool) -> Self {
        self.icon_only = icon_only;
        self
    }

    /// 设置前置 Lucide 图标。
    pub fn leading_icon(mut self, icon: LucideIcon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// 设置后置 Lucide 图标。
    pub fn trailing_icon(mut self, icon: LucideIcon) -> Self {
        self.trailing_icon = Some(icon);
        self
    }

    /// 设置 tooltip 文案。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 设置有效点击回调。
    pub fn on_click(mut self, handler: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}
