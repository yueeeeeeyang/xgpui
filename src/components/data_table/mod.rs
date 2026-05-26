//! 标准数据表格组件。
//!
//! `DataTable` 提供本地过滤、单列排序、分页、行选择、自定义展示/操作列、
//! 虚拟行渲染、键盘导航、状态样式、helper text、受控同步和明暗皮肤。

use gpui::prelude::*;
use gpui::{
    actions, div, px, uniform_list, App, Context, CursorStyle, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, MouseDownEvent, ParentElement,
    Pixels, Render, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window,
};
use std::collections::HashSet;

use crate::components::select::{Select, SelectOption, SelectProps, SelectSize, SelectVariant};
use crate::components::text_input::{TextInput, TextInputProps, TextInputSize};
use crate::foundation::icon::{self, LucideIcon};

mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

pub use props::{
    DataTableAlign, DataTableCellContext, DataTableColumn, DataTablePageState, DataTableProps,
    DataTableSelectionMode, DataTableSize, DataTableSort, DataTableSortDirection, DataTableStatus,
    DataTableVariant,
};
use state::{DataTableRowRecord, DataTableState, DataTableStateOutcome, DataTableView};
use style::{resolve_data_table_style, ResolvedDataTableStyle};

actions!(
    xgpui_data_table,
    [
        MoveUp,
        MoveDown,
        FirstRow,
        LastRow,
        CommitSelection,
        SelectAll,
    ]
);

/// 注册 `DataTable` 默认键盘快捷键。
pub fn register_data_table_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, Some("DataTable")),
        KeyBinding::new("down", MoveDown, Some("DataTable")),
        KeyBinding::new("home", FirstRow, Some("DataTable")),
        KeyBinding::new("end", LastRow, Some("DataTable")),
        KeyBinding::new("enter", CommitSelection, Some("DataTable")),
        KeyBinding::new("space", CommitSelection, Some("DataTable")),
        KeyBinding::new("cmd-a", SelectAll, Some("DataTable")),
        KeyBinding::new("ctrl-a", SelectAll, Some("DataTable")),
    ]);
}

/// 标准数据表格组件。
///
/// 组件内部维护过滤、排序、分页、选择和键盘活动行。调用方可以通过 `set_*` 方法做受控同步；
/// 这些同步方法不会触发用户交互回调，避免父组件写回状态时形成回调循环。
pub struct DataTable<T: 'static> {
    focus_handle: FocusHandle,
    rows: Vec<T>,
    columns: Vec<DataTableColumn<T>>,
    row_key: props::DataTableRowKey<T>,
    row_disabled: Option<props::DataTableRowDisabled<T>>,
    state: DataTableState,
    selection_mode: DataTableSelectionMode,
    loading: bool,
    disabled: bool,
    required: bool,
    size: DataTableSize,
    variant: DataTableVariant,
    status: DataTableStatus,
    helper_text: Option<SharedString>,
    empty_text: SharedString,
    loading_text: SharedString,
    max_height: Pixels,
    page_size_options: Vec<usize>,
    show_filter: bool,
    filter_input: Entity<TextInput>,
    page_size_select: Entity<Select>,
    /// 保持过滤输入框观察订阅存活。
    ///
    /// gpui 的 `Subscription` 在 drop 时会取消订阅；DataTable 需要持续观察内部 TextInput 的
    /// notify，才能把用户输入转换成表格过滤状态。
    _filter_subscription: Subscription,
    /// 保持每页条数 Select 的观察订阅存活。
    ///
    /// DataTable 使用现有 Select 承载 page size 选择；订阅负责把用户在 Select 中做出的选择
    /// 转换成分页状态变化和 `on_page_change` 回调。
    _page_size_subscription: Subscription,
    on_filter_change: Option<props::DataTableFilterChangeHandler>,
    on_sort_change: Option<props::DataTableSortChangeHandler>,
    on_page_change: Option<props::DataTablePageChangeHandler>,
    on_selection_change: Option<props::DataTableSelectionChangeHandler>,
    on_focus: Option<props::DataTableFocusHandler>,
    on_blur: Option<props::DataTableFocusHandler>,
    on_key_down: Option<props::DataTableKeyDownHandler>,
    scroll_handle: UniformListScrollHandle,
    is_focused: bool,
    suppress_next_focus_callback: bool,
}

