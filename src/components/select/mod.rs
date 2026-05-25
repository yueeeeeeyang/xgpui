//! 单选下拉框组件。
//!
//! `Select` 提供单选值同步、本地搜索、键盘导航、清除、禁用、状态样式、helper text、
//! 锚定下拉面板、虚拟列表和明暗皮肤。第一版聚焦普通前端 UI 框架中常见的单选下拉能力，不包含多选、
//! 异步加载、分组选项或自定义 option 渲染。

use std::{ops::Range, time::Duration};

use gpui::prelude::*;
use gpui::{
    actions, anchored, deferred, div, fill, point, px, relative, size, uniform_list,
    AnchoredPositionMode, App, AsyncApp, Bounds, ClipboardItem, Context, CursorStyle, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, KeyBinding, KeyDownEvent, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement,
    PathBuilder, Pixels, Point, Render, ScrollStrategy, ShapedLine, SharedString,
    StatefulInteractiveElement, Style, TextRun, Timer, UTF16Selection, UnderlineStyle,
    UniformListScrollHandle, WeakEntity, Window,
};

mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

pub use props::{SelectOption, SelectProps, SelectSize, SelectStatus, SelectVariant};
use state::{SelectState, SelectStateOutcome};
use style::{resolve_select_style, ResolvedSelectStyle};

actions!(
    xgpui_select,
    [
        Commit,
        Close,
        MoveUp,
        MoveDown,
        FirstOption,
        LastOption,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Paste,
        Cut,
        Copy,
        ShowCharacterPalette,
    ]
);

/// 搜索光标静止多久后开始进入闪烁周期。
const SEARCH_CURSOR_BLINK_IDLE_DELAY: Duration = Duration::from_millis(500);

/// 搜索光标进入闪烁周期后的可见状态切换间隔。
const SEARCH_CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// 注册 `Select` 默认键盘快捷键。
///
/// gpui 的键盘动作需要在应用启动时注册。调用方通常不需要直接调用本函数，
/// 使用 `xgpui::install(cx)` 即可同时安装主题状态、TextInput 和 Select 默认快捷键。
pub fn register_select_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Commit, Some("Select")),
        KeyBinding::new("escape", Close, Some("Select")),
        KeyBinding::new("up", MoveUp, Some("Select")),
        KeyBinding::new("down", MoveDown, Some("Select")),
        KeyBinding::new("home", FirstOption, Some("Select")),
        KeyBinding::new("end", LastOption, Some("Select")),
        KeyBinding::new("backspace", Backspace, Some("Select")),
        KeyBinding::new("delete", Delete, Some("Select")),
        KeyBinding::new("left", Left, Some("Select")),
        KeyBinding::new("right", Right, Some("Select")),
        KeyBinding::new("shift-left", SelectLeft, Some("Select")),
        KeyBinding::new("shift-right", SelectRight, Some("Select")),
        KeyBinding::new("cmd-a", SelectAll, Some("Select")),
        KeyBinding::new("cmd-v", Paste, Some("Select")),
        KeyBinding::new("cmd-c", Copy, Some("Select")),
        KeyBinding::new("cmd-x", Cut, Some("Select")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("Select")),
    ]);
}

/// 单选下拉框组件。
///
/// 组件内部维护打开状态、搜索词、当前值和高亮项，同时通过 `set_value`、`set_options`、
/// `set_disabled`、`set_status` 和 `set_helper_text` 支持外部受控同步。受控同步默认不触发
/// 用户交互回调，避免父组件写回状态时形成回调循环。调用方通常使用
/// `cx.new(|cx| Select::new(cx, props))` 创建实体，再把实体作为子元素渲染。
pub struct Select {
    focus_handle: FocusHandle,
    state: SelectState,
    placeholder: SharedString,
    options: Vec<SelectOption>,
    disabled: bool,
    clearable: bool,
    required: bool,
    searchable: bool,
    search_placeholder: SharedString,
    size: SelectSize,
    variant: SelectVariant,
    status: SelectStatus,
    helper_text: Option<SharedString>,
    max_popup_height: Pixels,
    empty_text: SharedString,
    on_change: Option<props::SelectChangeHandler>,
    on_open_change: Option<props::SelectOpenChangeHandler>,
    on_search_change: Option<props::SelectSearchChangeHandler>,
    scroll_handle: UniformListScrollHandle,
    /// 最近一次搜索文本排版结果。
    ///
    /// 鼠标定位、拖拽选区、IME 候选框定位都需要从屏幕坐标换算到文本字节偏移，
    /// 因此渲染后的 `ShapedLine` 会保存在组件状态中供事件处理使用。
    last_search_layout: Option<ShapedLine>,
    /// 最近一次搜索文本元素的屏幕边界。
    ///
    /// 该边界只覆盖可编辑文本区域，不包含下拉图标和清除按钮。
    last_search_bounds: Option<Bounds<Pixels>>,
    /// 搜索文本的当前横向滚动量。
    ///
    /// 搜索词较长时，光标移动或拖拽到边缘会推动该值变化，让光标和选区始终处于可视范围。
    last_search_scroll_x: Pixels,
    /// 当前是否正在拖拽选择搜索文本。
    is_search_selecting: bool,
    /// 拖拽选区时的自动横向滚动方向。
    search_auto_scroll_direction: Option<SearchAutoScrollDirection>,
    /// 拖拽自动滚动任务是否已经启动。
    search_auto_scroll_active: bool,
    /// 搜索输入区域当前是否处于焦点状态。
    search_focused: bool,
    /// 搜索光标是否处于可见帧。
    search_cursor_visible: bool,
    /// 搜索光标闪烁任务的版本号。
    ///
    /// 每次输入、移动光标、聚焦或关闭都会推进版本号，让旧的异步闪烁任务自然退出。
    search_cursor_epoch: u64,
    /// 最近一次渲染得到的触发器边界。
    ///
    /// gpui 的 `anchored()` 会把弹层作为绝对定位元素布局，此时弹层内部的 `w_full`
    /// 不会自动等于触发器宽度，局部锚点也可能受到正常文档流影响。这里保存触发器真实边界，
    /// 让下拉面板显式使用相同宽度，并直接锚定到触发器下边缘。
    trigger_bounds: Option<Bounds<Pixels>>,
    /// 延迟外部关闭任务的版本号。
    ///
    /// 点击下拉面板外部需要关闭面板，但点击 option 或触发器也可能先被面板的
    /// `on_mouse_down_out` 捕获。通过版本号延迟关闭，可以让组件内部点击有机会取消旧任务。
    outside_close_epoch: u64,
}

