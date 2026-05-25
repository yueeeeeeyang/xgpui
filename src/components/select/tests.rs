//! `Select` 状态与受控同步测试。
//!
//! 大多数测试聚焦选择、清除、搜索过滤和键盘高亮的纯状态逻辑；少量测试使用 gpui 测试上下文
//! 覆盖公开 Entity 方法，但不启动真实窗口，从而保持测试稳定和快速。

use std::{cell::Cell, rc::Rc};

use gpui::{AppContext, SharedString, TestAppContext};

use super::{
    props::{SelectOption, SelectProps, SelectStatus},
    state::{filtered_indices_for, SelectState},
    Select,
};

/// 构造测试选项列表。
fn options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("apple", "Apple"),
        SelectOption::new("banana", "Banana").disabled(true),
        SelectOption::new("blueberry", "Blueberry"),
        SelectOption::new("orange", "Orange"),
    ]
}

/// 选择可用选项应更新值并关闭面板。
#[test]
fn select_available_option_updates_value_and_closes_popup() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);

    let outcome = state.select_index(2, &options);

    assert!(outcome.value_changed);
    assert!(outcome.open_changed);
    assert_eq!(state.value(), Some(&SharedString::from("blueberry")));
    assert!(!state.is_open());
}

/// 外部同步值应更新展示值但不要求调用方触发 on_change。
#[test]
fn set_value_silent_updates_value_without_interaction_semantics() {
    let options = options();
    let mut state = SelectState::new(None, &options);

    let outcome = state.set_value_silent(Some(SharedString::from("orange")), &options);

    assert!(outcome.value_changed);
    assert_eq!(
        state.selected_label(&options),
        Some(SharedString::from("Orange"))
    );
}

/// 外部同步选项应保留当前值，并允许展示文案随新选项刷新。
#[test]
fn sync_options_keeps_value_and_refreshes_selected_label() {
    let options = options();
    let mut state = SelectState::new(Some(SharedString::from("apple")), &options);
    let renamed_options = vec![
        SelectOption::new("apple", "Apple Updated"),
        SelectOption::new("orange", "Orange"),
    ];

    let outcome = state.sync_options_silent(&renamed_options);

    assert!(!outcome.value_changed);
    assert_eq!(state.value(), Some(&SharedString::from("apple")));
    assert_eq!(
        state.selected_label(&renamed_options),
        Some(SharedString::from("Apple Updated"))
    );
}

/// 新选项缺少当前值时也不能静默清空，清空决策应由父组件显式同步。
#[test]
fn sync_options_preserves_missing_value() {
    let options = options();
    let mut state = SelectState::new(Some(SharedString::from("apple")), &options);
    let remote_options = vec![SelectOption::new("coffee", "Coffee")];

    let outcome = state.sync_options_silent(&remote_options);

    assert!(!outcome.value_changed);
    assert_eq!(state.value(), Some(&SharedString::from("apple")));
    assert_eq!(state.selected_label(&remote_options), None);
    assert_eq!(state.highlighted_index(), Some(0));
}

/// 下拉打开并带有搜索词时，选项同步应按当前搜索结果重新选择可高亮项。
#[test]
fn sync_options_recomputes_highlight_with_current_search() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    state.set_search("ap", &options);
    let remote_options = vec![
        SelectOption::new("disabled", "Apricot disabled").disabled(true),
        SelectOption::new("apricot", "Apricot"),
        SelectOption::new("orange", "Orange"),
    ];

    let outcome = state.sync_options_silent(&remote_options);

    assert!(outcome.highlight_changed);
    assert_eq!(state.filtered_indices(&remote_options), vec![0, 1]);
    assert_eq!(state.highlighted_index(), Some(1));
}

/// 清除应移除当前值并把高亮恢复到第一个可选项。
#[test]
fn clear_removes_value_and_restores_first_selectable_highlight() {
    let options = options();
    let mut state = SelectState::new(Some(SharedString::from("orange")), &options);

    let outcome = state.clear(&options);

    assert!(outcome.value_changed);
    assert!(state.value().is_none());
    assert_eq!(state.highlighted_index(), Some(0));
}

/// 搜索过滤应大小写不敏感并保留原始顺序。
#[test]
fn search_filter_is_case_insensitive_and_keeps_order() {
    let options = options();

    assert_eq!(filtered_indices_for("B", &options), vec![1, 2]);
    assert_eq!(filtered_indices_for("berry", &options), vec![2]);
    assert!(filtered_indices_for("missing", &options).is_empty());
}

/// 搜索词变化后高亮应跳过禁用项。
#[test]
fn search_highlight_skips_disabled_options() {
    let options = options();
    let mut state = SelectState::new(None, &options);

    let outcome = state.set_search("ban", &options);

    assert!(outcome.search_changed);
    assert_eq!(state.filtered_indices(&options), vec![1]);
    assert_eq!(state.highlighted_index(), None);
}

