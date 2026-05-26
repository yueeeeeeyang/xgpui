//! `DataTable` 的纯状态管理。
//!
//! 本模块不依赖 gpui 渲染上下文，只负责过滤、排序、分页、选择和键盘活动项。
//! 渲染层把这里的派生结果映射为具体元素，从而让核心表格规则可以通过普通单元测试覆盖。

use std::collections::HashSet;

use gpui::SharedString;

use super::props::{
    DataTableColumn, DataTablePageState, DataTableRowDisabled, DataTableRowKey,
    DataTableSelectionMode, DataTableSort, DataTableSortDirection,
};

/// DataTable 状态变更结果。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DataTableStateOutcome {
    /// 过滤文本是否变化。
    pub filter_changed: bool,
    /// 排序状态是否变化。
    pub sort_changed: bool,
    /// 分页状态是否变化。
    pub page_changed: bool,
    /// 选择集合是否变化。
    pub selection_changed: bool,
    /// 键盘活动行是否变化。
    pub active_changed: bool,
}

impl DataTableStateOutcome {
    /// 判断是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.filter_changed
            || self.sort_changed
            || self.page_changed
            || self.selection_changed
            || self.active_changed
    }
}

/// DataTable 行索引记录。
///
/// 该结构保存去重后的行元信息。重复 row key 会保留第一次出现的行，后续重复行不参与过滤、
/// 排序、分页或选择，避免同一个 key 在表格状态中对应多个位置。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTableRowRecord {
    /// 原始 rows 下标。
    pub row_index: usize,
    /// 稳定 row key。
    pub row_key: SharedString,
    /// 行是否禁用。
    pub disabled: bool,
}

/// DataTable 当前视图派生结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTableView {
    /// 去重后的所有行。
    pub records: Vec<DataTableRowRecord>,
    /// 过滤和排序后的行。
    pub processed_rows: Vec<DataTableRowRecord>,
    /// 当前页行。
    pub page_rows: Vec<DataTableRowRecord>,
    /// 当前分页状态。
    pub page_state: DataTablePageState,
}

/// DataTable 核心状态。
#[derive(Clone, Debug)]
pub struct DataTableState {
    filter_text: SharedString,
    sort: Option<DataTableSort>,
    page: usize,
    page_size: usize,
    selected_row_keys: Vec<SharedString>,
    active_row_key: Option<SharedString>,
}

impl DataTableState {
    /// 创建新的 DataTable 状态。
    pub fn new(
        filter_text: SharedString,
        sort: Option<DataTableSort>,
        page: usize,
        page_size: usize,
        selected_row_keys: Vec<SharedString>,
        selection_mode: DataTableSelectionMode,
    ) -> Self {
        Self {
            filter_text,
            sort,
            page: page.max(1),
            page_size: page_size.max(1),
            selected_row_keys: normalize_selected_keys(selected_row_keys, selection_mode),
            active_row_key: None,
        }
    }

    /// 返回过滤文本。
    pub fn filter_text(&self) -> &SharedString {
        &self.filter_text
    }

    /// 返回排序状态。
    pub fn sort(&self) -> Option<&DataTableSort> {
        self.sort.as_ref()
    }

    /// 返回当前页码。
    pub fn page(&self) -> usize {
        self.page
    }

    /// 返回每页条数。
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// 返回 selected row key。
    pub fn selected_row_keys(&self) -> &[SharedString] {
        &self.selected_row_keys
    }

    /// 返回键盘活动行 key。
    pub fn active_row_key(&self) -> Option<&SharedString> {
        self.active_row_key.as_ref()
    }

    /// 静默同步过滤文本。
    pub fn set_filter_text_silent<T: 'static>(
        &mut self,
        filter_text: SharedString,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let filter_changed = self.filter_text != filter_text;
        self.filter_text = filter_text;
        let page_changed = self.clamp_page(rows, columns, row_key, row_disabled);
        let active_changed = self.sync_active_to_view(rows, columns, row_key, row_disabled);