/// 搜索输入拖拽选区时的自动横向滚动方向。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchAutoScrollDirection {
    /// 向左滚动并扩展搜索选区。
    Left,
    /// 向右滚动并扩展搜索选区。
    Right,
}

impl Select {
    /// 创建新的 `Select`。
    pub fn new(cx: &mut Context<Self>, props: SelectProps) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state: SelectState::new(props.value, &props.options),
            placeholder: props.placeholder,
            options: props.options,
            disabled: props.disabled,
            clearable: props.clearable,
            required: props.required,
            searchable: props.searchable,
            search_placeholder: props.search_placeholder,
            size: props.size,
            variant: props.variant,
            status: props.status,
            helper_text: props.helper_text,
            max_popup_height: props.max_popup_height,
            empty_text: props.empty_text,
            on_change: props.on_change,
            on_open_change: props.on_open_change,
            on_search_change: props.on_search_change,
            scroll_handle: UniformListScrollHandle::new(),
            last_search_layout: None,
            last_search_bounds: None,
            last_search_scroll_x: px(0.0),
            is_search_selecting: false,
            search_auto_scroll_direction: None,
            search_auto_scroll_active: false,
            search_focused: false,
            search_cursor_visible: true,
            search_cursor_epoch: 0,
            trigger_bounds: None,
            outside_close_epoch: 0,
        }
    }

    /// 返回当前选中值。
    pub fn value(&self) -> Option<&SharedString> {
        self.state.value()
    }

    /// 从外部同步选中值。
    ///
    /// 该方法不会触发 `on_change`，避免调用方在受控同步时形成回调循环。
    pub fn set_value(&mut self, value: Option<SharedString>, cx: &mut Context<Self>) {
        self.cancel_outside_close();
        let outcome = self.state.set_value_silent(value, &self.options);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步选项列表。
    ///
    /// 该方法会保留当前选中值，即使新选项列表中暂时找不到该值也不会静默清空。这样父组件可以先同步
    /// 远端选项，再按自身业务规则决定是否调用 `set_value(None, cx)`。选项更新只会重新计算高亮项并刷新
    /// 展示，不会触发 `on_change`、`on_open_change` 或 `on_search_change`。
    pub fn set_options(&mut self, options: impl Into<Vec<SelectOption>>, cx: &mut Context<Self>) {
        let options = options.into();
        if self.options == options {
            return;
        }

        self.cancel_outside_close();
        self.options = options;
        let outcome = self.state.sync_options_silent(&self.options);
        if outcome.should_notify() {
            self.apply_outcome(outcome, false, cx);
        } else {
            // 选中值、高亮和搜索词都可能保持不变，但 label、禁用项或未来打开时的候选列表已经变化；
            // 因此即便状态层没有派生变化，也必须通知 gpui 重绘当前触发器或下拉面板。
            cx.notify();
        }
    }

    /// 从外部同步禁用状态。
    ///
    /// 禁用属于父组件受控输入，不表达用户主动关闭语义；因此禁用时会静默关闭面板并清理搜索交互状态，
    /// 但不会触发 `on_open_change`。重新启用只恢复可交互能力，不会自动打开下拉面板。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }

        self.cancel_outside_close();
        self.disabled = disabled;
        if disabled {
            let _ = self.state.close();
            self.stop_search_interaction();
        }
        cx.notify();
    }

    /// 从外部同步语义状态。
    ///
    /// 状态只影响视觉样式，不改变当前值、搜索词、打开状态或任何交互回调。
    pub fn set_status(&mut self, status: SelectStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }

        self.status = status;
        cx.notify();
    }

    /// 从外部同步辅助文本。
    ///
    /// helper text 是纯展示输入，设置或清空时只刷新渲染，不影响选择值和下拉交互状态。
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

    /// 清空当前选择并触发变化回调。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.clear(&self.options);
        self.apply_outcome(outcome, true, cx);
    }

    /// 打开下拉面板。
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.open(&self.options);
        self.apply_outcome(outcome, false, cx);
    }

    /// 关闭下拉面板。
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.cancel_outside_close();
        let outcome = self.state.close();
        self.apply_outcome(outcome, false, cx);
    }

    /// 切换下拉面板打开状态。
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.toggle(&self.options);
        self.apply_outcome(outcome, false, cx);
    }

    /// 返回焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 当前是否应该展示清除按钮。
    ///
    /// Select 失焦后隐藏清除按钮，只保留当前选中值展示；重新聚焦后再展示可清除操作。
    fn show_clear_button(&self, focused: bool) -> bool {
        focused && self.clearable && !self.disabled && self.state.value().is_some()
    }

    /// 当前渲染样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedSelectStyle {
        resolve_select_style(
            self.size,
            self.variant,
            self.status,
            focused,
            self.state.is_open(),
            self.disabled,
            cx,
        )
    }

    /// 应用状态变更结果。
    ///
    /// `emit_value_change` 用于区分用户交互和外部同步：只有用户选择或清除才会触发 `on_change`。
    fn apply_outcome(
        &mut self,
        outcome: SelectStateOutcome,
        emit_value_change: bool,
        cx: &mut Context<Self>,
    ) {
        if emit_value_change && outcome.value_changed {
            self.emit_change();
        }
        if outcome.open_changed {
            self.emit_open_change();
        }
        if outcome.search_changed {
            self.emit_search_change();
        }
        if outcome.search_changed || outcome.search_selection_changed {
            self.restart_search_cursor_blink(cx);
        }
        if outcome.open_changed && !self.state.is_open() {
            self.stop_search_interaction();
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 停止搜索输入相关的临时交互状态。
    ///
    /// 关闭下拉或禁用组件后，搜索光标闪烁、拖拽选区和自动滚动都不能继续影响组件状态。
    /// 异步自动滚动任务如果已经排队，下一帧会看到方向为空并自然退出。
    fn stop_search_interaction(&mut self) {
        self.stop_search_cursor_blink();
        self.is_search_selecting = false;
        self.search_auto_scroll_direction = None;
    }

    /// 触发选中值变化回调。
    fn emit_change(&mut self) {
        if let Some(on_change) = self.on_change.as_mut() {
            on_change(self.state.value_cloned());
        }
    }

    /// 触发打开状态变化回调。
    fn emit_open_change(&mut self) {
        if let Some(on_open_change) = self.on_open_change.as_mut() {
            on_open_change(self.state.is_open());
        }
    }

    /// 触发搜索词变化回调。
    fn emit_search_change(&mut self) {
        if let Some(on_search_change) = self.on_search_change.as_mut() {
            on_search_change(self.state.search().clone());
        }
    }

    /// 返回当前展示在触发器中的文本。
    fn selected_label(&self) -> Option<SharedString> {
        self.state.selected_label(&self.options)
    }

    /// 返回当前高亮项在指定过滤结果中的可视序号。
    ///
    /// 过滤结果由调用方在当前渲染帧统一计算后传入，避免滚动定位和虚拟列表渲染分别重复扫描大选项集。
    fn highlighted_visible_index(&self, filtered: &[usize]) -> Option<usize> {
        let highlighted = self.state.highlighted_index()?;
        filtered.iter().position(|index| *index == highlighted)
    }

    /// 同步触发器边界。
    ///
    /// 该方法在父容器的 `on_children_prepainted` 中调用。只有边界真正变化时才刷新，
    /// 避免每帧 prepaint 都触发无意义的重新渲染。
    fn sync_trigger_bounds(&mut self, bounds: &[Bounds<Pixels>], cx: &mut Context<Self>) {
        let Some(trigger_bounds) = bounds.first() else {
            return;
        };
        if self.trigger_bounds.as_ref() == Some(trigger_bounds) {
            return;
        }

        self.trigger_bounds = Some(*trigger_bounds);
        cx.notify();
    }

    /// 判断搜索输入是否处于可编辑状态。
    fn search_can_edit(&self) -> bool {
        self.searchable && self.state.is_open() && !self.disabled
    }

    /// 同步搜索输入焦点状态。
    ///
    /// Select 搜索输入不再嵌入 `TextInput`，因此需要由 Select 自己维护光标闪烁生命周期。
    fn sync_search_focus_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.search_can_edit() && self.focus_handle.is_focused(window);
        if focused == self.search_focused {
            return;
        }

        self.search_focused = focused;
        if focused {
            self.restart_search_cursor_blink(cx);
        } else {
            self.stop_search_cursor_blink();
        }
    }

    /// 重置搜索光标闪烁周期。
    ///
    /// 用户输入、点击或移动光标时，光标应保持常亮；静止超过固定延迟后再进入闪烁周期。
    fn restart_search_cursor_blink(&mut self, cx: &mut Context<Self>) {
        if !self.search_focused || !self.state.search_selected_range().is_empty() {
            self.stop_search_cursor_blink();
            return;
        }

        self.search_cursor_epoch = self.search_cursor_epoch.wrapping_add(1);
        let epoch = self.search_cursor_epoch;
        if !self.search_cursor_visible {
            self.search_cursor_visible = true;
            cx.notify();
        }

        cx.spawn(async move |this: WeakEntity<Select>, cx: &mut AsyncApp| {
            Timer::after(SEARCH_CURSOR_BLINK_IDLE_DELAY).await;
            loop {
                let keep_blinking = this
                    .update(cx, |select, cx| select.tick_search_cursor_blink(epoch, cx))
                    .unwrap_or(false);
                if !keep_blinking {
                    break;
                }
                Timer::after(SEARCH_CURSOR_BLINK_INTERVAL).await;
            }
        })
        .detach();
    }

    /// 停止搜索光标闪烁任务。
    fn stop_search_cursor_blink(&mut self) {
        self.search_cursor_epoch = self.search_cursor_epoch.wrapping_add(1);
        self.search_cursor_visible = true;
    }

    /// 执行一次搜索光标闪烁切换。
    fn tick_search_cursor_blink(&mut self, epoch: u64, cx: &mut Context<Self>) -> bool {
        if epoch != self.search_cursor_epoch
            || !self.search_focused
            || !self.state.search_selected_range().is_empty()
        {
            return false;
        }

        self.search_cursor_visible = !self.search_cursor_visible;
        cx.notify();
        true
    }

    /// 取消已经排队的外部关闭任务。
    fn cancel_outside_close(&mut self) {
        self.outside_close_epoch = self.outside_close_epoch.wrapping_add(1);
    }

    /// 安排一次延迟外部关闭。
    ///
    /// 关闭被延迟到当前鼠标点击事件之后执行。若点击实际发生在组件内部，内部处理器会推进
    /// `outside_close_epoch` 取消本次关闭，避免 option 点击或触发器点击被提前关闭打断。
    fn schedule_outside_close(&mut self, cx: &mut Context<Self>) {
        if !self.state.is_open() {
            return;
        }

        self.outside_close_epoch = self.outside_close_epoch.wrapping_add(1);
        let epoch = self.outside_close_epoch;
        cx.spawn(async move |this: WeakEntity<Select>, cx: &mut AsyncApp| {
            Timer::after(Duration::from_millis(60)).await;
            let _ = this.update(cx, |select, cx| {
                select.close_if_outside_epoch_matches(epoch, cx);
            });
        })
        .detach();
    }

    /// 当延迟任务仍然有效时关闭面板。
    fn close_if_outside_epoch_matches(&mut self, epoch: u64, cx: &mut Context<Self>) {
        if self.outside_close_epoch != epoch || !self.state.is_open() {
            return;
        }
        let outcome = self.state.close();
        self.apply_outcome(outcome, false, cx);
    }

    /// 响应触发器点击。
    fn on_trigger_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        window.focus(&self.focus_handle);
        self.toggle(cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应触发器外部鼠标按下。
    ///
    /// 关闭状态下点击空白区域应让 Select 失去焦点；打开状态由下拉面板的外部点击处理负责关闭和失焦，
    /// 避免点击 option 时触发器先收到外部按下并提前打断选项点击。
    fn on_trigger_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.is_open() || !self.focus_handle.is_focused(window) {
            return;
        }

        window.blur();
        self.search_focused = false;
        self.stop_search_cursor_blink();
        cx.notify();
    }

    /// 响应内部搜索输入框点击。
    ///
    /// 搜索输入框位于触发器内部，点击它只应该聚焦文本编辑区域，不应该继续冒泡到触发器并反向关闭面板。
    fn on_search_input_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_outside_close();
        cx.stop_propagation();
        self.focus_search_input(window, cx);
    }

    /// 响应下拉面板外部鼠标按下。
    fn on_popup_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.focus_handle.is_focused(window) {
            window.blur();
            self.search_focused = false;
            self.stop_search_cursor_blink();
        }
        self.schedule_outside_close(cx);
    }

    /// 响应清除按钮点击。
    fn on_clear_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.clearable || self.disabled || self.state.value().is_none() {
            return;
        }
        self.cancel_outside_close();
        cx.stop_propagation();
        self.clear(cx);
        window.focus(&self.focus_handle);
    }

    /// 响应选项点击。
    fn on_option_click(
        &mut self,
        index: usize,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        cx.stop_propagation();
        let outcome = self.state.select_index(index, &self.options);
        self.apply_outcome(outcome, true, cx);
        window.focus(&self.focus_handle);
    }

    /// 响应选项鼠标移动。
    ///
    /// 鼠标经过可选项时同步键盘高亮位置；禁用选项不参与高亮。
    fn on_option_mouse_move(
        &mut self,
        index: usize,
        _: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = self
            .state
            .highlight_index_if_selectable(index, &self.options);
        self.cancel_outside_close();
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 响应 Enter / Space 键盘动作。
    fn commit(&mut self, _: &Commit, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        window.focus(&self.focus_handle);
        let was_open = self.state.is_open();
        let outcome = if self.state.is_open() {
            self.state.select_highlighted(&self.options)
        } else {
            self.state.open(&self.options)
        };
        self.apply_outcome(outcome, was_open, cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应 Escape 键盘动作。
    fn close_action(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_outside_close();
        self.close(cx);
    }

    /// 响应 ArrowUp 键盘动作。
    fn move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let open_outcome = if self.state.is_open() {
            SelectStateOutcome::default()
        } else {
            self.state.open(&self.options)
        };
        let move_outcome = self.state.move_highlight(-1, &self.options);
        self.apply_outcome(open_outcome.merge(move_outcome), false, cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应 ArrowDown 键盘动作。
    fn move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let open_outcome = if self.state.is_open() {
            SelectStateOutcome::default()
        } else {
            self.state.open(&self.options)
        };
        let move_outcome = self.state.move_highlight(1, &self.options);
        self.apply_outcome(open_outcome.merge(move_outcome), false, cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应 Home 键盘动作。
    fn first_option(&mut self, _: &FirstOption, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        if self.search_can_edit() {
            let outcome = self.state.move_search_cursor(0);
            self.apply_search_outcome(outcome, cx);
            return;
        }
        let open_outcome = if self.state.is_open() {
            SelectStateOutcome::default()
        } else {
            self.state.open(&self.options)
        };
        let move_outcome = self.state.highlight_first(&self.options);
        self.apply_outcome(open_outcome.merge(move_outcome), false, cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应 End 键盘动作。
    fn last_option(&mut self, _: &LastOption, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        if self.search_can_edit() {
            let outcome = self.state.move_search_cursor(self.state.search().len());
            self.apply_search_outcome(outcome, cx);
            return;
        }
        let open_outcome = if self.state.is_open() {
            SelectStateOutcome::default()
        } else {
            self.state.open(&self.options)
        };
        let move_outcome = self.state.highlight_last(&self.options);
        self.apply_outcome(open_outcome.merge(move_outcome), false, cx);
        if self.state.is_open() {
            self.focus_search_input(window, cx);
        }
    }

    /// 响应普通键盘按下。
    ///
    /// Tab 不注册为 action，是为了保留系统焦点切换行为；这里仅在面板打开时关闭它。
    /// Space 不再注册为 `Commit`，避免搜索状态下空格被下拉框动作吞掉；关闭状态下仍可通过这里打开面板。
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();

        if event.keystroke.key == "tab" {
            self.close(cx);
        } else if event.keystroke.key == "space" && !self.state.is_open() {
            window.focus(&self.focus_handle);
            self.open(cx);
        }
    }

    /// 应用搜索输入编辑结果。
    fn apply_search_outcome(&mut self, outcome: SelectStateOutcome, cx: &mut Context<Self>) {
        if outcome.search_changed {
            self.emit_search_change();
        }
        if outcome.search_changed || outcome.search_selection_changed {
            self.restart_search_cursor_blink(cx);
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 聚焦 Select 自己的搜索输入区域。
    fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }

        window.focus(&self.focus_handle);
        self.search_focused = true;
        self.restart_search_cursor_blink(cx);
    }

    /// 根据鼠标位置和指定横向滚动量计算搜索文本字节偏移。
    fn search_index_for_mouse_position_with_scroll(
        &self,
        position: Point<Pixels>,
        scroll_x: Pixels,
    ) -> usize {
        if self.state.search().is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (
            self.last_search_bounds.as_ref(),
            self.last_search_layout.as_ref(),
        ) else {
            return self.state.search().len();
        };

        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.state.search().len();
        }

        line.closest_index_for_x(position.x - bounds.left() + scroll_x)
    }

    /// 根据鼠标位置计算搜索文本字节偏移。
    fn search_index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        self.search_index_for_mouse_position_with_scroll(position, self.last_search_scroll_x)
    }

    /// 根据拖拽鼠标位置更新搜索输入自动滚动方向。
    fn update_search_auto_scroll_direction(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.search_auto_scroll_direction =
            self.search_auto_scroll_direction_for_position(position);
        if self.search_auto_scroll_direction.is_some() {
            self.ensure_search_auto_scroll_task(cx);
        }
    }

    /// 判断当前位置是否需要触发搜索输入拖拽自动滚动。
    fn search_auto_scroll_direction_for_position(
        &self,
        position: Point<Pixels>,
    ) -> Option<SearchAutoScrollDirection> {
        let (Some(bounds), Some(line)) = (
            self.last_search_bounds.as_ref(),
            self.last_search_layout.as_ref(),
        ) else {
            return None;
        };
        let max_scroll = max_search_text_scroll(line.width, bounds.size.width);
        let edge = px(8.0);

        if position.x <= bounds.left() + edge && self.last_search_scroll_x > px(0.0) {
            Some(SearchAutoScrollDirection::Left)
        } else if position.x >= bounds.right() - edge && self.last_search_scroll_x < max_scroll {
            Some(SearchAutoScrollDirection::Right)
        } else {
            None
        }
    }

    /// 确保搜索输入拖拽自动滚动任务已经启动。
    fn ensure_search_auto_scroll_task(&mut self, cx: &mut Context<Self>) {
        if self.search_auto_scroll_active {
            return;
        }

        self.search_auto_scroll_active = true;
        cx.spawn(
            async move |this: WeakEntity<Select>, cx: &mut AsyncApp| loop {
                Timer::after(Duration::from_millis(16)).await;
                let keep_scrolling = this
                    .update(cx, |select, cx| select.tick_search_auto_scroll(cx))
                    .unwrap_or(false);
                if !keep_scrolling {
                    break;
                }
            },
        )
        .detach();
    }

    /// 执行一次搜索输入拖拽自动滚动。
    fn tick_search_auto_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(direction) = self.search_auto_scroll_direction else {
            self.search_auto_scroll_active = false;
            return false;
        };
        if !self.is_search_selecting || !self.search_can_edit() {
            self.search_auto_scroll_direction = None;
            self.search_auto_scroll_active = false;
            return false;
        }

        let Some(outcome) = self.scroll_search_selection_once(direction) else {
            self.search_auto_scroll_direction = None;
            self.search_auto_scroll_active = false;
            return false;
        };
        self.apply_search_outcome(outcome, cx);
        true
    }

    /// 按指定方向滚动一小步并扩展搜索选区。
    fn scroll_search_selection_once(
        &mut self,
        direction: SearchAutoScrollDirection,
    ) -> Option<SelectStateOutcome> {
        let (bounds, line) = (
            self.last_search_bounds.as_ref()?,
            self.last_search_layout.as_ref()?,
        );
        let max_scroll = max_search_text_scroll(line.width, bounds.size.width);
        let step = px(14.0);
        let current_scroll = self.last_search_scroll_x;

        let next_scroll = match direction {
            SearchAutoScrollDirection::Left => {
                if current_scroll <= px(0.0) {
                    return None;
                }
                if current_scroll > step {
                    current_scroll - step
                } else {
                    px(0.0)
                }
            }
            SearchAutoScrollDirection::Right => {
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

        self.last_search_scroll_x = next_scroll;
        let edge_x = match direction {
            SearchAutoScrollDirection::Left => bounds.left(),
            SearchAutoScrollDirection::Right => bounds.right(),
        };
        let target = line.closest_index_for_x(edge_x - bounds.left() + next_scroll);
        Some(self.state.select_search_to(target))
    }

    /// 搜索光标左移。
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        let target = if self.state.search_selected_range().is_empty() {
            self.state
                .previous_search_boundary(self.state.search_cursor_offset())
        } else {
            self.state.search_selected_range().start
        };
        let outcome = self.state.move_search_cursor(target);
        self.apply_search_outcome(outcome, cx);
    }

    /// 搜索光标右移。
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        let target = if self.state.search_selected_range().is_empty() {
            self.state
                .next_search_boundary(self.state.search_selected_range().end)
        } else {
            self.state.search_selected_range().end
        };
        let outcome = self.state.move_search_cursor(target);
        self.apply_search_outcome(outcome, cx);
    }

    /// 向左扩展搜索选区。
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        let target = self
            .state
            .previous_search_boundary(self.state.search_cursor_offset());
        let outcome = self.state.select_search_to(target);
        self.apply_search_outcome(outcome, cx);
    }

    /// 向右扩展搜索选区。
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        let target = self
            .state
            .next_search_boundary(self.state.search_cursor_offset());
        let outcome = self.state.select_search_to(target);
        self.apply_search_outcome(outcome, cx);
    }

    /// 选中全部搜索词。
    fn select_all_search_action(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        let outcome = self.state.select_all_search();
        self.apply_search_outcome(outcome, cx);
    }

    /// 删除搜索光标前的文本或当前搜索选区。
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        if self.state.search_selected_range().is_empty() {
            let target = self
                .state
                .previous_search_boundary(self.state.search_cursor_offset());
            self.state.select_search_to(target);
        }
        let outcome = self
            .state
            .replace_search_text_in_range(None, "", &self.options);
        self.apply_search_outcome(outcome, cx);
    }

    /// 删除搜索光标后的文本或当前搜索选区。
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        if self.state.search_selected_range().is_empty() {
            let target = self
                .state
                .next_search_boundary(self.state.search_cursor_offset());
            self.state.select_search_to(target);
        }
        let outcome = self
            .state
            .replace_search_text_in_range(None, "", &self.options);
        self.apply_search_outcome(outcome, cx);
    }

    /// 粘贴剪贴板文本到搜索输入。
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let outcome = self
                .state
                .replace_search_text_in_range(None, &text, &self.options);
            self.apply_search_outcome(outcome, cx);
        }
    }

    /// 复制当前搜索选区文本。
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled
            || !self.searchable
            || !self.state.is_open()
            || self.state.search_selected_range().is_empty()
        {
            return;
        }
        let range = self.state.search_selected_range();
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.state.search().as_str()[range].to_string(),
        ));
    }

    /// 剪切当前搜索选区文本。
    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_can_edit() || self.state.search_selected_range().is_empty() {
            return;
        }
        let range = self.state.search_selected_range();
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.state.search().as_str()[range].to_string(),
        ));
        let outcome = self
            .state
            .replace_search_text_in_range(None, "", &self.options);
        self.apply_search_outcome(outcome, cx);
    }

    /// 打开系统字符面板。
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        if self.search_can_edit() {
            window.show_character_palette();
        }
    }

    /// 搜索输入鼠标按下时聚焦并移动或扩展选区。
    fn on_search_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search_can_edit() {
            return;
        }
        self.cancel_outside_close();
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        self.search_focused = true;
        self.is_search_selecting = true;

        let target = self.search_index_for_mouse_position(event.position);
        let outcome = if event.modifiers.shift {
            self.state.select_search_to(target)
        } else {
            self.state.move_search_cursor(target)
        };
        self.update_search_auto_scroll_direction(event.position, cx);
        self.apply_search_outcome(outcome, cx);
    }

    /// 搜索输入鼠标松开时停止拖拽选择。
    fn on_search_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_search_selecting = false;
        self.search_auto_scroll_direction = None;
    }

    /// 搜索输入鼠标拖动时扩展选区。
    fn on_search_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search_can_edit() || !self.is_search_selecting {
            return;
        }

        let target = self.search_index_for_mouse_position(event.position);
        let outcome = self.state.select_search_to(target);
        self.update_search_auto_scroll_direction(event.position, cx);
        self.apply_search_outcome(outcome, cx);
    }

    /// 渲染嵌入触发器内的搜索输入框。
    ///
    /// 外层触发器继续负责 Select 的边框、状态色和下拉图标；该区域只负责绘制和接收搜索文本输入。
    fn render_search_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("xgpui-select-search-input-host")
            .flex()
            .flex_1()
            .items_center()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .cursor(CursorStyle::IBeam)
            .on_click(cx.listener(Self::on_search_input_click))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_search_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_search_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_search_mouse_up))
            .on_mouse_move(cx.listener(Self::on_search_mouse_move))
            .child(SelectSearchElement {
                select: cx.entity(),
            })
    }

    /// 渲染触发器。
    fn render_trigger(
        &self,
        resolved: ResolvedSelectStyle,
        show_clear: bool,
        selected_label: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (display_text, strong_text) = self.trigger_text(selected_label);
        let open = self.state.is_open();
        let render_search_input = self.searchable && open && !self.disabled;
        let show_clear = show_clear && !render_search_input;

        div()
            .id("xgpui-select-trigger")
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
            .text_size(resolved.font_size)
            .line_height(resolved.line_height)
            .opacity(resolved.opacity)
            .overflow_hidden()
            .when_else(
                self.disabled,
                |this| this.cursor(CursorStyle::Arrow),
                |this| {
                    this.track_focus(&self.focus_handle)
                        .cursor(CursorStyle::PointingHand)
                        .key_context("Select")
                        .on_action(cx.listener(Self::commit))
                        .on_action(cx.listener(Self::close_action))
                        .on_action(cx.listener(Self::move_up))
                        .on_action(cx.listener(Self::move_down))
                        .on_action(cx.listener(Self::first_option))
                        .on_action(cx.listener(Self::last_option))
                        .on_action(cx.listener(Self::backspace))
                        .on_action(cx.listener(Self::delete))
                        .on_action(cx.listener(Self::left))
                        .on_action(cx.listener(Self::right))
                        .on_action(cx.listener(Self::select_left))
                        .on_action(cx.listener(Self::select_right))
                        .on_action(cx.listener(Self::select_all_search_action))
                        .on_action(cx.listener(Self::paste))
                        .on_action(cx.listener(Self::cut))
                        .on_action(cx.listener(Self::copy))
                        .on_action(cx.listener(Self::show_character_palette))
                        .on_key_down(cx.listener(Self::on_key_down))
                        .on_mouse_down_out(cx.listener(Self::on_trigger_mouse_down_out))
                        .on_click(cx.listener(Self::on_trigger_click))
                },
            )
            .when_else(
                render_search_input,
                |this| this.child(self.render_search_input(cx)),
                |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .text_color(if strong_text {
                                resolved.text
                            } else {
                                resolved.placeholder
                            })
                            .child(display_text),
                    )
                },
            )
            .when(show_clear, |this| {
                this.child(clear_button(resolved).on_click(cx.listener(Self::on_clear_click)))
            })
            .when(self.required, |this| this.child(required_marker()))
            .child(chevron_icon(resolved, open))
    }

    /// 返回触发器当前展示文本和是否使用正文颜色。
    ///
    /// 搜索模式不再在弹层内部渲染独立搜索框，而是直接复用原选择框显示搜索词。
    /// 打开且已有搜索词时优先展示搜索词；未输入搜索词时保留当前选中项展示；
    /// 没有选中项时，打开状态展示 `search_placeholder`，关闭状态展示普通 `placeholder`。
    fn trigger_text(&self, selected_label: Option<SharedString>) -> (SharedString, bool) {
        if self.searchable && self.state.is_open() && !self.state.search().is_empty() {
            return (self.state.search().clone(), true);
        }

        if let Some(label) = selected_label {
            return (label, true);
        }

        if self.searchable && self.state.is_open() {
            return (self.search_placeholder.clone(), false);
        }

        (self.placeholder.clone(), false)
    }

    /// 渲染下拉面板。
    fn render_popup(
        &self,
        resolved: ResolvedSelectStyle,
        filtered: Vec<usize>,
        popup_width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let list_height = popup_list_height(filtered.len(), resolved, self.max_popup_height);
        let list = if filtered.is_empty() {
            div()
                .id("xgpui-select-empty-list")
                .flex()
                .flex_col()
                .w_full()
                .h(list_height)
                .child(empty_option(resolved, self.empty_text.clone()))
                .into_any_element()
        } else {
            // 下拉选项高度由 `ResolvedSelectStyle::option_height` 固定，符合 gpui uniform_list 的约束。
            // 使用虚拟列表后，大选项集只会构建当前可见范围内的元素，避免搜索光标闪烁、hover
            // 或高亮变化时反复创建所有匹配项。
            let select = cx.entity();
            let item_count = filtered.len();
            let scroll_handle = self.scroll_handle.clone();
            uniform_list(
                "xgpui-select-list",
                item_count,
                move |visible_range, window, cx| {
                    let select_state = select.read(cx);
                    let selected_value = select_state.state.value_cloned();
                    let highlighted = select_state.state.highlighted_index();

                    visible_range
                        .filter_map(|visible_index| {
                            let option_index = *filtered.get(visible_index)?;
                            let option = select_state.options.get(option_index)?.clone();
                            let selected = selected_value
                                .as_ref()
                                .map(|value| value == &option.value)
                                .unwrap_or(false);
                            let option_highlighted = highlighted == Some(option_index);

                            Some(select_option_element(
                                select.clone(),
                                option_index,
                                option,
                                selected,
                                option_highlighted,
                                resolved,
                                window,
                            ))
                        })
                        .collect()
                },
            )
            .w_full()
            .h(list_height)
            .track_scroll(scroll_handle)
            .into_any_element()
        };

        div()
            .id("xgpui-select-popup")
            .flex()
            .flex_col()
            .w(popup_width)
            .rounded(resolved.popup_radius)
            .border_1()
            .border_color(resolved.popup_border)
            .bg(resolved.popup_background)
            .overflow_hidden()
            .shadow_md()
            .occlude()
            .on_mouse_down_out(cx.listener(Self::on_popup_mouse_down_out))
            .child(list)
    }
}

