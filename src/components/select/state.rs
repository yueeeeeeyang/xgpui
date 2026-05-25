//! `Select` 的纯状态管理。
//!
//! 该模块不依赖 gpui 窗口、元素或渲染上下文，专门负责打开状态、当前值、搜索词、
//! 过滤结果、键盘高亮、清除和选择规则。这样可以用普通单元测试覆盖交互核心逻辑。

use std::ops::Range;

use gpui::{SharedString, UTF16Selection};
use unicode_segmentation::UnicodeSegmentation;

use super::props::SelectOption;

/// Select 状态变更结果。
///
/// 渲染层通过该结构判断是否需要触发外部回调或刷新界面，避免状态层直接依赖 gpui 上下文。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectStateOutcome {
    /// 选中值是否发生变化。
    pub value_changed: bool,
    /// 打开状态是否发生变化。
    pub open_changed: bool,
    /// 搜索词是否发生变化。
    pub search_changed: bool,
    /// 搜索输入框的光标、选区或 IME 标记区间是否发生变化。
    pub search_selection_changed: bool,
    /// 当前高亮选项是否发生变化。
    pub highlight_changed: bool,
}

impl SelectStateOutcome {
    /// 判断这次状态变更是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.value_changed
            || self.open_changed
            || self.search_changed
            || self.search_selection_changed
            || self.highlight_changed
    }

    /// 合并两个状态结果。
    ///
    /// 复杂操作会同时执行打开、重置搜索和移动高亮，合并结果可以让渲染层只处理一次回调和刷新。
    pub fn merge(self, other: Self) -> Self {
        Self {
            value_changed: self.value_changed || other.value_changed,
            open_changed: self.open_changed || other.open_changed,
            search_changed: self.search_changed || other.search_changed,
            search_selection_changed: self.search_selection_changed
                || other.search_selection_changed,
            highlight_changed: self.highlight_changed || other.highlight_changed,
        }
    }
}

/// Select 核心状态。
#[derive(Clone, Debug)]
pub struct SelectState {
    value: Option<SharedString>,
    open: bool,
    search: SharedString,
    search_selected_range: Range<usize>,
    search_selection_reversed: bool,
    search_marked_range: Option<Range<usize>>,
    highlighted: Option<usize>,
}

impl SelectState {
    /// 创建新的 Select 状态。
    pub fn new(value: Option<SharedString>, options: &[SelectOption]) -> Self {
        let mut state = Self {
            value,
            open: false,
            search: SharedString::default(),
            search_selected_range: 0..0,
            search_selection_reversed: false,
            search_marked_range: None,
            highlighted: None,
        };
        state.highlighted = state.preferred_highlight(options);
        state
    }

    /// 返回当前选中值。
    pub fn value(&self) -> Option<&SharedString> {
        self.value.as_ref()
    }

    /// 返回当前选中值的克隆。
    pub fn value_cloned(&self) -> Option<SharedString> {
        self.value.clone()
    }

    /// 返回下拉面板是否打开。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回当前搜索词。
    pub fn search(&self) -> &SharedString {
        &self.search
    }

    /// 返回当前搜索输入框选区。
    pub fn search_selected_range(&self) -> Range<usize> {
        self.search_selected_range.clone()
    }

    /// 返回当前搜索输入框 IME marked text 区间。
    pub fn search_marked_range(&self) -> Option<Range<usize>> {
        self.search_marked_range.clone()
    }

    /// 返回当前搜索输入框光标字节偏移。
    pub fn search_cursor_offset(&self) -> usize {
        if self.search_selection_reversed {
            self.search_selected_range.start
        } else {
            self.search_selected_range.end
        }
    }

