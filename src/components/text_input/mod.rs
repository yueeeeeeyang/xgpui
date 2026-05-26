//! 单行文本输入组件。
//!
//! `TextInput` 基于 gpui 的 `EntityInputHandler` 接入平台文本输入能力，
//! 支持 IME、选区、剪贴板、鼠标定位、键盘动作和常见输入框视觉状态。

use std::{ops::Range, time::Duration};

use gpui::prelude::*;
use gpui::{
    actions, div, fill, point, px, relative, size, App, AsyncApp, Bounds, ClipboardItem, Context,
    CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, GlobalElementId, Hsla, InspectorElementId, IntoElement, KeyBinding, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement,
    PathBuilder, Pixels, Point, Render, ShapedLine, SharedString, Style, TextRun, Timer,
    UTF16Selection, UnderlineStyle, WeakEntity, Window,
};

mod display;
mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

use display::TextDisplayText;
pub use props::{
    TextInputProps, TextInputSize, TextInputSlot, TextInputStatus, TextInputType, TextInputVariant,
};
use state::{TextEditOutcome, TextInputState};
use style::{resolve_text_input_style, ResolvedTextInputStyle};

actions!(
    xgpui_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
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

/// 注册 `TextInput` 默认键盘快捷键。
///
/// gpui 的键盘动作需要应用启动时注册。调用方应在 `Application::run` 的初始化闭包中调用本函数。
pub fn register_text_input_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
    ]);
}

/// 单行文本输入组件。
///
/// 组件内部维护完整编辑状态，同时通过 `on_change` 和 `set_value` 支持外部同步。
/// 调用方通常使用 `cx.new(|cx| TextInput::new(cx, props))` 创建实体，再把实体作为子元素渲染。
pub struct TextInput {
    focus_handle: FocusHandle,
    state: TextInputState,
    placeholder: SharedString,
    disabled: bool,
    readonly: bool,
    clearable: bool,
    required: bool,
    /// 当前输入框的内容类型，用于决定数字输入限制和密码显示方式。
    input_type: TextInputType,
    /// 密码类型当前是否明文展示；真实值始终保存在 `state` 中，不受该标记影响。
    password_visible: bool,
    size: TextInputSize,
    variant: TextInputVariant,
    status: TextInputStatus,
    helper_text: Option<SharedString>,
    prefix: Option<TextInputSlot>,
    suffix: Option<TextInputSlot>,
    on_change: Option<props::TextInputChangeHandler>,
    on_focus: Option<props::TextInputFocusHandler>,
    on_blur: Option<props::TextInputFocusHandler>,
    on_enter: Option<props::TextInputEnterHandler>,
    on_key_down: Option<props::TextInputKeyDownHandler>,
    last_layout: Option<ShapedLine>,
    /// 最近一次绘制使用的显示文本映射，用于把鼠标坐标和平台输入查询反算回真实文本偏移。
    last_display_text: Option<TextDisplayText>,
    last_bounds: Option<Bounds<Pixels>>,
    last_scroll_x: Pixels,
    is_selecting: bool,
    auto_scroll_direction: Option<AutoScrollDirection>,
    auto_scroll_active: bool,
    is_focused: bool,
    /// 当前光标是否处于可见帧。
    cursor_blink_visible: bool,
    /// 光标闪烁任务的版本号。
    ///
    /// 每次输入、移动或聚焦都会推进版本号，让旧的异步闪烁任务在下一次唤醒时自动退出。
    cursor_blink_epoch: u64,
    /// 最近一次非内容编辑交互事件。
    ///
    /// 组合型组件会把 TextInput 作为内部输入面板使用。内容变化已经有 `on_change` 和 notify，
    /// 但“按下 Enter 提交”和“失焦提交”本身不一定改变文本，因此需要一个 crate 内部可读的
    /// 事件快照，让父级组件通过 `cx.observe` 区分这些提交时机。
    interaction_event: TextInputInteractionEvent,
}

