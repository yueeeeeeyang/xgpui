//! 多行文本输入组件。
//!
//! `Textarea` 基于 gpui 的 `EntityInputHandler` 接入平台文本输入能力，
//! 支持 IME、硬换行、软换行、选区、剪贴板、鼠标定位、键盘动作、只读/禁用和常见表单状态。

use std::{ops::Range, time::Duration};

use gpui::prelude::*;
use gpui::{
    actions, div, fill, point, px, relative, size, App, AsyncApp, Bounds, ClipboardItem, Context,
    CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, GlobalElementId, Hsla, InspectorElementId, IntoElement, KeyBinding, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement,
    Pixels, Point, Render, ScrollWheelEvent, SharedString, Style, TextAlign, TextRun, Timer,
    UTF16Selection, UnderlineStyle, WeakEntity, Window, WrappedLine,
};

mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

pub use props::{TextareaProps, TextareaSize, TextareaStatus, TextareaVariant};
use state::{TextareaEditOutcome, TextareaState};
use style::{resolve_textarea_style, ResolvedTextareaStyle, TextareaRows};

actions!(
    xgpui_textarea,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        InsertNewline,
        Submit,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

/// 光标静止多久后开始进入闪烁周期。
const CURSOR_BLINK_IDLE_DELAY: Duration = Duration::from_millis(500);

/// 光标进入闪烁周期后的可见状态切换间隔。
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// 文本区域右侧预留给内部滚动条的宽度。
///
/// 这里始终预留 gutter，而不是只在可滚动时改变排版宽度，避免内容刚好达到最大行数附近时
/// 因滚动条出现/消失导致软换行反复变化。
const SCROLLBAR_GUTTER: Pixels = px(10.0);

/// 内部滚动条滑块宽度。
const SCROLLBAR_WIDTH: Pixels = px(4.0);

/// 内部滚动条滑块最小高度。
///
/// 内容很长时按比例计算出的滑块可能过小，保留最小高度可以保证用户始终能感知当前位置。
const SCROLLBAR_MIN_THUMB_HEIGHT: Pixels = px(20.0);

/// 滚动条距离文本视口上下边界的留白。
const SCROLLBAR_INSET_Y: Pixels = px(2.0);

/// 注册 `Textarea` 默认键盘快捷键。
///
/// gpui 的键盘动作需要应用启动时注册。调用方通常不需要直接调用本函数，
/// 使用 `xgpui::install(cx)` 即可同时安装所有内置组件的默认快捷键。
pub fn register_textarea_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("Textarea")),
        KeyBinding::new("delete", Delete, Some("Textarea")),
        KeyBinding::new("left", Left, Some("Textarea")),
        KeyBinding::new("right", Right, Some("Textarea")),
        KeyBinding::new("up", Up, Some("Textarea")),
        KeyBinding::new("down", Down, Some("Textarea")),
        KeyBinding::new("shift-left", SelectLeft, Some("Textarea")),
        KeyBinding::new("shift-right", SelectRight, Some("Textarea")),
        KeyBinding::new("shift-up", SelectUp, Some("Textarea")),
        KeyBinding::new("shift-down", SelectDown, Some("Textarea")),
        KeyBinding::new("cmd-a", SelectAll, Some("Textarea")),
        KeyBinding::new("cmd-v", Paste, Some("Textarea")),
        KeyBinding::new("cmd-c", Copy, Some("Textarea")),
        KeyBinding::new("cmd-x", Cut, Some("Textarea")),
        KeyBinding::new("home", Home, Some("Textarea")),
        KeyBinding::new("end", End, Some("Textarea")),
        KeyBinding::new("enter", InsertNewline, Some("Textarea")),
        KeyBinding::new("shift-enter", InsertNewline, Some("Textarea")),
        KeyBinding::new("cmd-enter", Submit, Some("Textarea")),
        KeyBinding::new("ctrl-enter", Submit, Some("Textarea")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("Textarea")),
    ]);
}

/// 标准多行文本输入组件。
///
/// 组件内部维护完整编辑状态，同时通过 `on_change` 和 `set_value` 支持外部同步。
/// `set_value`、`set_disabled`、`set_readonly`、`set_status` 和 `set_helper_text`
/// 都是受控同步方法，默认不会触发用户变化回调，避免父组件写回时形成回调循环。
pub struct Textarea {
    focus_handle: FocusHandle,
    state: TextareaState,
    placeholder: SharedString,
    disabled: bool,
    readonly: bool,
    required: bool,
    rows: usize,
    min_rows: Option<usize>,
    max_rows: Option<usize>,
    size: TextareaSize,
    variant: TextareaVariant,
    status: TextareaStatus,
    helper_text: Option<SharedString>,
    on_change: Option<props::TextareaChangeHandler>,
    on_focus: Option<props::TextareaFocusHandler>,
    on_blur: Option<props::TextareaFocusHandler>,
    on_submit: Option<props::TextareaSubmitHandler>,
    on_key_down: Option<props::TextareaKeyDownHandler>,
    last_layout: Option<TextareaLayout>,
    last_bounds: Option<Bounds<Pixels>>,
    last_scroll_y: Pixels,
    last_content_height: Pixels,
    last_viewport_height: Pixels,
    is_selecting: bool,
    auto_scroll_direction: Option<AutoScrollDirection>,
    auto_scroll_active: bool,
    is_focused: bool,
    cursor_blink_visible: bool,
    cursor_blink_epoch: u64,
    preferred_vertical_x: Option<Pixels>,
    /// 禁用态受控同步期间是否需要静默吸收下一次焦点变化。
    ///
    /// `set_disabled(true)` 是父组件写入的受控状态，不代表用户主动离开输入框。
    /// 如果禁用前组件处于聚焦态，下一次 render 会因为 `disabled` 把交互焦点视为 false；
    /// 这里记录该变化来自受控同步，让 `sync_focus_callbacks` 只更新内部状态而不触发
    /// `on_blur`。重新启用时如果底层 `FocusHandle` 仍然聚焦，同样静默恢复内部聚焦态，
    /// 避免父组件同步 disabled 时收到伪造的 `on_focus`。
    suppress_next_focus_callback: bool,
    /// 下一次排版是否需要把光标滚入可见区域。
    ///
    /// 多行输入框同时支持“编辑时跟随光标”和“滚轮查看非光标位置内容”两种语义。
    /// 如果每次重绘都强制按光标位置修正 `last_scroll_y`，用户向上滚动查看历史内容时，
    /// 下一帧会因为光标仍在底部而自动滚回底部。该标记把 reveal 行为改成一次性请求：
    /// 输入、选区移动、点击定位和重新聚焦会打开请求；纯滚轮滚动会取消请求。
    reveal_cursor_on_next_layout: bool,
}