/// 构造虚拟列表中的单个选项元素。
///
/// 该函数只接收渲染当前可视项所需的快照数据，避免虚拟列表闭包在持有 `Select` 只读借用时再进入
/// 事件监听注册。事件仍通过 `window.listener_for` 回到原始实体，因此不会改变外部回调语义。
fn select_option_element(
    select: Entity<Select>,
    index: usize,
    option: SelectOption,
    selected: bool,
    highlighted: bool,
    resolved: ResolvedSelectStyle,
    window: &mut Window,
) -> impl IntoElement {
    let disabled = option.disabled;
    let background = if highlighted {
        resolved.option_highlighted
    } else if selected {
        resolved.option_selected
    } else {
        resolved.popup_background
    };
    let text_color = if disabled {
        resolved.option_disabled_text
    } else if selected {
        resolved.option_selected_text
    } else {
        resolved.text
    };

    div()
        .id(("xgpui-select-option", index))
        .flex()
        .items_center()
        .w_full()
        .h(resolved.option_height)
        .px(resolved.padding_x)
        .gap(resolved.gap)
        .bg(background)
        .text_color(text_color)
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .opacity(if disabled { 0.58 } else { 1.0 })
        .when_else(
            disabled,
            |this| this.cursor(CursorStyle::Arrow),
            |this| {
                this.cursor(CursorStyle::PointingHand)
                    .hover(move |style| style.bg(resolved.option_hover))
                    .on_mouse_move(
                        window.listener_for(&select, move |this, event, window, cx| {
                            this.on_option_mouse_move(index, event, window, cx)
                        }),
                    )
                    .on_click(
                        window.listener_for(&select, move |this, event, window, cx| {
                            this.on_option_click(index, event, window, cx)
                        }),
                    )
            },
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(option.label),
        )
        .when(selected, |this| this.child(check_icon(resolved)))
}

