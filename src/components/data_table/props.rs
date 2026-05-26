//! `DataTable` 的公开参数和列模型。
//!
//! 本模块只定义调用方可见的 API：行 key、列配置、状态枚举和事件回调。
//! 过滤、排序、分页、选择等规则放在 `state.rs`，避免渲染层和外部 API 混在一起。

use std::rc::Rc;

use gpui::{div, AnyElement, IntoElement, Keystroke, ParentElement, Pixels, SharedString};

/// DataTable 行 key 生成器。
///
/// 表格排序、过滤和分页都会改变行的位置；使用稳定 row key 可以避免选择状态跟随下标漂移。
pub type DataTableRowKey<T> = Rc<dyn Fn(&T) -> SharedString>;

/// DataTable 行禁用判断器。
///
/// 禁用行仍然展示，但不能被行选择、键盘选择或表头全选影响。
pub type DataTableRowDisabled<T> = Rc<dyn Fn(&T) -> bool>;

/// DataTable 文本列取值器。
///
/// 第一版数据列统一使用 `SharedString`，过滤和排序都基于这个文本值执行，避免引入数值、
/// 日期和本地化比较策略导致 API 过早复杂化。
pub type DataTableTextAccessor<T> = Rc<dyn Fn(&T) -> SharedString>;

/// DataTable 自定义展示列渲染器。
///
/// 渲染器每次单元格渲染都会被调用并返回新的 `AnyElement`，和 `TextInputSlot` 一样避免复用
/// gpui element 值。展示列默认不参与过滤和排序，适合表格末尾的操作列。
pub type DataTableCellRenderer<T> = Rc<dyn for<'a> Fn(DataTableCellContext<'a, T>) -> AnyElement>;

/// 过滤文本变化回调。
pub type DataTableFilterChangeHandler = Box<dyn FnMut(SharedString)>;

/// 排序变化回调。
pub type DataTableSortChangeHandler = Box<dyn FnMut(Option<DataTableSort>)>;

/// 分页变化回调。
pub type DataTablePageChangeHandler = Box<dyn FnMut(DataTablePageState)>;

/// 选中行变化回调。
pub type DataTableSelectionChangeHandler = Box<dyn FnMut(Vec<SharedString>)>;

/// 焦点变化回调。
pub type DataTableFocusHandler = Box<dyn FnMut()>;

/// 键盘按下回调。
pub type DataTableKeyDownHandler = Box<dyn FnMut(Keystroke)>;

/// DataTable 单元格渲染上下文。
///
/// 自定义展示列通过该结构获得当前行、稳定 key、原始行下标、当前页行下标和派生状态。
/// 这里不暴露表格内部可变引用，避免操作列绕过公开方法直接修改 DataTable 状态。
#[derive(Clone, Copy)]
pub struct DataTableCellContext<'a, T> {
    /// 当前行数据引用。
    pub row: &'a T,
    /// 当前行稳定 key。
    pub row_key: &'a SharedString,
    /// 当前行在原始 `rows` 中的下标。
    pub row_index: usize,
    /// 当前行在当前分页页内的下标。
    pub page_row_index: usize,
    /// 当前行是否处于 selected 状态。
    pub selected: bool,
    /// 当前行是否禁用。
    pub disabled: bool,
}

/// DataTable 尺寸。
///
/// 尺寸会同时影响表头、行高、分页按钮、过滤输入和文本字号；虚拟列表依赖固定行高。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableSize {
    /// 小尺寸，适合密集后台列表。
    Small,
    /// 默认尺寸，适合大多数数据表格。
    Medium,
    /// 大尺寸，适合信息密度较低或需要强调可读性的表格。
    Large,
}

impl Default for DataTableSize {
    /// 返回默认尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// DataTable 视觉变体。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableVariant {
    /// 带边框的标准表格容器。
    Outlined,
    /// 浅色填充容器。
    Filled,
    /// 弱边界容器，适合嵌入式面板。
    Ghost,
}

impl Default for DataTableVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// DataTable 语义状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableStatus {
    /// 默认状态。
    Default,
    /// 错误状态。
    Error,
    /// 警告状态。
    Warning,
    /// 成功状态。
    Success,
}

impl Default for DataTableStatus {
    /// 返回默认状态。
    fn default() -> Self {
        Self::Default
    }
}

/// DataTable 单元格水平对齐方式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableAlign {
    /// 左对齐，默认文本列使用。
    Left,
    /// 居中对齐，适合状态、标签或短值。
    Center,
    /// 右对齐，适合数字或操作列。
    Right,
}