/// 拖选时的自动纵向滚动方向。
///
/// 鼠标拖到文本区域上下边缘时，组件会持续推进纵向滚动并扩展选区。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AutoScrollDirection {
    /// 向上滚动并扩展选区。
    Up,
    /// 向下滚动并扩展选区。
    Down,
}

impl Textarea {
    /// 创建新的 `Textarea`。
    pub fn new(cx: &mut Context<Self>, props: TextareaProps) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state: TextareaState::new(props.value, props.max_length),
            placeholder: props.placeholder,
            disabled: props.disabled,
            readonly: props.readonly,
            required: props.required,
            rows: props.rows,
            min_rows: props.min_rows,
            max_rows: props.max_rows,
            size: props.size,
            variant: props.variant,
            status: props.status,
            helper_text: props.helper_text,
            on_change: props.on_change,
            on_focus: props.on_focus,
            on_blur: props.on_blur,
            on_submit: props.on_submit,
            on_key_down: props.on_key_down,
            last_layout: None,
            last_bounds: None,
            last_scroll_y: px(0.0),
            last_content_height: px(0.0),
            last_viewport_height: px(0.0),
            is_selecting: false,
            auto_scroll_direction: None,
            auto_scroll_active: false,
            is_focused: false,
            cursor_blink_visible: true,
            cursor_blink_epoch: 0,
            preferred_vertical_x: None,
            suppress_next_focus_callback: false,
            reveal_cursor_on_next_layout: true,
        }
    }

    /// 返回当前文本值。
    pub fn value(&self) -> &SharedString {
        self.state.content()
    }

    /// 从外部同步文本值。
    ///
    /// 该方法不会触发 `on_change`，避免调用方在受控同步时形成回调循环。
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let before = self.state.content().clone();
        self.state.set_content_silent(value);
        if self.state.content() != &before {
            self.preferred_vertical_x = None;
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
            cx.notify();
        }
    }

    /// 从外部同步禁用状态。
    ///
    /// 禁用属于父组件受控输入，不表达用户交互语义；因此只清理内部交互状态并刷新界面，
    /// 不触发 `on_change`、`on_focus` 或 `on_blur`。重新启用只恢复可交互能力，不自动聚焦。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }

        self.disabled = disabled;
        if disabled {
            self.suppress_next_focus_callback |= self.is_focused;
            self.is_focused = false;
            self.stop_cursor_blink();
            self.is_selecting = false;
            self.auto_scroll_direction = None;
            self.auto_scroll_active = false;
        }
        cx.notify();
    }

    /// 从外部同步只读状态。
    ///
    /// 只读只影响编辑能力，不清空选区、不改变当前文本，也不触发内容变化回调。
    pub fn set_readonly(&mut self, readonly: bool, cx: &mut Context<Self>) {
        if self.readonly == readonly {
            return;
        }

        self.readonly = readonly;
        cx.notify();
    }

    /// 从外部同步语义状态。
    ///
    /// 状态只影响视觉样式，不改变文本、选区、滚动位置或任何交互回调。
    pub fn set_status(&mut self, status: TextareaStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }

        self.status = status;
        cx.notify();
    }

    /// 从外部同步辅助文本。
    ///
    /// helper text 是纯展示输入，不改变值、选区、焦点或滚动语义。
    pub fn set_helper_text(
        &mut self,
        helper_text: impl Into<Option<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let helper_text = helper_text.into();
        if self.helper_text == helper_text {
            return;
        }

        self.helper_text = helper_text;
        cx.notify();
    }

    /// 清空文本并触发变化回调。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let outcome = self.state.clear();
        self.apply_edit_outcome(outcome, cx);
    }

    /// 选中全部文本。
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.select_all();
        self.apply_selection_outcome(outcome, cx);
    }

    /// 返回焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 是否允许修改文本。
    fn can_edit(&self) -> bool {
        !self.disabled && !self.readonly
    }

    /// 当前渲染样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedTextareaStyle {
        resolve_textarea_style(
            self.size,
            self.variant,
            self.status,
            focused,
            self.disabled,
            TextareaRows {
                rows: self.rows,
                min_rows: self.min_rows,
                max_rows: self.max_rows,
                content_rows: self.state.hard_line_count(),
            },
            cx,
        )
    }

    /// 应用内容编辑结果。
    fn apply_edit_outcome(&mut self, outcome: TextareaEditOutcome, cx: &mut Context<Self>) {
        if outcome.content_changed {
            self.emit_change();
        }
        if outcome.should_notify() {
            self.preferred_vertical_x = None;
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
            cx.notify();
        }
    }

    /// 应用只影响选区的结果。
    fn apply_selection_outcome(&mut self, outcome: TextareaEditOutcome, cx: &mut Context<Self>) {
        if outcome.should_notify() {
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
            cx.notify();
        }
    }

    /// 请求下一次布局阶段把当前光标或选区端点滚入可见区域。
    ///
    /// 该请求只保留到下一次 `TextareaTextElement::paint`，避免后续光标闪烁、
    /// 滚动条刷新等非定位交互继续覆盖用户手动滚动位置。
    fn request_cursor_reveal(&mut self) {
        self.reveal_cursor_on_next_layout = true;
    }

    /// 触发内容变化回调。
    fn emit_change(&mut self) {
        if let Some(on_change) = self.on_change.as_mut() {
            on_change(self.state.content().clone());
        }
    }

    /// 同步焦点状态并触发焦点回调。
    fn sync_focus_callbacks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        if focused == self.is_focused {
            if !focused && !self.disabled {
                // 如果禁用期间底层焦点已经移动到其他组件，说明重新启用后不会发生
                // “静默恢复聚焦态”，此时需要清理抑制标记，避免下一次真实用户聚焦被吞掉。
                self.suppress_next_focus_callback = false;
            }
            return;
        }

        self.is_focused = focused;
        if focused {
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
            if self.suppress_next_focus_callback {
                self.suppress_next_focus_callback = false;
            } else if let Some(on_focus) = self.on_focus.as_mut() {
                on_focus();
            }
        } else {
            self.stop_cursor_blink();
            if self.suppress_next_focus_callback {
                self.suppress_next_focus_callback = false;
            } else if let Some(on_blur) = self.on_blur.as_mut() {
                on_blur();
            }
        }
    }

    /// 重置光标闪烁周期。
    fn restart_cursor_blink(&mut self, cx: &mut Context<Self>) {
        if !self.is_focused || self.disabled || !self.state.selected_range().is_empty() {
            self.stop_cursor_blink();
            return;
        }

        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        let epoch = self.cursor_blink_epoch;

        if !self.cursor_blink_visible {
            self.cursor_blink_visible = true;
            cx.notify();
        }

        cx.spawn(async move |this: WeakEntity<Textarea>, cx: &mut AsyncApp| {
            Timer::after(CURSOR_BLINK_IDLE_DELAY).await;
            loop {
                let should_continue = this
                    .update(cx, |textarea, cx| textarea.tick_cursor_blink(epoch, cx))
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
                Timer::after(CURSOR_BLINK_INTERVAL).await;
            }
        })
        .detach();
    }

    /// 停止当前光标闪烁任务。
    fn stop_cursor_blink(&mut self) {
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        self.cursor_blink_visible = true;
    }

    /// 执行一次光标闪烁切换。
    fn tick_cursor_blink(&mut self, epoch: u64, cx: &mut Context<Self>) -> bool {
        if epoch != self.cursor_blink_epoch
            || !self.is_focused
            || self.disabled
            || !self.state.selected_range().is_empty()
        {
            return false;
        }

        self.cursor_blink_visible = !self.cursor_blink_visible;
        cx.notify();
        true
    }

    /// 根据鼠标位置和指定滚动量计算文本字节偏移。
    fn index_for_mouse_position_with_scroll(
        &self,
        position: Point<Pixels>,
        scroll_y: Pixels,
    ) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return self.state.content().len();
        };

        layout.offset_for_point(position, *bounds, scroll_y)
    }

    /// 根据鼠标位置计算文本字节偏移。
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        self.index_for_mouse_position_with_scroll(position, self.last_scroll_y)
    }

    /// 根据拖选鼠标位置更新自动滚动方向。
    fn update_auto_scroll_direction(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.auto_scroll_direction = self.auto_scroll_direction_for_position(position);
        if self.auto_scroll_direction.is_some() {
            self.ensure_auto_scroll_task(cx);
        }
    }

    /// 判断当前位置是否需要触发拖选自动滚动。
    fn auto_scroll_direction_for_position(
        &self,
        position: Point<Pixels>,
    ) -> Option<AutoScrollDirection> {
        let bounds = self.last_bounds.as_ref()?;
        let max_scroll = max_scroll_y(self.last_content_height, self.last_viewport_height);
        let edge = px(8.0);

        if position.y <= bounds.top() + edge && self.last_scroll_y > px(0.0) {
            Some(AutoScrollDirection::Up)
        } else if position.y >= bounds.bottom() - edge && self.last_scroll_y < max_scroll {
            Some(AutoScrollDirection::Down)
        } else {
            None
        }
    }

    /// 确保拖选自动滚动任务已经启动。
    fn ensure_auto_scroll_task(&mut self, cx: &mut Context<Self>) {
        if self.auto_scroll_active {
            return;
        }

        self.auto_scroll_active = true;
        cx.spawn(
            async move |this: WeakEntity<Textarea>, cx: &mut AsyncApp| loop {
                Timer::after(Duration::from_millis(16)).await;
                let keep_scrolling = this
                    .update(cx, |textarea, cx| textarea.tick_auto_scroll(cx))
                    .unwrap_or(false);
                if !keep_scrolling {
                    break;
                }
            },
        )
        .detach();
    }

    /// 执行一次拖选自动滚动。
    fn tick_auto_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(direction) = self.auto_scroll_direction else {
            self.auto_scroll_active = false;
            return false;
        };
        if !self.is_selecting || self.disabled {
            self.auto_scroll_direction = None;
            self.auto_scroll_active = false;
            return false;
        }

        let Some(outcome) = self.scroll_selection_once(direction) else {
            self.auto_scroll_direction = None;
            self.auto_scroll_active = false;
            return false;
        };
        if outcome.should_notify() {
            cx.notify();
        }
        true
    }

    /// 按指定方向滚动一小步并扩展选区。
    fn scroll_selection_once(
        &mut self,
        direction: AutoScrollDirection,
    ) -> Option<TextareaEditOutcome> {
        let bounds = self.last_bounds?;
        let max_scroll = max_scroll_y(self.last_content_height, self.last_viewport_height);
        let current_scroll = self.last_scroll_y;
        let step = px(14.0);

        let next_scroll = match direction {
            AutoScrollDirection::Up => {
                if current_scroll <= px(0.0) {
                    return None;
                }
                if current_scroll > step {
                    current_scroll - step
                } else {
                    px(0.0)
                }
            }
            AutoScrollDirection::Down => {
                if current_scroll >= max_scroll {
                    return None;
                }
                let candidate = current_scroll + step;
                if candidate < max_scroll {
                    candidate
                } else {
                    max_scroll
                }
            }
        };

        self.last_scroll_y = next_scroll;
        // 拖选自动滚动本质上仍然是选区定位行为，下一帧需要继续保证选区端点可见；
        // 这与滚轮“浏览内容”的意图不同，因此不能取消 reveal 请求。
        self.request_cursor_reveal();
        let edge_y = match direction {
            AutoScrollDirection::Up => bounds.top(),
            AutoScrollDirection::Down => bounds.bottom(),
        };
        let target =
            self.index_for_mouse_position_with_scroll(point(bounds.left(), edge_y), next_scroll);
        let mut outcome = self.state.select_to(target);
        outcome.selection_changed = true;
        Some(outcome)
    }

    /// 光标左移。
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = if self.state.selected_range().is_empty() {
            self.state.previous_boundary(self.state.cursor_offset())
        } else {
            self.state.selected_range().start
        };
        self.preferred_vertical_x = None;
        let outcome = self.state.move_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 光标右移。
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = if self.state.selected_range().is_empty() {
            self.state.next_boundary(self.state.selected_range().end)
        } else {
            self.state.selected_range().end
        };
        self.preferred_vertical_x = None;
        let outcome = self.state.move_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 光标上移一行，优先按软换行后的可视行移动。
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, false, cx);
    }

    /// 光标下移一行，优先按软换行后的可视行移动。
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, false, cx);
    }

    /// 向左扩展选区。
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = self.state.previous_boundary(self.state.cursor_offset());
        self.preferred_vertical_x = None;
        let outcome = self.state.select_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 向右扩展选区。
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = self.state.next_boundary(self.state.cursor_offset());
        self.preferred_vertical_x = None;
        let outcome = self.state.select_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 向上扩展选区。
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, cx);
    }

    /// 向下扩展选区。
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, cx);
    }

    /// 按可视行移动或扩展选区。
    fn move_vertically(&mut self, rows: isize, selecting: bool, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }

        let target = self.vertical_move_target(rows);
        let outcome = if selecting {
            self.state.select_to(target)
        } else {
            self.state.move_to(target)
        };
        self.apply_selection_outcome(outcome, cx);
    }

    /// 计算纵向移动目标偏移。
    fn vertical_move_target(&mut self, rows: isize) -> usize {
        let Some(layout) = self.last_layout.as_ref() else {
            return if rows < 0 {
                self.state.start_of_hard_line(self.state.cursor_offset())
            } else {
                self.state.end_of_hard_line(self.state.cursor_offset())
            };
        };

        let cursor = self.state.cursor_offset();
        let Some(current) = layout.position_for_offset(cursor) else {
            return cursor;
        };
        let preferred_x = self.preferred_vertical_x.unwrap_or(current.x);
        self.preferred_vertical_x = Some(preferred_x);
        let target_y = current.y + layout.line_height * rows as f32;
        layout.offset_for_local_point(point(preferred_x, target_y))
    }

    /// 选中全部文本。
    fn select_all_action(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all(cx);
    }

    /// 光标移动到当前硬行开头。
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.preferred_vertical_x = None;
        let target = self.state.start_of_hard_line(self.state.cursor_offset());
        let outcome = self.state.move_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 光标移动到当前硬行末尾。
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.preferred_vertical_x = None;
        let target = self.state.end_of_hard_line(self.state.cursor_offset());
        let outcome = self.state.move_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 删除光标前的文本或当前选区。
    fn backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if self.state.selected_range().is_empty() {
            let target = self.state.previous_boundary(self.state.cursor_offset());
            self.state.select_to(target);
        }
        let outcome = self.state.replace_text_in_range(None, "");
        self.apply_edit_outcome(outcome, cx);
    }

    /// 删除光标后的文本或当前选区。
    fn delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if self.state.selected_range().is_empty() {
            let target = self.state.next_boundary(self.state.cursor_offset());
            self.state.select_to(target);
        }
        let outcome = self.state.replace_text_in_range(None, "");
        self.apply_edit_outcome(outcome, cx);
    }

    /// 插入换行。
    fn insert_newline(&mut self, _: &InsertNewline, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let outcome = self.state.replace_text_in_range(None, "\n");
        self.apply_edit_outcome(outcome, cx);
    }

    /// 触发提交回调。
    fn submit(&mut self, _: &Submit, _: &mut Window, _: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if let Some(on_submit) = self.on_submit.as_mut() {
            on_submit(self.state.content().clone());
        }
    }

    /// 鼠标按下时聚焦并移动或扩展选区。
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        self.preferred_vertical_x = None;

        let target = self.index_for_mouse_position(event.position);
        let outcome = if event.modifiers.shift {
            self.state.select_to(target)
        } else {
            self.state.move_to(target)
        };
        if !outcome.should_notify() {
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
        }
        self.update_auto_scroll_direction(event.position, cx);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 鼠标在输入框外按下时释放焦点。
    fn on_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus_handle.is_focused(window) {
            return;
        }

        self.is_selecting = false;
        self.auto_scroll_direction = None;
        window.blur();
        self.sync_focus_callbacks(window, cx);
        cx.notify();
    }

    /// 鼠标松开时停止拖选。
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
        self.auto_scroll_direction = None;
    }

    /// 鼠标拖动时扩展选区。
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || !self.is_selecting {
            return;
        }
        let target = self.index_for_mouse_position(event.position);
        let outcome = self.state.select_to(target);
        if !outcome.should_notify() {
            self.request_cursor_reveal();
            self.restart_cursor_blink(cx);
        }
        self.update_auto_scroll_direction(event.position, cx);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 鼠标滚轮滚动内部文本视口。
    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        let line_height = self
            .last_layout
            .as_ref()
            .map(|layout| layout.line_height)
            .unwrap_or(px(20.0));
        let delta = event.delta.pixel_delta(line_height);
        let next = self.last_scroll_y - delta.y;
        let max_scroll = max_scroll_y(self.last_content_height, self.last_viewport_height);
        let next = clamp_pixels(next, px(0.0), max_scroll);
        if next != self.last_scroll_y {
            // 滚轮表达的是用户主动查看其他垂直位置。此时必须关闭下一帧的光标 reveal，
            // 否则预绘制会再次以底部光标为准修正滚动量，造成“向上滚动后自动弹回底部”。
            self.reveal_cursor_on_next_layout = false;
            self.last_scroll_y = next;
            cx.notify();
        }
    }

    /// 打开系统字符面板。
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        if self.can_edit() {
            window.show_character_palette();
        }
    }

    /// 粘贴剪贴板文本。
    fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let outcome = self.state.replace_text_in_range(None, &text);
            self.apply_edit_outcome(outcome, cx);
        }
    }

    /// 复制当前选区文本。
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.state.selected_range().is_empty() {
            return;
        }
        let range = self.state.selected_range();
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.state.as_str()[range].to_string(),
        ));
    }

    /// 剪切当前选区文本。
    fn cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() || self.state.selected_range().is_empty() {
            return;
        }
        let range = self.state.selected_range();
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.state.as_str()[range].to_string(),
        ));
        let outcome = self.state.replace_text_in_range(None, "");
        self.apply_edit_outcome(outcome, cx);
    }

    /// 响应键盘按下事件。
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, _: &mut Context<Self>) {
        if let Some(on_key_down) = self.on_key_down.as_mut() {
            on_key_down(event.keystroke.clone());
        }
    }
}

