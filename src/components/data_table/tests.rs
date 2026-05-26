//! `DataTable` 状态与公开同步方法测试。
//!
//! 状态测试聚焦过滤、排序、分页、重复 row key 和选择规则；组件方法测试使用 gpui
//! 测试上下文确认受控同步方法不会意外触发用户交互回调。

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{div, AppContext, IntoElement, ParentElement, SharedString, TestAppContext};

use super::{
    props::{DataTableRowDisabled, DataTableRowKey, DataTableSortDirection},
    state::DataTableState,
    DataTable, DataTableCellContext, DataTableColumn, DataTableProps, DataTableSelectionMode,
    DataTableSort, DataTableStatus,
};

/// 测试用行数据。
#[derive(Clone, Debug)]
struct Row {
    /// 稳定行 key。
    id: &'static str,
    /// 名称列。
    name: &'static str,
    /// 状态列。
    status: &'static str,
}

/// 构造 SharedString，减少断言中的样板代码。
fn s(value: &str) -> SharedString {
    SharedString::from(value.to_owned())
}

/// 构造测试用标准行数据。
fn rows() -> Vec<Row> {
    vec![
        Row {
            id: "1",
            name: "Alpha",
            status: "Ready",
        },
        Row {
            id: "2",
            name: "Beta",
            status: "Blocked",
        },
        Row {
            id: "3",
            name: "Beta",
            status: "Ready",
        },
        Row {
            id: "4",
            name: "Gamma",
            status: "Done",
        },
    ]
}

/// DataTable 的 row key 使用业务稳定 id，不依赖过滤、排序和分页后的下标。
fn row_key() -> DataTableRowKey<Row> {
    Rc::new(|row: &Row| s(row.id))
}

/// 标准文本列配置。
fn text_columns() -> Vec<DataTableColumn<Row>> {
    vec![
        DataTableColumn::text("name", "名称", |row: &Row| s(row.name)),
        DataTableColumn::text("status", "状态", |row: &Row| s(row.status)),
    ]
}

/// 过滤只匹配可过滤文本列，不能匹配操作列按钮文案。
#[test]
fn filter_uses_text_columns_only() {
    let mut columns = text_columns();
    columns.push(DataTableColumn::actions("actions", "操作", |_| {
        div().child("删除").into_any_element()
    }));
    let state = DataTableState::new(
        s("删除"),
        None,
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::None,
    );

    let view = state.view(&rows(), &columns, &row_key(), None);

    assert!(view.processed_rows.is_empty());

    let state = DataTableState::new(
        s("alpha"),
        None,
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::None,
    );
    let view = state.view(&rows(), &columns, &row_key(), None);
    assert_eq!(view.processed_rows[0].row_key, s("1"));
}

/// 文本列排序应大小写不敏感，并在相同排序值时保留原始行顺序。
#[test]
fn text_sort_is_stable_for_equal_values() {
    let mut state = DataTableState::new(
        SharedString::default(),
        Some(DataTableSort::new("name", DataTableSortDirection::Asc)),
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::None,
    );
    state.sync_inputs_silent(&rows(), &text_columns(), &row_key(), None);

    let view = state.view(&rows(), &text_columns(), &row_key(), None);
    let keys = view
        .processed_rows
        .iter()
        .map(|record| record.row_key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["1", "2", "3", "4"]);

    state.set_sort_silent(
        Some(DataTableSort::new("name", DataTableSortDirection::Desc)),
        &rows(),
        &text_columns(),
        &row_key(),
        None,
    );
    let view = state.view(&rows(), &text_columns(), &row_key(), None);
    let keys = view
        .processed_rows
        .iter()
        .map(|record| record.row_key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["4", "2", "3", "1"]);
}

/// 展示列和操作列不可排序；外部同步到这些列时应被规范化为空排序。
#[test]
fn display_and_action_columns_are_not_sortable() {
    let columns = vec![
        DataTableColumn::text("name", "名称", |row: &Row| s(row.name)),
        DataTableColumn::actions("actions", "操作", |_| {
            div().child("编辑").into_any_element()
        }),
    ];
    let mut state = DataTableState::new(
        SharedString::default(),
        Some(DataTableSort::new("actions", DataTableSortDirection::Asc)),
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::None,
    );

    let outcome = state.sync_inputs_silent(&rows(), &columns, &row_key(), None);

    assert!(outcome.sort_changed);
    assert!(state.sort().is_none());
}