/// 根据选项数量计算虚拟列表视口高度。
///
/// `uniform_list` 默认依赖父级给出明确高度；只设置 `max_h` 时，在弹层首次布局中可能无法推导出
/// 可视区域高度，导致下拉框只剩很薄的一条。这里使用选项固定高度计算真实视口高度，既保留小列表
/// 按内容收缩的行为，也确保大列表达到最大高度后由虚拟列表内部滚动。
fn popup_list_height(
    item_count: usize,
    resolved: ResolvedSelectStyle,
    max_height: Pixels,
) -> Pixels {
    let visible_count = item_count.max(1);
    let desired_height = resolved.option_height * visible_count;
    if desired_height > max_height {
        max_height
    } else {
        desired_height
    }
}

impl Render for Select {
    /// 渲染 Select。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_search_focus_state(window, cx);

        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let show_clear = self.show_clear_button(focused);
        let selected_label = self.selected_label();
        let helper_text = self.helper_text.clone();
        // 过滤结果只在下拉打开时用于面板渲染和滚动定位；关闭状态跳过过滤，避免大型选项集在普通
        // 父视图刷新、主题切换或焦点变化时仍然做无意义的全量扫描。
        let filtered = if self.state.is_open() {
            self.state.filtered_indices(&self.options)
        } else {
            Vec::new()
        };
        let trigger_bounds = self.trigger_bounds;
        let popup_width = trigger_bounds
            .as_ref()
            .map(|bounds| bounds.size.width)
            .unwrap_or(px(0.0));
        let select_entity = cx.entity().downgrade();