/// TextInput 暴露给 crate 内组合组件的交互事件类型。
///
/// 该类型不是公开 API，只用于 DateTimePicker 等组合组件在观察内部输入框 notify 时判断
/// notify 是普通重绘还是一次需要提交/校验的输入事件。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TextInputInteractionEvent {
    /// 单调递增的事件编号。观察方记录上次处理过的编号即可避免重复处理同一次事件。
    pub id: u64,
    /// 事件种类。
    pub kind: TextInputInteractionKind,
}

/// TextInput 内部交互事件种类。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum TextInputInteractionKind {
    /// 尚未产生需要组合组件关心的交互事件。
    #[default]
    None,
    /// 输入框获得焦点。
    Focus,
    /// 输入框失去焦点。
    Blur,
    /// 用户按下 Enter。
    Enter,
    /// 用户点击输入框内部清除按钮。
    Clear,
}

/// 拖选时的自动横向滚动方向。
///
/// 鼠标拖到文本区域左右边缘时，组件会按该方向持续推进横向滚动并扩展选区。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AutoScrollDirection {
    /// 向左滚动并扩展选区。
    Left,
    /// 向右滚动并扩展选区。
    Right,
}

impl TextInput {
    /// 创建新的 `TextInput`。
    pub fn new(cx: &mut Context<Self>, props: TextInputProps) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state: TextInputState::new_with_type(props.value, props.max_length, props.input_type),
            placeholder: props.placeholder,
            disabled: props.disabled,
            readonly: props.readonly,
            clearable: props.clearable,
            required: props.required,
            input_type: props.input_type,
            password_visible: false,
            size: props.size,
            variant: props.variant,
            status: props.status,
            helper_text: props.helper_text,
            prefix: props.prefix,
            suffix: props.suffix,
            on_change: props.on_change,
            on_focus: props.on_focus,
            on_blur: props.on_blur,
            on_enter: props.on_enter,
            on_key_down: props.on_key_down,
            last_layout: None,
            last_display_text: None,
            last_bounds: None,
            last_scroll_x: px(0.0),
            is_selecting: false,
            auto_scroll_direction: None,
            auto_scroll_active: false,
            is_focused: false,
            cursor_blink_visible: true,
            cursor_blink_epoch: 0,
            interaction_event: TextInputInteractionEvent::default(),
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
            self.restart_cursor_blink(cx);
            cx.notify();
        }
    }

    /// 从组合型组件内部同步禁用状态。
    ///
    /// 这是受控状态写入，不代表用户主动离开输入框，因此不会触发 `on_change`。如果禁用前输入框
    /// 正在聚焦，组件会停止光标闪烁和拖拽选择状态，下一次渲染时也不会再注册键盘和鼠标编辑动作。
    /// 该方法只在 crate 内部开放，避免在本轮 DataTable 需求中顺带扩大 `TextInput` 的公开 API。
    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }

        self.disabled = disabled;
        if disabled {
            self.is_focused = false;
            self.is_selecting = false;
            self.auto_scroll_direction = None;
            self.auto_scroll_active = false;
            self.stop_cursor_blink();
        } else {
            self.restart_cursor_blink(cx);
        }
        cx.notify();
    }

    /// 从组合型组件内部同步只读状态。
    ///
    /// 只读状态允许聚焦、选择和复制，但不允许编辑内容。该方法不触发外部回调，只用于父组件
    /// 在自身 `set_readonly` 或 mode 切换时把受控状态同步给内部 TextInput。
    pub(crate) fn set_readonly(&mut self, readonly: bool, cx: &mut Context<Self>) {
        if self.readonly == readonly {
            return;
        }

        self.readonly = readonly;
        cx.notify();
    }

    /// 从组合型组件内部同步语义状态。
    ///
    /// 该方法只刷新视觉状态，不改变内容、选区或回调语义。DateTimePicker 会在手动输入解析失败时
    /// 临时把内部输入框切到错误态，解析成功或外部同步后再恢复父组件传入的状态。
    pub(crate) fn set_status(&mut self, status: TextInputStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }

        self.status = status;
        cx.notify();
    }

    /// 返回当前语义状态。
    ///
    /// 该 getter 只暴露给 crate 内组合组件和测试使用。组合组件在解析失败等场景会通过
    /// `set_status` 同步内部输入框视觉状态，测试需要读取快照确认状态同步没有遗漏。
    #[cfg(test)]
    pub(crate) fn status(&self) -> TextInputStatus {
        self.status
    }

    /// 从组合型组件内部同步前后缀插槽。
    ///
    /// DateTimePicker 这类组合组件需要把自身的类型图标和清除按钮放入内部 TextInput。
    /// 插槽本身是渲染闭包，无法可靠比较是否等价，因此调用方负责只在插槽语义变化时调用本方法。
    pub(crate) fn set_slots(
        &mut self,
        prefix: Option<TextInputSlot>,
        suffix: Option<TextInputSlot>,
        cx: &mut Context<Self>,
    ) {
        self.prefix = prefix;
        self.suffix = suffix;
        cx.notify();
    }

    /// 返回最近一次内部交互事件。
    ///
    /// 调用方应记录 `id`，只处理比自己已见编号更新的事件。这里返回值而非可变引用，
    /// 避免组合组件清空事件时影响其他观察方。
    pub(crate) fn interaction_event(&self) -> TextInputInteractionEvent {
        self.interaction_event
    }

    /// 清空文本并触发变化回调。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let outcome = self.state.clear();
        self.apply_edit_outcome(outcome, cx);
    }

    /// 返回焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 选中全部文本。
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.select_all();
        self.apply_selection_outcome(outcome, cx);
    }

    /// 把光标移动到文本末尾。
    pub fn move_to_end(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.move_to(self.state.content().len());
        self.apply_selection_outcome(outcome, cx);
    }

    /// 是否允许修改文本。
    fn can_edit(&self) -> bool {
        !self.disabled && !self.readonly
    }

    /// 是否应该显示清除按钮。
    ///
    /// 清除按钮只在输入框拥有焦点时展示；失焦后隐藏，避免静态表单里出现多余的悬浮操作入口。
    fn show_clear_button(&self, focused: bool) -> bool {
        focused && self.clearable && self.can_edit() && !self.state.content().is_empty()
    }

    /// 是否应该显示密码可见性切换按钮。
    ///
    /// 密码按钮与清除按钮不同，它用于控制安全展示语义，因此只要是可用的密码输入框就始终展示，
    /// 即使当前输入框没有焦点也允许用户直接查看或隐藏密码。
    fn show_password_visibility_button(&self) -> bool {
        self.input_type == TextInputType::Password && !self.disabled
    }

    /// 当前渲染样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedTextInputStyle {
        resolve_text_input_style(
            self.size,
            self.variant,
            self.status,
            focused,
            self.disabled,
            cx,
        )
    }

    /// 应用内容编辑结果。
    fn apply_edit_outcome(&mut self, outcome: TextEditOutcome, cx: &mut Context<Self>) {
        if outcome.content_changed {
            self.emit_change();
        }
        if outcome.should_notify() {
            self.restart_cursor_blink(cx);
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 应用只影响选区的结果。
    fn apply_selection_outcome(&mut self, outcome: TextEditOutcome, cx: &mut Context<Self>) {
        if outcome.should_notify() {
            self.restart_cursor_blink(cx);
        }
        if outcome.should_notify() {
            cx.notify();
        }
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
            return;
        }

        self.is_focused = focused;
        if focused {
            self.restart_cursor_blink(cx);
            self.record_interaction_event(TextInputInteractionKind::Focus);
            if let Some(on_focus) = self.on_focus.as_mut() {
                on_focus();
            }
            cx.notify();
        } else {
            self.stop_cursor_blink();
            self.record_interaction_event(TextInputInteractionKind::Blur);
            if let Some(on_blur) = self.on_blur.as_mut() {
                on_blur();
            }
            cx.notify();
        }
    }

    /// 记录一次需要组合组件观察的交互事件。
    fn record_interaction_event(&mut self, kind: TextInputInteractionKind) {
        self.interaction_event = TextInputInteractionEvent {
            id: self.interaction_event.id.wrapping_add(1),
            kind,
        };
    }

    /// 重置光标闪烁周期。
    ///
    /// 用户正在输入、点击或移动光标时，光标应该保持常亮；静止超过
    /// `CURSOR_BLINK_IDLE_DELAY` 后，再由异步任务按固定间隔切换可见性。
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

        cx.spawn(
            async move |this: WeakEntity<TextInput>, cx: &mut AsyncApp| {
                Timer::after(CURSOR_BLINK_IDLE_DELAY).await;
                loop {
                    let should_continue = this
                        .update(cx, |input, cx| input.tick_cursor_blink(epoch, cx))
                        .unwrap_or(false);
                    if !should_continue {
                        break;
                    }
                    Timer::after(CURSOR_BLINK_INTERVAL).await;
                }
            },
        )
        .detach();
    }

    /// 停止当前光标闪烁任务。
    ///
    /// 失焦或禁用时不再绘制光标，通过推进版本号使已经挂起的异步任务自然退出。
    fn stop_cursor_blink(&mut self) {
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        self.cursor_blink_visible = true;
    }

    /// 执行一次光标闪烁切换。
    ///
    /// 返回 `true` 表示当前任务仍然有效并可继续循环；返回 `false` 表示组件状态已经变化，
    /// 例如用户移动了光标、组件失焦、禁用或当前存在选区，此时旧任务应退出。
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

    /// 根据鼠标位置和指定横向滚动量计算文本字节偏移。
    ///
    /// `scroll_x` 参数让拖选自动滚动可以先推进滚动量，再用新的可视窗口换算选区终点。
    fn index_for_mouse_position_with_scroll(
        &self,
        position: Point<Pixels>,
        scroll_x: Pixels,
    ) -> usize {
        if self.state.content().is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return self.state.content().len();
        };

        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.state.content().len();
        }

        let display_index = line.closest_index_for_x(position.x - bounds.left() + scroll_x);
        self.last_display_text
            .as_ref()
            .map(|display| display.display_to_actual(display_index))
            .unwrap_or(display_index)
    }

    /// 根据鼠标位置计算文本字节偏移。
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        self.index_for_mouse_position_with_scroll(position, self.last_scroll_x)
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
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return None;
        };
        let max_scroll = max_text_scroll(line.width, bounds.size.width);
        let edge = px(8.0);

        if position.x <= bounds.left() + edge && self.last_scroll_x > px(0.0) {
            Some(AutoScrollDirection::Left)
        } else if position.x >= bounds.right() - edge && self.last_scroll_x < max_scroll {
            Some(AutoScrollDirection::Right)
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
            async move |this: WeakEntity<TextInput>, cx: &mut AsyncApp| loop {
                Timer::after(Duration::from_millis(16)).await;
                let keep_scrolling = this
                    .update(cx, |input, cx| input.tick_auto_scroll(cx))
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
    fn scroll_selection_once(&mut self, direction: AutoScrollDirection) -> Option<TextEditOutcome> {
        let (bounds, line) = (self.last_bounds.as_ref()?, self.last_layout.as_ref()?);
        let max_scroll = max_text_scroll(line.width, bounds.size.width);
        let current_scroll = self.last_scroll_x;
        let step = px(14.0);

        let next_scroll = match direction {
            AutoScrollDirection::Left => {
                if current_scroll <= px(0.0) {
                    return None;
                }
                if current_scroll > step {
                    current_scroll - step
                } else {
                    px(0.0)
                }
            }
            AutoScrollDirection::Right => {
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

        self.last_scroll_x = next_scroll;
        let edge_x = match direction {
            AutoScrollDirection::Left => bounds.left(),
            AutoScrollDirection::Right => bounds.right(),
        };
        let target = line.closest_index_for_x(edge_x - bounds.left() + next_scroll);
        let target = self
            .last_display_text
            .as_ref()
            .map(|display| display.display_to_actual(target))
            .unwrap_or(target);
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
        let outcome = self.state.move_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 向左扩展选区。
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = self.state.previous_boundary(self.state.cursor_offset());
        let outcome = self.state.select_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 向右扩展选区。
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let target = self.state.next_boundary(self.state.cursor_offset());
        let outcome = self.state.select_to(target);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 选中全部文本。
    fn select_all_action(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all(cx);
    }

    /// 光标移动到开头。
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.move_to(0);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 光标移动到末尾。
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_end(cx);
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

        let target = self.index_for_mouse_position(event.position);
        let outcome = if event.modifiers.shift {
            self.state.select_to(target)
        } else {
            self.state.move_to(target)
        };
        if !outcome.should_notify() {
            self.restart_cursor_blink(cx);
        }
        self.update_auto_scroll_direction(event.position, cx);
        self.apply_selection_outcome(outcome, cx);
    }

    /// 鼠标在输入框外按下时释放焦点。
    ///
    /// gpui 不会因为点击空白区域自动清空当前焦点；输入框需要主动监听外部鼠标按下，
    /// 这样用户点击表单空白处时，边框高亮、光标闪烁和平台文本输入状态都会结束。
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
            self.restart_cursor_blink(cx);
        }
        self.update_auto_scroll_direction(event.position, cx);
        self.apply_selection_outcome(outcome, cx);
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
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(on_key_down) = self.on_key_down.as_mut() {
            on_key_down(event.keystroke.clone());
        }
        if event.keystroke.key == "enter" {
            self.record_interaction_event(TextInputInteractionKind::Enter);
            if let Some(on_enter) = self.on_enter.as_mut() {
                on_enter(self.state.content().clone());
            }
            cx.notify();
        }
    }

    /// 响应清除按钮点击。
    fn on_clear_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.can_edit() {
            self.record_interaction_event(TextInputInteractionKind::Clear);
            self.clear(cx);
            window.focus(&self.focus_handle);
        }
    }

    /// 响应密码显示/隐藏按钮点击。
    fn on_password_visibility_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.input_type != TextInputType::Password {
            return;
        }

        self.password_visible = !self.password_visible;
        window.focus(&self.focus_handle);
        self.restart_cursor_blink(cx);
        cx.notify();
    }
}

