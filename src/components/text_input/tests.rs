//! `TextInput` 纯状态测试。
//!
//! 这些测试聚焦文本编辑核心行为，不启动 gpui 窗口，从而保持测试稳定和快速。

use super::{
    display::TextDisplayText,
    props::TextInputType,
    state::{is_valid_number_text, normalize_single_line, TextInputState},
};

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

/// 外部偏移即使落在复合字素内部，也应被夹到字素边界，避免后续编辑拆开 emoji。
#[test]
fn cursor_offsets_inside_grapheme_are_clamped_to_grapheme_boundary() {
    let mut state = TextInputState::new("a👨‍👩‍👧‍👦b", None);
    let family_start = "a".len();
    let inside_family = family_start + "👨".len();

    let outcome = state.move_to(inside_family);

    assert!(outcome.selection_changed);
    assert_eq!(state.selected_range(), family_start..family_start);
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

/// 平台替换范围如果只覆盖复合字素的一部分，应扩展到完整字素，避免留下非法或残缺显示。
#[test]
fn partial_grapheme_replacement_expands_to_full_grapheme() {
    let mut state = TextInputState::new("a👨‍👩‍👧‍👦b", None);
    let family_start = "a".len();
    let family_mid = family_start + "👨".len();
    let family_inner_end = family_mid + "\u{200d}".len();
    let start_utf16 = state.offset_to_utf16(family_mid);
    let end_utf16 = state.offset_to_utf16(family_inner_end);

    let outcome = state.replace_text_in_range(Some(start_utf16..end_utf16), "X");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "aXb");
    assert_eq!(state.selected_range(), 2..2);
}

/// IME 返回的新光标如果落在 marked text 的复合字素内部，也应被夹到字素边界。
#[test]
fn marked_selected_range_inside_grapheme_is_clamped() {
    let mut state = TextInputState::new("", None);
    let inserted = "a👨‍👩‍👧‍👦b";
    let family_start = "a".len();
    let inside_family_utf16 = "a👨".encode_utf16().count();

    let outcome = state.replace_and_mark_text_in_range(
        None,
        inserted,
        Some(inside_family_utf16..inside_family_utf16),
    );

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), inserted);
    assert_eq!(state.marked_range(), Some(0..inserted.len()));
    assert_eq!(state.selected_range(), family_start..family_start);
}

/// 反向平台替换范围应被安全忽略，不能用不可信区间直接切片导致 panic。
#[test]
fn reversed_platform_replacement_range_is_ignored() {
    let mut state = TextInputState::new("abc", None);
    let reversed_start = 3;
    let reversed_end = 1;

    let outcome = state.replace_text_in_range(Some(reversed_start..reversed_end), "X");

    assert!(!outcome.should_notify());
    assert_eq!(state.as_str(), "abc");
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

/// 密码隐藏时应按字素簇生成掩码，并能在真实偏移和显示偏移之间互相转换。
#[test]
fn password_display_maps_offsets_by_grapheme() {
    let display = TextDisplayText::new("a你👨‍👩‍👧‍👦", TextInputType::Password, false);

    assert_eq!(display.text().as_str(), "•••");
    assert_eq!(display.actual_to_display("a".len()), "•".len());
    assert_eq!(display.actual_to_display("a你".len()), "••".len());
    assert_eq!(display.display_to_actual("••".len()), "a你".len());
}

/// 密码可见时应展示真实文本，偏移映射也应保持真实文本边界。
#[test]
fn visible_password_keeps_plain_display_text() {
    let display = TextDisplayText::new("secret", TextInputType::Password, true);

    assert_eq!(display.text().as_str(), "secret");
    assert_eq!(display.actual_to_display(3), 3);
    assert_eq!(display.display_to_actual(3), 3);
}

/// 数字类型应允许用户输入过程中的合理中间态。
#[test]
fn number_type_accepts_intermediate_number_text() {
    for text in ["", "-", ".", "-.", "1", "-1", "1.", ".5", "-0.5"] {
        assert!(is_valid_number_text(text), "{text} should be valid");
    }
}

/// 数字类型应拒绝非数字形态文本。
#[test]
fn number_type_rejects_invalid_number_text() {
    for text in ["a", "1a", "1.2.3", "1-2", "--1", "1 2"] {
        assert!(!is_valid_number_text(text), "{text} should be invalid");
    }
}

/// 数字状态应拒绝会让完整内容变成非法数字形态的替换。
#[test]
fn number_state_rejects_invalid_replacement() {
    let mut state = TextInputState::new_with_type("12", None, TextInputType::Number);
    state.move_to(state.as_str().len());

    let outcome = state.replace_text_in_range(None, "a");

    assert!(!outcome.content_changed);
    assert_eq!(state.as_str(), "12");
}

/// 数字状态应允许合法小数和负数中间态。
#[test]
fn number_state_accepts_valid_replacement() {
    let mut state = TextInputState::new_with_type("-", None, TextInputType::Number);
    state.move_to(state.as_str().len());

    let dot = state.replace_text_in_range(None, ".");
    let digit = state.replace_text_in_range(None, "5");

    assert!(dot.content_changed);
    assert!(digit.content_changed);
    assert_eq!(state.as_str(), "-.5");
}

/// 外部同步数字值时，非法完整值不能进入内部状态。
#[test]
fn number_state_drops_invalid_external_value() {
    let state = TextInputState::new_with_type("abc", None, TextInputType::Number);

    assert_eq!(state.as_str(), "");
}