        if self.state.is_open() {
            if let Some(visible_index) = self.highlighted_visible_index(&filtered) {
                self.scroll_handle
                    .scroll_to_item(visible_index, ScrollStrategy::Center);
            }
        }

        let control = div()
            .relative()
            .w_full()
            .on_children_prepainted(move |bounds, _window, cx| {
                let _ = select_entity.update(cx, |select, cx| {
                    select.sync_trigger_bounds(&bounds, cx);
                });
            })
            .child(self.render_trigger(resolved, show_clear, selected_label, cx))
            .when(self.state.is_open(), |this| {
                let mut popup_anchor = anchored().snap_to_window_with_margin(px(8.0));
                popup_anchor = if let Some(bounds) = trigger_bounds {
                    popup_anchor
                        .position_mode(AnchoredPositionMode::Window)
                        .position(point(
                            bounds.left(),
                            bounds.bottom() + resolved.popup_offset,
                        ))
                } else {
                    popup_anchor
                        .position_mode(AnchoredPositionMode::Local)
                        .position(point(px(0.0), resolved.height + resolved.popup_offset))
                };

                this.child(
                    deferred(popup_anchor.child(self.render_popup(
                        resolved,
                        filtered,
                        popup_width,
                        cx,
                    )))
                    .priority(10_000),
                )
            });

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(control)
            .when_some(helper_text, |this, helper_text| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(resolved.helper)
                        .child(helper_text),
                )
            })
    }
}