impl Default for DataTableAlign {
    /// 返回默认对齐。
    fn default() -> Self {
        Self::Left
    }
}

/// DataTable 行选择模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableSelectionMode {
    /// 不维护 selected 状态。
    None,
    /// 单选模式。
    Single,
    /// 多选模式。
    Multiple,
}

impl Default for DataTableSelectionMode {
    /// 默认不启用行选择，避免普通只读表格出现多余选择列。
    fn default() -> Self {
        Self::None
    }
}

/// DataTable 排序方向。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableSortDirection {
    /// 升序。
    Asc,
    /// 降序。
    Desc,
}

/// DataTable 单列排序状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTableSort {
    /// 当前排序列 key。
    pub column_key: SharedString,
    /// 当前排序方向。
    pub direction: DataTableSortDirection,
}

impl DataTableSort {
    /// 创建排序状态。
    pub fn new(column_key: impl Into<SharedString>, direction: DataTableSortDirection) -> Self {
        Self {
            column_key: column_key.into(),
            direction,
        }
    }
}

/// DataTable 分页派生状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataTablePageState {
    /// 当前页码，使用 1-based 语义。
    pub page: usize,
    /// 当前每页条数。
    pub page_size: usize,
    /// 过滤后的总行数。
    pub total_rows: usize,
    /// 总页数。即使没有数据也返回 1，便于分页 UI 保持稳定。
    pub total_pages: usize,
}

/// DataTable 列类型。
pub(crate) enum DataTableColumnKind<T: 'static> {
    /// 文本数据列，默认参与过滤和排序。
    Text {
        /// 文本取值器。
        accessor: DataTableTextAccessor<T>,
        /// 是否参与全局过滤。
        filterable: bool,
        /// 是否允许表头点击排序。
        sortable: bool,
    },
    /// 展示列，默认只负责渲染。
    Display {
        /// 自定义单元格渲染器。
        renderer: DataTableCellRenderer<T>,
    },
}

impl<T: 'static> Clone for DataTableColumnKind<T> {
    /// 克隆列类型。
    ///
    /// 闭包都保存在 `Rc` 中，克隆列时只复制引用计数，不要求行数据 `T` 自身实现 Clone。
    fn clone(&self) -> Self {
        match self {
            Self::Text {
                accessor,
                filterable,
                sortable,
            } => Self::Text {
                accessor: accessor.clone(),
                filterable: *filterable,
                sortable: *sortable,
            },
            Self::Display { renderer } => Self::Display {
                renderer: renderer.clone(),
            },
        }
    }
}

/// DataTable 列配置。
///
/// 第一版明确区分文本数据列和展示/操作列：文本列负责数据处理，展示列负责自定义渲染。
/// 这样可以支持操作列，同时不让按钮文字、图标或任意元素影响过滤和排序结果。
pub struct DataTableColumn<T: 'static> {
    /// 列稳定 key。
    pub key: SharedString,
    /// 表头标题。
    pub title: SharedString,
    /// 列宽。未设置时按 flex 均分剩余空间。
    pub width: Option<Pixels>,
    /// 最小列宽。
    pub min_width: Pixels,
    /// 单元格对齐。
    pub align: DataTableAlign,
    /// 列具体类型。
    pub(crate) kind: DataTableColumnKind<T>,
}

impl<T: 'static> Clone for DataTableColumn<T> {
    /// 克隆列配置。
    ///
    /// DataTable 渲染表头时需要持有列快照；手写 Clone 可以避免派生宏给泛型 `T`
    /// 增加不必要的 `T: Clone` 约束。
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            title: self.title.clone(),
            width: self.width,
            min_width: self.min_width,
            align: self.align,
            kind: self.kind.clone(),
        }
    }
}