impl<T: 'static> DataTable<T> {
    /// 创建新的 `DataTable`。
    pub fn new(cx: &mut Context<Self>, props: DataTableProps<T>) -> Self {
        let filter_input = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .value(props.filter_text.clone())
                    .placeholder(props.filter_placeholder.clone())
                    .clearable(true)
                    .disabled(props.disabled || props.loading)
                    // 内置过滤输入框复用 TextInput 自身的明暗主题。这里不额外放搜索图标，
                    // 避免 slot 闭包无法读取当前主题时引入单一皮肤的硬编码颜色。
                    // 搜索框属于表格工具区控件而不是行内容，固定使用 small 尺寸可以让不同
                    // DataTableSize 下的工具栏信息密度保持稳定。
                    .size(TextInputSize::Small),
            )
        });
        let filter_subscription = cx.observe(&filter_input, |table, input, cx| {
            let value = input.read(cx).value().clone();
            table.on_filter_input_change(value, cx);
        });
        let initial_page_size = props.page_size.max(1);
        let page_size_select = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .value(Some(page_size_value(initial_page_size)))
                    .options(page_size_select_options(
                        &props.page_size_options,
                        initial_page_size,
                    ))
                    .disabled(props.disabled || props.loading)
                    .size(SelectSize::Small)
                    .variant(SelectVariant::Outlined)
                    .placeholder("每页条数")
                    .max_popup_height(px(180.0))
                    .empty_text("暂无分页尺寸"),
            )
        });
        let page_size_subscription = cx.observe(&page_size_select, |table, select, cx| {
            let value = select.read(cx).value().cloned();
            table.on_page_size_select_change(value, cx);
        });

        let mut state = DataTableState::new(
            props.filter_text,
            props.sort,
            props.page,
            props.page_size,
            props.selected_row_keys,
            props.selection_mode,
        );
        state.sync_inputs_silent(
            &props.rows,
            &props.columns,
            &props.row_key,
            props.row_disabled.as_ref(),
        );

        Self {
            focus_handle: cx.focus_handle(),
            rows: props.rows,
            columns: props.columns,
            row_key: props.row_key,
            row_disabled: props.row_disabled,
            state,
            selection_mode: props.selection_mode,
            loading: props.loading,
            disabled: props.disabled,
            required: props.required,
            size: props.size,
            variant: props.variant,
            status: props.status,
            helper_text: props.helper_text,
            empty_text: props.empty_text,
            loading_text: props.loading_text,
            max_height: props.max_height,
            page_size_options: props.page_size_options,
            show_filter: props.show_filter,
            filter_input,
            page_size_select,
            _filter_subscription: filter_subscription,
            _page_size_subscription: page_size_subscription,
            on_filter_change: props.on_filter_change,
            on_sort_change: props.on_sort_change,
            on_page_change: props.on_page_change,
            on_selection_change: props.on_selection_change,
            on_focus: props.on_focus,
            on_blur: props.on_blur,
            on_key_down: props.on_key_down,
            scroll_handle: UniformListScrollHandle::new(),
            is_focused: false,
            suppress_next_focus_callback: false,
        }
    }

    /// 返回原始行数据。
    pub fn rows(&self) -> &[T] {
        &self.rows
    }

    /// 返回过滤后的行数。
    pub fn filtered_row_count(&self) -> usize {
        self.view().page_state.total_rows
    }

    /// 返回当前分页状态。
    pub fn page_state(&self) -> DataTablePageState {
        self.view().page_state
    }

    /// 返回当前排序状态。
    pub fn sort(&self) -> Option<&DataTableSort> {
        self.state.sort()
    }

    /// 返回当前过滤文本。
    pub fn filter_text(&self) -> &SharedString {
        self.state.filter_text()
    }

    /// 返回 selected row key。
    pub fn selected_row_keys(&self) -> &[SharedString] {
        self.state.selected_row_keys()
    }

    /// 从外部同步行数据。
    pub fn set_rows(&mut self, rows: impl Into<Vec<T>>, cx: &mut Context<Self>) {
        self.rows = rows.into();
        let outcome = self.state.sync_inputs_silent(
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步列配置。
    pub fn set_columns(
        &mut self,
        columns: impl Into<Vec<DataTableColumn<T>>>,
        cx: &mut Context<Self>,
    ) {
        self.columns = columns.into();
        let outcome = self.state.sync_inputs_silent(
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步过滤文本。
    pub fn set_filter_text(
        &mut self,
        filter_text: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let filter_text = filter_text.into();
        let outcome = self.state.set_filter_text_silent(
            filter_text.clone(),
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.filter_input.update(cx, |input, cx| {
            input.set_value(filter_text, cx);
        });
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步排序状态。
    pub fn set_sort(&mut self, sort: impl Into<Option<DataTableSort>>, cx: &mut Context<Self>) {
        let outcome = self.state.set_sort_silent(
            sort.into(),
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步页码。
    pub fn set_page(&mut self, page: usize, cx: &mut Context<Self>) {
        let outcome = self.state.set_page_silent(
            page,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步每页条数。
    pub fn set_page_size(&mut self, page_size: usize, cx: &mut Context<Self>) {
        let outcome = self.state.set_page_size_silent(
            page_size,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.sync_page_size_select(cx);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步 selected row key。
    pub fn set_selected_row_keys(
        &mut self,
        selected_row_keys: impl Into<Vec<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let outcome = self
            .state
            .set_selected_row_keys_silent(selected_row_keys.into(), self.selection_mode);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步加载状态。
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        if self.loading == loading {
            return;
        }
        self.loading = loading;
        self.sync_filter_disabled(cx);
        self.sync_page_size_select(cx);
        cx.notify();
    }

    /// 从外部同步禁用状态。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled {
            self.suppress_next_focus_callback |= self.is_focused;
            self.is_focused = false;
        }
        self.sync_filter_disabled(cx);
        self.sync_page_size_select(cx);
        cx.notify();
    }

    /// 从外部同步语义状态。
    pub fn set_status(&mut self, status: DataTableStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }
        self.status = status;
        cx.notify();
    }

    /// 从外部同步辅助文本。
    pub fn set_helper_text(
        &mut self,
        helper_text: impl Into<Option<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let helper_text = helper_text.into();
        if self.helper_text == helper_text {
            return;
        }
        self.helper_text = helper_text;
        cx.notify();
    }

    /// 清空过滤文本并触发用户变化语义。
    pub fn clear_filter(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let outcome = self.state.clear_filter(
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.filter_input.update(cx, |input, cx| {
            input.set_value(SharedString::default(), cx);
        });
        self.apply_outcome(outcome, true, cx);
    }

    /// 清空 selected row key 并触发用户变化语义。
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let outcome = self.state.clear_selection();
        self.apply_outcome(outcome, true, cx);
    }

    /// 返回焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 构建当前视图派生状态。
    fn view(&self) -> DataTableView {
        self.state.view(
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        )
    }

    /// 同步内部过滤输入框禁用态。
    fn sync_filter_disabled(&mut self, cx: &mut Context<Self>) {
        let disabled = self.disabled || self.loading;
        self.filter_input.update(cx, |input, cx| {
            input.set_disabled(disabled, cx);
        });
    }

    /// 同步内部 page size Select 的选项、选中值和禁用态。
    ///
    /// 该同步发生在 DataTable 自身状态已经更新之后，因此 Select 的 notify 即使被 observe 捕获，
    /// 也会因为值已经一致而直接返回，不会把受控同步误报成用户分页操作。
    fn sync_page_size_select(&mut self, cx: &mut Context<Self>) {
        let page_sizes = self.page_size_options();
        let options = page_size_select_options_from_sizes(&page_sizes);
        let value = Some(page_size_value(self.state.page_size()));
        let disabled = self.disabled || self.loading;
        self.page_size_select.update(cx, |select, cx| {
            select.set_options(options, cx);
            select.set_value(value, cx);
            select.set_disabled(disabled, cx);
        });
    }

    /// 响应内部过滤输入框变化。
    fn on_filter_input_change(&mut self, filter_text: SharedString, cx: &mut Context<Self>) {
        if self.disabled || self.loading || self.state.filter_text() == &filter_text {
            return;
        }
        let outcome = self.state.set_filter_text_silent(
            filter_text,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应内部 page size Select 变化。
    ///
    /// Select 的值使用纯数字字符串，展示 label 才包含“条/页”。解析失败或选中当前值时忽略，
    /// 避免异常选项或受控同步造成分页回调噪声。
    fn on_page_size_select_change(&mut self, value: Option<SharedString>, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let Some(page_size) = value
            .as_ref()
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .filter(|page_size| *page_size > 0)
        else {
            return;
        };
        if self.state.page_size() == page_size {
            return;
        }
        let outcome = self.state.set_page_size_silent(
            page_size,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.sync_page_size_select(cx);
        self.apply_outcome(outcome, true, cx);
    }

    /// 应用状态结果。
    fn apply_outcome(
        &mut self,
        outcome: DataTableStateOutcome,
        emit_callbacks: bool,
        cx: &mut Context<Self>,
    ) {
        if emit_callbacks {
            if outcome.filter_changed {
                self.emit_filter_change();
            }
            if outcome.sort_changed {
                self.emit_sort_change();
            }
            if outcome.page_changed {
                self.emit_page_change();
            }
            if outcome.selection_changed {
                self.emit_selection_change();
            }
        }
        if outcome.active_changed {
            self.scroll_active_into_view();
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 触发过滤变化回调。
    fn emit_filter_change(&mut self) {
        if let Some(on_filter_change) = self.on_filter_change.as_mut() {
            on_filter_change(self.state.filter_text().clone());
        }
    }

    /// 触发排序变化回调。
    fn emit_sort_change(&mut self) {
        if let Some(on_sort_change) = self.on_sort_change.as_mut() {
            on_sort_change(self.state.sort().cloned());
        }
    }

    /// 触发分页变化回调。
    fn emit_page_change(&mut self) {
        let page_state = self.view().page_state;
        if let Some(on_page_change) = self.on_page_change.as_mut() {
            on_page_change(page_state);
        }
    }

    /// 触发行选择变化回调。
    fn emit_selection_change(&mut self) {
        if let Some(on_selection_change) = self.on_selection_change.as_mut() {
            on_selection_change(self.state.selected_row_keys().to_vec());
        }
    }

    /// 同步焦点状态并触发焦点回调。
    fn sync_focus_callbacks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        if focused == self.is_focused {
            if !focused && !self.disabled {
                self.suppress_next_focus_callback = false;
            }
            return;
        }

        self.is_focused = focused;
        if focused {
            if self.suppress_next_focus_callback {
                self.suppress_next_focus_callback = false;
            } else if let Some(on_focus) = self.on_focus.as_mut() {
                on_focus();
            }
        } else if self.suppress_next_focus_callback {
            self.suppress_next_focus_callback = false;
        } else if let Some(on_blur) = self.on_blur.as_mut() {
            on_blur();
        }
        cx.notify();
    }

    /// 将活动行滚入可见区域。
    fn scroll_active_into_view(&mut self) {
        let view = self.view();
        if let Some(active) = self.state.active_row_key() {
            if let Some(index) = view
                .page_rows
                .iter()
                .position(|record| &record.row_key == active)
            {
                self.scroll_handle
                    .scroll_to_item(index, ScrollStrategy::Center);
            }
        }
    }

    /// 响应表头排序点击。
    fn on_header_click(
        &mut self,
        column_key: SharedString,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.loading {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        let outcome = self.state.cycle_sort(
            &column_key,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应行点击。
    fn on_row_click(
        &mut self,
        row_key: SharedString,
        disabled: bool,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.loading {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        let outcome = self
            .state
            .toggle_row_selection(&row_key, disabled, self.selection_mode);
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应表头全选点击。
    fn on_select_all_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.loading {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        let page_rows = self.view().page_rows;
        let outcome = self
            .state
            .toggle_page_selection(&page_rows, self.selection_mode);
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应首页。
    fn on_first_page(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let outcome = self.state.set_page_silent(
            1,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应上一页。
    fn on_prev_page(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page = self.state.page().saturating_sub(1).max(1);
        let outcome = self.state.set_page_silent(
            page,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应下一页。
    fn on_next_page(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page = self.state.page() + 1;
        let outcome = self.state.set_page_silent(
            page,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应末页。
    fn on_last_page(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page = self.view().page_state.total_pages;
        let outcome = self.state.set_page_silent(
            page,
            &self.rows,
            &self.columns,
            &self.row_key,
            self.row_disabled.as_ref(),
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 鼠标在表格外按下时释放焦点。
    fn on_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus_handle.is_focused(window) {
            return;
        }
        window.blur();
        self.sync_focus_callbacks(window, cx);
    }

    /// 响应普通键盘按下。
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, _: &mut Context<Self>) {
        if let Some(on_key_down) = self.on_key_down.as_mut() {
            on_key_down(event.keystroke.clone());
        }
    }

    /// 向上移动活动行。
    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_by(-1, cx);
    }

    /// 向下移动活动行。
    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_by(1, cx);
    }

    /// 按指定步长移动活动行。
    fn move_active_by(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page_rows = self.view().page_rows;
        let outcome = self.state.move_active_by(&page_rows, delta);
        self.apply_outcome(outcome, false, cx);
    }

    /// 移动到当前页第一行。
    fn first_row(&mut self, _: &FirstRow, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page_rows = self.view().page_rows;
        let outcome = self.state.move_active_first(&page_rows);
        self.apply_outcome(outcome, false, cx);
    }

    /// 移动到当前页最后一行。
    fn last_row(&mut self, _: &LastRow, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page_rows = self.view().page_rows;
        let outcome = self.state.move_active_last(&page_rows);
        self.apply_outcome(outcome, false, cx);
    }

    /// Enter/Space 切换当前活动行选择。
    fn commit_selection(&mut self, _: &CommitSelection, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page_rows = self.view().page_rows;
        let outcome = self
            .state
            .toggle_active_selection(&page_rows, self.selection_mode);
        self.apply_outcome(outcome, true, cx);
    }

    /// 多选模式下选择当前页可选行。
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.loading {
            return;
        }
        let page_rows = self.view().page_rows;
        let outcome = self
            .state
            .toggle_page_selection(&page_rows, self.selection_mode);
        self.apply_outcome(outcome, true, cx);
    }
}

impl<T: 'static> Focusable for DataTable<T> {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<T: 'static> Render for DataTable<T> {
    /// 渲染 DataTable。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus_callbacks(window, cx);

        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let view = self.view();
        let helper_text = self.helper_text.clone();
        let required = self.required;
        let show_filter = self.show_filter;
        let filter_input = self.filter_input.clone();
        let page_size_select = self.page_size_select.clone();
        let loading = self.loading;
        let loading_text = self.loading_text.clone();
        let empty_text = self.empty_text.clone();
        let interactive = !self.disabled && !self.loading;
        let page_state = view.page_state;
        let selected_keys = self.state.selected_set();
        let table = cx.entity();

        let table_box = div()
            .id("xgpui-data-table")
            .flex()
            .flex_col()
            .w_full()
            .rounded(resolved.radius)
            .border_1()
            .border_color(resolved.border)
            .bg(resolved.background)
            .text_color(resolved.text)
            .text_size(resolved.font_size)
            .line_height(resolved.line_height)
            .opacity(resolved.opacity)
            .overflow_hidden()
            .when_else(
                self.disabled,
                |this| this.cursor(CursorStyle::Arrow),
                |this| {
                    this.track_focus(&self.focus_handle)
                        .key_context("DataTable")
                        .on_action(cx.listener(Self::move_up))
                        .on_action(cx.listener(Self::move_down))
                        .on_action(cx.listener(Self::first_row))
                        .on_action(cx.listener(Self::last_row))
                        .on_action(cx.listener(Self::commit_selection))
                        .on_action(cx.listener(Self::select_all))
                        .on_key_down(cx.listener(Self::on_key_down))
                        .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
                },
            )
            .when(show_filter, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .px(resolved.padding)
                        .py(resolved.gap)
                        .border_1()
                        .border_color(resolved.row_border)
                        .child(div().w(px(260.0)).child(filter_input)),
                )
            })
            .child(header_element(
                table.clone(),
                self.columns.clone(),
                self.selection_mode,
                &view,
                selected_keys,
                self.state.sort().cloned(),
                interactive,
                resolved,
                window,
            ))
            .child(body_element(
                table.clone(),
                view.page_rows.clone(),
                loading,
                loading_text,
                empty_text,
                interactive,
                resolved,
                self.max_height,
                self.scroll_handle.clone(),
            ))
            .child(footer_element(
                table,
                page_state,
                resolved,
                page_size_select,
                interactive,
                window,
            ));

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(table_box)
            .when(required, |this| {
                this.child(
                    div()
                        .text_color(crate::foundation::color::danger_500())
                        .child("*"),
                )
            })
            .when_some(helper_text, |this, helper_text| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(resolved.helper)
                        .child(helper_text),
                )
            })
    }
}

impl<T: 'static> DataTable<T> {
    /// 解析当前样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedDataTableStyle {
        resolve_data_table_style(
            self.size,
            self.variant,
            self.status,
            focused,
            self.disabled,
            cx,
        )
    }

    /// 返回去重后的 page size 选项。
    fn page_size_options(&self) -> Vec<usize> {
        normalized_page_sizes(&self.page_size_options, self.state.page_size())
    }
}

/// 渲染表头。
///
/// 表头渲染需要同时读取列、选择态、排序态和当前页可选行。这里保留扁平参数，
/// 是为了让调用点清晰展示每个渲染快照的来源，避免额外引入只服务单个函数的临时结构。
#[allow(clippy::too_many_arguments)]
fn header_element<T: 'static>(
    table: Entity<DataTable<T>>,
    columns: Vec<DataTableColumn<T>>,
    selection_mode: DataTableSelectionMode,
    view: &DataTableView,
    selected_keys: HashSet<SharedString>,
    sort: Option<DataTableSort>,
    interactive: bool,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
) -> impl IntoElement {
    div()
        .id("xgpui-data-table-header")
        .flex()
        .items_center()
        .w_full()
        .h(resolved.header_height)
        .bg(resolved.header_background)
        .border_1()
        .border_color(resolved.row_border)
        .when(selection_mode != DataTableSelectionMode::None, |this| {
            this.child(selection_header_cell(
                table.clone(),
                selection_mode,
                view,
                &selected_keys,
                interactive,
                resolved,
                window,
            ))
        })
        .children(
            columns
                .into_iter()
                .enumerate()
                .map(|(column_index, column)| {
                    header_cell(
                        table.clone(),
                        column_index,
                        column,
                        sort.clone(),
                        interactive,
                        resolved,
                        window,
                    )
                    .into_any_element()
                }),
        )
}

/// 渲染选择列表头。
fn selection_header_cell<T: 'static>(
    table: Entity<DataTable<T>>,
    selection_mode: DataTableSelectionMode,
    view: &DataTableView,
    selected_keys: &HashSet<SharedString>,
    interactive: bool,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
) -> impl IntoElement {
    let selectable_count = view
        .page_rows
        .iter()
        .filter(|record| !record.disabled)
        .count();
    let selected = selectable_count > 0
        && view
            .page_rows
            .iter()
            .filter(|record| !record.disabled)
            .all(|record| selected_keys.contains(&record.row_key));

    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w(resolved.selection_width)
        .h_full()
        .when(selection_mode == DataTableSelectionMode::Multiple, |this| {
            this.child(
                checkbox_element(selected, false, resolved)
                    .id("xgpui-data-table-select-all")
                    .cursor(if interactive && selectable_count > 0 {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::Arrow
                    })
                    .when(interactive && selectable_count > 0, |this| {
                        this.on_click(window.listener_for(&table, |this, event, window, cx| {
                            this.on_select_all_click(event, window, cx)
                        }))
                    }),
            )
        })
}

/// 渲染普通表头单元格。
fn header_cell<T: 'static>(
    table: Entity<DataTable<T>>,
    column_index: usize,
    column: DataTableColumn<T>,
    sort: Option<DataTableSort>,
    interactive: bool,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
) -> impl IntoElement {
    let column_key = column.key.clone();
    let sorted = sort
        .as_ref()
        .filter(|sort| sort.column_key == column.key)
        .map(|sort| sort.direction);
    let sortable = column.is_sortable();
    let clickable = interactive && sortable;
    let cell = column_container(column.width, column.min_width)
        .h_full()
        .px(resolved.cell_padding_x)
        .items_center();

    apply_align(cell, column.align)
        // GPUI 的点击和悬停交互需要 stateful 元素；列索引在当前表头渲染内稳定且唯一，
        // 同时避免把 SharedString 暴露给 element id 的类型约束。
        .id(("xgpui-data-table-header-cell", column_index))
        .gap(resolved.gap)
        .text_color(resolved.header_text)
        .cursor(if clickable {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .when(clickable, |this| {
            this.hover(move |style| style.bg(resolved.row_hover))
                .on_click(window.listener_for(&table, move |this, event, window, cx| {
                    this.on_header_click(column_key.clone(), event, window, cx)
                }))
        })
        .child(div().overflow_hidden().child(column.title))
        .when_some(sorted, |this, direction| {
            let icon = match direction {
                DataTableSortDirection::Asc => LucideIcon::ChevronUp,
                DataTableSortDirection::Desc => LucideIcon::ChevronDown,
            };
            this.child(icon::lucide_icon(
                icon,
                resolved.muted_text,
                resolved.icon_size,
            ))
        })
}

/// 渲染表体。
///
/// 表体需要处理 loading、empty、虚拟列表高度、滚动句柄和行快照。参数数量来自渲染分支输入，
/// 不代表公开 API 复杂度，因此在该内部 helper 上局部放宽 clippy 限制。
#[allow(clippy::too_many_arguments)]
fn body_element<T: 'static>(
    table: Entity<DataTable<T>>,
    page_rows: Vec<DataTableRowRecord>,
    loading: bool,
    loading_text: SharedString,
    empty_text: SharedString,
    interactive: bool,
    resolved: ResolvedDataTableStyle,
    max_height: Pixels,
    scroll_handle: UniformListScrollHandle,
) -> impl IntoElement {
    let height = body_height(page_rows.len(), resolved, max_height);
    if loading {
        return state_message_element("xgpui-data-table-loading", loading_text, height, resolved)
            .into_any_element();
    }
    if page_rows.is_empty() {
        return state_message_element("xgpui-data-table-empty", empty_text, height, resolved)
            .into_any_element();
    }

    let count = page_rows.len();
    uniform_list("xgpui-data-table-body", count, move |range, window, cx| {
        let table_state = table.read(cx);
        let selected = table_state.state.selected_set();
        range
            .filter_map(|page_row_index| {
                let record = page_rows.get(page_row_index)?.clone();
                let is_selected = selected.contains(&record.row_key);
                Some(row_element(
                    table.clone(),
                    table_state,
                    record,
                    page_row_index,
                    is_selected,
                    interactive,
                    resolved,
                    window,
                ))
            })
            .collect()
    })
    .w_full()
    .h(height)
    .track_scroll(scroll_handle)
    .into_any_element()
}

/// 渲染空状态或加载状态。
fn state_message_element(
    id: &'static str,
    text: SharedString,
    height: Pixels,
    resolved: ResolvedDataTableStyle,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h(height)
        .text_color(resolved.empty_text)
        .child(text)
}

/// 渲染单行。
///
/// 行元素既要展示派生状态，又要根据表格整体交互态和行级禁用态决定是否挂载点击行为。
/// 参数保持显式传入，便于看出禁用和 loading 对行交互的影响。
#[allow(clippy::too_many_arguments)]
fn row_element<T: 'static>(
    table: Entity<DataTable<T>>,
    table_state: &DataTable<T>,
    record: DataTableRowRecord,
    page_row_index: usize,
    selected: bool,
    interactive: bool,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
) -> impl IntoElement {
    let active = table_state.state.active_row_key() == Some(&record.row_key);
    let background = if active {
        resolved.row_active
    } else if selected {
        resolved.row_selected
    } else {
        crate::foundation::color::transparent()
    };
    let row_key = record.row_key.clone();
    let disabled = record.disabled;
    let row_interactive = interactive && !disabled;

    div()
        .id(("xgpui-data-table-row", page_row_index))
        .flex()
        .items_center()
        .w_full()
        .h(resolved.row_height)
        .border_1()
        .border_color(resolved.row_border)
        .bg(background)
        .opacity(if disabled { 0.58 } else { 1.0 })
        .cursor(if row_interactive {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .when(row_interactive, |this| {
            this.hover(move |style| style.bg(resolved.row_hover))
                .on_click(window.listener_for(&table, move |this, event, window, cx| {
                    this.on_row_click(row_key.clone(), disabled, event, window, cx)
                }))
        })
        .when(
            table_state.selection_mode != DataTableSelectionMode::None,
            |this| {
                this.child(selection_row_cell(
                    table.clone(),
                    record.row_key.clone(),
                    page_row_index,
                    selected,
                    disabled,
                    interactive,
                    table_state.selection_mode,
                    resolved,
                    window,
                ))
            },
        )
        .children(table_state.columns.iter().map(|column| {
            let row = &table_state.rows[record.row_index];
            let context = DataTableCellContext {
                row,
                row_key: &record.row_key,
                row_index: record.row_index,
                page_row_index,
                selected,
                disabled,
            };
            column_cell(column, context, resolved).into_any_element()
        }))
}

/// 渲染行选择单元格。
///
/// 选择单元格同时依赖 row key、页内位置、选择模式和禁用态；这些值来自已派生的行快照，
/// 保持显式参数能避免在渲染层重新查询状态。
#[allow(clippy::too_many_arguments)]
fn selection_row_cell<T: 'static>(
    table: Entity<DataTable<T>>,
    row_key: SharedString,
    page_row_index: usize,
    selected: bool,
    disabled: bool,
    interactive: bool,
    selection_mode: DataTableSelectionMode,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
) -> impl IntoElement {
    let radio = selection_mode == DataTableSelectionMode::Single;
    let cell_interactive = interactive && !disabled;
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w(resolved.selection_width)
        .h_full()
        .child(
            checkbox_element(selected, radio, resolved)
                .id(("xgpui-data-table-row-selector", page_row_index))
                .cursor(if cell_interactive {
                    CursorStyle::PointingHand
                } else {
                    CursorStyle::Arrow
                })
                .when(cell_interactive, |this| {
                    this.on_click(window.listener_for(&table, move |this, event, window, cx| {
                        this.on_row_click(row_key.clone(), disabled, event, window, cx)
                    }))
                }),
        )
}

/// 渲染普通单元格。
fn column_cell<T: 'static>(
    column: &DataTableColumn<T>,
    context: DataTableCellContext<'_, T>,
    resolved: ResolvedDataTableStyle,
) -> impl IntoElement {
    let cell = column_container(column.width, column.min_width)
        .h_full()
        .px(resolved.cell_padding_x)
        .items_center();

    apply_align(cell, column.align)
        .text_color(if context.disabled {
            resolved.disabled_text
        } else {
            resolved.text
        })
        .overflow_hidden()
        .child(column.render_cell(context))
}

/// 渲染分页栏。
fn footer_element<T: 'static>(
    table: Entity<DataTable<T>>,
    page_state: DataTablePageState,
    resolved: ResolvedDataTableStyle,
    page_size_select: Entity<Select>,
    interactive: bool,
    window: &mut Window,
) -> impl IntoElement {
    div()
        .id("xgpui-data-table-footer")
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(resolved.footer_height)
        .px(resolved.padding)
        .gap(resolved.gap)
        .border_1()
        .border_color(resolved.row_border)
        .text_color(resolved.muted_text)
        .child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .child(SharedString::from(format!(
                    "共 {} 条",
                    page_state.total_rows
                ))),
        )
        .child(
            // 分页控件和 page size Select 作为一个整体靠右排列，避免中间控件与 Select
            // 被两个 flex_1 区域强行拉开，形成截图中的大段空白。
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .child(page_button(
                            table.clone(),
                            LucideIcon::ChevronsLeft,
                            !interactive || page_state.page <= 1,
                            resolved,
                            window,
                            DataTablePageButtonKind::First,
                        ))
                        .child(page_button(
                            table.clone(),
                            LucideIcon::ChevronLeft,
                            !interactive || page_state.page <= 1,
                            resolved,
                            window,
                            DataTablePageButtonKind::Previous,
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_w(px(36.0))
                                .text_color(resolved.text)
                                .child(SharedString::from(format!(
                                    "{}/{}",
                                    page_state.page, page_state.total_pages
                                ))),
                        )
                        .child(page_button(
                            table.clone(),
                            LucideIcon::ChevronRight,
                            !interactive || page_state.page >= page_state.total_pages,
                            resolved,
                            window,
                            DataTablePageButtonKind::Next,
                        ))
                        .child(page_button(
                            table,
                            LucideIcon::ChevronsRight,
                            !interactive || page_state.page >= page_state.total_pages,
                            resolved,
                            window,
                            DataTablePageButtonKind::Last,
                        )),
                )
                .child(
                    // Select 自身高度接近小尺寸表格 footer，高度由外层 footer 兜底；
                    // 这里保留稳定宽度并依赖 footer padding 形成右侧边距，避免紧凑表格贴边。
                    div().w(px(118.0)).child(page_size_select),
                ),
        )
}

/// 分页按钮类型。
///
/// 底部分页参考常见表格布局提供首页、上一页、下一页和末页四个按钮。使用枚举可以让按钮渲染
/// 与事件分发保持集中，避免为四个按钮复制几乎相同的构建代码。
#[derive(Clone, Copy)]
enum DataTablePageButtonKind {
    /// 跳转到第一页。
    First,
    /// 跳转到上一页。
    Previous,
    /// 跳转到下一页。
    Next,
    /// 跳转到最后一页。
    Last,
}

impl DataTablePageButtonKind {
    /// 返回分页按钮的稳定 element id。
    fn element_id(self) -> &'static str {
        match self {
            Self::First => "xgpui-data-table-first-page",
            Self::Previous => "xgpui-data-table-prev-page",
            Self::Next => "xgpui-data-table-next-page",
            Self::Last => "xgpui-data-table-last-page",
        }
    }
}

/// 渲染分页按钮。
fn page_button<T: 'static>(
    table: Entity<DataTable<T>>,
    icon_name: LucideIcon,
    disabled: bool,
    resolved: ResolvedDataTableStyle,
    window: &mut Window,
    kind: DataTablePageButtonKind,
) -> impl IntoElement {
    div()
        .id(kind.element_id())
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded(px(4.0))
        .opacity(if disabled { 0.45 } else { 1.0 })
        .cursor(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .child(icon::lucide_icon(
            icon_name,
            resolved.muted_text,
            resolved.icon_size,
        ))
        .when(!disabled, |this| {
            this.hover(move |style| style.bg(resolved.row_hover))
                .on_click(
                    window.listener_for(&table, move |this, event, window, cx| match kind {
                        DataTablePageButtonKind::First => this.on_first_page(event, window, cx),
                        DataTablePageButtonKind::Previous => this.on_prev_page(event, window, cx),
                        DataTablePageButtonKind::Next => this.on_next_page(event, window, cx),
                        DataTablePageButtonKind::Last => this.on_last_page(event, window, cx),
                    }),
                )
        })
}

/// 创建复选框或 radio-like 选择器。
fn checkbox_element(checked: bool, radio: bool, resolved: ResolvedDataTableStyle) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(resolved.checkbox_size)
        .rounded(if radio {
            resolved.checkbox_size / 2.0
        } else {
            px(4.0)
        })
        .border_1()
        .border_color(resolved.checkbox_border)
        .bg(if checked {
            resolved.checkbox_checked_background
        } else {
            resolved.checkbox_background
        })
        .when(checked, |this| {
            this.child(icon::lucide_icon(
                LucideIcon::Check,
                resolved.checkbox_checked_text,
                resolved.checkbox_size * 0.82,
            ))
        })
}

/// 创建列容器。
fn column_container(width: Option<Pixels>, min_width: Pixels) -> gpui::Div {
    div()
        .flex()
        .min_w(min_width)
        .when_some(width, |this, width| this.w(width).flex_none())
        .when(width.is_none(), |this| this.flex_1())
}

/// 根据列对齐设置 flex 主轴对齐。
fn apply_align(this: gpui::Div, align: DataTableAlign) -> gpui::Div {
    match align {
        DataTableAlign::Left => this.justify_start(),
        DataTableAlign::Center => this.justify_center(),
        DataTableAlign::Right => this.justify_end(),
    }
}

/// 根据当前页行数计算表体高度。
fn body_height(row_count: usize, resolved: ResolvedDataTableStyle, max_height: Pixels) -> Pixels {
    let rows = row_count.max(1);
    (resolved.row_height * rows as f32)
        .min(max_height)
        .max(resolved.row_height)
}

/// 规范化分页尺寸候选。
///
/// Props 允许调用方传入任意 `page_size_options`；这里会过滤 0、去重并补入当前 page size，
/// 保证 Select 始终能展示当前受控值。
fn normalized_page_sizes(options: &[usize], current_page_size: usize) -> Vec<usize> {
    let mut sizes = options
        .iter()
        .copied()
        .filter(|size| *size > 0)
        .collect::<Vec<_>>();
    let current_page_size = current_page_size.max(1);
    if !sizes.contains(&current_page_size) {
        sizes.push(current_page_size);
    }
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

/// 根据原始分页尺寸候选生成 Select 选项。
fn page_size_select_options(options: &[usize], current_page_size: usize) -> Vec<SelectOption> {
    page_size_select_options_from_sizes(&normalized_page_sizes(options, current_page_size))
}

/// 根据已经规范化的分页尺寸生成 Select 选项。
fn page_size_select_options_from_sizes(sizes: &[usize]) -> Vec<SelectOption> {
    sizes
        .iter()
        .map(|size| {
            SelectOption::new(
                page_size_value(*size),
                SharedString::from(format!("{} 条/页", size)),
            )
        })
        .collect()
}

/// 把分页尺寸转换为 Select 的稳定值。
fn page_size_value(page_size: usize) -> SharedString {
    SharedString::from(page_size.max(1).to_string())
}
