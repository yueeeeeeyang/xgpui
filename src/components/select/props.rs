//! `Select` 的公开参数类型。
//!
//! 本文件只定义组件 API、选项模型、尺寸、变体、状态和回调类型，不包含渲染或状态变更逻辑。
//! 这样可以让外部 API 保持清晰，也方便 `state.rs` 用普通单元测试验证行为。

use gpui::{px, Pixels, SharedString};

/// 选中值变化回调。
///
/// 回调参数为新的选中值；当用户清空选择时传入 `None`。组件只在用户交互导致值变化时触发，
/// 外部调用 `Select::set_value` 同步值不会触发该回调，避免受控组件形成回调循环。
pub type SelectChangeHandler = Box<dyn FnMut(Option<SharedString>)>;

/// 下拉面板打开状态变化回调。
///
/// 回调参数为新的打开状态。组件在用户点击、键盘动作或外部调用 `open` / `close` / `toggle`
/// 导致状态变化时触发。
pub type SelectOpenChangeHandler = Box<dyn FnMut(bool)>;

/// 搜索词变化回调。
///
/// 回调参数为当前搜索词。只有 `searchable = true` 且用户在打开的原选择框内输入搜索内容时才会触发。
pub type SelectSearchChangeHandler = Box<dyn FnMut(SharedString)>;

/// Select 选项。
///
/// `value` 是组件对外同步的稳定值，`label` 是当前第一版用于展示和本地搜索的文本。
/// 同一组选项中建议保持 `value` 唯一；如果出现重复值，组件会按第一个匹配项展示和选择，
/// 这一约定由状态层实现并在测试中覆盖。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectOption {
    /// 选项值，用于 `SelectProps::value`、`on_change` 和外部同步。
    pub value: SharedString,
    /// 选项展示文本，也用于本地搜索过滤。
    pub label: SharedString,
    /// 选项是否禁用。禁用选项会显示但不能被键盘高亮或鼠标选中。
    pub disabled: bool,
}

impl SelectOption {
    /// 创建一个可选中的 Select 选项。
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// 创建 `value` 和 `label` 相同的 Select 选项。
    ///
    /// 该辅助方法适合选项值就是展示文案的简单场景，避免示例和业务代码重复书写同一字符串。
    pub fn simple(value: impl Into<SharedString>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
            disabled: false,
        }
    }

    /// 设置选项禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Select 尺寸。
///
/// 尺寸会同时影响触发器高度、字号、行高、水平内边距和选项高度。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectSize {
    /// 小尺寸，适合表格筛选、工具栏或密集配置区域。
    Small,
    /// 默认尺寸，适合大多数表单项。
    Medium,
    /// 大尺寸，适合更强调的选择区域。
    Large,
}

impl Default for SelectSize {
    /// 返回默认尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// Select 视觉变体。
///
/// 变体只影响触发器背景和边框，不改变选择、搜索或键盘导航行为。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectVariant {
    /// 带边框的默认选择框。
    Outlined,
    /// 浅色填充选择框。
    Filled,
    /// 弱边界选择框，适合嵌入式工具区域。
    Ghost,
}

impl Default for SelectVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// Select 语义状态。
///
/// 状态只负责展示语义颜色，不执行内置校验逻辑；复杂校验应交给未来的表单组件或业务层。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectStatus {
    /// 默认状态。
    Default,
    /// 错误状态，通常配合错误 helper text 使用。
    Error,
    /// 警告状态，用于提示当前选择可能需要用户确认。
    Warning,
    /// 成功状态，用于展示校验通过或保存成功。
    Success,
}

impl Default for SelectStatus {
    /// 返回默认语义状态。
    fn default() -> Self {
        Self::Default
    }
}