impl EntityInputHandler for Textarea {
    /// 返回指定 UTF-16 区间文本。
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        self.state.text_for_range_utf16(range_utf16, actual_range)
    }

    /// 返回当前 UTF-16 选区。
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.disabled && !ignore_disabled_input {
            return None;
        }
        Some(self.state.selected_text_range_utf16())
    }

    /// 返回当前 IME marked text 区间。
    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state
            .marked_range()
            .as_ref()
            .map(|range| self.state.range_to_utf16(range))
    }

    /// 取消 IME marked text。
    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let outcome = self.state.unmark_text();
        self.apply_selection_outcome(outcome, cx);
    }

    /// 处理平台普通文本替换。
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let outcome = self.state.replace_text_in_range(range_utf16, new_text);
        self.apply_edit_outcome(outcome, cx);
    }

    /// 处理平台 IME marked text 替换。
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let outcome = self.state.replace_and_mark_text_in_range(
            range_utf16,
            new_text,
            new_selected_range_utf16,
        );
        self.apply_edit_outcome(outcome, cx);
    }

    /// 返回指定文本范围在屏幕中的边界，用于定位输入法候选框。
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.state.range_from_utf16(&range_utf16);
        let start = layout.position_for_offset(range.start)?;
        let end = layout.position_for_offset(range.end).unwrap_or(start);
        let x1 = bounds.left() + start.x;
        let x2 = bounds.left() + end.x;
        let y = bounds.top() + start.y - self.last_scroll_y;
        Some(Bounds::from_corners(
            point(if x1 < x2 { x1 } else { x2 }, y),
            point(
                if x1 > x2 { x1 } else { x2 } + px(1.0),
                y + layout.line_height,
            ),
        ))
    }

    /// 根据屏幕坐标返回 UTF-16 字符偏移。
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let utf8_index = layout.offset_for_point(point, bounds, self.last_scroll_y);
        Some(self.state.offset_to_utf16(utf8_index))
    }
}