/// 搜索文本替换应更新过滤词、光标和过滤结果。
#[test]
fn search_replacement_updates_filter_and_cursor() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);

    let outcome = state.replace_search_text_in_range(None, "Blue", &options);

    assert!(outcome.search_changed);
    assert!(outcome.search_selection_changed);
    assert_eq!(state.search(), &SharedString::from("Blue"));
    assert_eq!(state.search_selected_range(), 4..4);
    assert_eq!(state.filtered_indices(&options), vec![2]);
}

/// 搜索选区替换应按单行输入处理粘贴内容，并保持字素簇级光标移动。
#[test]
fn search_selection_replace_normalizes_multiline_and_moves_by_grapheme() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    state.replace_search_text_in_range(None, "a👨‍👩‍👧‍👦b", &options);

    let cursor_before_b = state.previous_search_boundary(state.search_cursor_offset());
    state.move_search_cursor(cursor_before_b);
    let family_start = state.previous_search_boundary(state.search_cursor_offset());
    state.select_search_to(family_start);
    let outcome = state.replace_search_text_in_range(None, "x\ny", &options);

    assert!(outcome.search_changed);
    assert_eq!(state.search(), &SharedString::from("ax yb"));
    assert_eq!(state.search_selected_range(), 4..4);
}

/// 搜索光标即使被鼠标定位到复合字素内部，也应回到字素边界，避免搜索词被拆坏。
#[test]
fn search_cursor_offsets_inside_grapheme_are_clamped() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    state.replace_search_text_in_range(None, "a👨‍👩‍👧‍👦b", &options);
    let family_start = "a".len();
    let inside_family = family_start + "👨".len();

    let outcome = state.move_search_cursor(inside_family);

    assert!(outcome.search_selection_changed);
    assert_eq!(state.search_selected_range(), family_start..family_start);
}

/// 搜索输入的部分字素替换应扩展到完整字素，避免过滤词留下半个复合 emoji。
#[test]
fn partial_search_grapheme_replacement_expands_to_full_grapheme() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    state.replace_search_text_in_range(None, "a👨‍👩‍👧‍👦b", &options);
    let family_start = "a".len();
    let family_mid = family_start + "👨".len();
    let family_inner_end = family_mid + "\u{200d}".len();
    let start_utf16 = state.search_offset_to_utf16(family_mid);
    let end_utf16 = state.search_offset_to_utf16(family_inner_end);

    let outcome = state.replace_search_text_in_range(Some(start_utf16..end_utf16), "X", &options);

    assert!(outcome.search_changed);
    assert_eq!(state.search(), &SharedString::from("aXb"));
    assert_eq!(state.search_selected_range(), 2..2);
}

/// 搜索框 IME 返回的新光标如果落在 marked text 的复合字素内部，也应夹到字素边界。
#[test]
fn marked_search_selected_range_inside_grapheme_is_clamped() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    let inserted = "a👨‍👩‍👧‍👦b";
    let family_start = "a".len();
    let inside_family_utf16 = "a👨".encode_utf16().count();

    let outcome = state.replace_and_mark_search_text_in_range(
        None,
        inserted,
        Some(inside_family_utf16..inside_family_utf16),
        &options,
    );

    assert!(outcome.search_changed);
    assert_eq!(state.search(), &SharedString::from(inserted));
    assert_eq!(state.search_marked_range(), Some(0..inserted.len()));
    assert_eq!(state.search_selected_range(), family_start..family_start);
}

/// 反向平台替换范围应被安全忽略，防止系统输入服务异常范围造成 panic。
#[test]
fn reversed_search_replacement_range_is_ignored() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);
    state.replace_search_text_in_range(None, "abc", &options);
    let reversed_start = 3;
    let reversed_end = 1;

    let outcome =
        state.replace_search_text_in_range(Some(reversed_start..reversed_end), "X", &options);

    assert!(!outcome.should_notify());
    assert_eq!(state.search(), &SharedString::from("abc"));
}

/// 键盘移动高亮时应跳过禁用选项。
#[test]
fn keyboard_highlight_skips_disabled_options() {
    let options = options();
    let mut state = SelectState::new(None, &options);

    let outcome = state.move_highlight(1, &options);

    assert!(outcome.highlight_changed);
    assert_eq!(state.highlighted_index(), Some(2));
}

/// 选择禁用选项不应改变值。
#[test]
fn selecting_disabled_option_is_ignored() {
    let options = options();
    let mut state = SelectState::new(None, &options);
    state.open(&options);

    let outcome = state.select_index(1, &options);

    assert!(!outcome.value_changed);
    assert!(state.value().is_none());
    assert!(state.is_open());
}