    /// 返回当前高亮选项在原始选项列表中的索引。
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted
    }

    /// 返回当前选中选项在原始选项列表中的索引。
    ///
    /// 如果多个选项使用同一个 `value`，这里按第一个匹配项返回，保持展示和选择行为可预测。
    pub fn selected_index(&self, options: &[SelectOption]) -> Option<usize> {
        let value = self.value.as_ref()?;
        options.iter().position(|option| &option.value == value)
    }

    /// 返回当前选中选项的展示文本。
    pub fn selected_label(&self, options: &[SelectOption]) -> Option<SharedString> {
        self.selected_index(options)
            .map(|index| options[index].label.clone())
    }

    /// 静默设置当前值。
    ///
    /// 该方法用于外部受控同步，不表达用户交互语义，因此状态层只返回刷新需求，不要求渲染层触发
    /// `on_change`。
    pub fn set_value_silent(
        &mut self,
        value: Option<SharedString>,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        let value_changed = self.value != value;
        self.value = value;
        let highlight_changed = self.sync_highlight_to_preferred(options);
        SelectStateOutcome {
            value_changed,
            open_changed: false,
            search_changed: false,
            search_selection_changed: false,
            highlight_changed,
        }
    }

    /// 打开下拉面板。
    ///
    /// 打开时会清空搜索词并把高亮移动到当前选中项或第一个可选项，保证键盘导航从可预期位置开始。
    pub fn open(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        let mut outcome = SelectStateOutcome::default();
        if !self.open {
            self.open = true;
            outcome.open_changed = true;
        }
        outcome = outcome.merge(self.set_search_silent(SharedString::default(), options));
        if !outcome.highlight_changed {
            outcome.highlight_changed = self.sync_highlight_to_preferred(options);
        }
        outcome
    }

    /// 关闭下拉面板。
    pub fn close(&mut self) -> SelectStateOutcome {
        if self.open {
            self.open = false;
            SelectStateOutcome {
                open_changed: true,
                ..SelectStateOutcome::default()
            }
        } else {
            SelectStateOutcome::default()
        }
    }

    /// 切换下拉面板打开状态。
    pub fn toggle(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        if self.open {
            self.close()
        } else {
            self.open(options)
        }
    }

    /// 清空当前选择。
    pub fn clear(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        let value_changed = self.value.take().is_some();
        let highlight_changed = self.sync_highlight_to_preferred(options);
        SelectStateOutcome {
            value_changed,
            open_changed: false,
            search_changed: false,
            search_selection_changed: false,
            highlight_changed,
        }
    }

    /// 选择指定索引的选项。
    ///
    /// 禁用选项会被忽略，返回结果不会标记值变化；成功选择后会关闭面板。
    pub fn select_index(&mut self, index: usize, options: &[SelectOption]) -> SelectStateOutcome {
        let Some(option) = options.get(index) else {
            return SelectStateOutcome::default();
        };
        if option.disabled {
            return SelectStateOutcome::default();
        }

        let value = Some(option.value.clone());
        let value_changed = self.value != value;
        self.value = value;
        let open_changed = self.open;
        self.open = false;
        let highlight_changed = self.highlighted != Some(index);
        self.highlighted = Some(index);

        SelectStateOutcome {
            value_changed,
            open_changed,
            search_changed: false,
            search_selection_changed: false,
            highlight_changed,
        }
    }

    /// 选择当前高亮选项。
    pub fn select_highlighted(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        let Some(index) = self.highlighted else {
            return SelectStateOutcome::default();
        };
        self.select_index(index, options)
    }

    /// 设置搜索词并同步过滤后的高亮位置。
    #[cfg(test)]
    pub fn set_search(
        &mut self,
        search: impl Into<SharedString>,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        self.set_search_silent(search.into(), options)
    }

    /// 把搜索光标移动到指定 UTF-8 字节偏移。
    pub fn move_search_cursor(&mut self, offset: usize) -> SelectStateOutcome {
        let offset = self.clamp_search_offset_to_grapheme_boundary(offset);
        let old_range = self.search_selected_range.clone();
        let old_reversed = self.search_selection_reversed;
        let old_marked = self.search_marked_range.clone();

        self.search_selected_range = offset..offset;
        self.search_selection_reversed = false;
        self.search_marked_range = None;

        SelectStateOutcome {
            search_selection_changed: self.search_selected_range != old_range
                || self.search_selection_reversed != old_reversed
                || self.search_marked_range != old_marked,
            ..SelectStateOutcome::default()
        }
    }

    /// 把搜索输入框选区扩展到指定 UTF-8 字节偏移。
    pub fn select_search_to(&mut self, offset: usize) -> SelectStateOutcome {
        let offset = self.clamp_search_offset_to_grapheme_boundary(offset);
        let old_range = self.search_selected_range.clone();
        let old_reversed = self.search_selection_reversed;
        let old_marked = self.search_marked_range.clone();

        if self.search_selection_reversed {
            self.search_selected_range.start = offset;
        } else {
            self.search_selected_range.end = offset;
        }
        if self.search_selected_range.end < self.search_selected_range.start {
            self.search_selection_reversed = !self.search_selection_reversed;
            self.search_selected_range =
                self.search_selected_range.end..self.search_selected_range.start;
        }
        self.search_marked_range = None;

        SelectStateOutcome {
            search_selection_changed: self.search_selected_range != old_range
                || self.search_selection_reversed != old_reversed
                || self.search_marked_range != old_marked,
            ..SelectStateOutcome::default()
        }
    }

    /// 选中全部搜索词。
    pub fn select_all_search(&mut self) -> SelectStateOutcome {
        let old_range = self.search_selected_range.clone();
        let old_reversed = self.search_selection_reversed;
        let old_marked = self.search_marked_range.clone();

        self.search_selected_range = 0..self.search.len();
        self.search_selection_reversed = false;
        self.search_marked_range = None;

        SelectStateOutcome {
            search_selection_changed: self.search_selected_range != old_range
                || self.search_selection_reversed != old_reversed
                || self.search_marked_range != old_marked,
            ..SelectStateOutcome::default()
        }
    }

    /// 返回搜索光标前一个 Unicode 字素簇边界。
    pub fn previous_search_boundary(&self, offset: usize) -> usize {
        self.search
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    /// 返回搜索光标后一个 Unicode 字素簇边界。
    pub fn next_search_boundary(&self, offset: usize) -> usize {
        self.search
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.search.len())
    }

    /// 替换搜索输入框当前选区或指定 UTF-16 区间。
    pub fn replace_search_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        self.replace_search_text(range_utf16, new_text, None, false, options)
    }

    /// 替换搜索输入框文本，并把新文本记录为 IME marked text。
    pub fn replace_and_mark_search_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        self.replace_search_text(
            range_utf16,
            new_text,
            new_selected_range_utf16,
            true,
            options,
        )
    }

    /// 取消搜索输入框 IME marked text。
    pub fn unmark_search_text(&mut self) -> SelectStateOutcome {
        let had_mark = self.search_marked_range.take().is_some();
        SelectStateOutcome {
            search_selection_changed: had_mark,
            ..SelectStateOutcome::default()
        }
    }

    /// 返回指定 UTF-16 区间内的搜索文本，并写回实际 UTF-16 区间。
    pub fn search_text_for_range_utf16(
        &self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
    ) -> Option<String> {
        let range = self.search_range_from_utf16(&range_utf16);
        if range.start > range.end || range.end > self.search.len() {
            return None;
        }
        actual_range.replace(self.search_range_to_utf16(&range));
        Some(self.search[range].to_string())
    }

    /// 返回当前搜索输入框 UTF-16 选区。
    pub fn search_selected_text_range_utf16(&self) -> UTF16Selection {
        UTF16Selection {
            range: self.search_range_to_utf16(&self.search_selected_range),
            reversed: self.search_selection_reversed,
        }
    }

    /// 把搜索 UTF-16 区间转换成 UTF-8 字节区间。
    pub fn search_range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        offset_from_utf16_in(self.search.as_str(), range_utf16.start)
            ..offset_from_utf16_in(self.search.as_str(), range_utf16.end)
    }

    /// 把搜索 UTF-8 字节区间转换成 UTF-16 区间。
    pub fn search_range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        offset_to_utf16_in(self.search.as_str(), range.start)
            ..offset_to_utf16_in(self.search.as_str(), range.end)
    }

    /// 把搜索 UTF-8 字节偏移转换成 UTF-16 偏移。
    pub fn search_offset_to_utf16(&self, offset: usize) -> usize {
        offset_to_utf16_in(self.search.as_str(), offset)
    }

    /// 返回按当前搜索词过滤后的原始选项索引。
    ///
    /// 过滤规则为 `label` 的大小写不敏感子串匹配，并保留原始选项顺序。
    pub fn filtered_indices(&self, options: &[SelectOption]) -> Vec<usize> {
        filtered_indices_for(self.search.as_str(), options)
    }

    /// 把高亮移动到下一个或上一个可选项。
    pub fn move_highlight(&mut self, delta: isize, options: &[SelectOption]) -> SelectStateOutcome {
        let filtered = self.filtered_indices(options);
        let selectable: Vec<usize> = filtered
            .into_iter()
            .filter(|index| !options[*index].disabled)
            .collect();
        if selectable.is_empty() {
            return self.set_highlight(None);
        }

        let current_position = self
            .highlighted
            .and_then(|highlighted| selectable.iter().position(|index| *index == highlighted));
        let next_position = match (current_position, delta >= 0) {
            (Some(position), true) => (position + 1).min(selectable.len() - 1),
            (Some(position), false) => position.saturating_sub(1),
            (None, true) => 0,
            (None, false) => selectable.len() - 1,
        };
        self.set_highlight(Some(selectable[next_position]))
    }

    /// 把高亮移动到第一个可选项。
    pub fn highlight_first(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        self.set_highlight(first_selectable_index(
            &self.filtered_indices(options),
            options,
        ))
    }

    /// 把高亮移动到最后一个可选项。
    pub fn highlight_last(&mut self, options: &[SelectOption]) -> SelectStateOutcome {
        self.set_highlight(last_selectable_index(
            &self.filtered_indices(options),
            options,
        ))
    }

    /// 如果目标选项存在且可选，则直接设置为高亮项。
    ///
    /// 该方法主要服务鼠标 hover，同步视觉高亮时不改变当前选择值或打开状态。
    pub fn highlight_index_if_selectable(
        &mut self,
        index: usize,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        if options
            .get(index)
            .map(|option| option.disabled)
            .unwrap_or(true)
        {
            return SelectStateOutcome::default();
        }
        self.set_highlight(Some(index))
    }

    /// 设置搜索词但不直接触发回调语义。
    fn set_search_silent(
        &mut self,
        search: SharedString,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        let search_changed = self.search != search;
        let old_range = self.search_selected_range.clone();
        let old_reversed = self.search_selection_reversed;
        let old_marked = self.search_marked_range.clone();
        self.search = search;
        self.search_selected_range = self.search.len()..self.search.len();
        self.search_selection_reversed = false;
        self.search_marked_range = None;
        let highlight_changed = self.sync_highlight_to_preferred(options);
        SelectStateOutcome {
            value_changed: false,
            open_changed: false,
            search_changed,
            search_selection_changed: self.search_selected_range != old_range
                || self.search_selection_reversed != old_reversed
                || self.search_marked_range != old_marked,
            highlight_changed,
        }
    }

    /// 执行搜索输入框文本替换的共享逻辑。
    ///
    /// `range_utf16` 为 `None` 时，优先替换 IME marked text，其次替换当前选区；
    /// 所有输入都会被规范化为单行文本，避免粘贴换行破坏 Select 触发器布局。
    fn replace_search_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        mark_inserted_text: bool,
        options: &[SelectOption],
    ) -> SelectStateOutcome {
        let before_search = self.search.clone();
        let before_range = self.search_selected_range.clone();
        let before_reversed = self.search_selection_reversed;
        let before_marked = self.search_marked_range.clone();

        let Some(replacement_range) = self.search_replacement_range(range_utf16) else {
            return SelectStateOutcome::default();
        };
        let normalized_text = normalize_search_text(new_text);
        let next_search = format!(
            "{}{}{}",
            &self.search[0..replacement_range.start],
            normalized_text,
            &self.search[replacement_range.end..]
        );
        self.search = next_search.into();

        let inserted_len = normalized_text.len();
        if mark_inserted_text && inserted_len > 0 {
            self.search_marked_range =
                Some(replacement_range.start..replacement_range.start + inserted_len);
        } else {
            self.search_marked_range = None;
        }

        self.search_selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| {
                // 平台输入法的新选区相对本次插入文本，而不是完整搜索词。
                // 搜索框同样要防御复合 emoji、组合音标等字素内部偏移，避免 marked text
                // 更新后重新保存一个会拆开用户可感知字符的光标范围。
                let relative = normalize_inserted_search_selection_range(
                    normalized_text.as_str(),
                    range_utf16,
                );
                replacement_range.start + relative.start..replacement_range.start + relative.end
            })
            .unwrap_or_else(|| {
                let cursor = replacement_range.start + inserted_len;
                cursor..cursor
            });
        self.search_selection_reversed = false;
        self.clamp_search_selection_to_content();

        let highlight_changed = self.sync_highlight_to_preferred(options);
        SelectStateOutcome {
            search_changed: self.search != before_search,
            search_selection_changed: self.search_selected_range != before_range
                || self.search_selection_reversed != before_reversed
                || self.search_marked_range != before_marked,
            highlight_changed,
            ..SelectStateOutcome::default()
        }
    }

    /// 返回本次搜索输入应替换的 UTF-8 字节区间。
    fn search_replacement_range(&self, range_utf16: Option<Range<usize>>) -> Option<Range<usize>> {
        let raw_range = range_utf16
            .as_ref()
            .map(|range| self.search_range_from_utf16(range))
            .or_else(|| self.search_marked_range.clone())
            .unwrap_or_else(|| self.search_selected_range.clone());
        normalize_search_replacement_range(self.search.as_str(), raw_range)
    }

    /// 把搜索偏移夹到合法 Unicode 字素簇边界。
    ///
    /// 搜索输入虽然只是 Select 内部能力，但它同样接入平台输入法和鼠标定位。
    /// 使用字素簇边界可以保证复合 emoji、组合音标等用户可感知字符不会被搜索光标或替换范围拆开。
    fn clamp_search_offset_to_grapheme_boundary(&self, offset: usize) -> usize {
        previous_search_grapheme_boundary(self.search.as_str(), offset)
    }

    /// 保证搜索选区和 IME 标记区间不会超出当前搜索词长度。
    fn clamp_search_selection_to_content(&mut self) {
        let len = self.search.len();
        self.search_selected_range.start = self.search_selected_range.start.min(len);
        self.search_selected_range.end = self.search_selected_range.end.min(len);
        if self.search_selected_range.start > self.search_selected_range.end {
            self.search_selected_range =
                self.search_selected_range.end..self.search_selected_range.start;
            self.search_selection_reversed = !self.search_selection_reversed;
        }
        if let Some(marked_range) = self.search_marked_range.as_mut() {
            marked_range.start = marked_range.start.min(len);
            marked_range.end = marked_range.end.min(len);
        }
    }

    /// 返回当前过滤结果中的优先高亮项。
    ///
    /// 如果当前选中项仍在过滤结果里且未禁用，则优先高亮它；否则高亮第一个可选项。
    fn preferred_highlight(&self, options: &[SelectOption]) -> Option<usize> {
        let filtered = self.filtered_indices(options);
        if let Some(selected) = self.selected_index(options) {
            if filtered.contains(&selected) && !options[selected].disabled {
                return Some(selected);
            }
        }
        first_selectable_index(&filtered, options)
    }

    /// 同步高亮到优先项，并返回高亮是否变化。
    fn sync_highlight_to_preferred(&mut self, options: &[SelectOption]) -> bool {
        let next = self.preferred_highlight(options);
        let changed = self.highlighted != next;
        self.highlighted = next;
        changed
    }

    /// 设置高亮索引。
    fn set_highlight(&mut self, highlighted: Option<usize>) -> SelectStateOutcome {
        let highlight_changed = self.highlighted != highlighted;
        self.highlighted = highlighted;
        SelectStateOutcome {
            highlight_changed,
            ..SelectStateOutcome::default()
        }
    }
}