/// 负责绘制文本、选区、光标并接入平台输入的底层元素。
struct TextareaTextElement {
    input: Entity<Textarea>,
}

/// `TextareaTextElement` 在 prepaint 阶段计算出的绘制状态。
struct PrepaintState {
    layout: TextareaLayout,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
    scrollbar: Option<TextareaScrollbar>,
    scroll_y: Pixels,
}

/// textarea 内部滚动条的绘制结果。
///
/// 滚动条只在内容高度超过视口高度时存在；轨道和滑块都在文本元素内部绘制，
/// 因而无需额外的 gpui 子元素，也不会影响平台输入处理器的 bounds。
struct TextareaScrollbar {
    /// 滚动条轨道。
    track: PaintQuad,
    /// 表示当前滚动位置的滑块。
    thumb: PaintQuad,
}

impl IntoElement for TextareaTextElement {
    type Element = Self;

    /// 将底层文本元素转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextareaTextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    /// 文本元素不需要稳定 id，输入处理通过外层 `FocusHandle` 关联。
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// 当前元素由组件内部生成，不暴露源码位置给 inspector。
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// 请求文本视口布局。
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let focused = input.focus_handle.is_focused(window);
        let resolved = input.resolved_style(focused, cx);
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = resolved.viewport_height.into();
        (window.request_layout(style, [], cx), ())
    }

    /// 计算多行文本布局、选区、光标和纵向滚动。
    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let focused = input.focus_handle.is_focused(window);
        let resolved = input.resolved_style(focused, cx);
        let content = input.state.content().clone();
        let selected_range = input.state.selected_range();
        let cursor_offset = input.state.cursor_offset();
        let marked_range = input.state.marked_range();
        let text_style = window.text_style();

        let showing_placeholder = content.is_empty();
        let display_text = if showing_placeholder {
            input.placeholder.clone()
        } else {
            content.clone()
        };
        let text_color = if showing_placeholder {
            resolved.placeholder
        } else {
            resolved.text
        };
        let layout_text = if display_text.is_empty() {
            SharedString::from(" ")
        } else {
            display_text.clone()
        };

        let base_run = TextRun {
            len: layout_text.len(),
            font: text_style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if !showing_placeholder {
            marked_runs(
                layout_text.len(),
                marked_range,
                base_run,
                resolved.marked_underline,
            )
        } else {
            vec![base_run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let wrap_width = textarea_wrap_width(bounds.size.width);
        let lines = window
            .text_system()
            .shape_text(
                layout_text.clone(),
                font_size,
                &runs,
                Some(wrap_width),
                None,
            )
            .unwrap_or_else(|_| Vec::new().into());
        let lines = lines.into_iter().collect::<Vec<_>>();
        let layout = TextareaLayout::new(lines, resolved.line_height);
        let content_height = layout.content_height();
        let scroll_y = next_scroll_y(
            input.last_scroll_y,
            &layout,
            cursor_offset,
            bounds.size.height,
            content_height,
            input.cursor_blink_visible,
            input.reveal_cursor_on_next_layout,
        );

        let selections = if showing_placeholder || selected_range.is_empty() {
            Vec::new()
        } else {
            layout.selection_quads(selected_range.clone(), bounds, scroll_y, resolved.selection)
        };

        let cursor = if selected_range.is_empty() && input.cursor_blink_visible {
            layout
                .position_for_offset(cursor_offset)
                .map(|cursor_position| {
                    fill(
                        Bounds::new(
                            point(
                                bounds.left() + cursor_position.x,
                                bounds.top() + cursor_position.y - scroll_y,
                            ),
                            size(px(1.5), resolved.line_height),
                        ),
                        resolved.cursor,
                    )
                })
        } else {
            None
        };
        let scrollbar = scrollbar_for(
            bounds,
            scroll_y,
            content_height,
            bounds.size.height,
            resolved.scrollbar_track,
            resolved.scrollbar_thumb,
        );

        PrepaintState {
            layout,
            cursor,
            selections,
            scrollbar,
            scroll_y,
        }
    }

    /// 绘制文本并把文本元素注册为平台输入处理器。
    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, disabled, line_height) = {
            let input = self.input.read(cx);
            (
                input.focus_handle.clone(),
                input.disabled,
                prepaint.layout.line_height,
            )
        };

        if !disabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
        }

        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }

        let mut offset_y = px(0.0);
        for line in &prepaint.layout.lines {
            let line_height_total = line.size(line_height).height;
            let screen_y = bounds.top() + offset_y - prepaint.scroll_y;
            if screen_y + line_height_total >= bounds.top() && screen_y <= bounds.bottom() {
                line.paint(
                    point(bounds.left(), screen_y),
                    line_height,
                    TextAlign::Left,
                    Some(bounds),
                    window,
                    cx,
                )
                .expect("Textarea line painting should succeed");
            }
            offset_y += line_height_total;
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        if let Some(scrollbar) = prepaint.scrollbar.take() {
            window.paint_quad(scrollbar.track);
            window.paint_quad(scrollbar.thumb);
        }

        self.input.update(cx, |input, _cx| {
            input.last_content_height = prepaint.layout.content_height();
            input.last_viewport_height = bounds.size.height;
            input.last_scroll_y = prepaint.scroll_y;
            // 光标滚入视口是一次性定位请求。当前绘制已经消费了请求后的滚动量，
            // 后续由光标闪烁、滚动条重绘等原因触发的刷新不应继续改写用户滚动位置。
            input.reveal_cursor_on_next_layout = false;
            input.last_bounds = Some(bounds);
            input.last_layout = Some(prepaint.layout.clone());
        });
    }
}

