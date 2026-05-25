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
        let offset = self.clamp_to_grapheme_boundary(offset);
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
        let offset = self.clamp_to_grapheme_boundary(offset);
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

        let Some(replacement_range) = self.replacement_range(range_utf16) else {
            return TextEditOutcome::default();
        };
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
                // 平台输入法返回的新选区是相对插入文本的 UTF-16 区间。
                // 即使替换目标区间已经规范化，新的光标位置仍可能落在插入文本的复合字素内部；
                // 因此这里先在插入文本内部做字素簇规范化，再平移回完整内容的字节区间。
                let relative = normalize_inserted_selection_range(&inserted_text, range_utf16);
                replacement_range.start + relative.start..replacement_range.start + relative.end
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
    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Option<Range<usize>> {
        let raw_range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        normalize_replacement_range(self.content.as_str(), raw_range)
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

    /// 把偏移夹到有效 Unicode 字素簇边界上。
    ///
    /// 鼠标定位和平台输入回调传入的偏移可能落在复合 emoji、组合音标或 ZWJ 序列中间。
    /// 内部状态只能保存用户可感知字符的边界，否则后续删除、复制或渲染选区会把一个字素拆坏。
    fn clamp_to_grapheme_boundary(&self, offset: usize) -> usize {
        previous_grapheme_boundary(self.content.as_str(), offset)
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

/// 返回不大于指定偏移的最近 Unicode 字素簇边界。
///
/// 与 `str::is_char_boundary` 不同，字素簇边界会把复合 emoji、组合音标等多码点字符
/// 作为一个用户可感知字符处理。光标移动到非法位置时向前夹取，可以保持“点击字符内部等于点击字符前”
/// 的稳定行为，也避免后续字符串切片拆开复合字符。
fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }

    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index <= offset).then_some(index))
        .unwrap_or(0)
}

/// 返回不小于指定偏移的最近 Unicode 字素簇边界。
///
/// 非空替换区间的结束位置如果落在某个字素内部，需要向后扩展到该字素末尾；
/// 这样平台输入法或系统文本服务传入半个复合字符范围时，组件会替换整个字素而不是留下残缺序列。
fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }

    for (index, grapheme) in text.grapheme_indices(true) {
        let end = index + grapheme.len();
        if offset == index {
            return index;
        }
        if offset > index && offset < end {
            return end;
        }
    }

    text.len()
}

/// 规范化平台文本替换范围。
///
/// 平台输入回调使用 UTF-16 区间，转换到 UTF-8 后仍可能出现反向区间、越界区间或落在字素内部的区间。
/// 反向区间直接忽略，避免用不可信范围切片；空区间按光标规则向前夹到字素边界；非空区间则向外扩展，
/// 确保替换操作不会拆开复合字符。
fn normalize_replacement_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }

    if range.start == range.end {
        let cursor = previous_grapheme_boundary(text, range.start);
        return Some(cursor..cursor);
    }

    let start = previous_grapheme_boundary(text, range.start);
    let end = next_grapheme_boundary(text, range.end);
    Some(start..end)
}

/// 规范化插入文本内部的新选区范围。
///
/// `replace_and_mark_text_in_range` 会收到平台输入法给出的“插入文本内 UTF-16 选区”。
/// 该范围并不一定落在 Unicode 字素簇边界上，所以不能只做长度裁剪；否则 marked text
/// 内部的光标仍可能把复合 emoji 或组合音标拆开。这里保留既有的反向选区语义：正向选区向外
/// 覆盖完整字素，反向选区保持 start > end 交给后续 `clamp_selection_to_content` 标记为反向。
fn normalize_inserted_selection_range(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    let range = range_from_utf16_in(text, range_utf16);
    if range.start == range.end {
        let cursor = previous_grapheme_boundary(text, range.start);
        return cursor..cursor;
    }

    if range.start < range.end {
        let start = previous_grapheme_boundary(text, range.start);
        let end = next_grapheme_boundary(text, range.end);
        start..end
    } else {
        let start = next_grapheme_boundary(text, range.start);
        let end = previous_grapheme_boundary(text, range.end);
        start..end
    }
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