/// 返回指定搜索词下的过滤选项索引。
pub fn filtered_indices_for(search: &str, options: &[SelectOption]) -> Vec<usize> {
    let normalized_search = search.to_lowercase();
    options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            if normalized_search.is_empty()
                || option.label.to_lowercase().contains(&normalized_search)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

/// 返回过滤结果中的第一个可选项索引。
fn first_selectable_index(filtered: &[usize], options: &[SelectOption]) -> Option<usize> {
    filtered
        .iter()
        .copied()
        .find(|index| !options[*index].disabled)
}

/// 返回过滤结果中的最后一个可选项索引。
fn last_selectable_index(filtered: &[usize], options: &[SelectOption]) -> Option<usize> {
    filtered
        .iter()
        .rev()
        .copied()
        .find(|index| !options[*index].disabled)
}

/// 把搜索输入规范化为单行文本。
fn normalize_search_text(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// 返回不大于指定偏移的最近搜索文本字素簇边界。
///
/// 搜索框中的偏移来源包括鼠标、键盘和平台输入服务。它们都可能在复合字符中间给出 UTF-8
/// 字节位置，因此状态层统一向前夹到字素簇边界，避免保存会拆坏用户可感知字符的光标位置。
fn previous_search_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }

    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index <= offset).then_some(index))
        .unwrap_or(0)
}