impl<T: 'static> DataTableColumn<T> {
    /// 创建文本数据列。
    pub fn text(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        accessor: impl Fn(&T) -> SharedString + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            width: None,
            min_width: gpui::px(96.0),
            align: DataTableAlign::Left,
            kind: DataTableColumnKind::Text {
                accessor: Rc::new(accessor),
                filterable: true,
                sortable: true,
            },
        }
    }

    /// 创建自定义展示列。
    pub fn display(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        renderer: impl for<'a> Fn(DataTableCellContext<'a, T>) -> AnyElement + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            width: None,
            min_width: gpui::px(96.0),
            align: DataTableAlign::Left,
            kind: DataTableColumnKind::Display {
                renderer: Rc::new(renderer),
            },
        }
    }

    /// 创建表格操作列。
    ///
    /// 操作列默认右对齐并使用较小宽度，且不会参与过滤和排序。
    pub fn actions(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        renderer: impl for<'a> Fn(DataTableCellContext<'a, T>) -> AnyElement + 'static,
    ) -> Self {
        Self::display(key, title, renderer)
            .align(DataTableAlign::Right)
            .width(gpui::px(128.0))
            .min_width(gpui::px(96.0))
    }

    /// 设置固定列宽。
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// 设置最小列宽。
    pub fn min_width(mut self, min_width: Pixels) -> Self {
        self.min_width = min_width;
        self
    }

    /// 设置列对齐。
    pub fn align(mut self, align: DataTableAlign) -> Self {
        self.align = align;
        self
    }

    /// 设置文本列是否参与全局过滤。
    pub fn filterable(mut self, filterable: bool) -> Self {
        if let DataTableColumnKind::Text { filterable: f, .. } = &mut self.kind {
            *f = filterable;
        }
        self
    }

    /// 设置文本列是否允许排序。
    pub fn sortable(mut self, sortable: bool) -> Self {
        if let DataTableColumnKind::Text { sortable: s, .. } = &mut self.kind {
            *s = sortable;
        }
        self
    }

    /// 返回文本列值。
    pub(crate) fn text_value(&self, row: &T) -> Option<SharedString> {
        match &self.kind {
            DataTableColumnKind::Text { accessor, .. } => Some(accessor(row)),
            DataTableColumnKind::Display { .. } => None,
        }
    }

    /// 判断列是否可过滤。
    pub(crate) fn is_filterable(&self) -> bool {
        matches!(
            self.kind,
            DataTableColumnKind::Text {
                filterable: true,
                ..
            }
        )
    }

    /// 判断列是否可排序。
    pub(crate) fn is_sortable(&self) -> bool {
        matches!(self.kind, DataTableColumnKind::Text { sortable: true, .. })
    }

    /// 渲染单元格内容。
    pub(crate) fn render_cell(&self, context: DataTableCellContext<'_, T>) -> AnyElement {
        match &self.kind {
            DataTableColumnKind::Text { accessor, .. } => {
                div().child(accessor(context.row)).into_any_element()
            }
            DataTableColumnKind::Display { renderer } => renderer(context),
        }
    }
}

/// `DataTable` 创建参数。
///
/// 参数结构包含原始数据、列、过滤/排序/分页/选择初始状态和回调。组件创建后可通过
/// `set_*` 方法做受控同步；同步方法不会触发用户交互回调。
pub struct DataTableProps<T: 'static> {
    /// 原始行数据。
    pub rows: Vec<T>,
    /// 列配置。
    pub columns: Vec<DataTableColumn<T>>,
    /// 稳定 row key 生成器。
    pub row_key: DataTableRowKey<T>,
    /// 行禁用判断器。
    pub row_disabled: Option<DataTableRowDisabled<T>>,
    /// 初始过滤文本。
    pub filter_text: SharedString,
    /// 是否显示内置过滤输入框。
    pub show_filter: bool,
    /// 过滤输入框 placeholder。
    pub filter_placeholder: SharedString,
    /// 初始排序状态。
    pub sort: Option<DataTableSort>,
    /// 初始页码。
    pub page: usize,
    /// 初始每页条数。
    pub page_size: usize,
    /// 分页尺寸候选。
    pub page_size_options: Vec<usize>,
    /// 行选择模式。
    pub selection_mode: DataTableSelectionMode,
    /// 初始 selected row key。
    pub selected_row_keys: Vec<SharedString>,
    /// 加载状态。
    pub loading: bool,
    /// 禁用状态。
    pub disabled: bool,
    /// 是否必填。
    pub required: bool,
    /// 尺寸。
    pub size: DataTableSize,
    /// 视觉变体。
    pub variant: DataTableVariant,
    /// 语义状态。
    pub status: DataTableStatus,
    /// 辅助文本。
    pub helper_text: Option<SharedString>,
    /// 空状态文本。
    pub empty_text: SharedString,
    /// 加载状态文本。
    pub loading_text: SharedString,
    /// 表格主体最大高度。
    pub max_height: Pixels,
    /// 过滤变化回调。
    pub on_filter_change: Option<DataTableFilterChangeHandler>,
    /// 排序变化回调。
    pub on_sort_change: Option<DataTableSortChangeHandler>,
    /// 分页变化回调。
    pub on_page_change: Option<DataTablePageChangeHandler>,
    /// selected row key 变化回调。
    pub on_selection_change: Option<DataTableSelectionChangeHandler>,
    /// 聚焦回调。
    pub on_focus: Option<DataTableFocusHandler>,
    /// 失焦回调。
    pub on_blur: Option<DataTableFocusHandler>,
    /// 键盘按下回调。
    pub on_key_down: Option<DataTableKeyDownHandler>,
}