/// 过滤、page size 和 rows 更新后页码应夹到合法范围。
#[test]
fn pagination_clamps_after_filter_page_size_and_rows_change() {
    let mut state = DataTableState::new(
        SharedString::default(),
        None,
        3,
        2,
        Vec::new(),
        DataTableSelectionMode::None,
    );
    state.sync_inputs_silent(&rows(), &text_columns(), &row_key(), None);
    assert_eq!(
        state
            .view(&rows(), &text_columns(), &row_key(), None)
            .page_state
            .page,
        2
    );

    state.set_filter_text_silent(s("alpha"), &rows(), &text_columns(), &row_key(), None);
    let page_state = state
        .view(&rows(), &text_columns(), &row_key(), None)
        .page_state;
    assert_eq!(page_state.page, 1);
    assert_eq!(page_state.total_rows, 1);

    state.set_page_size_silent(1, &rows(), &text_columns(), &row_key(), None);
    assert_eq!(state.page_size(), 1);

    let smaller_rows = vec![Row {
        id: "1",
        name: "Alpha",
        status: "Ready",
    }];
    state.set_page_silent(8, &smaller_rows, &text_columns(), &row_key(), None);
    assert_eq!(
        state
            .view(&smaller_rows, &text_columns(), &row_key(), None)
            .page_state
            .page,
        1
    );
}

/// 重复 row key 保留第一次出现的行，并且不应导致状态派生 panic。
#[test]
fn duplicate_row_keys_keep_first_record() {
    let duplicate_rows = vec![
        Row {
            id: "1",
            name: "First",
            status: "Ready",
        },
        Row {
            id: "1",
            name: "Duplicate",
            status: "Ready",
        },
        Row {
            id: "2",
            name: "Second",
            status: "Ready",
        },
    ];
    let state = DataTableState::new(
        SharedString::default(),
        None,
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::None,
    );

    let view = state.view(&duplicate_rows, &text_columns(), &row_key(), None);

    assert_eq!(view.records.len(), 2);
    assert_eq!(view.records[0].row_index, 0);
    assert_eq!(view.records[1].row_key, s("2"));
}

/// 禁用行应排除在单行选择、当前页全选和键盘选择之外。
#[test]
fn disabled_rows_are_excluded_from_selection() {
    let disabled: DataTableRowDisabled<Row> = Rc::new(|row: &Row| row.id == "2");
    let mut state = DataTableState::new(
        SharedString::default(),
        None,
        1,
        10,
        Vec::new(),
        DataTableSelectionMode::Multiple,
    );
    state.sync_inputs_silent(&rows(), &text_columns(), &row_key(), Some(&disabled));
    let view = state.view(&rows(), &text_columns(), &row_key(), Some(&disabled));

    let disabled_row = view
        .records
        .iter()
        .find(|record| record.row_key == s("2"))
        .unwrap();
    state.toggle_row_selection(
        &disabled_row.row_key,
        disabled_row.disabled,
        DataTableSelectionMode::Multiple,
    );
    assert!(state.selected_row_keys().is_empty());

    state.toggle_page_selection(&view.page_rows, DataTableSelectionMode::Multiple);
    assert_eq!(state.selected_row_keys(), &[s("1"), s("3"), s("4")]);

    state.set_selected_row_keys_silent(Vec::new(), DataTableSelectionMode::Multiple);
    state.move_active_by(&view.page_rows, 1);
    state.toggle_active_selection(&view.page_rows, DataTableSelectionMode::Multiple);
    assert_eq!(state.selected_row_keys(), &[s("3")]);
}

/// 受控同步方法只更新内部状态，不触发过滤、排序、分页或选择回调。
#[gpui::test]
fn controlled_setters_do_not_emit_interaction_callbacks(cx: &mut TestAppContext) {
    let filter_changes = Rc::new(Cell::new(0));
    let sort_changes = Rc::new(Cell::new(0));
    let page_changes = Rc::new(Cell::new(0));
    let selection_changes = Rc::new(Cell::new(0));
    let filter_changes_for_callback = filter_changes.clone();
    let sort_changes_for_callback = sort_changes.clone();
    let page_changes_for_callback = page_changes.clone();
    let selection_changes_for_callback = selection_changes.clone();

    let table = cx.new(|cx| {
        DataTable::new(
            cx,
            DataTableProps::new(|row: &Row| s(row.id))
                .rows(rows())
                .columns(text_columns())
                .selection_mode(DataTableSelectionMode::Multiple)
                .on_filter_change(move |_| {
                    filter_changes_for_callback.set(filter_changes_for_callback.get() + 1);
                })
                .on_sort_change(move |_| {
                    sort_changes_for_callback.set(sort_changes_for_callback.get() + 1);
                })
                .on_page_change(move |_| {
                    page_changes_for_callback.set(page_changes_for_callback.get() + 1);
                })
                .on_selection_change(move |_| {
                    selection_changes_for_callback.set(selection_changes_for_callback.get() + 1);
                }),
        )
    });

    table.update(cx, |table, cx| {
        table.set_rows(rows(), cx);
        table.set_columns(text_columns(), cx);
        table.set_filter_text("alpha", cx);
        table.set_sort(
            Some(DataTableSort::new("name", DataTableSortDirection::Asc)),
            cx,
        );
        table.set_page(8, cx);
        table.set_page_size(2, cx);
        table.set_selected_row_keys(vec![s("1"), s("3")], cx);
        table.set_loading(true, cx);
        table.set_status(DataTableStatus::Warning, cx);
        table.set_helper_text(Some(s("外部同步")), cx);
    });

    assert_eq!(filter_changes.get(), 0);
    assert_eq!(sort_changes.get(), 0);
    assert_eq!(page_changes.get(), 0);
    assert_eq!(selection_changes.get(), 0);
}

