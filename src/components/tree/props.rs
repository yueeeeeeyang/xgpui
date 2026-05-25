//! `Tree` 的公开参数类型。
//!
//! 本模块只定义树组件的外部 API、节点模型、视觉枚举和回调类型。树的展开、
//! 选择、复选级联和过滤规则都放在 `state.rs`，避免渲染层和调用方直接依赖内部状态结构。

use gpui::{Keystroke, Pixels, SharedString};

use crate::foundation::icon::LucideIcon;

/// 展开节点集合变化回调。
///
/// 回调只在用户点击展开按钮或使用键盘展开/折叠时触发。外部调用 `Tree::set_expanded_keys`
/// 属于受控同步，不会触发该回调，避免父组件写回状态时形成循环。
pub type TreeExpandHandler = Box<dyn FnMut(Vec<SharedString>)>;

/// 选中节点集合变化回调。
///
/// 选中状态表示当前行选择，和复选框 checked 状态互相独立。回调参数已经按当前树的 DFS 顺序
/// 规范化，便于调用方直接保存为受控状态。
pub type TreeSelectHandler = Box<dyn FnMut(Vec<SharedString>)>;

/// 复选状态变化回调。
///
/// 第一个参数是完全选中的节点 key，第二个参数是半选节点 key。两个列表都按当前 DFS 顺序输出，
/// 并且只包含当前节点树中可检查的节点。
pub type TreeCheckHandler = Box<dyn FnMut(Vec<SharedString>, Vec<SharedString>)>;

/// 过滤文本变化回调。
///
/// 第一版 Tree 不内置搜索输入框；调用方可以用 `TextInput` 管理输入，再通过 `set_filter_text`
/// 同步。该回调仅保留给未来内部过滤输入或键盘输入扩展使用。
pub type TreeFilterChangeHandler = Box<dyn FnMut(SharedString)>;

/// 树组件焦点变化回调。
pub type TreeFocusHandler = Box<dyn FnMut()>;

/// 树组件键盘按下回调。
///
/// 回调传出 gpui 的 `Keystroke`，适合父组件记录快捷键或实现额外业务响应。
/// Tree 内部仍会继续按自身规则处理方向键、Enter、Space 和全选。
pub type TreeKeyDownHandler = Box<dyn FnMut(Keystroke)>;

/// 树节点数据。
///
/// `key` 是节点的稳定身份，要求在同一棵树内唯一。状态层会保留第一次出现的 key，
/// 并忽略后续重复节点及其子树，避免重复 key 导致展开、选择和复选状态不可预测。
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// 节点稳定 key，用于展开、选择、复选和外部同步。
    pub key: SharedString,
    /// 节点展示文本，也用于默认本地过滤。
    pub label: SharedString,
    /// 可选 Lucide 图标。第一版不开放任意节点 renderer，以保持布局和性能边界稳定。
    pub icon: Option<LucideIcon>,
    /// 子节点列表。嵌套结构更贴近 Tree 组件心智，内部状态层会扁平化为 DFS 记录。
    pub children: Vec<TreeNode>,
    /// 节点是否禁用。禁用节点不能被选中、复选或作为键盘操作目标，但仍可展示其子节点。
    pub disabled: bool,
    /// 节点是否允许行选中。该开关只影响 selected 状态，不影响复选框 checked 状态。
    pub selectable: bool,
    /// 节点是否允许显示并参与复选框级联。全局 `TreeProps::checkable = false` 时该字段被忽略。
    pub checkable: bool,
}

impl TreeNode {
    /// 创建一个默认可选、可检查的树节点。
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            children: Vec::new(),
            disabled: false,
            selectable: true,
            checkable: true,
        }
    }

    /// 设置节点图标。
    pub fn icon(mut self, icon: LucideIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设置子节点。
    pub fn children(mut self, children: impl Into<Vec<TreeNode>>) -> Self {
        self.children = children.into();
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置是否允许行选中。
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// 设置是否允许复选。
    pub fn checkable(mut self, checkable: bool) -> Self {
        self.checkable = checkable;
        self
    }
}

/// Tree 尺寸。
///
/// 尺寸会同时影响行高、字号、缩进步长和图标尺寸；虚拟列表要求每行高度固定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeSize {
    /// 小尺寸，适合密集导航或设置侧栏。
    Small,
    /// 默认尺寸，适合大多数表单和面板树。
    Medium,
    /// 大尺寸，适合需要更高可读性的资源树。
    Large,
}

