//! `TextInput` 的纯文本编辑状态。
//!
//! 该模块不依赖窗口和渲染环境，专门负责内容、选区、IME 标记文本和长度限制。
//! 这样可以用普通单元测试覆盖输入行为，降低 gpui 窗口环境对测试的影响。

use std::ops::Range;

use gpui::{SharedString, UTF16Selection};
use unicode_segmentation::UnicodeSegmentation;

use super::props::TextInputType;

/// 文本编辑后的结果。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextEditOutcome {
    /// 文本内容是否发生变化。
    pub content_changed: bool,
    /// 光标、选区或 marked text 是否发生变化。
    pub selection_changed: bool,
}

impl TextEditOutcome {
    /// 判断这次编辑是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.content_changed || self.selection_changed
    }
}

/// 单行文本输入的核心状态。
#[derive(Clone, Debug)]
pub struct TextInputState {
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    max_length: Option<usize>,
    input_type: TextInputType,
}

impl TextInputState {
    /// 创建新的普通文本测试状态。
    #[cfg(test)]
    pub fn new(content: impl Into<SharedString>, max_length: Option<usize>) -> Self {
        Self::new_with_type(content, max_length, TextInputType::Text)
    }

    /// 创建带指定输入类型的文本状态。
    pub fn new_with_type(
        content: impl Into<SharedString>,
        max_length: Option<usize>,
        input_type: TextInputType,
    ) -> Self {
        let mut state = Self {
            content: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            max_length,
            input_type,
        };
        state.set_content_silent(content);
        state
    }

    /// 返回当前内容。
    pub fn content(&self) -> &SharedString {
        &self.content
    }

    /// 返回当前内容的字符串切片。
    pub fn as_str(&self) -> &str {
        self.content.as_str()
    }

    /// 返回当前选区。
    pub fn selected_range(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    /// 返回当前 IME marked text 区间。
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    /// 静默设置内容。
    ///
    /// 该方法用于构造或外部强制同步值，不触发回调语义；调用方负责决定是否刷新界面。
    pub fn set_content_silent(&mut self, content: impl Into<SharedString>) {
        let content = content.into();
        let normalized = normalize_single_line(content.as_str());
        let typed = self.normalize_full_content_for_type(&normalized);
        self.content = self.truncate_to_limit(&typed).into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
    }

    /// 移动光标到指定字节偏移。
    pub fn move_to(&mut self, offset: usize) -> TextEditOutcome {
        let offset = self.clamp_to_boundary(offset);
        let old_range = self.selected_range.clone();
        let old_reversed = self.selection_reversed;
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.marked_range = None;
        TextEditOutcome {
            content_changed: false,
            selection_changed: self.selected_range != old_range
                || self.selection_reversed != old_reversed,
        }
    }

    /// 将选区扩展到指定字节偏移。
    pub fn select_to(&mut self, offset: usize) -> TextEditOutcome {
        let offset = self.clamp_to_boundary(offset);
        let old_range = self.selected_range.clone();
        let old_reversed = self.selection_reversed;

        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }

        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }

        self.marked_range = None;
        TextEditOutcome {
            content_changed: false,
            selection_changed: self.selected_range != old_range
                || self.selection_reversed != old_reversed,
        }
    }

    /// 选中全部文本。
    pub fn select_all(&mut self) -> TextEditOutcome {
        let old_range = self.selected_range.clone();
        let old_reversed = self.selection_reversed;
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        TextEditOutcome {
            content_changed: false,
            selection_changed: self.selected_range != old_range
                || self.selection_reversed != old_reversed,
        }
    }

    /// 返回当前光标偏移。
    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// 返回光标前一个字素簇边界。
    pub fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// 返回光标后一个字素簇边界。
    pub fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// 把 UTF-16 偏移转换成 UTF-8 字节偏移。
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        offset_from_utf16_in(self.content.as_str(), offset)
    }

    /// 把 UTF-8 字节偏移转换成 UTF-16 偏移。
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        offset_to_utf16_in(self.content.as_str(), offset)
    }

    /// 把 UTF-16 区间转换成 UTF-8 字节区间。
    pub fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// 把 UTF-8 字节区间转换成 UTF-16 区间。
    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    /// 返回当前 UTF-16 选区。
    pub fn selected_text_range_utf16(&self) -> UTF16Selection {
        UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        }
    }

    /// 返回指定 UTF-16 区间内的文本，并写回实际区间。
    pub fn text_for_range_utf16(
        &self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        if range.start > range.end || range.end > self.content.len() {
            return None;
        }
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    /// 取消 IME marked text。
    pub fn unmark_text(&mut self) -> TextEditOutcome {
        let had_mark = self.marked_range.take().is_some();
        TextEditOutcome {
            content_changed: false,
            selection_changed: had_mark,
        }
    }

    /// 替换指定区间文本。
    ///
    /// `range_utf16` 为 `None` 时，会优先替换 marked text，其次替换当前选区。
    pub fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
    ) -> TextEditOutcome {
        self.replace_text(range_utf16, new_text, None, false)
    }

    /// 替换指定区间文本，并把新文本标记为 IME composing 状态。
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) -> TextEditOutcome {
        self.replace_text(range_utf16, new_text, new_selected_range_utf16, true)
    }

    /// 清空文本。
    pub fn clear(&mut self) -> TextEditOutcome {
        let content_changed = !self.content.is_empty();
        let selection_changed =
            self.selected_range != (0..0) || self.selection_reversed || self.marked_range.is_some();
        self.content = SharedString::default();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        TextEditOutcome {
            content_changed,
            selection_changed,
        }
    }

    /// 执行文本替换的共享实现。
    fn replace_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        mark_inserted_text: bool,
    ) -> TextEditOutcome {
        let before_content = self.content.clone();
        let before_selection = self.selected_range.clone();
        let before_reversed = self.selection_reversed;
        let before_marked = self.marked_range.clone();

        let replacement_range = self.replacement_range(range_utf16);
        let normalized_text = normalize_single_line(new_text);
        let inserted_text = self.truncate_replacement(&replacement_range, &normalized_text);

        let next_content = format!(
            "{}{}{}",
            &self.content[0..replacement_range.start],
            inserted_text,
            &self.content[replacement_range.end..]
        );
        if !self.accepts_content_for_type(&next_content) {
            return TextEditOutcome::default();
        }
        self.content = next_content.into();

        let inserted_len = inserted_text.len();
        if mark_inserted_text && inserted_len > 0 {
            self.marked_range =
                Some(replacement_range.start..replacement_range.start + inserted_len);
        } else {
            self.marked_range = None;
        }

        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| {
                let relative = range_from_utf16_in(&inserted_text, range_utf16);
                let start = relative.start.min(inserted_len);
                let end = relative.end.min(inserted_len);
                replacement_range.start + start..replacement_range.start + end
            })
            .unwrap_or_else(|| {
                let cursor = replacement_range.start + inserted_len;
                cursor..cursor
            });
        self.selection_reversed = false;
        self.clamp_selection_to_content();

        TextEditOutcome {
            content_changed: self.content != before_content,
            selection_changed: self.selected_range != before_selection
                || self.selection_reversed != before_reversed
                || self.marked_range != before_marked,
        }
    }

    /// 返回本次输入应该替换的 UTF-8 字节区间。
    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone())
    }

    /// 根据最大长度截断待插入文本。
    fn truncate_replacement(&self, range: &Range<usize>, text: &str) -> String {
        let Some(max_length) = self.max_length else {
            return text.to_string();
        };

        let existing_before = self.content[0..range.start].graphemes(true).count();
        let existing_after = self.content[range.end..].graphemes(true).count();
        let available = max_length.saturating_sub(existing_before + existing_after);
        text.graphemes(true).take(available).collect()
    }

    /// 根据最大长度截断完整文本。
    fn truncate_to_limit(&self, text: &str) -> String {
        match self.max_length {
            Some(max_length) => text.graphemes(true).take(max_length).collect(),
            None => text.to_string(),
        }
    }

    /// 按输入类型规范化完整文本。
    ///
    /// 外部同步值没有“拒绝本次编辑”的上下文，因此数字类型遇到非法完整值时会回退为空字符串，
    /// 保证状态内部不会保存不符合类型约束的内容。
    fn normalize_full_content_for_type(&self, text: &str) -> String {
        match self.input_type {
            TextInputType::Text | TextInputType::Password => text.to_string(),
            TextInputType::Number if is_valid_number_text(text) => text.to_string(),
            TextInputType::Number => String::new(),
        }
    }

    /// 判断候选完整文本是否满足当前输入类型。
    fn accepts_content_for_type(&self, text: &str) -> bool {
        match self.input_type {
            TextInputType::Text | TextInputType::Password => true,
            TextInputType::Number => is_valid_number_text(text),
        }
    }

    /// 把偏移夹到有效字素簇边界上。
    fn clamp_to_boundary(&self, offset: usize) -> usize {
        if offset >= self.content.len() {
            return self.content.len();
        }
        if self.content.is_char_boundary(offset) {
            offset
        } else {
            self.content
                .char_indices()
                .rev()
                .find_map(|(idx, _)| (idx < offset).then_some(idx))
                .unwrap_or(0)
        }
    }

    /// 保证选区不会超出当前文本长度。
    fn clamp_selection_to_content(&mut self) {
        let len = self.content.len();
        self.selected_range.start = self.selected_range.start.min(len);
        self.selected_range.end = self.selected_range.end.min(len);
        if self.selected_range.start > self.selected_range.end {
            self.selected_range = self.selected_range.end..self.selected_range.start;
            self.selection_reversed = !self.selection_reversed;
        }
        if let Some(marked_range) = self.marked_range.as_mut() {
            marked_range.start = marked_range.start.min(len);
            marked_range.end = marked_range.end.min(len);
        }
    }
}

/// 把多行文本规范化为单行文本。
pub fn normalize_single_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// 判断字符串是否是数字输入允许的中间态。
///
/// 该规则故意不把字符串解析成数值，因为用户输入过程中需要保留 `-`、`.`、`-.` 和 `1.`
/// 这类尚未形成最终数值但在编辑流程中合理的中间内容。
pub(super) fn is_valid_number_text(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }

    let mut chars = text.chars().peekable();
    if chars.peek() == Some(&'-') {
        chars.next();
    }

    let mut seen_dot = false;
    for ch in chars {
        if ch.is_ascii_digit() {
            continue;
        }
        if ch == '.' && !seen_dot {
            seen_dot = true;
            continue;
        }
        return false;
    }

    true
}

/// 在任意字符串中把 UTF-16 偏移转换成 UTF-8 字节偏移。
fn offset_from_utf16_in(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;

    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }

    utf8_offset
}

/// 在任意字符串中把 UTF-8 字节偏移转换成 UTF-16 偏移。
fn offset_to_utf16_in(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;

    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }

    utf16_offset
}

/// 在任意字符串中把 UTF-16 区间转换成 UTF-8 字节区间。
fn range_from_utf16_in(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    offset_from_utf16_in(text, range_utf16.start)..offset_from_utf16_in(text, range_utf16.end)
}
