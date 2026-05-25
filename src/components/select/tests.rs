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