impl Focusable for Select {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for Select {
    /// 返回指定 UTF-16 区间内的搜索文本。
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        self.state
            .search_text_for_range_utf16(range_utf16, actual_range)
    }

    /// 返回当前搜索输入框的 UTF-16 选区。
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if (!self.searchable || !self.state.is_open()) || (self.disabled && !ignore_disabled_input)
        {
            return None;
        }
        Some(self.state.search_selected_text_range_utf16())
    }

    /// 返回当前搜索输入框 IME marked text 区间。
    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state
            .search_marked_range()
            .as_ref()
            .map(|range| self.state.search_range_to_utf16(range))
    }

    /// 取消搜索输入框 IME marked text。
    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let outcome = self.state.unmark_search_text();
        self.apply_search_outcome(outcome, cx);
    }

    /// 处理平台普通文本替换。
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search_can_edit() {
            return;
        }
        let outcome = self
            .state
            .replace_search_text_in_range(range_utf16, new_text, &self.options);
        self.apply_search_outcome(outcome, cx);
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
        if !self.search_can_edit() {
            return;
        }
        let outcome = self.state.replace_and_mark_search_text_in_range(
            range_utf16,
            new_text,
            new_selected_range_utf16,
            &self.options,
        );
        self.apply_search_outcome(outcome, cx);
    }

    /// 返回指定搜索文本范围在屏幕中的边界，用于定位输入法候选框。
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_search_layout.as_ref()?;
        let range = self.state.search_range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start) - self.last_search_scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end) - self.last_search_scroll_x,
                bounds.bottom(),
            ),
        ))
    }

    /// 根据屏幕坐标返回搜索文本 UTF-16 字符偏移。
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_search_bounds?;
        let line = self.last_search_layout.as_ref()?;
        let _local_point = bounds.localize(&point)?;
        let utf8_index = line.index_for_x(point.x - bounds.left() + self.last_search_scroll_x)?;
        Some(self.state.search_offset_to_utf16(utf8_index))
    }
}

