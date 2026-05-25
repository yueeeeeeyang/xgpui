//! `TextInput` 纯状态测试。
//!
//! 这些测试聚焦文本编辑核心行为，不启动 gpui 窗口，从而保持测试稳定和快速。

use super::state::{normalize_single_line, TextInputState};

/// UTF-8 与 UTF-16 偏移应能正确处理非 ASCII 字符。
#[test]
fn utf16_offsets_handle_non_ascii_text() {
    let state = TextInputState::new("a你😀b", None);

    assert_eq!(state.offset_to_utf16(0), 0);
    assert_eq!(state.offset_to_utf16("a".len()), 1);
    assert_eq!(state.offset_to_utf16("a你".len()), 2);
    assert_eq!(state.offset_to_utf16("a你😀".len()), 4);

    assert_eq!(state.offset_from_utf16(0), 0);
    assert_eq!(state.offset_from_utf16(1), "a".len());
    assert_eq!(state.offset_from_utf16(2), "a你".len());
    assert_eq!(state.offset_from_utf16(4), "a你😀".len());
}

/// 光标移动应按 Unicode 字素簇移动，不能切开组合字符。
#[test]
fn cursor_moves_by_grapheme_boundary() {
    let state = TextInputState::new("a👨‍👩‍👧‍👦b", None);
    let after_a = state.next_boundary(0);
    let after_family = state.next_boundary(after_a);

    assert_eq!(&state.as_str()[0..after_a], "a");
    assert_eq!(&state.as_str()[after_a..after_family], "👨‍👩‍👧‍👦");
    assert_eq!(state.previous_boundary(after_family), after_a);
}

/// 普通替换应替换当前选区并把光标移动到插入内容末尾。
#[test]
fn replace_selected_text_updates_content_and_cursor() {
    let mut state = TextInputState::new("hello world", None);
    state.move_to(6);
    state.select_to(11);

    let outcome = state.replace_text_in_range(None, "gpui");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "hello gpui");
    assert_eq!(state.selected_range(), 10..10);
}

/// 最大长度应按用户可感知的字素簇计数，而不是按字节数计数。
#[test]
fn max_length_limits_by_grapheme_count() {
    let mut state = TextInputState::new("", Some(3));

    let outcome = state.replace_text_in_range(None, "a你👨‍👩‍👧‍👦b");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "a你👨‍👩‍👧‍👦");
}

/// 粘贴或平台输入中的换行应被规范化为空格，保证组件始终是单行输入。
#[test]
fn normalize_multiline_text_to_single_line() {
    assert_eq!(normalize_single_line("a\nb\rc"), "a b c");
}

/// 清空应重置文本、选区和 marked text。
#[test]
fn clear_resets_content_and_selection() {
    let mut state = TextInputState::new("abc", None);
    state.select_all();
    state.replace_and_mark_text_in_range(None, "你", Some(1..1));

    let outcome = state.clear();

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "");
    assert_eq!(state.selected_range(), 0..0);
    assert!(state.marked_range().is_none());
}

/// IME marked text 应记录 marked 区间，并允许后续替换该区间。
#[test]
fn ime_marked_text_can_be_replaced() {
    let mut state = TextInputState::new("", None);

    let marked = state.replace_and_mark_text_in_range(None, "ni", Some(2..2));
    assert!(marked.content_changed);
    assert_eq!(state.as_str(), "ni");
    assert_eq!(state.marked_range(), Some(0..2));

    let committed = state.replace_text_in_range(None, "你");
    assert!(committed.content_changed);
    assert_eq!(state.as_str(), "你");
    assert!(state.marked_range().is_none());
}

/// 删除当前选区时，即使最大长度已满也必须允许删除。
#[test]
fn deletion_is_allowed_when_max_length_is_reached() {
    let mut state = TextInputState::new("abc", Some(3));
    state.move_to(1);
    state.select_to(3);

    let outcome = state.replace_text_in_range(None, "");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "a");
}