impl Render for Textarea {
    /// 渲染多行输入框。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus_callbacks(window, cx);

        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let helper_text = self.helper_text.clone();
        let required = self.required;

        let field = div()
            .flex()
            .items_start()
            .w_full()
            .h(resolved.height)
            .px(resolved.padding_x)
            .py(resolved.padding_y)
            .rounded(resolved.radius)
            .border_1()
            .border_color(resolved.border)
            .bg(resolved.background)
            .text_color(resolved.text)
            .text_size(resolved.font_size)
            .line_height(resolved.line_height)
            .opacity(resolved.opacity)
            .overflow_hidden()
            .when_else(
                self.disabled,
                |this| this.cursor(CursorStyle::Arrow),
                |this| {
                    this.track_focus(&self.focus_handle)
                        .cursor(CursorStyle::IBeam)
                        .key_context("Textarea")
                        .on_action(cx.listener(Self::backspace))
                        .on_action(cx.listener(Self::delete))
                        .on_action(cx.listener(Self::left))
                        .on_action(cx.listener(Self::right))
                        .on_action(cx.listener(Self::up))
                        .on_action(cx.listener(Self::down))
                        .on_action(cx.listener(Self::select_left))
                        .on_action(cx.listener(Self::select_right))
                        .on_action(cx.listener(Self::select_up))
                        .on_action(cx.listener(Self::select_down))
                        .on_action(cx.listener(Self::select_all_action))
                        .on_action(cx.listener(Self::home))
                        .on_action(cx.listener(Self::end))
                        .on_action(cx.listener(Self::insert_newline))
                        .on_action(cx.listener(Self::submit))
                        .on_action(cx.listener(Self::show_character_palette))
                        .on_action(cx.listener(Self::paste))
                        .on_action(cx.listener(Self::cut))
                        .on_action(cx.listener(Self::copy))
                        .on_key_down(cx.listener(Self::on_key_down))
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                        .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                        .on_mouse_move(cx.listener(Self::on_mouse_move))
                        .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                },
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(TextareaTextElement { input: cx.entity() }),
            )
            .when(required, |this| {
                this.child(
                    div()
                        .flex_none()
                        .text_color(crate::foundation::color::danger_500())
                        .child("*"),
                )
            });