/// 负责绘制 Select 搜索文本、选区和光标的底层元素。
struct SelectSearchElement {
    select: Entity<Select>,
}

/// `SelectSearchElement` 在 prepaint 阶段计算出的绘制状态。
struct SelectSearchPrepaintState {
    /// 已排版的搜索文本行。
    line: Option<ShapedLine>,
    /// 当前帧需要绘制的光标矩形。
    cursor: Option<PaintQuad>,
    /// 当前帧需要绘制的选区矩形。
    selection: Option<PaintQuad>,
    /// 当前帧计算得到的横向滚动量。
    scroll_x: Pixels,
}

impl IntoElement for SelectSearchElement {
    type Element = Self;

    /// 将搜索文本元素转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectSearchElement {
    type RequestLayoutState = ();
    type PrepaintState = SelectSearchPrepaintState;

    /// 搜索文本元素由 Select 内部生成，不需要稳定 id。
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// 搜索文本元素不暴露源码位置给 inspector。
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// 请求单行搜索文本布局。
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let select = self.select.read(cx);
        let focused = !select.disabled && select.focus_handle.is_focused(window);
        let resolved = select.resolved_style(focused, cx);
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = resolved.line_height.into();
        (window.request_layout(style, [], cx), ())
    }

    /// 计算搜索文本布局、选区、光标和横向滚动。
    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let select = self.select.read(cx);
        let focused = !select.disabled && select.focus_handle.is_focused(window);
        let resolved = select.resolved_style(focused, cx);
        let content = select.state.search().clone();
        let selected_range = select.state.search_selected_range();
        let cursor = select.state.search_cursor_offset();
        let marked_range = select.state.search_marked_range();
        let text_style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (select.search_placeholder.clone(), resolved.placeholder)
        } else {
            (content, resolved.text)
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
            select_marked_runs(display_text.len(), marked_range, base_run, resolved.cursor)
        } else {
            vec![base_run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let cursor_x = line.x_for_index(cursor);
        let scroll_x =
            next_search_scroll_x(select.last_search_scroll_x, cursor_x, bounds.size.width);
        let cursor_screen_x = bounds.left() + cursor_x - scroll_x;
        let selection = if selected_range.is_empty() || select.state.search().is_empty() {
            None
        } else {
            Some(fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start) - scroll_x,
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end) - scroll_x,
                        bounds.bottom(),
                    ),
                ),
                resolved.selection,
            ))
        };
        let cursor = if selected_range.is_empty() && select.search_cursor_visible {
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

        SelectSearchPrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_x,
        }
    }

    /// 绘制搜索文本并把文本元素注册为平台输入处理器。
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
        let (focus_handle, search_enabled) = {
            let select = self.select.read(cx);
            (
                select.focus_handle.clone(),
                select.searchable && select.state.is_open() && !select.disabled,
            )
        };

        if search_enabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.select.clone()),
                cx,
            );
        }

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }

        let line = prepaint
            .line
            .take()
            .expect("Select search prepaint must shape a line");
        line.paint(
            point(bounds.left() - prepaint.scroll_x, bounds.top()),
            window.line_height(),
            window,
            cx,
        )
        .expect("Select search line painting should succeed");

        if search_enabled && focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.select.update(cx, |select, _cx| {
            select.last_search_layout = Some(line);
            select.last_search_bounds = Some(bounds);
            select.last_search_scroll_x = prepaint.scroll_x;
        });
    }
}