impl EntityInputHandler for TextInput {
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
        let line = self.last_layout.as_ref()?;
        let range = self.state.range_from_utf16(&range_utf16);
        let display_range = self
            .last_display_text
            .as_ref()
            .map(|display| display.actual_range_to_display(range.clone()))
            .unwrap_or(range);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(display_range.start) - self.last_scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(display_range.end) - self.last_scroll_x,
                bounds.bottom(),
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
        let line = self.last_layout.as_ref()?;
        let _local_point = bounds.localize(&point)?;
        let display_index = line.index_for_x(point.x - bounds.left() + self.last_scroll_x)?;
        let utf8_index = self
            .last_display_text
            .as_ref()
            .map(|display| display.display_to_actual(display_index))
            .unwrap_or(display_index);
        Some(self.state.offset_to_utf16(utf8_index))
    }
}

/// 负责绘制文本、选区、光标并接入平台输入的底层元素。
struct TextElement {
    input: Entity<TextInput>,
}

/// `TextElement` 在 prepaint 阶段计算出的绘制状态。
struct PrepaintState {
    line: Option<ShapedLine>,
    display_text: TextDisplayText,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}

/// 清除按钮内部的叉号图标。
///
/// 这里不用文本字符 `×`，因为不同字体的字形框和基线位置会让视觉中心偏下；
/// 使用路径绘制两条固定斜线，可以让图标在 20px 圆形按钮内保持精确居中。
struct ClearIconElement {
    color: Hsla,
}