impl Default for TreeSize {
    /// 返回默认 Tree 尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// Tree 视觉变体。
///
/// 变体只影响容器背景和边框，不改变展开、选择、复选或键盘导航规则。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeVariant {
    /// 带边框的默认树容器。
    Outlined,
    /// 浅色填充树容器。
    Filled,
    /// 弱边界树容器，适合嵌入式侧栏或面板。
    Ghost,
}

impl Default for TreeVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// Tree 语义状态。
///
/// 状态只负责展示语义颜色，不执行内置校验逻辑。调用方应根据业务校验结果主动同步该状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeStatus {
    /// 默认状态。
    Default,
    /// 错误状态，通常配合错误 helper text 使用。
    Error,
    /// 警告状态，用于提示当前选择可能需要用户确认。
    Warning,
    /// 成功状态，用于展示校验通过或保存成功。
    Success,
}

impl Default for TreeStatus {
    /// 返回默认语义状态。
    fn default() -> Self {
        Self::Default
    }
}

/// Tree 行选中模式。
///
/// 行选中用于表达当前业务选择，和复选框 checked 状态互相独立。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeSelectionMode {
    /// 不维护 selected 状态，点击行只更新键盘活动项。
    None,
    /// 单选模式，点击可选节点会替换当前 selected key。
    Single,
    /// 多选模式，支持普通点击替换、Cmd/Ctrl 点击切换和 Shift 范围选择。
    Multiple,
}

impl Default for TreeSelectionMode {
    /// 默认使用单选，贴近导航树和大多数资源树场景。
    fn default() -> Self {
        Self::Single
    }
}

/// Tree 复选框状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeCheckState {
    /// 未选中。
    Unchecked,
    /// 已选中。
    Checked,
    /// 子孙节点部分选中。
    Indeterminate,
}

impl Default for TreeCheckState {
    /// 返回默认未选中状态。
    fn default() -> Self {
        Self::Unchecked
    }
}

/// `Tree` 创建参数。
///
/// 参数结构包含初始节点、展开/选择/复选/过滤状态、交互开关和回调。组件创建后可通过
/// 公开 `set_*` 方法做受控同步；这些同步方法默认不触发用户变化回调。
pub struct TreeProps {
    /// 初始节点树。节点 key 要求在同一棵树内唯一。
    pub nodes: Vec<TreeNode>,
    /// 初始展开节点 key。只有有子节点的 key 会影响可见行。
    pub expanded_keys: Vec<SharedString>,
    /// 初始选中节点 key。具体语义由 `selection_mode` 决定。
    pub selected_keys: Vec<SharedString>,
    /// 初始完全选中的节点 key，会按级联复选规则规范化。
    pub checked_keys: Vec<SharedString>,
    /// 初始过滤文本。非空时按 label 大小写不敏感匹配并显示祖先路径。
    pub filter_text: SharedString,
    /// 行选中模式。
    pub selection_mode: TreeSelectionMode,
    /// 是否显示复选框并启用 checked 状态。
    pub checkable: bool,
    /// 禁用状态。禁用后不能聚焦、展开、选择、复选或键盘导航。
    pub disabled: bool,
    /// 是否标记为必填。当前版本只保留语义和视觉标记，不执行内置校验。
    pub required: bool,
    /// Tree 尺寸。
    pub size: TreeSize,
    /// Tree 视觉变体。
    pub variant: TreeVariant,
    /// Tree 语义状态。
    pub status: TreeStatus,
    /// Tree 下方辅助文本。
    pub helper_text: Option<SharedString>,
    /// 无可见节点时展示的文本。
    pub empty_text: SharedString,
    /// Tree 视口最大高度，超过后内部虚拟列表滚动。
    pub max_height: Pixels,
    /// 展开 key 变化回调。
    pub on_expand: Option<TreeExpandHandler>,
    /// selected key 变化回调。
    pub on_select: Option<TreeSelectHandler>,
    /// checked / half checked key 变化回调。
    pub on_check: Option<TreeCheckHandler>,
    /// 过滤文本变化回调。
    pub on_filter_change: Option<TreeFilterChangeHandler>,
    /// 聚焦回调。
    pub on_focus: Option<TreeFocusHandler>,
    /// 失焦回调。
    pub on_blur: Option<TreeFocusHandler>,
    /// 键盘按下回调。
    pub on_key_down: Option<TreeKeyDownHandler>,
}