        div().flex().flex_col().gap(px(4.0)).child(field).when_some(
            helper_text,
            |this, helper_text| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(resolved.helper)
                        .child(helper_text),
                )
            },
        )
    }
}

impl Focusable for Textarea {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// 返回 textarea 文本实际用于软换行的宽度。
///
/// 滚动条是否出现依赖内容排版后的高度；如果只在可滚动时才扣除滚动条宽度，
/// 某些临界宽度下会出现“扣除宽度后多换一行，从而出现滚动条”的循环变化。
/// 因此这里始终预留一个很窄的 gutter，让文本排版和滚动条显示状态彼此独立。
fn textarea_wrap_width(width: Pixels) -> Pixels {
    if width > SCROLLBAR_GUTTER {
        width - SCROLLBAR_GUTTER
    } else {
        width
    }
}

/// 根据当前滚动状态计算滚动条绘制结果。
fn scrollbar_for(
    bounds: Bounds<Pixels>,
    scroll_y: Pixels,
    content_height: Pixels,
    viewport_height: Pixels,
    track_color: Hsla,
    thumb_color: Hsla,
) -> Option<TextareaScrollbar> {
    let max_scroll = max_scroll_y(content_height, viewport_height);
    if max_scroll <= px(0.0) || viewport_height <= SCROLLBAR_INSET_Y * 2.0 {
        return None;
    }

    let track_height = viewport_height - SCROLLBAR_INSET_Y * 2.0;
    let raw_thumb_height = track_height * (viewport_height / content_height);
    let thumb_height = if raw_thumb_height < SCROLLBAR_MIN_THUMB_HEIGHT {
        SCROLLBAR_MIN_THUMB_HEIGHT.min(track_height)
    } else {
        raw_thumb_height
    };
    let scroll_ratio = if max_scroll > px(0.0) {
        scroll_y / max_scroll
    } else {
        0.0
    };
    let thumb_top = bounds.top() + SCROLLBAR_INSET_Y + (track_height - thumb_height) * scroll_ratio;
    let track_left = bounds.right() - SCROLLBAR_WIDTH;

    let track = fill(
        Bounds::new(
            point(track_left, bounds.top() + SCROLLBAR_INSET_Y),
            size(SCROLLBAR_WIDTH, track_height),
        ),
        track_color,
    )
    .corner_radii(SCROLLBAR_WIDTH / 2.0);
    let thumb = fill(
        Bounds::new(
            point(track_left, thumb_top),
            size(SCROLLBAR_WIDTH, thumb_height),
        ),
        thumb_color,
    )
    .corner_radii(SCROLLBAR_WIDTH / 2.0);

    Some(TextareaScrollbar { track, thumb })
}

/// 最近一次多行文本排版结果。
#[derive(Clone)]
struct TextareaLayout {
    /// gpui 已排版并按宽度软换行后的逻辑行。
    lines: Vec<WrappedLine>,
    /// 每个逻辑行在完整文本中的 UTF-8 起始偏移。
    line_starts: Vec<usize>,
    /// 当前排版使用的行高。
    line_height: Pixels,
}

impl TextareaLayout {
    /// 创建排版结果并同步逻辑行起始偏移。
    fn new(lines: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let line_starts = line_starts_for_wrapped_lines(&lines);
        Self {
            lines,
            line_starts,
            line_height,
        }
    }