impl IntoElement for ClearIconElement {
    type Element = Self;

    /// 将清除图标转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ClearIconElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    /// 清除图标是无状态装饰元素，不需要稳定 id。
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// 清除图标由组件内部生成，不暴露源码位置给 inspector。
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// 请求固定 12px 图标布局。
    ///
    /// 外层清除按钮负责 20px 点击区域和圆形悬浮背景，这里只保留叉号本身的视觉尺寸。
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = px(12.0).into();
        style.size.height = px(12.0).into();
        (window.request_layout(style, [], cx), ())
    }

    /// 清除图标不需要预绘制状态。
    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    /// 绘制两条以布局框中心对称的斜线。
    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let inset = px(2.5);
        let left = bounds.left() + inset;
        let right = bounds.right() - inset;
        let top = bounds.top() + inset;
        let bottom = bounds.bottom() - inset;

        paint_clear_icon_line(window, self.color, point(left, top), point(right, bottom));
        paint_clear_icon_line(window, self.color, point(right, top), point(left, bottom));
    }
}

/// 绘制清除图标的一条斜线。
///
/// 路径构造失败时直接跳过本次线段绘制，避免图标绘制异常影响输入框主体交互。
fn paint_clear_icon_line(
    window: &mut Window,
    color: Hsla,
    start: Point<Pixels>,
    end: Point<Pixels>,
) {
    // 12px 小图标内使用 1.5px 线宽更接近常规图标粗细，避免清除按钮看起来被加粗。
    let mut builder = PathBuilder::stroke(px(1.5));
    builder.move_to(start);
    builder.line_to(end);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

impl IntoElement for TextElement {
    type Element = Self;

    /// 将底层文本元素转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
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

    /// 请求单行文本布局。
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
        style.size.height = resolved.line_height.into();
        (window.request_layout(style, [], cx), ())
    }

    /// 计算文本布局、选区、光标和横向滚动。
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
        let cursor = input.state.cursor_offset();
        let marked_range = input.state.marked_range();
        let display =
            TextDisplayText::new(content.as_str(), input.input_type, input.password_visible);
        let text_style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), resolved.placeholder)
        } else {
            (display.text().clone(), resolved.text)
        };

        let base_run = TextRun {
            len: display_text.len(),
            font: text_style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = if !display_text.is_empty() {
            marked_runs(
                display_text.len(),
                marked_range.map(|range| display.actual_range_to_display(range)),
                base_run,
                resolved.marked_underline,
            )
        } else {
            vec![base_run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_x = line.x_for_index(display.actual_to_display(cursor));
        let scroll_x = next_scroll_x(input.last_scroll_x, cursor_x, bounds.size.width);
        let cursor_screen_x = bounds.left() + cursor_x - scroll_x;
        let selection = if selected_range.is_empty() || input.state.content().is_empty() {
            None
        } else {
            let display_selection = display.actual_range_to_display(selected_range.clone());
            Some(fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(display_selection.start) - scroll_x,
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(display_selection.end) - scroll_x,
                        bounds.bottom(),
                    ),
                ),
                resolved.selection,
            ))
        };
        let cursor = if selected_range.is_empty() && input.cursor_blink_visible {
            Some(fill(
                Bounds::new(
                    point(cursor_screen_x, bounds.top()),
                    size(px(1.5), bounds.bottom() - bounds.top()),
                ),
                resolved.cursor,
            ))
        } else {
            None
        };

        PrepaintState {
            line: Some(line),
            display_text: display,
            cursor,
            selection,
            scroll_x,
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
        let (focus_handle, disabled) = {
            let input = self.input.read(cx);
            (input.focus_handle.clone(), input.disabled)
        };

        if !disabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
        }

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }

        let line = prepaint
            .line
            .take()
            .expect("TextInput prepaint must shape a line");
        line.paint(
            point(bounds.left() - prepaint.scroll_x, bounds.top()),
            window.line_height(),
            window,
            cx,
        )
        .expect("TextInput line painting should succeed");

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_display_text = Some(prepaint.display_text.clone());
            input.last_bounds = Some(bounds);
            input.last_scroll_x = prepaint.scroll_x;
        });
    }
}