impl Default for TreeProps {
    /// 返回默认 Tree 参数。
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            expanded_keys: Vec::new(),
            selected_keys: Vec::new(),
            checked_keys: Vec::new(),
            filter_text: SharedString::default(),
            selection_mode: TreeSelectionMode::default(),
            checkable: false,
            disabled: false,
            required: false,
            size: TreeSize::default(),
            variant: TreeVariant::default(),
            status: TreeStatus::default(),
            helper_text: None,
            empty_text: SharedString::from("暂无节点"),
            max_height: gpui::px(320.0),
            on_expand: None,
            on_select: None,
            on_check: None,
            on_filter_change: None,
            on_focus: None,
            on_blur: None,
            on_key_down: None,
        }
    }
}

impl TreeProps {
    /// 设置节点树。
    pub fn nodes(mut self, nodes: impl Into<Vec<TreeNode>>) -> Self {
        self.nodes = nodes.into();
        self
    }

    /// 设置初始展开 key。
    pub fn expanded_keys(mut self, expanded_keys: impl Into<Vec<SharedString>>) -> Self {
        self.expanded_keys = expanded_keys.into();
        self
    }

    /// 设置初始 selected key。
    pub fn selected_keys(mut self, selected_keys: impl Into<Vec<SharedString>>) -> Self {
        self.selected_keys = selected_keys.into();
        self
    }

    /// 设置初始 checked key。
    pub fn checked_keys(mut self, checked_keys: impl Into<Vec<SharedString>>) -> Self {
        self.checked_keys = checked_keys.into();
        self
    }

    /// 设置初始过滤文本。
    pub fn filter_text(mut self, filter_text: impl Into<SharedString>) -> Self {
        self.filter_text = filter_text.into();
        self
    }

    /// 设置行选中模式。
    pub fn selection_mode(mut self, selection_mode: TreeSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// 设置是否显示复选框。
    pub fn checkable(mut self, checkable: bool) -> Self {
        self.checkable = checkable;
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置是否必填。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: TreeSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: TreeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: TreeStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置辅助文本。
    pub fn helper_text(mut self, helper_text: impl Into<Option<SharedString>>) -> Self {
        self.helper_text = helper_text.into();
        self
    }

    /// 设置空状态文本。
    pub fn empty_text(mut self, empty_text: impl Into<SharedString>) -> Self {
        self.empty_text = empty_text.into();
        self
    }

    /// 设置最大视口高度。
    pub fn max_height(mut self, max_height: Pixels) -> Self {
        self.max_height = max_height;
        self
    }

    /// 设置展开变化回调。
    pub fn on_expand(mut self, handler: impl FnMut(Vec<SharedString>) + 'static) -> Self {
        self.on_expand = Some(Box::new(handler));
        self
    }

    /// 设置选中变化回调。
    pub fn on_select(mut self, handler: impl FnMut(Vec<SharedString>) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// 设置复选变化回调。
    pub fn on_check(
        mut self,
        handler: impl FnMut(Vec<SharedString>, Vec<SharedString>) + 'static,
    ) -> Self {
        self.on_check = Some(Box::new(handler));
        self
    }

    /// 设置过滤文本变化回调。
    pub fn on_filter_change(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_filter_change = Some(Box::new(handler));
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

    /// 设置键盘按下回调。
    pub fn on_key_down(mut self, handler: impl FnMut(Keystroke) + 'static) -> Self {
        self.on_key_down = Some(Box::new(handler));
        self
    }
}