/// 内部过滤输入框变化代表用户输入，应触发 on_filter_change 并重算分页。
#[gpui::test]
fn filter_input_change_emits_callback_and_recomputes_page(cx: &mut TestAppContext) {
    let filter_changes = Rc::new(Cell::new(0));
    let filter_changes_for_callback = filter_changes.clone();
    let table = cx.new(|cx| {
        DataTable::new(
            cx,
            DataTableProps::new(|row: &Row| s(row.id))
                .rows(rows())
                .columns(text_columns())
                .page(2)
                .page_size(2)
                .on_filter_change(move |_| {
                    filter_changes_for_callback.set(filter_changes_for_callback.get() + 1);
                }),
        )
    });

    table.update(cx, |table, cx| {
        table.filter_input.update(cx, |input, cx| {
            input.set_value("alpha", cx);
        });
    });

    assert_eq!(filter_changes.get(), 1);
    table.read_with(cx, |table, _| {
        assert_eq!(table.filter_text(), &s("alpha"));
        assert_eq!(table.page_state().page, 1);
        assert_eq!(table.filtered_row_count(), 1);
    });
}

/// 每页条数 Select 变化代表用户切换分页尺寸，应触发 on_page_change 并重算当前页。
#[gpui::test]
fn page_size_select_change_emits_page_callback(cx: &mut TestAppContext) {
    let page_changes = Rc::new(Cell::new(0));
    let page_changes_for_callback = page_changes.clone();
    let table = cx.new(|cx| {
        DataTable::new(
            cx,
            DataTableProps::new(|row: &Row| s(row.id))
                .rows(rows())
                .columns(text_columns())
                .page(2)
                .page_size(2)
                .page_size_options(vec![1, 2, 4])
                .on_page_change(move |_| {
                    page_changes_for_callback.set(page_changes_for_callback.get() + 1);
                }),
        )
    });

    table.update(cx, |table, cx| {
        table.page_size_select.update(cx, |select, cx| {
            select.set_value(Some(s("4")), cx);
        });
    });

    assert_eq!(page_changes.get(), 1);
    table.read_with(cx, |table, _| {
        let page_state = table.page_state();
        assert_eq!(page_state.page_size, 4);
        assert_eq!(page_state.page, 1);
        assert_eq!(page_state.total_pages, 1);
    });
}

/// 操作列 renderer 应收到完整单元格上下文，便于调用方渲染按钮或图标。
#[test]
fn action_column_renderer_receives_cell_context() {
    let captured = Rc::new(RefCell::new(None));
    let captured_for_renderer = captured.clone();
    let column = DataTableColumn::actions(
        "actions",
        "操作",
        move |ctx: DataTableCellContext<'_, Row>| {
            *captured_for_renderer.borrow_mut() = Some((
                ctx.row.name.to_owned(),
                ctx.row_key.clone(),
                ctx.row_index,
                ctx.page_row_index,
                ctx.selected,
                ctx.disabled,
            ));
            div().child("查看").into_any_element()
        },
    );
    let row = Row {
        id: "9",
        name: "Context",
        status: "Ready",
    };

    let _ = column.render_cell(DataTableCellContext {
        row: &row,
        row_key: &s("9"),
        row_index: 7,
        page_row_index: 2,
        selected: true,
        disabled: false,
    });

    assert_eq!(
        captured.borrow().as_ref(),
        Some(&(String::from("Context"), s("9"), 7, 2, true, false))
    );
}