impl Render for TextInput {
    /// 渲染输入框。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus_callbacks(window, cx);

        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let show_clear = self.show_clear_button(focused);
        let show_password_visibility = self.show_password_visibility_button();
        let password_visible = self.password_visible;
        let prefix = self.prefix.clone();
        let suffix = self.suffix.clone();
        let helper_text = self.helper_text.clone();
        let required = self.required;
        let clear_button_has_following_content =
            show_password_visibility || required || suffix.is_some();
        let password_button_has_following_content = required || suffix.is_some();

        let field = div()
            .flex()
            .items_center()
            .w_full()
            .h(resolved.height)
            .px(resolved.padding_x)
            .gap(resolved.gap)
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
                        .key_context("TextInput")
                        .on_action(cx.listener(Self::backspace))
                        .on_action(cx.listener(Self::delete))
                        .on_action(cx.listener(Self::left))
                        .on_action(cx.listener(Self::right))
                        .on_action(cx.listener(Self::select_left))
                        .on_action(cx.listener(Self::select_right))
                        .on_action(cx.listener(Self::select_all_action))
                        .on_action(cx.listener(Self::home))
                        .on_action(cx.listener(Self::end))
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
                },
            )
            .when_some(prefix, |this, slot| this.child(slot.render()))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .overflow_hidden()
                    .child(TextElement { input: cx.entity() }),
            )
            .when(show_clear, |this| {
                this.child(
                    clear_button(resolved, clear_button_has_following_content)
                        .on_click(cx.listener(Self::on_clear_click)),
                )
            })
            .when(show_password_visibility, |this| {
                this.child(
                    password_visibility_button(
                        resolved,
                        password_visible,
                        password_button_has_following_content,
                    )
                    .on_click(cx.listener(Self::on_password_visibility_click)),
                )
            })
            .when(required, |this| this.child(required_marker()))
            .when_some(suffix, |this, slot| this.child(slot.render()));

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