/// 重复 value 按第一个匹配项展示。
#[test]
fn duplicate_values_use_first_match_for_display() {
    let options = vec![
        SelectOption::new("same", "First"),
        SelectOption::new("same", "Second"),
    ];
    let state = SelectState::new(Some(SharedString::from("same")), &options);

    assert_eq!(state.selected_index(&options), Some(0));
    assert_eq!(
        state.selected_label(&options),
        Some(SharedString::from("First"))
    );
}

/// 公开 set_options 方法应更新选项展示但不触发任何用户交互回调。
#[gpui::test]
fn set_options_updates_entity_without_emitting_callbacks(cx: &mut TestAppContext) {
    let value_changes = Rc::new(Cell::new(0));
    let open_changes = Rc::new(Cell::new(0));
    let search_changes = Rc::new(Cell::new(0));
    let value_changes_for_callback = value_changes.clone();
    let open_changes_for_callback = open_changes.clone();
    let search_changes_for_callback = search_changes.clone();
    let select = cx.new(|cx| {
        Select::new(
            cx,
            SelectProps::default()
                .value(Some(SharedString::from("apple")))
                .options(options())
                .on_change(move |_| {
                    value_changes_for_callback.set(value_changes_for_callback.get() + 1);
                })
                .on_open_change(move |_| {
                    open_changes_for_callback.set(open_changes_for_callback.get() + 1);
                })
                .on_search_change(move |_| {
                    search_changes_for_callback.set(search_changes_for_callback.get() + 1);
                }),
        )
    });

    select.update(cx, |select, cx| {
        select.set_options(vec![SelectOption::new("apple", "Apple Updated")], cx);
        assert_eq!(select.value(), Some(&SharedString::from("apple")));
        assert_eq!(
            select.selected_label(),
            Some(SharedString::from("Apple Updated"))
        );

        select.set_options(vec![SelectOption::new("coffee", "Coffee")], cx);
        assert_eq!(select.value(), Some(&SharedString::from("apple")));
        assert_eq!(select.selected_label(), None);
    });

    assert_eq!(value_changes.get(), 0);
    assert_eq!(open_changes.get(), 0);
    assert_eq!(search_changes.get(), 0);
}

/// 公开 set_disabled 方法应静默关闭并阻止禁用期间的打开、切换和清空。
#[gpui::test]
fn set_disabled_closes_silently_and_blocks_interactions(cx: &mut TestAppContext) {
    let open_changes = Rc::new(Cell::new(0));
    let open_changes_for_callback = open_changes.clone();
    let select = cx.new(|cx| {
        Select::new(
            cx,
            SelectProps::default()
                .value(Some(SharedString::from("orange")))
                .options(options())
                .clearable(true)
                .on_open_change(move |_| {
                    open_changes_for_callback.set(open_changes_for_callback.get() + 1);
                }),
        )
    });

    select.update(cx, |select, cx| {
        select.open(cx);
        assert!(select.state.is_open());

        select.set_disabled(true, cx);
        assert!(select.disabled);
        assert!(!select.state.is_open());
        assert!(!select.is_search_selecting);
        assert!(select.search_auto_scroll_direction.is_none());

        select.open(cx);
        select.toggle(cx);
        select.clear(cx);
        assert!(!select.state.is_open());
        assert_eq!(select.value(), Some(&SharedString::from("orange")));

        select.set_disabled(false, cx);
        select.open(cx);
        assert!(!select.disabled);
        assert!(select.state.is_open());
    });

    assert_eq!(open_changes.get(), 2);
}

/// 语义状态和辅助文本同步只影响展示输入，不改变选择、搜索或打开状态。
#[gpui::test]
fn set_status_and_helper_text_only_update_visual_inputs(cx: &mut TestAppContext) {
    let select = cx.new(|cx| {
        Select::new(
            cx,
            SelectProps::default()
                .value(Some(SharedString::from("apple")))
                .options(options()),
        )
    });

    select.update(cx, |select, cx| {
        select.open(cx);
        select.state.set_search("ap", &select.options);
        let before_value = select.value().cloned();
        let before_open = select.state.is_open();
        let before_search = select.state.search().clone();

        select.set_status(SelectStatus::Error, cx);
        assert_eq!(select.status, SelectStatus::Error);
        assert_eq!(select.value().cloned(), before_value);
        assert_eq!(select.state.is_open(), before_open);
        assert_eq!(select.state.search(), &before_search);

        select.set_helper_text(Some(SharedString::from("支付方式不能为空")), cx);
        assert_eq!(
            select.helper_text,
            Some(SharedString::from("支付方式不能为空"))
        );
        assert_eq!(select.value().cloned(), before_value);
        assert_eq!(select.state.is_open(), before_open);
        assert_eq!(select.state.search(), &before_search);

        select.set_helper_text(None::<SharedString>, cx);
        assert!(select.helper_text.is_none());
        assert_eq!(select.value().cloned(), before_value);
        assert_eq!(select.state.is_open(), before_open);
        assert_eq!(select.state.search(), &before_search);
    });
}
