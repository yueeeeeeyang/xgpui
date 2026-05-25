//! `Select` 纯状态测试。
//!
//! 这些测试聚焦选择、清除、搜索过滤和键盘高亮核心逻辑，不启动 gpui 窗口，从而保持测试稳定。

use gpui::SharedString;

use super::{
    props::SelectOption,
    state::{filtered_indices_for, SelectState},
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
