//! `TextInput` 的公开参数类型。
//!
//! 该文件只描述组件输入和回调，不包含渲染或编辑状态逻辑，便于后续保持 API 稳定。

use std::rc::Rc;

use gpui::{div, AnyElement, IntoElement, Keystroke, ParentElement, SharedString};

/// 文本变化回调。
///
/// 回调参数是编辑后的完整值。组件内部会在内容确实变化时触发该回调，
/// 光标移动、选区变化和焦点变化不会触发它。
pub type TextInputChangeHandler = Box<dyn FnMut(SharedString)>;

/// 焦点变化回调。
///
/// 当前版本只传递事件语义，不暴露底层窗口参数，避免调用方依赖内部实现细节。
pub type TextInputFocusHandler = Box<dyn FnMut()>;

/// 回车提交回调。
///
/// 回调参数是触发回车时的当前完整文本值，适合搜索框或简单表单提交场景。
pub type TextInputEnterHandler = Box<dyn FnMut(SharedString)>;

/// 键盘按下回调。
///
/// 回调传出 gpui 的 `Keystroke`，让调用方可以记录或响应特殊按键。
pub type TextInputKeyDownHandler = Box<dyn FnMut(Keystroke)>;

/// 输入框尺寸。
///
/// 尺寸会同时影响高度、字号、行高和水平内边距。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputSize {
    /// 小尺寸，适合紧凑表格、工具栏或密集设置面板。
    Small,
    /// 默认尺寸，适合大多数表单和设置项。
    Medium,
    /// 大尺寸，适合更强调的输入区域。
    Large,
}

impl Default for TextInputSize {
    /// 返回默认尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// 输入框视觉变体。
///
/// 变体只影响输入框容器的背景和边框，不改变文本编辑行为。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputVariant {
    /// 带边框的默认输入框。
    Outlined,
    /// 浅色填充输入框。
    Filled,
    /// 弱边界输入框，适合嵌入式工具区域。
    Ghost,
}

impl Default for TextInputVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// 输入框语义状态。
///
/// 状态只负责展示语义颜色，不执行内置校验逻辑。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputStatus {
    /// 默认状态。
    Default,
    /// 错误状态，通常配合错误 helper text 使用。
    Error,
    /// 警告状态，用于提示输入值可能需要用户确认。
    Warning,
    /// 成功状态，用于展示校验通过或保存成功。
    Success,
}

impl Default for TextInputStatus {
    /// 返回默认语义状态。
    fn default() -> Self {
        Self::Default
    }
}

/// 输入框内容类型。
///
/// 类型用于描述当前输入框的基础输入语义。组件仍然把值保存为字符串，避免数字中间态
/// 例如 `-`、`.` 或 `1.` 被过早解析成数值后丢失用户正在输入的内容。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputType {
    /// 普通单行文本，不做额外字符限制。
    Text,
    /// 密码文本，真实值照常保存，渲染时默认显示为掩码字符。
    Password,
    /// 数字形态文本，只允许空值、负号、小数点、数字以及它们组成的合法中间态。
    Number,
}

impl Default for TextInputType {
    /// 返回默认输入类型。
    fn default() -> Self {
        Self::Text
    }
}

/// `TextInput` 前后缀插槽。
///
/// gpui 的 element 通常是一次性构建值，因此插槽保存的是可重复调用的渲染闭包。
/// 每次组件渲染时闭包都会生成新的 `AnyElement`，避免复用旧 element 导致生命周期问题。
#[derive(Clone)]
pub struct TextInputSlot {
    renderer: Rc<dyn Fn() -> AnyElement>,
}

impl TextInputSlot {
    /// 使用自定义渲染闭包创建插槽。
    pub fn new(renderer: impl Fn() -> AnyElement + 'static) -> Self {
        Self {
            renderer: Rc::new(renderer),
        }
    }

    /// 创建简单文本插槽。
    ///
    /// 这个辅助方法适合单位、固定标签或简单图标字符等轻量前后缀内容。
    pub fn text(text: impl Into<SharedString>) -> Self {
        let text = text.into();
        Self::new(move || div().child(text.clone()).into_any_element())
    }

