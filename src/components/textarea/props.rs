//! `Textarea` 的公开参数类型。
//!
//! 该模块只定义组件输入、视觉枚举和回调类型，不包含任何编辑或渲染实现。
//! 这样可以让公开 API 与内部状态机解耦，后续扩展多行输入能力时不需要把调用方暴露在实现细节中。

use gpui::{Keystroke, SharedString};

/// 多行文本变化回调。
///
/// 回调参数是编辑后的完整文本。组件只在内容真实变化时触发该回调；
/// 光标移动、选区变化、滚动、焦点变化和外部 `set_value` 同步都不会触发它。
pub type TextareaChangeHandler = Box<dyn FnMut(SharedString)>;

/// 多行文本焦点变化回调。
///
/// 当前版本只暴露“发生了聚焦/失焦”的语义，不透传 gpui 窗口对象，
/// 避免调用方依赖组件内部焦点处理细节。
pub type TextareaFocusHandler = Box<dyn FnMut()>;

/// 多行文本提交回调。
///
/// 标准 textarea 的 `Enter` 用于输入换行，因此本组件用 `Cmd+Enter` / `Ctrl+Enter`
/// 表达提交语义。回调参数是触发提交时的完整文本。
pub type TextareaSubmitHandler = Box<dyn FnMut(SharedString)>;

/// 键盘按下回调。
///
/// 回调传出 gpui 的 `Keystroke`，适合父组件记录快捷键或实现额外的业务级键盘响应。
/// 该回调不参与内部编辑判定，组件仍会按自身规则处理换行、删除、移动等标准行为。
pub type TextareaKeyDownHandler = Box<dyn FnMut(Keystroke)>;

/// 多行输入框尺寸。
///
/// 尺寸会同时影响字号、行高、内边距和默认圆角映射；行数由 `rows/min_rows/max_rows` 控制。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextareaSize {
    /// 小尺寸，适合密集表单或设置面板中的备注输入。
    Small,
    /// 默认尺寸，适合大多数多行表单场景。
    Medium,
    /// 大尺寸，适合正文、说明和更强调的输入区域。
    Large,
}

impl Default for TextareaSize {
    /// 返回默认尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// 多行输入框视觉变体。
///
/// 变体只影响容器背景和边框，不改变文本编辑、滚动或平台输入行为。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextareaVariant {
    /// 带边框的默认输入框。
    Outlined,
    /// 浅色填充输入框。
    Filled,
    /// 弱边界输入框，适合嵌入式编辑区域。
    Ghost,
}

impl Default for TextareaVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// 多行输入框语义状态。
///
/// 状态只负责展示语义颜色，不执行内置校验逻辑。调用方应根据业务校验结果主动同步该状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextareaStatus {
    /// 默认状态。
    Default,
    /// 错误状态，通常配合错误 helper text 使用。
    Error,
    /// 警告状态，用于提示输入值可能需要用户确认。
    Warning,
    /// 成功状态，用于展示校验通过或保存成功。
    Success,
}

impl Default for TextareaStatus {
    /// 返回默认语义状态。
    fn default() -> Self {
        Self::Default
    }
}

/// `Textarea` 创建参数。
///
/// 参数结构包含初始值、展示属性、交互开关和回调。组件创建后可通过公开 `set_*`
/// 方法做受控同步；这些同步方法默认不触发用户变化回调，避免父组件写回状态时形成循环。
pub struct TextareaProps {
    /// 初始文本值，保留多行内容；`\r\n` 和裸 `\r` 会规范化为 `\n`。
    pub value: SharedString,
    /// 空值时显示在首行位置的占位文本。
    pub placeholder: SharedString,
    /// 禁用状态。禁用后不能聚焦、选择、复制或编辑。
    pub disabled: bool,
    /// 只读状态。只读允许聚焦、选择和复制，但不允许修改文本。
    pub readonly: bool,
    /// 是否标记为必填。当前版本只保留语义，不执行内置校验。
    pub required: bool,
    /// 最大输入长度，按 Unicode 字素簇计数，换行同样计入长度。
    pub max_length: Option<usize>,
    /// 默认可见行数，最小会规范化为 1。
    pub rows: usize,
    /// 最小可见行数。设置后会与 `rows` 一起决定初始高度下限。
    pub min_rows: Option<usize>,
    /// 最大可见行数。内容超过该行数后在内部滚动，不继续撑高组件。
    pub max_rows: Option<usize>,
    /// 输入框尺寸。
    pub size: TextareaSize,
    /// 输入框视觉变体。
    pub variant: TextareaVariant,
    /// 输入框语义状态。
    pub status: TextareaStatus,
    /// 输入框下方辅助文本。
    pub helper_text: Option<SharedString>,
    /// 内容变化回调。
    pub on_change: Option<TextareaChangeHandler>,
    /// 聚焦回调。
    pub on_focus: Option<TextareaFocusHandler>,
    /// 失焦回调。
    pub on_blur: Option<TextareaFocusHandler>,
    /// `Cmd+Enter` / `Ctrl+Enter` 提交回调。
    pub on_submit: Option<TextareaSubmitHandler>,
    /// 键盘按下回调。
    pub on_key_down: Option<TextareaKeyDownHandler>,
}

impl Default for TextareaProps {
    /// 返回默认多行输入参数。
    fn default() -> Self {
        Self {
            value: SharedString::default(),
            placeholder: SharedString::default(),
            disabled: false,
            readonly: false,
            required: false,
            max_length: None,
            rows: 3,
            min_rows: None,
            max_rows: None,
            size: TextareaSize::default(),
            variant: TextareaVariant::default(),
            status: TextareaStatus::default(),
            helper_text: None,
            on_change: None,
            on_focus: None,
            on_blur: None,
            on_submit: None,
            on_key_down: None,
        }
    }
}

impl TextareaProps {
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

    /// 设置默认可见行数。
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    /// 设置最小可见行数。
    pub fn min_rows(mut self, min_rows: impl Into<Option<usize>>) -> Self {
        self.min_rows = min_rows.into();
        self
    }

    /// 设置最大可见行数。
    pub fn max_rows(mut self, max_rows: impl Into<Option<usize>>) -> Self {
        self.max_rows = max_rows.into();
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: TextareaSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: TextareaVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: TextareaStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置辅助文本。
    pub fn helper_text(mut self, helper_text: impl Into<Option<SharedString>>) -> Self {
        self.helper_text = helper_text.into();
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

    /// 设置提交回调。
    pub fn on_submit(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    /// 设置键盘按下回调。
    pub fn on_key_down(mut self, handler: impl FnMut(Keystroke) + 'static) -> Self {
        self.on_key_down = Some(Box::new(handler));
        self
    }
}
