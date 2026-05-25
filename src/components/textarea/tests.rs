//! `Textarea` 状态与公开同步方法测试。
//!
//! 大多数测试聚焦多行编辑核心行为，不启动真实窗口；少量测试使用 gpui 测试上下文
//! 覆盖 Entity 公开同步方法，确保受控更新不会意外触发用户变化回调。

use std::{cell::Cell, rc::Rc};

use gpui::{AppContext, SharedString, TestAppContext};

use super::{
    props::{TextareaProps, TextareaStatus},
    state::{normalize_textarea_text, TextareaState},
    Textarea,
};

/// 多行状态应保留 LF 换行，不做单行输入那样的空格替换。
#[test]
fn multiline_content_preserves_newlines() {
    let state = TextareaState::new("第一行\n第二行", None);

    assert_eq!(state.as_str(), "第一行\n第二行");
    assert_eq!(state.hard_line_count(), 2);
}

/// 外部和平台文本中的 CRLF/CR 应规范化为 LF，保证内部偏移和渲染拆行统一。
#[test]
fn carriage_returns_are_normalized_to_lf() {
    assert_eq!(normalize_textarea_text("a\r\nb\rc"), "a\nb\nc");
}

/// 插入和粘贴多行文本应保留换行，并把光标移动到插入内容末尾。
#[test]
fn replace_selected_text_supports_multiline_text() {
    let mut state = TextareaState::new("hello world", None);
    state.move_to(6);
    state.select_to(11);

    let outcome = state.replace_text_in_range(None, "gpui\ntextarea");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "hello gpui\ntextarea");
    assert_eq!(
        state.selected_range(),
        state.as_str().len()..state.as_str().len()
    );
}

/// 删除跨行选区应把前后文本重新拼接。
#[test]
fn deletion_can_cross_line_boundaries() {
    let mut state = TextareaState::new("a\nb\nc", None);
    state.move_to(1);
    state.select_to(4);

    let outcome = state.replace_text_in_range(None, "");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "ac");
    assert_eq!(state.selected_range(), 1..1);
}

/// 最大长度应按 Unicode 字素簇计数，换行、组合字符和 emoji 都按用户可感知字符处理。
#[test]
fn max_length_limits_by_grapheme_count_including_newlines() {
    let mut state = TextareaState::new("", Some(4));

    let outcome = state.replace_text_in_range(None, "a\né👨‍👩‍👧‍👦b");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "a\né👨‍👩‍👧‍👦");
}

/// UTF-8 与 UTF-16 偏移应能正确处理多行和非 ASCII 字符。
#[test]
fn utf16_offsets_handle_multiline_non_ascii_text() {
    let state = TextareaState::new("a\n你😀b", None);

    assert_eq!(state.offset_to_utf16(0), 0);
    assert_eq!(state.offset_to_utf16("a\n".len()), 2);
    assert_eq!(state.offset_to_utf16("a\n你".len()), 3);
    assert_eq!(state.offset_to_utf16("a\n你😀".len()), 5);

    assert_eq!(state.offset_from_utf16(0), 0);
    assert_eq!(state.offset_from_utf16(2), "a\n".len());
    assert_eq!(state.offset_from_utf16(3), "a\n你".len());
    assert_eq!(state.offset_from_utf16(5), "a\n你😀".len());
}

/// 平台替换范围如果只覆盖复合字素的一部分，应扩展到完整字素，避免留下残缺序列。
#[test]
fn partial_grapheme_replacement_expands_to_full_grapheme() {
    let mut state = TextareaState::new("a\n👨‍👩‍👧‍👦b", None);
    let family_start = "a\n".len();
    let family_mid = family_start + "👨".len();
    let family_inner_end = family_mid + "\u{200d}".len();
    let start_utf16 = state.offset_to_utf16(family_mid);
    let end_utf16 = state.offset_to_utf16(family_inner_end);

    let outcome = state.replace_text_in_range(Some(start_utf16..end_utf16), "X");

    assert!(outcome.content_changed);
    assert_eq!(state.as_str(), "a\nXb");
    assert_eq!(state.selected_range(), "a\nX".len().."a\nX".len());
}

/// IME 返回的新光标如果落在 marked text 的复合字素内部，也应被夹到字素边界。
#[test]
fn marked_selected_range_inside_grapheme_is_clamped() {
    let mut state = TextareaState::new("", None);
    let inserted = "n\n👨‍👩‍👧‍👦i";
    let family_start = "n\n".len();
    let inside_family_utf16 = "n\n👨".encode_utf16().count();

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
    let mut state = TextareaState::new("a\nb", None);

    let reversed_start = 3;
    let reversed_end = 1;

    let outcome = state.replace_text_in_range(Some(reversed_start..reversed_end), "X");

    assert!(!outcome.should_notify());
    assert_eq!(state.as_str(), "a\nb");
}

/// Home/End 的硬行边界应能跨多行计算。
#[test]
fn hard_line_boundaries_are_resolved_from_cursor() {
    let state = TextareaState::new("abc\ndef\nghi", None);
    let cursor = "abc\nde".len();

    assert_eq!(state.start_of_hard_line(cursor), "abc\n".len());
    assert_eq!(state.end_of_hard_line(cursor), "abc\ndef".len());
}