impl<T: 'static> DataTableProps<T> {
    /// 创建 DataTable 参数。
    pub fn new(row_key: impl Fn(&T) -> SharedString + 'static) -> Self {
        Self {
            rows: Vec::new(),
            columns: Vec::new(),
            row_key: Rc::new(row_key),
            row_disabled: None,
            filter_text: SharedString::default(),
            show_filter: true,
            filter_placeholder: SharedString::from("搜索"),
            sort: None,
            page: 1,
            page_size: 10,
            page_size_options: vec![10, 20, 50],
            selection_mode: DataTableSelectionMode::default(),
            selected_row_keys: Vec::new(),
            loading: false,
            disabled: false,
            required: false,
            size: DataTableSize::default(),
            variant: DataTableVariant::default(),
            status: DataTableStatus::default(),
            helper_text: None,
            empty_text: SharedString::from("暂无数据"),
            loading_text: SharedString::from("加载中"),
            max_height: gpui::px(360.0),
            on_filter_change: None,
            on_sort_change: None,
            on_page_change: None,
            on_selection_change: None,
            on_focus: None,
            on_blur: None,
            on_key_down: None,
        }
    }

    /// 设置行数据。
    pub fn rows(mut self, rows: impl Into<Vec<T>>) -> Self {
        self.rows = rows.into();
        self
    }

    /// 设置列配置。
    pub fn columns(mut self, columns: impl Into<Vec<DataTableColumn<T>>>) -> Self {
        self.columns = columns.into();
        self
    }

    /// 设置行禁用判断器。
    pub fn row_disabled(mut self, row_disabled: impl Fn(&T) -> bool + 'static) -> Self {
        self.row_disabled = Some(Rc::new(row_disabled));
        self
    }

    /// 设置初始过滤文本。
    pub fn filter_text(mut self, filter_text: impl Into<SharedString>) -> Self {
        self.filter_text = filter_text.into();
        self
    }

    /// 设置是否显示内置过滤输入框。
    pub fn show_filter(mut self, show_filter: bool) -> Self {
        self.show_filter = show_filter;
        self
    }

    /// 设置过滤输入框 placeholder。
    pub fn filter_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.filter_placeholder = placeholder.into();
        self
    }

    /// 设置初始排序状态。
    pub fn sort(mut self, sort: impl Into<Option<DataTableSort>>) -> Self {
        self.sort = sort.into();
        self
    }

    /// 设置初始页码。
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// 设置初始每页条数。
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    /// 设置分页尺寸候选。
    pub fn page_size_options(mut self, options: impl Into<Vec<usize>>) -> Self {
        self.page_size_options = options.into();
        self
    }

    /// 设置行选择模式。
    pub fn selection_mode(mut self, selection_mode: DataTableSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// 设置初始 selected row key。
    pub fn selected_row_keys(mut self, selected_row_keys: impl Into<Vec<SharedString>>) -> Self {
        self.selected_row_keys = selected_row_keys.into();
        self
    }

    /// 设置加载状态。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置必填语义。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: DataTableSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: DataTableVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: DataTableStatus) -> Self {
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

    /// 设置加载文本。
    pub fn loading_text(mut self, loading_text: impl Into<SharedString>) -> Self {
        self.loading_text = loading_text.into();
        self
    }

    /// 设置表格主体最大高度。
    pub fn max_height(mut self, max_height: Pixels) -> Self {
        self.max_height = max_height;
        self
    }

    /// 设置过滤变化回调。
    pub fn on_filter_change(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_filter_change = Some(Box::new(handler));
        self
    }

    /// 设置排序变化回调。
    pub fn on_sort_change(mut self, handler: impl FnMut(Option<DataTableSort>) + 'static) -> Self {
        self.on_sort_change = Some(Box::new(handler));
        self
    }

    /// 设置分页变化回调。
    pub fn on_page_change(mut self, handler: impl FnMut(DataTablePageState) + 'static) -> Self {
        self.on_page_change = Some(Box::new(handler));
        self
    }

    /// 设置选择变化回调。
    pub fn on_selection_change(mut self, handler: impl FnMut(Vec<SharedString>) + 'static) -> Self {
        self.on_selection_change = Some(Box::new(handler));
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