/// 构造搜索输入 IME marked text 的文本 run。
fn select_marked_runs(
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

/// 根据搜索光标位置计算下一帧横向滚动量。
fn next_search_scroll_x(current: Pixels, cursor_x: Pixels, visible_width: Pixels) -> Pixels {
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

/// 返回搜索文本在当前可视宽度下允许的最大横向滚动量。
fn max_search_text_scroll(text_width: Pixels, visible_width: Pixels) -> Pixels {
    if text_width > visible_width {
        text_width - visible_width + px(4.0)
    } else {
        px(0.0)
    }
}

/// 构造清除按钮元素。
fn clear_button(resolved: ResolvedSelectStyle) -> gpui::Stateful<gpui::Div> {
    div()
        .id("xgpui-select-clear")
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(20.0))
        .rounded(crate::foundation::radius::full())
        .cursor(CursorStyle::PointingHand)
        .text_color(resolved.clear_button_text)
        .child("×")
        .hover(move |style| style.bg(resolved.clear_button_background))
}

/// 构造下拉箭头图标。
fn chevron_icon(resolved: ResolvedSelectStyle, open: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(20.0))
        .child(ChevronIconElement {
            color: resolved.icon,
            open,
        })
}

/// 构造已选选项对勾图标。
fn check_icon(resolved: ResolvedSelectStyle) -> impl IntoElement {
    div().flex_none().text_color(resolved.icon).child("✓")
}

/// 下拉箭头图标。
///
/// 这里不用文本字符 `⌄` / `⌃`，因为不同字体的字形框和基线位置不一致，会导致图标看起来偏下。
/// 使用路径绘制同一套 chevron，只切换折线方向，保证展开和收起状态的尺寸、线宽和视觉中心一致。
struct ChevronIconElement {
    color: Hsla,
    open: bool,
}

impl IntoElement for ChevronIconElement {
    type Element = Self;

    /// 将箭头图标转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ChevronIconElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    /// 箭头图标是无状态装饰元素，不需要稳定 id。
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// 箭头图标由组件内部生成，不暴露源码位置给 inspector。
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// 请求固定 12px 图标布局。
    ///
    /// 外层容器负责 20px 点击和对齐区域，这里只声明 chevron 本体的视觉尺寸。
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

    /// 箭头图标不需要预绘制状态。
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

    /// 绘制展开或收起方向的 chevron 折线。
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
        let inset_x = px(2.0);
        let upper = bounds.top() + px(4.5);
        let middle = bounds.top() + px(7.5);
        let lower = bounds.top() + px(7.5);
        let center = bounds.left() + bounds.size.width * 0.5;
        let left = bounds.left() + inset_x;
        let right = bounds.right() - inset_x;

        let (start, control, end) = if self.open {
            (
                point(left, lower),
                point(center, upper),
                point(right, lower),
            )
        } else {
            (
                point(left, upper),
                point(center, middle),
                point(right, upper),
            )
        };

        paint_chevron_line(window, self.color, start, control, end);
    }
}

/// 绘制箭头图标的一条折线。
///
/// 路径构造失败时直接跳过本次绘制，避免装饰图标异常影响 Select 主体交互。
fn paint_chevron_line(
    window: &mut Window,
    color: Hsla,
    start: Point<Pixels>,
    control: Point<Pixels>,
    end: Point<Pixels>,
) {
    let mut builder = PathBuilder::stroke(px(1.8));
    builder.move_to(start);
    builder.line_to(control);
    builder.line_to(end);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// 构造必填标记。
fn required_marker() -> impl IntoElement {
    div()
        .flex_none()
        .text_color(crate::foundation::color::danger_500())
        .child("*")
}

/// 构造空结果选项。
fn empty_option(resolved: ResolvedSelectStyle, empty_text: SharedString) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(resolved.option_height)
        .px(resolved.padding_x)
        .text_color(resolved.empty_text)
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .child(empty_text)
}