/// `set_value` 是受控同步方法，应更新值但不触发 on_change。
#[gpui::test]
fn set_value_syncs_without_emitting_change(cx: &mut TestAppContext) {
    let changes = Rc::new(Cell::new(0));
    let changes_for_callback = changes.clone();
    let textarea = cx.new(|cx| {
        Textarea::new(
            cx,
            TextareaProps::default().on_change(move |_| {
                changes_for_callback.set(changes_for_callback.get() + 1);
            }),
        )
    });

    textarea.update(cx, |textarea, cx| {
        textarea.set_value("外部\n同步", cx);
        assert_eq!(textarea.value(), &SharedString::from("外部\n同步"));
    });

    assert_eq!(changes.get(), 0);
}

/// 内容或选区定位变化应重新请求光标 reveal，保证输入、外部同步和选择操作后端点仍可见。
#[gpui::test]
fn content_and_selection_changes_request_cursor_reveal(cx: &mut TestAppContext) {
    let textarea = cx.new(|cx| Textarea::new(cx, TextareaProps::default().value("第一行\n第二行")));

    textarea.update(cx, |textarea, cx| {
        textarea.reveal_cursor_on_next_layout = false;
        textarea.set_value("外部\n同步", cx);
        assert!(textarea.reveal_cursor_on_next_layout);

        textarea.reveal_cursor_on_next_layout = false;
        textarea.select_all(cx);
        assert!(textarea.reveal_cursor_on_next_layout);
    });
}

/// 禁用状态应阻止后续清空；重新启用后恢复编辑能力。
#[gpui::test]
fn set_disabled_blocks_clear_until_reenabled(cx: &mut TestAppContext) {
    let textarea = cx.new(|cx| {
        Textarea::new(
            cx,
            TextareaProps::default()
                .value("可编辑")
                .helper_text(Some(SharedString::from("测试禁用同步"))),
        )
    });

    textarea.update(cx, |textarea, cx| {
        textarea.set_disabled(true, cx);
        textarea.clear(cx);
        assert_eq!(textarea.value(), &SharedString::from("可编辑"));

        textarea.set_disabled(false, cx);
        textarea.clear(cx);
        assert_eq!(textarea.value(), &SharedString::default());
    });
}

/// 禁用同步来自父组件受控写入，不应把内部聚焦态变化伪装成用户 blur/focus 回调。
#[gpui::test]
fn set_disabled_suppresses_controlled_focus_transition(cx: &mut TestAppContext) {
    let textarea = cx.new(|cx| Textarea::new(cx, TextareaProps::default().value("focused")));

    textarea.update(cx, |textarea, cx| {
        textarea.is_focused = true;
        textarea.set_disabled(true, cx);

        assert!(!textarea.is_focused);
        assert!(textarea.suppress_next_focus_callback);

        textarea.set_disabled(false, cx);
        assert!(textarea.suppress_next_focus_callback);
    });
}

/// 只读状态允许保留选区，但禁止内容修改。
#[gpui::test]
fn set_readonly_blocks_editing_without_clearing_selection(cx: &mut TestAppContext) {
    let textarea = cx.new(|cx| Textarea::new(cx, TextareaProps::default().value("readonly")));

    textarea.update(cx, |textarea, cx| {
        textarea.select_all(cx);
        let selected = textarea.state.selected_range();

        textarea.set_readonly(true, cx);
        textarea.clear(cx);

        assert_eq!(textarea.value(), &SharedString::from("readonly"));
        assert_eq!(textarea.state.selected_range(), selected);
    });
}

/// 语义状态和辅助文本同步只影响展示输入，不改变当前文本和选区。
#[gpui::test]
fn set_status_and_helper_text_only_update_visual_inputs(cx: &mut TestAppContext) {
    let textarea = cx.new(|cx| Textarea::new(cx, TextareaProps::default().value("备注")));

    textarea.update(cx, |textarea, cx| {
        textarea.select_all(cx);
        let before_value = textarea.value().clone();
        let before_selection = textarea.state.selected_range();

        textarea.reveal_cursor_on_next_layout = false;
        textarea.set_status(TextareaStatus::Error, cx);
        assert_eq!(textarea.status, TextareaStatus::Error);
        assert_eq!(textarea.value(), &before_value);
        assert_eq!(textarea.state.selected_range(), before_selection);
        assert!(!textarea.reveal_cursor_on_next_layout);

        textarea.set_helper_text(Some(SharedString::from("备注不能为空")), cx);
        assert_eq!(
            textarea.helper_text,
            Some(SharedString::from("备注不能为空"))
        );
        assert_eq!(textarea.value(), &before_value);
        assert_eq!(textarea.state.selected_range(), before_selection);
        assert!(!textarea.reveal_cursor_on_next_layout);

        textarea.set_helper_text(None::<SharedString>, cx);
        assert!(textarea.helper_text.is_none());
        assert_eq!(textarea.value(), &before_value);
        assert_eq!(textarea.state.selected_range(), before_selection);
        assert!(!textarea.reveal_cursor_on_next_layout);
    });
}