    /// 渲染插槽内容。
    pub fn render(&self) -> AnyElement {
        (self.renderer)()
    }
}

/// `TextInput` 创建参数。
///
/// 参数结构既包含展示属性，也包含事件回调。组件创建后仍可通过公开方法修改值；
/// 如果需要完全由外部同步，可在 `on_change` 中更新外部状态，再调用 `set_value`。
pub struct TextInputProps {
    /// 初始输入值。
    pub value: SharedString,
    /// 空值时显示的占位文本。
    pub placeholder: SharedString,
    /// 禁用状态。禁用后不能聚焦、选择或编辑。
    pub disabled: bool,
    /// 只读状态。只读后可以聚焦、选择和复制，但不能修改内容。
    pub readonly: bool,
    /// 是否显示清除按钮。
    pub clearable: bool,
    /// 是否标记为必填。当前版本只保留语义，不执行内置校验。
    pub required: bool,
    /// 最大输入长度，按 Unicode 字素簇计数。
    pub max_length: Option<usize>,
    /// 输入框尺寸。
    pub size: TextInputSize,
    /// 输入框视觉变体。
    pub variant: TextInputVariant,
    /// 输入框语义状态。
    pub status: TextInputStatus,
    /// 输入框内容类型。
    pub input_type: TextInputType,
    /// 输入框下方辅助文本。
    pub helper_text: Option<SharedString>,
    /// 输入框前缀插槽。
    pub prefix: Option<TextInputSlot>,
    /// 输入框后缀插槽。
    pub suffix: Option<TextInputSlot>,
    /// 内容变化回调。
    pub on_change: Option<TextInputChangeHandler>,
    /// 聚焦回调。
    pub on_focus: Option<TextInputFocusHandler>,
    /// 失焦回调。
    pub on_blur: Option<TextInputFocusHandler>,
    /// 回车回调。
    pub on_enter: Option<TextInputEnterHandler>,
    /// 键盘按下回调。
    pub on_key_down: Option<TextInputKeyDownHandler>,
}

impl Default for TextInputProps {
    /// 返回默认输入框参数。
    fn default() -> Self {
        Self {
            value: SharedString::default(),
            placeholder: SharedString::default(),
            disabled: false,
            readonly: false,
            clearable: false,
            required: false,
            max_length: None,
            size: TextInputSize::default(),
            variant: TextInputVariant::default(),
            status: TextInputStatus::default(),
            input_type: TextInputType::default(),
            helper_text: None,
            prefix: None,
            suffix: None,
            on_change: None,
            on_focus: None,
            on_blur: None,
            on_enter: None,
            on_key_down: None,
        }
    }
}

impl TextInputProps {
    /// 设置初始值。
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置只读状态。
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// 设置是否显示清除按钮。
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    /// 设置是否必填。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置最大长度。
    pub fn max_length(mut self, max_length: impl Into<Option<usize>>) -> Self {
        self.max_length = max_length.into();
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: TextInputSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: TextInputVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: TextInputStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置输入框内容类型。
    pub fn input_type(mut self, input_type: TextInputType) -> Self {
        self.input_type = input_type;
        self
    }

    /// 设置辅助文本。
    pub fn helper_text(mut self, helper_text: impl Into<Option<SharedString>>) -> Self {
        self.helper_text = helper_text.into();
        self
    }

    /// 设置前缀插槽。
    pub fn prefix(mut self, prefix: impl Into<Option<TextInputSlot>>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// 设置后缀插槽。
    pub fn suffix(mut self, suffix: impl Into<Option<TextInputSlot>>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// 设置内容变化回调。
    pub fn on_change(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// 设置聚焦回调。
    pub fn on_focus(mut self, handler: impl FnMut() + 'static) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }

    /// 设置失焦回调。
    pub fn on_blur(mut self, handler: impl FnMut() + 'static) -> Self {
        self.on_blur = Some(Box::new(handler));
        self
    }

    /// 设置回车回调。
    pub fn on_enter(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_enter = Some(Box::new(handler));
        self
    }

    /// 设置键盘按下回调。
    pub fn on_key_down(mut self, handler: impl FnMut(Keystroke) + 'static) -> Self {
        self.on_key_down = Some(Box::new(handler));
        self
    }
}