impl Focusable for TextInput {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// 构造清除按钮元素。
fn clear_button(
    resolved: ResolvedTextInputStyle,
    has_following_content: bool,
) -> gpui::Stateful<gpui::Div> {
    // 无后缀内容时，清除按钮是输入框最右侧交互元素，需要比普通内部元素更靠近右边界；
    // 有必填标记或后缀插槽时保持默认间距，避免清除按钮压缩后续内容。
    let end_margin = if has_following_content {
        px(0.0)
    } else {
        -px(6.0)
    };

    div()
        .id("xgpui-text-input-clear")
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(20.0))
        .mr(end_margin)
        .rounded(crate::foundation::radius::full())
        .cursor(CursorStyle::PointingHand)
        .child(ClearIconElement {
            color: resolved.clear_button_text,
        })
        .hover(move |style| style.bg(resolved.clear_button_background))
}

/// 构造密码显示/隐藏按钮元素。
fn password_visibility_button(
    resolved: ResolvedTextInputStyle,
    password_visible: bool,
    has_following_content: bool,
) -> gpui::Stateful<gpui::Div> {
    // 当密码按钮是输入框最右侧交互元素时，向右贴近边界；后面还有必填标记或后缀时则保持普通间距。
    let end_margin = if has_following_content {
        px(0.0)
    } else {
        -px(6.0)
    };
    let icon = if password_visible {
        crate::foundation::icon::LucideIcon::EyeOff
    } else {
        crate::foundation::icon::LucideIcon::Eye
    };

    div()
        .id("xgpui-text-input-password-visibility")
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(20.0))
        .mr(end_margin)
        .rounded(crate::foundation::radius::full())
        .cursor(CursorStyle::PointingHand)
        .child(crate::foundation::icon::lucide_icon(
            icon,
            resolved.clear_button_text,
            px(16.0),
        ))
        .hover(move |style| style.bg(resolved.clear_button_background))
}

/// 构造必填标记。
fn required_marker() -> impl IntoElement {
    div()
        .flex_none()
        .text_color(crate::foundation::color::danger_500())
        .child("*")
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

/// 根据光标位置计算下一帧横向滚动量。
fn next_scroll_x(current: Pixels, cursor_x: Pixels, visible_width: Pixels) -> Pixels {
    let padding = px(4.0);
    if visible_width <= padding {
        return px(0.0);
    }

    let max_visible_x = visible_width - padding;
    if cursor_x - current > max_visible_x {
        cursor_x - max_visible_x
    } else if cursor_x < current {
        cursor_x
    } else {
        current
    }
}

/// 返回文本在当前可视宽度下允许的最大横向滚动量。
fn max_text_scroll(text_width: Pixels, visible_width: Pixels) -> Pixels {
    if text_width > visible_width {
        text_width - visible_width + px(4.0)
    } else {
        px(0.0)
    }
}