    /// 返回文本总高度。
    fn content_height(&self) -> Pixels {
        self.lines
            .iter()
            .map(|line| line.size(self.line_height).height)
            .fold(px(0.0), |height, line_height| height + line_height)
            .max(self.line_height)
    }

    /// 返回指定 UTF-8 偏移的局部坐标。
    fn position_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let mut y = px(0.0);
        for (index, line) in self.lines.iter().enumerate() {
            let line_start = self.line_starts.get(index).copied().unwrap_or(0);
            let line_end = line_start + line.len();
            if offset >= line_start && offset <= line_end {
                let local = offset.saturating_sub(line_start);
                return line
                    .position_for_index(local, self.line_height)
                    .map(|position| point(position.x, y + position.y));
            }
            y += line.size(self.line_height).height;
        }

        self.lines.last().and_then(|line| {
            let y = self.content_height() - line.size(self.line_height).height;
            line.position_for_index(line.len(), self.line_height)
                .map(|position| point(position.x, y + position.y))
        })
    }

    /// 根据屏幕坐标返回 UTF-8 偏移。
    fn offset_for_point(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        scroll_y: Pixels,
    ) -> usize {
        self.offset_for_local_point(point(
            position.x - bounds.left(),
            position.y - bounds.top() + scroll_y,
        ))
    }

    /// 根据文本局部坐标返回 UTF-8 偏移。
    fn offset_for_local_point(&self, position: Point<Pixels>) -> usize {
        if position.y <= px(0.0) {
            return 0;
        }

        let mut y = px(0.0);
        for (index, line) in self.lines.iter().enumerate() {
            let line_height = line.size(self.line_height).height;
            if position.y <= y + line_height {
                let local = point(position.x, position.y - y);
                let line_offset = line
                    .closest_index_for_position(local, self.line_height)
                    .unwrap_or_else(|offset| offset);
                let line_start = self.line_starts.get(index).copied().unwrap_or(0);
                return line_start + line_offset.min(line.len());
            }
            y += line_height;
        }

        self.line_starts
            .last()
            .zip(self.lines.last())
            .map(|(start, line)| *start + line.len())
            .unwrap_or(0)
    }