/// `Select` 创建参数。
///
/// 第一版为单选本地 Select：选项展示只使用文本 `label`，搜索只在本地选项中执行大小写不敏感
/// 子串匹配。外部可以通过 `value` 初始化，也可以在实体创建后调用 `set_value` 做受控同步。
pub struct SelectProps {
    /// 初始选中值。传入 `None` 表示未选择。
    pub value: Option<SharedString>,
    /// 未选择时显示的占位文本。
    pub placeholder: SharedString,
    /// 下拉选项列表。建议每个选项的 `value` 唯一，重复值会按第一个匹配项处理。
    pub options: Vec<SelectOption>,
    /// 禁用状态。禁用后不能聚焦、打开、选择或清除。
    pub disabled: bool,
    /// 是否显示清除按钮。清除按钮只在有值、非禁用且 `clearable = true` 时显示。
    pub clearable: bool,
    /// 是否标记为必填。当前版本只保留语义和视觉标记，不执行内置校验。
    pub required: bool,
    /// 是否允许在原选择框内直接输入内容进行本地搜索过滤。
    pub searchable: bool,
    /// 搜索模式下原选择框为空时显示的占位文本。
    pub search_placeholder: SharedString,
    /// Select 尺寸。
    pub size: SelectSize,
    /// Select 视觉变体。
    pub variant: SelectVariant,
    /// Select 语义状态。
    pub status: SelectStatus,
    /// Select 下方辅助文本。
    pub helper_text: Option<SharedString>,
    /// 下拉面板最大高度，超过后内部滚动。
    pub max_popup_height: Pixels,
    /// 过滤结果为空或选项为空时展示的文本。
    pub empty_text: SharedString,
    /// 选中值变化回调。
    pub on_change: Option<SelectChangeHandler>,
    /// 打开状态变化回调。
    pub on_open_change: Option<SelectOpenChangeHandler>,
    /// 搜索词变化回调。
    pub on_search_change: Option<SelectSearchChangeHandler>,
}

impl Default for SelectProps {
    /// 返回默认 Select 参数。
    fn default() -> Self {
        Self {
            value: None,
            placeholder: SharedString::from("请选择"),
            options: Vec::new(),
            disabled: false,
            clearable: false,
            required: false,
            searchable: false,
            search_placeholder: SharedString::from("搜索选项"),
            size: SelectSize::default(),
            variant: SelectVariant::default(),
            status: SelectStatus::default(),
            helper_text: None,
            max_popup_height: px(240.0),
            empty_text: SharedString::from("暂无选项"),
            on_change: None,
            on_open_change: None,
            on_search_change: None,
        }
    }
}

impl SelectProps {
    /// 设置初始选中值。
    pub fn value(mut self, value: impl Into<Option<SharedString>>) -> Self {
        self.value = value.into();
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置选项列表。
    pub fn options(mut self, options: impl Into<Vec<SelectOption>>) -> Self {
        self.options = options.into();
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// 设置是否允许在原选择框内直接输入内容进行本地搜索。
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// 设置搜索模式下原选择框的占位文本。
    pub fn search_placeholder(mut self, search_placeholder: impl Into<SharedString>) -> Self {
        self.search_placeholder = search_placeholder.into();
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: SelectSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: SelectVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: SelectStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置辅助文本。
    pub fn helper_text(mut self, helper_text: impl Into<Option<SharedString>>) -> Self {
        self.helper_text = helper_text.into();
        self
    }

    /// 设置下拉面板最大高度。
    pub fn max_popup_height(mut self, max_popup_height: Pixels) -> Self {
        self.max_popup_height = max_popup_height;
        self
    }

    /// 设置空结果文本。
    pub fn empty_text(mut self, empty_text: impl Into<SharedString>) -> Self {
        self.empty_text = empty_text.into();
        self
    }

    /// 设置选中值变化回调。
    pub fn on_change(mut self, handler: impl FnMut(Option<SharedString>) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// 设置下拉面板打开状态变化回调。
    pub fn on_open_change(mut self, handler: impl FnMut(bool) + 'static) -> Self {
        self.on_open_change = Some(Box::new(handler));
        self
    }

    /// 设置搜索词变化回调。
    pub fn on_search_change(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_search_change = Some(Box::new(handler));
        self
    }
}
