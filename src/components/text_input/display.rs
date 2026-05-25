//! `TextInput` 的显示文本映射。
//!
//! 输入框在密码模式下需要把真实文本显示为掩码字符，但光标、选区、鼠标定位、IME 候选框、
//! 复制和剪切都必须继续基于真实文本工作。本模块专门维护真实字节偏移与显示字节偏移之间的映射，
//! 避免把密码掩码逻辑散落在渲染和事件处理代码中。

use std::ops::Range;

use gpui::SharedString;
use unicode_segmentation::UnicodeSegmentation;

use super::props::TextInputType;

/// 密码隐藏状态下用于展示单个字素簇的掩码字符。
const PASSWORD_MASK_GRAPHEME: &str = "•";

/// 单次渲染使用的显示文本和偏移映射。
///
/// `actual_offsets` 与 `display_offsets` 一一对应，保存每个字素簇边界在真实文本和显示文本中的
/// UTF-8 字节偏移。普通文本下两组偏移相同；密码隐藏下，显示文本会变成等字素簇数量的掩码字符。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDisplayText {
    text: SharedString,
    actual_offsets: Vec<usize>,
    display_offsets: Vec<usize>,
}

impl TextDisplayText {
    /// 根据真实文本和输入类型创建显示文本。
    pub fn new(content: &str, input_type: TextInputType, password_visible: bool) -> Self {
        if input_type == TextInputType::Password && !password_visible {
            Self::masked_password(content)
        } else {
            Self::plain(content)
        }
    }

    /// 返回渲染时应该传给文本系统的显示文本。
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// 把真实文本字节偏移转换成显示文本字节偏移。
    ///
    /// 传入偏移如果不在记录的字素簇边界上，会向前夹到最近边界，保证不会落在 UTF-8 字符内部。
    pub fn actual_to_display(&self, actual_offset: usize) -> usize {
        boundary_lookup(&self.actual_offsets, &self.display_offsets, actual_offset)
    }

    /// 把显示文本字节偏移转换成真实文本字节偏移。
    ///
    /// 鼠标定位和平台输入查询会从显示文本坐标反查真实内容，密码掩码不会改变最终编辑的真实索引。
    pub fn display_to_actual(&self, display_offset: usize) -> usize {
        boundary_lookup(&self.display_offsets, &self.actual_offsets, display_offset)
    }

    /// 把真实文本区间转换成显示文本区间。
    pub fn actual_range_to_display(&self, range: Range<usize>) -> Range<usize> {
        self.actual_to_display(range.start)..self.actual_to_display(range.end)
    }

    /// 创建普通显示文本映射。
    fn plain(content: &str) -> Self {
        let mut actual_offsets = Vec::new();
        let mut display_offsets = Vec::new();
        actual_offsets.push(0);
        display_offsets.push(0);

        for (idx, grapheme) in content.grapheme_indices(true) {
            let next = idx + grapheme.len();
            actual_offsets.push(next);
            display_offsets.push(next);
        }

        Self {
            text: SharedString::from(content.to_string()),
            actual_offsets,
            display_offsets,
        }
    }

    /// 创建密码隐藏状态下的掩码显示文本映射。
    fn masked_password(content: &str) -> Self {
        let mut text = String::new();
        let mut actual_offsets = Vec::new();
        let mut display_offsets = Vec::new();
        actual_offsets.push(0);
        display_offsets.push(0);

        for (idx, grapheme) in content.grapheme_indices(true) {
            let actual_next = idx + grapheme.len();
            text.push_str(PASSWORD_MASK_GRAPHEME);
            actual_offsets.push(actual_next);
            display_offsets.push(text.len());
        }

        Self {
            text: text.into(),
            actual_offsets,
            display_offsets,
        }
    }
}

/// 在一组边界中查找输入偏移对应的目标偏移。
///
/// 边界向量总是至少包含 `0`。如果输入偏移位于两个边界之间，函数返回前一个边界对应的目标偏移；
/// 如果输入偏移超过末尾，函数返回末尾边界对应的目标偏移。
fn boundary_lookup(from_offsets: &[usize], to_offsets: &[usize], offset: usize) -> usize {
    match from_offsets.binary_search(&offset) {
        Ok(index) => to_offsets[index],
        Err(0) => to_offsets[0],
        Err(index) if index >= from_offsets.len() => *to_offsets.last().unwrap_or(&0),
        Err(index) => to_offsets[index - 1],
    }
}