    /// 根据选区生成一组矩形绘制指令。
    fn selection_quads(
        &self,
        selection: Range<usize>,
        bounds: Bounds<Pixels>,
        scroll_y: Pixels,
        color: Hsla,
    ) -> Vec<PaintQuad> {
        let mut quads = Vec::new();
        let mut y = px(0.0);
        for (index, line) in self.lines.iter().enumerate() {
            let line_start = self.line_starts.get(index).copied().unwrap_or(0);
            let line_end = line_start + line.len();
            let next_line_start = self.line_starts.get(index + 1).copied().unwrap_or(line_end);

            if selection.start < next_line_start && selection.end > line_start {
                let start = selection.start.clamp(line_start, line_end);
                let end = selection.end.clamp(line_start, line_end);
                self.push_line_selection_quads(
                    line,
                    start - line_start,
                    end - line_start,
                    y,
                    bounds,
                    scroll_y,
                    color,
                    &mut quads,
                );
            }
            y += line.size(self.line_height).height;
        }
        quads
    }

    /// 为单个逻辑行内的选区生成矩形。
    #[allow(clippy::too_many_arguments)]
    fn push_line_selection_quads(
        &self,
        line: &WrappedLine,
        start: usize,
        end: usize,
        line_y: Pixels,
        bounds: Bounds<Pixels>,
        scroll_y: Pixels,
        color: Hsla,
        quads: &mut Vec<PaintQuad>,
    ) {
        let Some(start_position) = line.position_for_index(start, self.line_height) else {
            return;
        };
        let Some(end_position) = line.position_for_index(end, self.line_height) else {
            return;
        };

        let start_row = (start_position.y / self.line_height) as usize;
        let end_row = (end_position.y / self.line_height) as usize;
        let line_width = line.size(self.line_height).width.max(px(6.0));

        for row in start_row..=end_row {
            let row_y = self.line_height * row as f32;
            let mut x1 = if row == start_row {
                start_position.x
            } else {
                px(0.0)
            };
            let mut x2 = if row == end_row {
                end_position.x
            } else {
                line_width
            };
            if x2 < x1 {
                std::mem::swap(&mut x1, &mut x2);
            }
            if x2 == x1 {
                x2 += px(6.0);
            }

            let top = bounds.top() + line_y + row_y - scroll_y;
            let bottom = top + self.line_height;
            if bottom < bounds.top() || top > bounds.bottom() {
                continue;
            }

            quads.push(fill(
                Bounds::from_corners(
                    point(bounds.left() + x1, top),
                    point(bounds.left() + x2, bottom),
                ),
                color,
            ));
        }
    }
}

/// 根据 gpui 返回的 `WrappedLine` 顺序计算每条硬换行逻辑行在完整文本中的起始偏移。
///
/// `WrappedLine` 的命名容易和“软换行后的视觉行”混淆。gpui 的 `shape_text` 会先按 `\n`
/// 拆成硬换行逻辑行，每个 `WrappedLine` 内部再通过 `wrap_boundaries` 记录软换行位置；
/// 因此这里必须按每个 `WrappedLine::len()` 累计硬行长度，并额外跳过原始文本中的 `\n`。
/// 这样光标定位、鼠标命中、选区绘制和 IME bounds 都使用与文本系统一致的行起点。
fn line_starts_for_wrapped_lines(lines: &[WrappedLine]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len().max(1));
    let mut offset = 0;

    for line in lines {
        starts.push(offset);
        offset += line.len() + '\n'.len_utf8();
    }

    if starts.is_empty() {
        starts.push(0);
    }

    starts
}

/// 构造 marked text 的文本 run。
fn marked_runs(
    display_len: usize,
    marked_range: Option<Range<usize>>,
    base_run: TextRun,
    underline_color: Hsla,
) -> Vec<TextRun> {
    let Some(marked_range) = marked_range else {
        return vec![base_run];
    };
    if marked_range.start >= marked_range.end || marked_range.end > display_len {
        return vec![base_run];
    }

    vec![
        TextRun {
            len: marked_range.start,
            ..base_run.clone()
        },
        TextRun {
            len: marked_range.end - marked_range.start,
            underline: Some(UnderlineStyle {
                color: Some(underline_color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..base_run.clone()
        },
        TextRun {
            len: display_len - marked_range.end,
            ..base_run
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

/// 根据光标位置和一次性 reveal 请求计算下一帧纵向滚动量。
///
/// `cursor_visible` 只表示当前帧是否会绘制光标，`reveal_cursor` 才表示是否允许本次
/// 排版用光标位置改写滚动量。两者分离后，光标闪烁导致的普通重绘不会覆盖用户滚轮
/// 滚动，而输入、方向键和鼠标定位仍然可以主动把光标滚入视口。
fn next_scroll_y(
    current: Pixels,
    layout: &TextareaLayout,
    cursor_offset: usize,
    visible_height: Pixels,
    content_height: Pixels,
    cursor_visible: bool,
    reveal_cursor: bool,
) -> Pixels {
    let max_scroll = max_scroll_y(content_height, visible_height);
    if !cursor_visible || !reveal_cursor {
        return clamp_pixels(current, px(0.0), max_scroll);
    }

    let Some(cursor_position) = layout.position_for_offset(cursor_offset) else {
        return clamp_pixels(current, px(0.0), max_scroll);
    };
    let padding = px(2.0);
    let cursor_top = cursor_position.y;
    let cursor_bottom = cursor_position.y + layout.line_height;

    if cursor_top < current + padding {
        clamp_pixels(cursor_top - padding, px(0.0), max_scroll)
    } else if cursor_bottom > current + visible_height - padding {
        clamp_pixels(
            cursor_bottom - visible_height + padding,
            px(0.0),
            max_scroll,
        )
    } else {
        clamp_pixels(current, px(0.0), max_scroll)
    }
}

/// 返回文本在当前视口下允许的最大纵向滚动量。
fn max_scroll_y(content_height: Pixels, visible_height: Pixels) -> Pixels {
    if content_height > visible_height {
        content_height - visible_height
    } else {
        px(0.0)
    }
}

/// 将像素值夹在指定范围内。
fn clamp_pixels(value: Pixels, min: Pixels, max: Pixels) -> Pixels {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