/// 返回不小于指定偏移的最近搜索文本字素簇边界。
///
/// 非空替换区间触碰到复合字符内部时，需要把区间末尾扩展到完整字素末尾；
/// 这样搜索词不会留下半个 emoji 或半个组合音标，过滤逻辑也能继续处理合法 UTF-8 文本。
fn next_search_grapheme_boundary(text: &str, offset: usize) -> usize {
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

/// 规范化搜索输入的平台替换区间。
///
/// 平台文本服务传入的 UTF-16 区间转换后不一定适合直接切片：可能反向、越界或落在字素内部。
/// 这里复用 TextInput 的边界策略：反向区间忽略，空区间按光标向前夹取，非空区间向外覆盖完整字素。
fn normalize_search_replacement_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }

    if range.start == range.end {
        let cursor = previous_search_grapheme_boundary(text, range.start);
        return Some(cursor..cursor);
    }

    let start = previous_search_grapheme_boundary(text, range.start);
    let end = next_search_grapheme_boundary(text, range.end);
    Some(start..end)
}

/// 规范化搜索插入文本内部的新选区范围。
///
/// IME composing 过程会把新光标或选区作为“插入文本内 UTF-16 区间”传回组件。
/// 该区间可能位于复合字素内部，因此需要在相对文本中先做字素簇规范化，再由调用方平移回
/// 完整搜索词。若平台传入反向范围，这里保持 start > end，沿用后续状态夹取逻辑来记录反向选区。
fn normalize_inserted_search_selection_range(
    text: &str,
    range_utf16: &Range<usize>,
) -> Range<usize> {
    let range = range_from_utf16_in(text, range_utf16);
    if range.start == range.end {
        let cursor = previous_search_grapheme_boundary(text, range.start);
        return cursor..cursor;
    }

    if range.start < range.end {
        let start = previous_search_grapheme_boundary(text, range.start);
        let end = next_search_grapheme_boundary(text, range.end);
        start..end
    } else {
        let start = next_search_grapheme_boundary(text, range.start);
        let end = previous_search_grapheme_boundary(text, range.end);
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