        DataTableStateOutcome {
            filter_changed,
            page_changed,
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 静默同步排序状态。
    pub fn set_sort_silent<T: 'static>(
        &mut self,
        sort: Option<DataTableSort>,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let sort = normalize_sort(sort, columns);
        let sort_changed = self.sort != sort;
        self.sort = sort;
        let page_changed = self.clamp_page(rows, columns, row_key, row_disabled);
        let active_changed = self.sync_active_to_view(rows, columns, row_key, row_disabled);

        DataTableStateOutcome {
            sort_changed,
            page_changed,
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 静默同步页码。
    pub fn set_page_silent<T: 'static>(
        &mut self,
        page: usize,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let before = self.page;
        self.page = page.max(1);
        self.clamp_page(rows, columns, row_key, row_disabled);
        let active_changed = self.sync_active_to_view(rows, columns, row_key, row_disabled);

        DataTableStateOutcome {
            page_changed: self.page != before,
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 静默同步每页条数。
    pub fn set_page_size_silent<T: 'static>(
        &mut self,
        page_size: usize,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let before_size = self.page_size;
        let before_page = self.page;
        self.page_size = page_size.max(1);
        self.clamp_page(rows, columns, row_key, row_disabled);
        let active_changed = self.sync_active_to_view(rows, columns, row_key, row_disabled);

        DataTableStateOutcome {
            page_changed: self.page_size != before_size || self.page != before_page,
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 静默同步 selected row key。
    pub fn set_selected_row_keys_silent(
        &mut self,
        selected_row_keys: Vec<SharedString>,
        selection_mode: DataTableSelectionMode,
    ) -> DataTableStateOutcome {
        let selected_row_keys = normalize_selected_keys(selected_row_keys, selection_mode);
        let selection_changed = self.selected_row_keys != selected_row_keys;
        self.selected_row_keys = selected_row_keys;

        DataTableStateOutcome {
            selection_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// rows 或 columns 改变后同步派生状态。
    pub fn sync_inputs_silent<T: 'static>(
        &mut self,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let sort_before = self.sort.clone();
        self.sort = normalize_sort(self.sort.clone(), columns);
        let sort_changed = self.sort != sort_before;
        let page_changed = self.clamp_page(rows, columns, row_key, row_disabled);
        let active_changed = self.sync_active_to_view(rows, columns, row_key, row_disabled);

        DataTableStateOutcome {
            sort_changed,
            page_changed,
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 表头点击排序时循环 None -> Asc -> Desc -> None。
    pub fn cycle_sort<T: 'static>(
        &mut self,
        column_key: &SharedString,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        let Some(column) = columns
            .iter()
            .find(|column| &column.key == column_key && column.is_sortable())
        else {
            return DataTableStateOutcome::default();
        };

        let next_sort = match &self.sort {
            Some(sort)
                if sort.column_key == column.key
                    && sort.direction == DataTableSortDirection::Asc =>
            {
                Some(DataTableSort::new(
                    column.key.clone(),
                    DataTableSortDirection::Desc,
                ))
            }
            Some(sort)
                if sort.column_key == column.key
                    && sort.direction == DataTableSortDirection::Desc =>
            {
                None
            }
            _ => Some(DataTableSort::new(
                column.key.clone(),
                DataTableSortDirection::Asc,
            )),
        };
        self.set_sort_silent(next_sort, rows, columns, row_key, row_disabled)
    }

    /// 切换指定行选择状态。
    pub fn toggle_row_selection(
        &mut self,
        row_key: &SharedString,
        disabled: bool,
        selection_mode: DataTableSelectionMode,
    ) -> DataTableStateOutcome {
        if disabled || selection_mode == DataTableSelectionMode::None {
            return DataTableStateOutcome::default();
        }

        let before = self.selected_row_keys.clone();
        match selection_mode {
            DataTableSelectionMode::None => {}
            DataTableSelectionMode::Single => {
                self.selected_row_keys = vec![row_key.clone()];
            }
            DataTableSelectionMode::Multiple => {
                if self.selected_row_keys.iter().any(|key| key == row_key) {
                    self.selected_row_keys.retain(|key| key != row_key);
                } else {
                    self.selected_row_keys.push(row_key.clone());
                }
                self.selected_row_keys = dedupe_keys(self.selected_row_keys.clone());
            }
        }

        DataTableStateOutcome {
            selection_changed: self.selected_row_keys != before,
            ..DataTableStateOutcome::default()
        }
    }

    /// 多选模式下切换当前页全选。
    pub fn toggle_page_selection(
        &mut self,
        page_rows: &[DataTableRowRecord],
        selection_mode: DataTableSelectionMode,
    ) -> DataTableStateOutcome {
        if selection_mode != DataTableSelectionMode::Multiple {
            return DataTableStateOutcome::default();
        }

        let selectable = page_rows
            .iter()
            .filter(|record| !record.disabled)
            .map(|record| record.row_key.clone())
            .collect::<Vec<_>>();
        if selectable.is_empty() {
            return DataTableStateOutcome::default();
        }

        let before = self.selected_row_keys.clone();
        let mut selected = self.selected_set();
        let all_selected = selectable.iter().all(|key| selected.contains(key));
        if all_selected {
            // 当前页已经全选时只移除当前页可选行，保留其他页面或外部同步的选择。
            let selectable_set = selectable.into_iter().collect::<HashSet<_>>();
            self.selected_row_keys
                .retain(|key| !selectable_set.contains(key));
        } else {
            // 当前页尚未全选时按页内顺序追加缺失 key，保持回调中 selected_row_keys 的顺序
            // 接近用户看到和操作的顺序，而不是 HashSet 的随机顺序。
            for key in selectable {
                if selected.insert(key.clone()) {
                    self.selected_row_keys.push(key);
                }
            }
        }
        self.selected_row_keys = dedupe_keys(self.selected_row_keys.clone());

        DataTableStateOutcome {
            selection_changed: self.selected_row_keys != before,
            ..DataTableStateOutcome::default()
        }
    }

    /// 清空过滤文本。
    pub fn clear_filter<T: 'static>(
        &mut self,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableStateOutcome {
        self.set_filter_text_silent(
            SharedString::default(),
            rows,
            columns,
            row_key,
            row_disabled,
        )
    }

    /// 清空行选择。
    pub fn clear_selection(&mut self) -> DataTableStateOutcome {
        let selection_changed = !self.selected_row_keys.is_empty();
        self.selected_row_keys.clear();
        DataTableStateOutcome {
            selection_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 移动键盘活动行。
    pub fn move_active_by(
        &mut self,
        page_rows: &[DataTableRowRecord],
        delta: isize,
    ) -> DataTableStateOutcome {
        let Some(next_index) =
            enabled_page_index_after_move(page_rows, self.active_row_key.as_ref(), delta)
        else {
            return self.clear_active();
        };
        self.set_active_to_page_index(page_rows, next_index)
    }

    /// 移动到当前页首个可操作行。
    pub fn move_active_first(&mut self, page_rows: &[DataTableRowRecord]) -> DataTableStateOutcome {
        let Some(index) = first_enabled_page_index(page_rows) else {
            return self.clear_active();
        };
        self.set_active_to_page_index(page_rows, index)
    }

    /// 移动到当前页最后一个可操作行。
    pub fn move_active_last(&mut self, page_rows: &[DataTableRowRecord]) -> DataTableStateOutcome {
        let Some(index) = last_enabled_page_index(page_rows) else {
            return self.clear_active();
        };
        self.set_active_to_page_index(page_rows, index)
    }

    /// 切换当前活动行选择状态。
    pub fn toggle_active_selection(
        &mut self,
        page_rows: &[DataTableRowRecord],
        selection_mode: DataTableSelectionMode,
    ) -> DataTableStateOutcome {
        let Some(active) = self.active_row_key.clone() else {
            return DataTableStateOutcome::default();
        };
        let Some(record) = page_rows.iter().find(|record| record.row_key == active) else {
            return DataTableStateOutcome::default();
        };
        self.toggle_row_selection(&record.row_key, record.disabled, selection_mode)
    }

    /// 构建当前表格视图。
    pub fn view<T: 'static>(
        &self,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> DataTableView {
        let records = build_records(rows, row_key, row_disabled);
        let mut processed_rows = filter_records(rows, columns, &records, self.filter_text.as_str());
        sort_records(rows, columns, &mut processed_rows, self.sort.as_ref());
        let page_state = page_state(processed_rows.len(), self.page, self.page_size);
        let page_rows = page_records(&processed_rows, page_state);

        DataTableView {
            records,
            processed_rows,
            page_rows,
            page_state,
        }
    }

    /// 返回 selected 集合。
    pub fn selected_set(&self) -> HashSet<SharedString> {
        self.selected_row_keys.iter().cloned().collect()
    }

    /// 根据当前数据把页码限制到合法范围。
    fn clamp_page<T: 'static>(
        &mut self,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> bool {
        let before = self.page;
        let view = self.view(rows, columns, row_key, row_disabled);
        self.page = self.page.clamp(1, view.page_state.total_pages);
        self.page != before
    }

    /// 保证活动行仍处于当前页可操作行内。
    fn sync_active_to_view<T: 'static>(
        &mut self,
        rows: &[T],
        columns: &[DataTableColumn<T>],
        row_key: &DataTableRowKey<T>,
        row_disabled: Option<&DataTableRowDisabled<T>>,
    ) -> bool {
        let view = self.view(rows, columns, row_key, row_disabled);
        if self.active_row_key.as_ref().is_some_and(|key| {
            view.page_rows
                .iter()
                .any(|record| &record.row_key == key && !record.disabled)
        }) {
            return false;
        }

        let next = view
            .page_rows
            .iter()
            .find(|record| !record.disabled)
            .map(|record| record.row_key.clone());
        let changed = self.active_row_key != next;
        self.active_row_key = next;
        changed
    }

    /// 清空活动行。
    fn clear_active(&mut self) -> DataTableStateOutcome {
        let active_changed = self.active_row_key.take().is_some();
        DataTableStateOutcome {
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }

    /// 设置活动行为当前页指定下标。
    fn set_active_to_page_index(
        &mut self,
        page_rows: &[DataTableRowRecord],
        page_index: usize,
    ) -> DataTableStateOutcome {
        let Some(record) = page_rows.get(page_index) else {
            return DataTableStateOutcome::default();
        };
        if record.disabled {
            return DataTableStateOutcome::default();
        }
        let active_changed = self.active_row_key.as_ref() != Some(&record.row_key);
        self.active_row_key = Some(record.row_key.clone());
        DataTableStateOutcome {
            active_changed,
            ..DataTableStateOutcome::default()
        }
    }
}

/// 构建去重后的行记录。
pub fn build_records<T: 'static>(
    rows: &[T],
    row_key: &DataTableRowKey<T>,
    row_disabled: Option<&DataTableRowDisabled<T>>,
) -> Vec<DataTableRowRecord> {
    let mut seen = HashSet::new();
    let mut records = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let key = row_key(row);
        if !seen.insert(key.clone()) {
            continue;
        }
        let disabled = row_disabled.map(|disabled| disabled(row)).unwrap_or(false);
        records.push(DataTableRowRecord {
            row_index,
            row_key: key,
            disabled,
        });
    }
    records
}

/// 过滤行记录。
fn filter_records<T: 'static>(
    rows: &[T],
    columns: &[DataTableColumn<T>],
    records: &[DataTableRowRecord],
    filter_text: &str,
) -> Vec<DataTableRowRecord> {
    let filter = filter_text.trim().to_lowercase();
    if filter.is_empty() {
        return records.to_vec();
    }

    let filterable_columns = columns
        .iter()
        .filter(|column| column.is_filterable())
        .collect::<Vec<_>>();
    records
        .iter()
        .filter(|record| {
            let row = &rows[record.row_index];
            filterable_columns.iter().any(|column| {
                column
                    .text_value(row)
                    .map(|value| value.as_str().to_lowercase().contains(&filter))
                    .unwrap_or(false)
            })
        })
        .cloned()
        .collect()
}

/// 排序行记录。
fn sort_records<T: 'static>(
    rows: &[T],
    columns: &[DataTableColumn<T>],
    records: &mut [DataTableRowRecord],
    sort: Option<&DataTableSort>,
) {
    let Some(sort) = sort else {
        return;
    };
    let Some(column) = columns
        .iter()
        .find(|column| column.key == sort.column_key && column.is_sortable())
    else {
        return;
    };

    records.sort_by(|a, b| {
        let left = column
            .text_value(&rows[a.row_index])
            .map(|value| value.as_str().to_lowercase())
            .unwrap_or_default();
        let right = column
            .text_value(&rows[b.row_index])
            .map(|value| value.as_str().to_lowercase())
            .unwrap_or_default();
        match sort.direction {
            DataTableSortDirection::Asc => left.cmp(&right).then(a.row_index.cmp(&b.row_index)),
            DataTableSortDirection::Desc => right.cmp(&left).then(a.row_index.cmp(&b.row_index)),
        }
    });
}

/// 计算分页状态。
fn page_state(total_rows: usize, page: usize, page_size: usize) -> DataTablePageState {
    let page_size = page_size.max(1);
    let total_pages = total_rows.div_ceil(page_size).max(1);
    DataTablePageState {
        page: page.clamp(1, total_pages),
        page_size,
        total_rows,
        total_pages,
    }
}

/// 返回当前页行。
fn page_records(
    records: &[DataTableRowRecord],
    page_state: DataTablePageState,
) -> Vec<DataTableRowRecord> {
    let start = (page_state.page - 1) * page_state.page_size;
    let end = (start + page_state.page_size).min(records.len());
    records[start..end].to_vec()
}

/// 规范化 selected row key。
fn normalize_selected_keys(
    keys: Vec<SharedString>,
    selection_mode: DataTableSelectionMode,
) -> Vec<SharedString> {
    match selection_mode {
        DataTableSelectionMode::None => Vec::new(),
        DataTableSelectionMode::Single => keys.into_iter().next().into_iter().collect(),
        DataTableSelectionMode::Multiple => dedupe_keys(keys),
    }
}

/// 对 key 列表去重并保留首次出现顺序。
fn dedupe_keys(keys: Vec<SharedString>) -> Vec<SharedString> {
    let mut seen = HashSet::new();
    keys.into_iter()
        .filter(|key| seen.insert(key.clone()))
        .collect()
}

/// 根据当前列规范化排序状态。
fn normalize_sort<T: 'static>(
    sort: Option<DataTableSort>,
    columns: &[DataTableColumn<T>],
) -> Option<DataTableSort> {
    let sort = sort?;
    columns
        .iter()
        .any(|column| column.key == sort.column_key && column.is_sortable())
        .then_some(sort)
}

/// 根据当前方向键位移计算下一个非禁用当前页行下标。
fn enabled_page_index_after_move(
    page_rows: &[DataTableRowRecord],
    active_key: Option<&SharedString>,
    delta: isize,
) -> Option<usize> {
    let enabled_indices = page_rows
        .iter()
        .enumerate()
        .filter_map(|(index, record)| (!record.disabled).then_some(index))
        .collect::<Vec<_>>();
    if enabled_indices.is_empty() {
        return None;
    }

    let current = active_key
        .and_then(|key| {
            enabled_indices
                .iter()
                .position(|page_index| &page_rows[*page_index].row_key == key)
        })
        .unwrap_or(0);
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current + delta as usize).min(enabled_indices.len() - 1)
    };
    enabled_indices.get(next).copied()
}

/// 返回当前页首个非禁用行下标。
fn first_enabled_page_index(page_rows: &[DataTableRowRecord]) -> Option<usize> {
    page_rows.iter().position(|record| !record.disabled)
}

/// 返回当前页最后一个非禁用行下标。
fn last_enabled_page_index(page_rows: &[DataTableRowRecord]) -> Option<usize> {
    page_rows.iter().rposition(|record| !record.disabled)
}
