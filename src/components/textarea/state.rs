//! `Textarea` 的纯文本编辑状态。
//!
//! 该模块不依赖窗口和渲染环境，专门负责多行内容、选区、IME marked text、
//! UTF-16/UTF-8 转换、字素簇边界和 maxlength。把这些规则放在纯状态层，
//! 可以用普通单元测试覆盖平台输入的边界条件，而不需要启动 gpui 窗口。

use std::ops::Range;

use gpui::{SharedString, UTF16Selection};
use unicode_segmentation::UnicodeSegmentation;

/// 文本编辑后的结果。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextareaEditOutcome {
    /// 文本内容是否发生变化。
    pub content_changed: bool,
    /// 光标、选区或 marked text 是否发生变化。
    pub selection_changed: bool,
}

impl TextareaEditOutcome {
    /// 判断这次操作是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.content_changed || self.selection_changed
    }
}

/// 多行文本输入的核心状态。
#[derive(Clone, Debug)]
pub struct TextareaState {
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    max_length: Option<usize>,
}

impl TextareaState {
    /// 创建新的多行文本状态。
    pub fn new(content: impl Into<SharedString>, max_length: Option<usize>) -> Self {
        let mut state = Self {
            content: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            max_length,
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

    /// 静默设置完整内容。
    ///
    /// 该方法用于构造或外部受控同步，不触发回调语义；调用方负责决定是否刷新界面。
    /// 多行输入保留 `\n`，但会把 Windows/旧 Mac 风格换行规范化为 LF，保证内部偏移规则统一。
    pub fn set_content_silent(&mut self, content: impl Into<SharedString>) {
        let normalized = normalize_textarea_text(content.into().as_str());
        self.content = self.truncate_to_limit(&normalized).into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
    }

    /// 返回硬换行行数，空文本也按一行计算。
    ///
    /// 该值用于 rows/min_rows/max_rows 的初始高度估算；软换行高度由渲染层在拿到实际宽度后再精确计算。
    pub fn hard_line_count(&self) -> usize {
        self.content.split('\n').count().max(1)
    }

    /// 移动光标到指定 UTF-8 字节偏移。
    pub fn move_to(&mut self, offset: usize) -> TextareaEditOutcome {
        let offset = self.clamp_to_grapheme_boundary(offset);
        let old_range = self.selected_range.clone();
        let old_reversed = self.selection_reversed;
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.marked_range = None;
        TextareaEditOutcome {
            content_changed: false,
            selection_changed: self.selected_range != old_range
                || self.selection_reversed != old_reversed,
        }
    }

    /// 将选区扩展到指定 UTF-8 字节偏移。
    pub fn select_to(&mut self, offset: usize) -> TextareaEditOutcome {
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
        TextareaEditOutcome {
            content_changed: false,
            selection_changed: self.selected_range != old_range
                || self.selection_reversed != old_reversed,
        }
    }

    /// 选中全部文本。
    pub fn select_all(&mut self) -> TextareaEditOutcome {
        let old_range = self.selected_range.clone();
        let old_reversed = self.selection_reversed;
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        TextareaEditOutcome {
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

    /// 返回光标前一个 Unicode 字素簇边界。
    pub fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// 返回光标后一个 Unicode 字素簇边界。
    pub fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// 返回当前硬行的起始偏移。
    pub fn start_of_hard_line(&self, offset: usize) -> usize {
        let offset = self.clamp_to_grapheme_boundary(offset);
        self.content[..offset]
            .rfind('\n')
            .map(|index| index + '\n'.len_utf8())
            .unwrap_or(0)
    }

    /// 返回当前硬行的结束偏移，不包含换行符本身。
    pub fn end_of_hard_line(&self, offset: usize) -> usize {
        let offset = self.clamp_to_grapheme_boundary(offset);
        self.content[offset..]
            .find('\n')
            .map(|relative| offset + relative)
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
    pub fn unmark_text(&mut self) -> TextareaEditOutcome {
        let had_mark = self.marked_range.take().is_some();
        TextareaEditOutcome {
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
    ) -> TextareaEditOutcome {
        self.replace_text(range_utf16, new_text, None, false)
    }

    /// 替换指定区间文本，并把新文本标记为 IME composing 状态。
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) -> TextareaEditOutcome {
        self.replace_text(range_utf16, new_text, new_selected_range_utf16, true)
    }

    /// 清空文本。
    pub fn clear(&mut self) -> TextareaEditOutcome {
        let content_changed = !self.content.is_empty();
        let selection_changed =
            self.selected_range != (0..0) || self.selection_reversed || self.marked_range.is_some();
        self.content = SharedString::default();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        TextareaEditOutcome {
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
    ) -> TextareaEditOutcome {
        let before_content = self.content.clone();
        let before_selection = self.selected_range.clone();
        let before_reversed = self.selection_reversed;
        let before_marked = self.marked_range.clone();

        let Some(replacement_range) = self.replacement_range(range_utf16) else {
            return TextareaEditOutcome::default();
        };
        let normalized_text = normalize_textarea_text(new_text);
        let inserted_text = self.truncate_replacement(&replacement_range, &normalized_text);

        let next_content = format!(
            "{}{}{}",
            &self.content[0..replacement_range.start],
            inserted_text,
            &self.content[replacement_range.end..]
        );
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
                // 平台输入法给出的新选区是“插入文本内部”的 UTF-16 区间。
                // 插入文本可能包含 emoji、组合音标或换行，因此必须先在插入片段内部规范化到字素簇边界，
                // 再平移回完整内容偏移，避免 marked text 内部保存半个用户可感知字符。
                let relative = normalize_inserted_selection_range(&inserted_text, range_utf16);
                replacement_range.start + relative.start..replacement_range.start + relative.end
            })
            .unwrap_or_else(|| {
                let cursor = replacement_range.start + inserted_len;
                cursor..cursor
            });
        self.selection_reversed = false;
        self.clamp_selection_to_content();

        TextareaEditOutcome {
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

    /// 把偏移夹到有效 Unicode 字素簇边界上。
    ///
    /// 鼠标定位和平台输入回调传入的偏移可能落在复合 emoji、组合音标或 ZWJ 序列中间。
    /// 状态层只保存用户可感知字符的边界，避免删除、复制或渲染选区时拆坏一个字素。
    fn clamp_to_grapheme_boundary(&self, offset: usize) -> usize {
        previous_grapheme_boundary(self.content.as_str(), offset)
    }

    /// 保证选区和 marked text 不会超出当前文本长度。
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

/// 把平台和外部输入中的换行统一规范化为 LF。
///
/// 标准 textarea 会保留换行语义，但跨平台剪贴板和输入法可能传入 `\r\n` 或裸 `\r`。
/// 内部统一为 `\n` 后，UTF-16/UTF-8 偏移、行数计算和渲染拆行都可以使用同一套规则。
pub fn normalize_textarea_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 返回不大于指定偏移的最近 Unicode 字素簇边界。
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
/// 平台文本服务使用 UTF-16 区间，转换到 UTF-8 后仍可能出现反向区间、越界区间或落在字素内部。
/// 反向区间直接忽略；空区间按光标规则向前夹到字素边界；非空区间向外扩展到完整字素。
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
