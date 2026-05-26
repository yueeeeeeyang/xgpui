//! 日期时间选择器组件。
//!
//! `DateTimePicker` 支持日期、时间、日期时间以及对应范围模式。组件内部复用 `TextInput`
//! 承载严格手动输入，并使用锚定弹层提供日历和时间选择面板。所有 `set_*` 方法都是受控同步，
//! 不触发外部交互回调，避免父组件写回状态时形成循环。

use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use gpui::prelude::*;
use gpui::{
    actions, anchored, deferred, div, point, px, AnchoredPositionMode, App, Bounds, Context,
    CursorStyle, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding,
    KeyDownEvent, MouseDownEvent, ParentElement, Pixels, Render, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
};
use std::{rc::Rc, time::Duration};

use crate::components::text_input::{
    TextInput, TextInputInteractionKind, TextInputProps, TextInputSize, TextInputSlot,
    TextInputStatus, TextInputVariant,
};
use crate::foundation::icon::{self, LucideIcon};
use crate::foundation::theme::{theme_mode, ThemeMode};

mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

pub use props::{
    DateTimePickerMode, DateTimePickerProps, DateTimePickerSize, DateTimePickerStatus,
    DateTimePickerValue, DateTimePickerVariant, DateTimeRange,
};
use state::{
    calendar_days, date_allowed, default_time, now_time, stepped_values, today,
    DateTimePickerConstraints, DateTimePickerDraft, DateTimePickerFormats,
    DateTimePickerRangeEndpoint, DateTimePickerState, DateTimePickerStateOutcome,
};
use style::{resolve_date_time_picker_style, ResolvedDateTimePickerStyle};

/// 时间滚轮中心行上下额外展示的行数。
///
/// 每列只渲染 `2 * radius + 1` 个固定槽位，滚轮事件只改变当前值，不进入通用列表滚动管线。
const TIME_WHEEL_RADIUS: i32 = 3;

/// 时间滚轮固定展示的行数。
const TIME_COLUMN_VISIBLE_ROWS: f32 = (TIME_WHEEL_RADIUS * 2 + 1) as f32;

/// 日期网格横向单元间距。
///
/// 选中态和范围态会给日期格绘制块状背景；保留少量横向间距能避免相邻 hover/selected 背景
/// 贴在一起，同时不明显拉大日期选择器宽度。
const DATE_CELL_COLUMN_GAP: f32 = 2.0;

/// 日期时间联选弹层当前展示的面板。
///
/// 这是组件内部布局状态，不进入公开 API，也不触发任何外部回调。它只决定弹层里展示日历还是
/// 时间滚轮，用于避免 DateTime/DateTimeRange 同时横向渲染日期和时间面板导致弹层过宽。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DateTimePickerPopupPanel {
    /// 日期日历面板。
    Date,
    /// 时间滚轮面板。
    Time,
}

actions!(
    xgpui_date_time_picker,
    [
        Commit, Close, PrevMonth, NextMonth, PrevYear, NextYear, PrevDay, NextDay, PrevWeek,
        NextWeek, FirstDay, LastDay,
    ]
);

/// 注册 `DateTimePicker` 默认键盘快捷键。
pub fn register_date_time_picker_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Commit, Some("DateTimePicker")),
        KeyBinding::new("escape", Close, Some("DateTimePicker")),
        KeyBinding::new("pageup", PrevMonth, Some("DateTimePicker")),
        KeyBinding::new("pagedown", NextMonth, Some("DateTimePicker")),
        KeyBinding::new("shift-pageup", PrevYear, Some("DateTimePicker")),
        KeyBinding::new("shift-pagedown", NextYear, Some("DateTimePicker")),
        KeyBinding::new("left", PrevDay, Some("DateTimePicker")),
        KeyBinding::new("right", NextDay, Some("DateTimePicker")),
        KeyBinding::new("up", PrevWeek, Some("DateTimePicker")),
        KeyBinding::new("down", NextWeek, Some("DateTimePicker")),
        KeyBinding::new("home", FirstDay, Some("DateTimePicker")),
        KeyBinding::new("end", LastDay, Some("DateTimePicker")),
    ]);
}

/// DateTimePicker 主组件。
pub struct DateTimePicker {
    focus_handle: FocusHandle,
    state: DateTimePickerState,
    input: Entity<TextInput>,
    /// 保持内部 TextInput 观察订阅存活。
    ///
    /// TextInput 的内容、Enter 和 Blur 都通过 notify 暴露给组合组件；Subscription drop 后观察会取消。
    _input_subscription: Subscription,
    disabled: bool,
    readonly: bool,
    required: bool,
    clearable: bool,
    min: Option<DateTimePickerValue>,
    max: Option<DateTimePickerValue>,
    disabled_date: Option<props::DateDisabledPredicate>,
    disabled_time: Option<props::TimeDisabledPredicate>,
    formats: DateTimePickerFormats,
    minute_step: usize,
    second_step: usize,
    size: DateTimePickerSize,
    status: DateTimePickerStatus,
    helper_text: Option<SharedString>,
    parse_error_text: SharedString,
    max_popup_height: Pixels,
    on_change: Option<props::DateTimePickerChangeHandler>,
    on_open_change: Option<props::DateTimePickerOpenChangeHandler>,
    on_focus: Option<props::DateTimePickerFocusHandler>,
    on_blur: Option<props::DateTimePickerFocusHandler>,
    on_key_down: Option<props::DateTimePickerKeyDownHandler>,
    on_parse_error: Option<props::DateTimePickerParseErrorHandler>,
    /// 最近处理过的 TextInput 内部事件编号。
    seen_input_event_id: u64,
    /// 最近一次渲染得到的触发器边界，用于锚定弹层宽度和位置。
    trigger_bounds: Option<Bounds<Pixels>>,
    /// 外部点击延迟关闭版本号。
    outside_close_epoch: u64,
    /// 最近同步到内部 TextInput 的类型图标模式。
    ///
    /// 前缀插槽由闭包构造，不能直接比较闭包内容；组件记录语义快照，只在模式变化时重建。
    input_slot_mode: Option<DateTimePickerMode>,
    /// 最近同步到内部 TextInput 的清除按钮显隐。
    input_slot_show_clear: Option<bool>,
    /// 最近同步到内部 TextInput 图标插槽的皮肤模式。
    input_slot_theme_mode: Option<ThemeMode>,
    /// 日期时间联选弹层当前展示的面板。
    ///
    /// 单纯 Date/DateRange/Time/TimeRange 模式会被规范化到唯一可用面板；DateTime 和
    /// DateTimeRange 模式通过分段按钮在日期和时间之间切换，避免弹层横向过长。
    active_panel: DateTimePickerPopupPanel,
    /// 起点小时列滚轮累计量。
    ///
    /// 触控板会产生小于一行的连续 delta；累计到一行后才切换值，避免轻微滑动导致跳动。
    /// 每个端点和列都必须独立累计，否则范围模式两端时间会互相影响滚动手感。
    start_hour_wheel_delta: f32,
    /// 起点分钟列滚轮累计量。
    start_minute_wheel_delta: f32,
    /// 起点秒钟列滚轮累计量。
    start_second_wheel_delta: f32,
    /// 终点小时列滚轮累计量。
    end_hour_wheel_delta: f32,
    /// 终点分钟列滚轮累计量。
    end_minute_wheel_delta: f32,
    /// 终点秒钟列滚轮累计量。
    end_second_wheel_delta: f32,
}

impl DateTimePicker {
    /// 创建新的 DateTimePicker。
    pub fn new(cx: &mut Context<Self>, props: DateTimePickerProps) -> Self {
        let formats = DateTimePickerFormats {
            date_format: props.date_format,
            time_format: props.time_format,
            datetime_format: props.datetime_format,
            range_separator: props.range_separator,
        };
        let state = DateTimePickerState::new(props.mode, props.value, &formats);
        let input_value = state.input_text().clone();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .value(input_value)
                    .placeholder(props.placeholder.clone())
                    .disabled(props.disabled)
                    .readonly(props.readonly)
                    .clearable(false)
                    .required(props.required)
                    .size(map_input_size(props.size))
                    .variant(map_input_variant(props.variant))
                    .status(map_input_status(props.status)),
            )
        });
        let input_subscription = cx.observe(&input, |picker, input, cx| {
            picker.on_input_notify(&input, cx);
        });
        let focus_handle = input.read(cx).focus_handle();

        let mut picker = Self {
            focus_handle,
            state,
            input,
            _input_subscription: input_subscription,
            disabled: props.disabled,
            readonly: props.readonly,
            required: props.required,
            clearable: props.clearable,
            min: props.min,
            max: props.max,
            disabled_date: props.disabled_date,
            disabled_time: props.disabled_time,
            formats,
            minute_step: props.minute_step,
            second_step: props.second_step,
            size: props.size,
            status: props.status,
            helper_text: props.helper_text,
            parse_error_text: props.parse_error_text,
            max_popup_height: props.max_popup_height,
            on_change: props.on_change,
            on_open_change: props.on_open_change,
            on_focus: props.on_focus,
            on_blur: props.on_blur,
            on_key_down: props.on_key_down,
            on_parse_error: props.on_parse_error,
            seen_input_event_id: 0,
            trigger_bounds: None,
            outside_close_epoch: 0,
            input_slot_mode: None,
            input_slot_show_clear: None,
            input_slot_theme_mode: None,
            active_panel: initial_popup_panel(props.mode),
            start_hour_wheel_delta: 0.0,
            start_minute_wheel_delta: 0.0,
            start_second_wheel_delta: 0.0,
            end_hour_wheel_delta: 0.0,
            end_minute_wheel_delta: 0.0,
            end_second_wheel_delta: 0.0,
        };
        picker.sync_input_slots(cx);
        picker
    }

    /// 返回当前值。
    pub fn value(&self) -> Option<&DateTimePickerValue> {
        self.state.value()
    }

    /// 返回当前模式。
    pub fn mode(&self) -> DateTimePickerMode {
        self.state.mode()
    }

    /// 返回弹层是否打开。
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }

    /// 返回当前输入文本。
    pub fn input_text(&self) -> &SharedString {
        self.state.input_text()
    }

    /// 返回内部输入框焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 从外部同步值。
    pub fn set_value(&mut self, value: Option<DateTimePickerValue>, cx: &mut Context<Self>) {
        let outcome = self.state.set_value_silent(value, &self.formats);
        self.sync_input_value(cx);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 从外部同步模式。
    pub fn set_mode(&mut self, mode: DateTimePickerMode, cx: &mut Context<Self>) {
        let outcome = self.state.set_mode_silent(mode, &self.formats);
        self.active_panel = initial_popup_panel(mode);
        self.sync_input_value(cx);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 从外部同步禁用状态。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        self.sync_input_slots(cx);
        if disabled {
            let outcome = self.state.close_discard(&self.formats);
            self.apply_outcome(outcome, false, false, cx);
        } else {
            cx.notify();
        }
    }

    /// 从外部同步只读状态。
    pub fn set_readonly(&mut self, readonly: bool, cx: &mut Context<Self>) {
        if self.readonly == readonly {
            return;
        }
        self.readonly = readonly;
        self.input
            .update(cx, |input, cx| input.set_readonly(readonly, cx));
        self.sync_input_slots(cx);
        cx.notify();
    }

    /// 从外部同步语义状态。
    pub fn set_status(&mut self, status: DateTimePickerStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }
        self.status = status;
        self.sync_input_status(cx);
        cx.notify();
    }

    /// 从外部同步辅助文本。
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

    /// 从外部同步最小值。
    pub fn set_min(&mut self, min: Option<DateTimePickerValue>, cx: &mut Context<Self>) {
        self.min = min;
        cx.notify();
    }

    /// 从外部同步最大值。
    pub fn set_max(&mut self, max: Option<DateTimePickerValue>, cx: &mut Context<Self>) {
        self.max = max;
        cx.notify();
    }

    /// 打开弹层。
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.open();
        if outcome.open_changed {
            self.active_panel = initial_popup_panel(self.state.mode());
            self.reset_time_wheel_accumulators();
        }
        self.apply_outcome(outcome, false, true, cx);
    }

    /// 关闭弹层并丢弃未确认草稿。
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.cancel_outside_close();
        let outcome = self.state.close_discard(&self.formats);
        self.sync_input_value(cx);
        self.apply_outcome(outcome, false, true, cx);
    }

    /// 切换弹层。
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.toggle(&self.formats);
        if !self.state.is_open() {
            self.sync_input_value(cx);
        } else if outcome.open_changed {
            self.active_panel = initial_popup_panel(self.state.mode());
            self.reset_time_wheel_accumulators();
        }
        self.apply_outcome(outcome, false, true, cx);
    }

    /// 清空当前值。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.readonly {
            return;
        }
        let outcome = self.state.clear();
        self.sync_input_value(cx);
        self.apply_outcome(outcome, true, false, cx);
    }

    /// 观察内部 TextInput 的内容、Enter 和 Blur。
    fn on_input_notify(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let input_state = input.read(cx);
        let value = input_state.value().clone();
        let event = input_state.interaction_event();

        let outcome = self.state.set_input_text_silent(value);
        if outcome.should_notify() {
            self.sync_input_slots(cx);
            cx.notify();
        }

        if event.id == self.seen_input_event_id {
            return;
        }
        self.seen_input_event_id = event.id;
        match event.kind {
            TextInputInteractionKind::Focus => self.emit_focus(),
            TextInputInteractionKind::Blur => {
                self.commit_input_from_text(cx);
                self.emit_blur();
            }
            TextInputInteractionKind::Enter => {
                self.commit_input_from_text(cx);
            }
            TextInputInteractionKind::Clear => {
                self.clear(cx);
            }
            TextInputInteractionKind::None => {}
        }
    }

    /// 从输入文本提交值。
    fn commit_input_from_text(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.readonly {
            return;
        }
        let result = self.state.commit_input(&self.formats, self.constraints());
        match result {
            Ok(outcome) => {
                self.sync_input_value(cx);
                self.apply_outcome(outcome, true, false, cx);
            }
            Err(_) => {
                self.sync_input_status(cx);
                self.emit_parse_error();
                cx.notify();
            }
        }
    }

    /// 应用状态结果并按需触发回调。
    fn apply_outcome(
        &mut self,
        outcome: DateTimePickerStateOutcome,
        emit_change: bool,
        emit_open: bool,
        cx: &mut Context<Self>,
    ) {
        if emit_change && outcome.value_changed {
            self.emit_change();
        }
        if emit_open && outcome.open_changed {
            self.emit_open_change();
        }
        if outcome.parse_error_changed {
            self.sync_input_status(cx);
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 构造当前提交约束。
    fn constraints(&self) -> DateTimePickerConstraints {
        DateTimePickerConstraints {
            min: self.min.clone(),
            max: self.max.clone(),
            disabled_date: self.disabled_date.clone(),
            disabled_time: self.disabled_time.clone(),
        }
    }

    /// 同步内部 TextInput 值。
    fn sync_input_value(&mut self, cx: &mut Context<Self>) {
        let value = self.state.input_text().clone();
        self.input.update(cx, |input, cx| {
            input.set_value(value, cx);
        });
        self.sync_input_slots(cx);
    }

    /// 同步内部 TextInput 状态色。
    fn sync_input_status(&mut self, cx: &mut Context<Self>) {
        let status = if self.state.has_parse_error() {
            TextInputStatus::Error
        } else {
            map_input_status(self.status)
        };
        self.input.update(cx, |input, cx| {
            input.set_status(status, cx);
        });
    }

    /// 同步内部 TextInput 的前后缀插槽。
    ///
    /// DateTimePicker 的类型图标属于选择器语义，应放在输入内容之前；清除按钮属于值操作，
    /// 应放在输入内容之后。这里通过 TextInput 的插槽能力实现“看起来在输入框内部”的布局，
    /// 同时保留 DateTimePicker 自己的 clear 回调语义。
    fn sync_input_slots(&mut self, cx: &mut Context<Self>) {
        let mode = self.state.mode();
        let theme_mode = theme_mode(cx);
        let show_clear = self.clearable
            && !self.disabled
            && !self.readonly
            && !self.state.input_text().is_empty();
        if self.input_slot_mode == Some(mode)
            && self.input_slot_show_clear == Some(show_clear)
            && self.input_slot_theme_mode == Some(theme_mode)
        {
            return;
        }

        self.input_slot_mode = Some(mode);
        self.input_slot_show_clear = Some(show_clear);
        self.input_slot_theme_mode = Some(theme_mode);
        let resolved = self.resolved_style(false, cx);
        let prefix = Some(type_icon_slot(trigger_icon(mode), resolved));
        let suffix = show_clear.then(|| clear_icon_slot(cx.entity().downgrade(), resolved));
        self.input
            .update(cx, |input, cx| input.set_slots(prefix, suffix, cx));
    }

    /// 重置时间滚轮累计量。
    ///
    /// 打开弹层时当前值天然处在固定槽位中心；清空累计量可以避免上一次半行滚动残留到下一次打开。
    fn reset_time_wheel_accumulators(&mut self) {
        self.start_hour_wheel_delta = 0.0;
        self.start_minute_wheel_delta = 0.0;
        self.start_second_wheel_delta = 0.0;
        self.end_hour_wheel_delta = 0.0;
        self.end_minute_wheel_delta = 0.0;
        self.end_second_wheel_delta = 0.0;
    }

    /// 切换日期时间联选弹层面板。
    ///
    /// 该切换只影响内部布局，不属于用户值变更；因此不会触发 `on_change` 或 `on_open_change`。
    /// 切到时间面板时同步清空滚轮累计量，让当前草稿时间稳定显示在中心槽位。
    fn set_active_panel(&mut self, panel: DateTimePickerPopupPanel, cx: &mut Context<Self>) {
        if !panel_allowed(self.state.mode(), panel) || self.active_panel == panel {
            return;
        }
        self.active_panel = panel;
        if panel == DateTimePickerPopupPanel::Time {
            self.reset_time_wheel_accumulators();
        }
        cx.notify();
    }

    /// 返回指定端点和列对应的滚轮累计量。
    fn time_wheel_delta_mut(
        &mut self,
        endpoint: DateTimePickerRangeEndpoint,
        kind: TimeColumnKind,
    ) -> &mut f32 {
        match (endpoint, kind) {
            (DateTimePickerRangeEndpoint::Start, TimeColumnKind::Hour) => {
                &mut self.start_hour_wheel_delta
            }
            (DateTimePickerRangeEndpoint::Start, TimeColumnKind::Minute) => {
                &mut self.start_minute_wheel_delta
            }
            (DateTimePickerRangeEndpoint::Start, TimeColumnKind::Second) => {
                &mut self.start_second_wheel_delta
            }
            (DateTimePickerRangeEndpoint::End, TimeColumnKind::Hour) => {
                &mut self.end_hour_wheel_delta
            }
            (DateTimePickerRangeEndpoint::End, TimeColumnKind::Minute) => {
                &mut self.end_minute_wheel_delta
            }
            (DateTimePickerRangeEndpoint::End, TimeColumnKind::Second) => {
                &mut self.end_second_wheel_delta
            }
        }
    }

    /// 触发值变化回调。
    fn emit_change(&mut self) {
        if let Some(on_change) = self.on_change.as_mut() {
            on_change(self.state.value_cloned());
        }
    }

    /// 触发打开状态回调。
    fn emit_open_change(&mut self) {
        if let Some(on_open_change) = self.on_open_change.as_mut() {
            on_open_change(self.state.is_open());
        }
    }

    /// 触发聚焦回调。
    fn emit_focus(&mut self) {
        if let Some(on_focus) = self.on_focus.as_mut() {
            on_focus();
        }
    }

    /// 触发失焦回调。
    fn emit_blur(&mut self) {
        if let Some(on_blur) = self.on_blur.as_mut() {
            on_blur();
        }
    }

    /// 触发解析错误回调。
    fn emit_parse_error(&mut self) {
        if let Some(on_parse_error) = self.on_parse_error.as_mut() {
            on_parse_error(self.state.input_text().clone());
        }
    }

    /// 取消排队的外部关闭任务。
    fn cancel_outside_close(&mut self) {
        self.outside_close_epoch = self.outside_close_epoch.wrapping_add(1);
    }

    /// 延迟关闭弹层。
    fn schedule_outside_close(&mut self, cx: &mut Context<Self>) {
        if !self.state.is_open() {
            return;
        }
        self.outside_close_epoch = self.outside_close_epoch.wrapping_add(1);
        let epoch = self.outside_close_epoch;
        cx.spawn(async move |this: WeakEntity<DateTimePicker>, cx| {
            gpui::Timer::after(Duration::from_millis(60)).await;
            let _ = this.update(cx, |picker, cx| {
                if picker.outside_close_epoch == epoch {
                    picker.close(cx);
                }
            });
        })
        .detach();
    }

    /// 同步触发器边界。
    fn sync_trigger_bounds(&mut self, bounds: &[Bounds<Pixels>], cx: &mut Context<Self>) {
        let Some(bounds) = bounds.first() else {
            return;
        };
        if self.trigger_bounds.as_ref() == Some(bounds) {
            return;
        }
        self.trigger_bounds = Some(*bounds);
        cx.notify();
    }

    /// 触发器点击打开弹层。
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
        self.open(cx);
    }

    /// 弹层外部点击关闭并丢弃草稿。
    fn on_popup_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.schedule_outside_close(cx);
    }

    /// 清空按钮点击。
    fn on_clear_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_outside_close();
        cx.stop_propagation();
        self.clear(cx);
        window.focus(&self.focus_handle);
    }

    /// 日期单元点击。
    fn on_date_click(
        &mut self,
        date: NaiveDate,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.readonly || !date_allowed(date, self.constraints()) {
            return;
        }
        self.cancel_outside_close();
        self.select_date_interaction(date, cx);
    }

    /// 执行一次用户语义的日期选择。
    ///
    /// 鼠标点击和键盘 Enter 都走同一条路径，确保 Date 模式立即提交并关闭，
    /// DateTime/Range 模式只更新草稿并等待确认。
    fn select_date_interaction(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        match self
            .state
            .select_date(date, &self.formats, self.constraints())
        {
            Ok(outcome) => {
                let date_mode = self.state.mode() == DateTimePickerMode::Date;
                if date_mode {
                    let close_outcome = self.state.close_discard(&self.formats);
                    self.sync_input_value(cx);
                    self.apply_outcome(outcome.merge(close_outcome), true, true, cx);
                } else {
                    // 日期时间联选模式由用户通过顶部“日期 / 时间”分段显式切换。选择日期只更新
                    // 草稿，避免用户还想调整日期范围时被自动带到时间面板。
                    self.apply_outcome(outcome, false, false, cx);
                }
            }
            Err(_) => {
                self.emit_parse_error();
                cx.notify();
            }
        }
    }

    /// 时间项点击。
    fn on_time_click(
        &mut self,
        time: NaiveTime,
        endpoint: DateTimePickerRangeEndpoint,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.readonly {
            return;
        }
        self.cancel_outside_close();
        let outcome = self.state.select_time(time, endpoint);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 日期/时间分段按钮点击。
    fn on_panel_switch_click(
        &mut self,
        panel: DateTimePickerPopupPanel,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_outside_close();
        self.set_active_panel(panel, cx);
    }

    /// 时间滚轮滚动。
    ///
    /// 时间列采用固定槽位 wheel，不让 GPUI 通用滚动容器参与。滚轮 delta 先按行高换算并累计，
    /// 每跨过一行才切换一次候选值；这样触控板的小幅连续滚动不会造成过快跳动，也不会在滚动期间
    /// 重建 60 个候选项或触发列表测量。
    fn on_time_wheel(
        &mut self,
        kind: TimeColumnKind,
        endpoint: DateTimePickerRangeEndpoint,
        values: Rc<Vec<u32>>,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.readonly || values.is_empty() {
            return;
        }

        cx.stop_propagation();
        let item_height = self.resolved_style(false, cx).time_item_height;
        let item_height_px = f32::from(item_height);
        if item_height_px <= f32::EPSILON {
            return;
        }

        let delta = event.delta.pixel_delta(item_height);
        // GPUI 的滚动语义中，向下滚动通常表现为负 y delta；转换成正数表示向后一个候选值移动。
        let row_delta = -f32::from(delta.y) / item_height_px;
        if row_delta.abs() < f32::EPSILON {
            return;
        }

        let steps = {
            let accumulator = self.time_wheel_delta_mut(endpoint, kind);
            *accumulator += row_delta;
            let steps = accumulator.trunc() as i32;
            if steps != 0 {
                *accumulator -= steps as f32;
            }
            steps
        };

        if steps == 0 {
            return;
        }

        let Some(next_time) = self.time_for_wheel_delta(kind, endpoint, values.as_slice(), steps)
        else {
            return;
        };
        let outcome = self.state.select_time(next_time, endpoint);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 根据滚轮步数计算下一次草稿时间。
    fn time_for_wheel_delta(
        &self,
        kind: TimeColumnKind,
        endpoint: DateTimePickerRangeEndpoint,
        values: &[u32],
        steps: i32,
    ) -> Option<NaiveTime> {
        let selected_time =
            current_time_for_endpoint(self.state.draft(), self.state.value(), endpoint)
                .unwrap_or_else(default_time);
        let current = current_time_part(selected_time, kind);
        let current_index = time_value_scroll_index(values, current)?;
        let next_index = time_wheel_next_enabled_index(
            values,
            current_index,
            steps,
            selected_time,
            kind,
            self.disabled_time.as_ref(),
        )?;
        values
            .get(next_index)
            .map(|value| build_time_from_part(selected_time, kind, *value))
    }

    /// 执行用户语义的草稿确认。
    ///
    /// 点击“确认”和键盘 Enter 都复用该路径。失败时状态层已经记录 parse_error，
    /// 这里必须同步内部 TextInput 的语义状态，否则 helper text 会显示错误但输入框边框仍停留在旧状态。
    fn confirm_draft_interaction(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.readonly {
            return;
        }
        match self.state.confirm_draft(&self.formats, self.constraints()) {
            Ok(outcome) => {
                let close_outcome = self.state.close_discard(&self.formats);
                self.sync_input_value(cx);
                self.apply_outcome(outcome.merge(close_outcome), true, true, cx);
            }
            Err(_) => {
                self.sync_input_status(cx);
                self.emit_parse_error();
                cx.notify();
            }
        }
    }

    /// 确认按钮点击。
    fn on_confirm_click(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.confirm_draft_interaction(cx);
    }

    /// 今天快捷操作。
    fn on_today_click(
        &mut self,
        event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_date_click(today(), event, window, cx);
    }

    /// 现在快捷操作。
    fn on_now_click(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.readonly {
            return;
        }
        let time = now_time();
        let outcome = match self.state.mode() {
            DateTimePickerMode::Time => self
                .state
                .select_time(time, DateTimePickerRangeEndpoint::Start),
            DateTimePickerMode::DateTime => self
                .state
                .select_time(time, DateTimePickerRangeEndpoint::Start),
            _ => DateTimePickerStateOutcome::default(),
        };
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 上一月。
    fn on_prev_month_click(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = self.state.move_month(-1);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 下一月。
    fn on_next_month_click(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = self.state.move_month(1);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 键盘回调转发。
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, _: &mut Context<Self>) {
        if let Some(on_key_down) = self.on_key_down.as_mut() {
            on_key_down(event.keystroke.clone());
        }
    }

    /// 键盘确认。
    fn commit_action(&mut self, _: &Commit, _: &mut Window, cx: &mut Context<Self>) {
        if self.state.is_open() {
            if mode_has_date(self.state.mode())
                && self.active_panel == DateTimePickerPopupPanel::Date
            {
                let date = self.state.active_date();
                if !self.disabled && !self.readonly && date_allowed(date, self.constraints()) {
                    self.select_date_interaction(date, cx);
                }
            } else {
                self.confirm_draft_interaction(cx);
            }
        } else {
            self.open(cx);
        }
    }

    /// 键盘关闭。
    fn close_action(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
    }

    /// 键盘移动月份。
    fn move_month_action(&mut self, delta: i32, cx: &mut Context<Self>) {
        if !self.state.is_open() || self.active_panel != DateTimePickerPopupPanel::Date {
            return;
        }
        let outcome = self.state.move_month(delta);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 键盘移动活动日期。
    fn move_active_date_action(&mut self, delta_days: i64, cx: &mut Context<Self>) {
        if !self.state.is_open()
            || !mode_has_date(self.state.mode())
            || self.active_panel != DateTimePickerPopupPanel::Date
        {
            return;
        }
        let outcome = self.state.move_active_date(delta_days);
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 键盘移动到当前月份边界。
    fn move_active_month_boundary(&mut self, to_end: bool, cx: &mut Context<Self>) {
        if !self.state.is_open()
            || !mode_has_date(self.state.mode())
            || self.active_panel != DateTimePickerPopupPanel::Date
        {
            return;
        }
        let outcome = if to_end {
            self.state.move_active_to_month_end()
        } else {
            self.state.move_active_to_month_start()
        };
        self.apply_outcome(outcome, false, false, cx);
    }

    /// 上一月键盘动作。
    fn prev_month(&mut self, _: &PrevMonth, _: &mut Window, cx: &mut Context<Self>) {
        self.move_month_action(-1, cx);
    }

    /// 下一月键盘动作。
    fn next_month(&mut self, _: &NextMonth, _: &mut Window, cx: &mut Context<Self>) {
        self.move_month_action(1, cx);
    }

    /// 上一年键盘动作。
    fn prev_year(&mut self, _: &PrevYear, _: &mut Window, cx: &mut Context<Self>) {
        self.move_month_action(-12, cx);
    }

    /// 下一年键盘动作。
    fn next_year(&mut self, _: &NextYear, _: &mut Window, cx: &mut Context<Self>) {
        self.move_month_action(12, cx);
    }

    /// 上一天键盘动作。
    fn prev_day(&mut self, _: &PrevDay, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_date_action(-1, cx);
    }

    /// 下一天键盘动作。
    fn next_day(&mut self, _: &NextDay, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_date_action(1, cx);
    }

    /// 上一周键盘动作。
    fn prev_week(&mut self, _: &PrevWeek, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_date_action(-7, cx);
    }

    /// 下一周键盘动作。
    fn next_week(&mut self, _: &NextWeek, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_date_action(7, cx);
    }

    /// 当前月份首日键盘动作。
    fn first_day(&mut self, _: &FirstDay, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_month_boundary(false, cx);
    }

    /// 当前月份末日键盘动作。
    fn last_day(&mut self, _: &LastDay, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_month_boundary(true, cx);
    }

    /// 解析当前样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedDateTimePickerStyle {
        resolve_date_time_picker_style(
            self.size,
            self.status,
            focused,
            self.state.is_open(),
            self.disabled,
            self.state.has_parse_error(),
            cx,
        )
    }
}

impl Focusable for DateTimePicker {
    /// 返回内部输入框焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DateTimePicker {
    /// 渲染 DateTimePicker。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let helper_text = if self.state.has_parse_error() {
            Some(self.parse_error_text.clone())
        } else {
            self.helper_text.clone()
        };
        let picker_entity = cx.entity().downgrade();
        let popup_width = popup_content_width(self.state.mode(), resolved);
        // 主题切换会改变前后缀图标颜色；插槽由内部 TextInput 持有，因此渲染前按语义快照
        // 同步一次，只有模式、清除显隐或皮肤变化时才会真正更新子实体。
        self.sync_input_slots(cx);
        let input = self.input.clone();

        let control = div()
            .relative()
            .w_full()
            .on_children_prepainted(move |bounds, _window, cx| {
                let _ = picker_entity.update(cx, |picker, cx| {
                    picker.sync_trigger_bounds(&bounds, cx);
                });
            })
            .child(
                div()
                    .id("xgpui-date-time-picker-trigger")
                    .flex()
                    .items_center()
                    .w_full()
                    .gap(resolved.gap)
                    .text_size(resolved.font_size)
                    .line_height(resolved.line_height)
                    .opacity(resolved.opacity)
                    .key_context("DateTimePicker")
                    .on_action(cx.listener(Self::commit_action))
                    .on_action(cx.listener(Self::close_action))
                    .on_action(cx.listener(Self::prev_month))
                    .on_action(cx.listener(Self::next_month))
                    .on_action(cx.listener(Self::prev_year))
                    .on_action(cx.listener(Self::next_year))
                    .on_action(cx.listener(Self::prev_day))
                    .on_action(cx.listener(Self::next_day))
                    .on_action(cx.listener(Self::prev_week))
                    .on_action(cx.listener(Self::next_week))
                    .on_action(cx.listener(Self::first_day))
                    .on_action(cx.listener(Self::last_day))
                    .on_key_down(cx.listener(Self::on_key_down))
                    .on_click(cx.listener(Self::on_trigger_click))
                    .child(div().flex_1().min_w(px(0.0)).child(input)),
            )
            .when(self.state.is_open(), |this| {
                let mut popup_anchor = anchored().snap_to_window_with_margin(px(8.0));
                popup_anchor = if let Some(bounds) = self.trigger_bounds {
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
                    deferred(popup_anchor.child(self.render_popup(resolved, popup_width, cx)))
                        .priority(10_000),
                )
            });

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(control)
            .when(self.required, |this| {
                this.child(
                    div()
                        .text_color(crate::foundation::color::danger_500())
                        .child("*"),
                )
            })
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

impl DateTimePicker {
    /// 渲染弹层。
    fn render_popup(
        &self,
        resolved: ResolvedDateTimePickerStyle,
        popup_width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active_panel = normalized_popup_panel(self.state.mode(), self.active_panel);
        div()
            .id("xgpui-date-time-picker-popup")
            .flex()
            .flex_col()
            .w(popup_width)
            .max_h(self.max_popup_height)
            .rounded(resolved.popup_radius)
            .border_1()
            .border_color(resolved.popup_border)
            .bg(resolved.popup_background)
            .shadow_md()
            .occlude()
            .overflow_hidden()
            .on_mouse_down_out(cx.listener(Self::on_popup_mouse_down_out))
            .when(mode_uses_panel_switch(self.state.mode()), |this| {
                this.child(
                    div()
                        .flex_none()
                        .p(resolved.popup_padding)
                        .child(self.render_panel_switcher(active_panel, resolved, cx)),
                )
            })
            .child(
                div()
                    .id("xgpui-date-time-picker-popup-body")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .p(resolved.popup_padding)
                    .when(popup_body_can_scroll(self.state.mode()), |this| {
                        // 弹层根节点有最大高度限制；内容区允许纵向滚动，才能保证底部
                        // footer 始终完整显示，不会因为双月日历过高而被 overflow 裁掉。
                        this.overflow_y_scroll()
                    })
                    .when(active_panel == DateTimePickerPopupPanel::Date, |this| {
                        this.child(self.render_calendar_panel(resolved, cx))
                    })
                    .when(active_panel == DateTimePickerPopupPanel::Time, |this| {
                        this.child(self.render_time_panel(resolved, cx))
                    }),
            )
            .child(self.render_popup_footer(resolved, cx))
    }

    /// 渲染日期/时间分段切换。
    ///
    /// DateTime 和 DateTimeRange 只在同一时间展示一个面板；分段切换放在弹层顶部，既保持
    /// 选择路径清晰，也让弹层宽度由日期面板和时间面板的较大者决定，而不是二者横向相加。
    fn render_panel_switcher(
        &self,
        active_panel: DateTimePickerPopupPanel,
        resolved: ResolvedDateTimePickerStyle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .w_full()
            .gap(px(4.0))
            .p(px(4.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(resolved.popup_border)
            .bg(resolved.cell_active)
            .child(
                panel_switch_button(
                    "xgpui-date-time-picker-panel-date",
                    LucideIcon::Calendar,
                    "日期",
                    DateTimePickerPopupPanel::Date,
                    active_panel,
                    resolved,
                )
                .on_click(cx.listener(move |picker, event, window, cx| {
                    picker.on_panel_switch_click(DateTimePickerPopupPanel::Date, event, window, cx)
                })),
            )
            .child(
                panel_switch_button(
                    "xgpui-date-time-picker-panel-time",
                    LucideIcon::Clock,
                    "时间",
                    DateTimePickerPopupPanel::Time,
                    active_panel,
                    resolved,
                )
                .on_click(cx.listener(move |picker, event, window, cx| {
                    picker.on_panel_switch_click(DateTimePickerPopupPanel::Time, event, window, cx)
                })),
            )
    }

    /// 渲染日期面板。
    fn render_calendar_panel(
        &self,
        resolved: ResolvedDateTimePickerStyle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let two_months = matches!(
            self.state.mode(),
            DateTimePickerMode::DateRange | DateTimePickerMode::DateTimeRange
        );
        let calendar_width = calendar_grid_width(resolved);
        let panel_gap = if two_months {
            range_panel_gap(resolved)
        } else {
            resolved.popup_padding
        };
        let (next_year, next_month_value) =
            next_month(self.state.view_year(), self.state.view_month());
        div()
            .flex()
            .flex_col()
            .gap(resolved.gap)
            .child(
                div()
                    .flex()
                    .gap(panel_gap)
                    .when(!two_months, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .w(calendar_width)
                                .child(
                                    icon_button(
                                        "xgpui-date-time-picker-prev-month",
                                        LucideIcon::ChevronLeft,
                                        resolved,
                                    )
                                    .on_click(cx.listener(Self::on_prev_month_click)),
                                )
                                .child(month_header_title(
                                    self.state.view_year(),
                                    self.state.view_month(),
                                    resolved,
                                ))
                                .child(
                                    icon_button(
                                        "xgpui-date-time-picker-next-month",
                                        LucideIcon::ChevronRight,
                                        resolved,
                                    )
                                    .on_click(cx.listener(Self::on_next_month_click)),
                                ),
                        )
                    })
                    .when(two_months, |this| {
                        // 双月范围面板需要左右各自显示月份标题；左右标题宽度与下方日历网格一致，
                        // 并用等宽占位图标维持标题居中，避免只显示一个月份造成用户误判。
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .w(calendar_width)
                                .child(
                                    icon_button(
                                        "xgpui-date-time-picker-prev-month",
                                        LucideIcon::ChevronLeft,
                                        resolved,
                                    )
                                    .on_click(cx.listener(Self::on_prev_month_click)),
                                )
                                .child(month_header_title(
                                    self.state.view_year(),
                                    self.state.view_month(),
                                    resolved,
                                ))
                                .child(month_header_spacer(resolved)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .w(calendar_width)
                                .child(month_header_spacer(resolved))
                                .child(month_header_title(next_year, next_month_value, resolved))
                                .child(
                                    icon_button(
                                        "xgpui-date-time-picker-next-month",
                                        LucideIcon::ChevronRight,
                                        resolved,
                                    )
                                    .on_click(cx.listener(Self::on_next_month_click)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(panel_gap)
                    .child(month_grid(
                        self,
                        self.state.view_year(),
                        self.state.view_month(),
                        resolved,
                        cx,
                    ))
                    .when(two_months, |this| {
                        this.child(month_grid(self, next_year, next_month_value, resolved, cx))
                    }),
            )
    }

    /// 渲染时间面板。
    fn render_time_panel(
        &self,
        resolved: ResolvedDateTimePickerStyle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let range = matches!(
            self.state.mode(),
            DateTimePickerMode::TimeRange | DateTimePickerMode::DateTimeRange
        );
        let panel_gap = if range {
            range_panel_gap(resolved)
        } else {
            resolved.popup_padding
        };
        let panel = div().flex().gap(panel_gap);
        if range {
            panel
                .child(time_group(
                    self,
                    Some("开始"),
                    DateTimePickerRangeEndpoint::Start,
                    resolved,
                    cx,
                ))
                .child(time_group(
                    self,
                    Some("结束"),
                    DateTimePickerRangeEndpoint::End,
                    resolved,
                    cx,
                ))
        } else {
            panel.child(time_group(
                self,
                None,
                DateTimePickerRangeEndpoint::Start,
                resolved,
                cx,
            ))
        }
    }

    /// 渲染弹层底部快捷操作。
    fn render_popup_footer(
        &self,
        resolved: ResolvedDateTimePickerStyle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active_panel = normalized_popup_panel(self.state.mode(), self.active_panel);
        let show_date_shortcuts = mode_has_date(self.state.mode())
            && (!mode_uses_panel_switch(self.state.mode())
                || active_panel == DateTimePickerPopupPanel::Date);
        let show_time_shortcuts = matches!(
            self.state.mode(),
            DateTimePickerMode::Time | DateTimePickerMode::DateTime
        ) && (!mode_uses_panel_switch(self.state.mode())
            || active_panel == DateTimePickerPopupPanel::Time);
        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .flex_none()
            .px(resolved.popup_padding)
            .py(px(8.0))
            .border_t_1()
            .border_color(resolved.popup_border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(resolved.gap)
                    .when(show_date_shortcuts, |this| {
                        this.child(
                            text_button("xgpui-date-time-picker-today", "今天", resolved)
                                .on_click(cx.listener(Self::on_today_click)),
                        )
                    })
                    .when(show_time_shortcuts, |this| {
                        this.child(
                            text_button("xgpui-date-time-picker-now", "现在", resolved)
                                .on_click(cx.listener(Self::on_now_click)),
                        )
                    })
                    .when(self.clearable && !self.readonly && !self.disabled, |this| {
                        this.child(
                            text_button("xgpui-date-time-picker-footer-clear", "清空", resolved)
                                .on_click(cx.listener(Self::on_clear_click)),
                        )
                    }),
            )
            .when(self.state.mode() != DateTimePickerMode::Date, |this| {
                this.child(
                    text_button("xgpui-date-time-picker-confirm", "确认", resolved)
                        .on_click(cx.listener(Self::on_confirm_click)),
                )
            })
    }
}

/// 渲染单个月份日历网格。
fn month_grid(
    picker: &DateTimePicker,
    year: i32,
    month: u32,
    resolved: ResolvedDateTimePickerStyle,
    cx: &mut Context<DateTimePicker>,
) -> impl IntoElement {
    let days = calendar_days(year, month);
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .w(calendar_grid_width(resolved))
        .children(
            ["一", "二", "三", "四", "五", "六", "日"]
                .chunks(7)
                .map(|week| {
                    div()
                        .flex()
                        .gap(px(DATE_CELL_COLUMN_GAP))
                        .children(week.iter().map(|label| {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(resolved.cell_size)
                                .text_color(resolved.muted_text)
                                .child(*label)
                        }))
                }),
        )
        .children(days.chunks(7).map(|week| {
            div().flex().gap(px(DATE_CELL_COLUMN_GAP)).children(
                week.iter()
                    .map(|date| date_cell(picker, *date, month, resolved, cx)),
            )
        }))
}

/// 渲染日期单元。
fn date_cell(
    picker: &DateTimePicker,
    date: NaiveDate,
    current_month: u32,
    resolved: ResolvedDateTimePickerStyle,
    cx: &mut Context<DateTimePicker>,
) -> impl IntoElement {
    let disabled = !date_allowed(date, picker.constraints()) || picker.disabled || picker.readonly;
    let selected = date_selected(picker.state.draft(), picker.state.value(), date);
    let in_range = date_in_range(picker.state.draft(), picker.state.value(), date);
    let active = picker.state.is_open() && picker.state.active_date() == date;
    let today = date == state::today();
    let background = if selected {
        resolved.cell_selected
    } else if active {
        resolved.cell_active
    } else if in_range {
        resolved.range_background
    } else if today {
        resolved.cell_active
    } else {
        resolved.popup_background
    };
    let text_color = if disabled {
        resolved.disabled_text
    } else if selected {
        resolved.cell_selected_text
    } else if date.month() == current_month {
        resolved.text
    } else {
        resolved.muted_text
    };

    div()
        .id(("xgpui-date-time-picker-day", date.num_days_from_ce() as u64))
        .flex()
        .items_center()
        .justify_center()
        .size(resolved.cell_size)
        .rounded(px(4.0))
        .bg(background)
        .text_color(text_color)
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .opacity(if disabled { 0.48 } else { 1.0 })
        .cursor(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .child(SharedString::from(date.day().to_string()))
        .when(!disabled, |this| {
            this.hover(move |style| style.bg(resolved.cell_hover))
                .on_click(cx.listener(move |picker, event, window, cx| {
                    picker.on_date_click(date, event, window, cx)
                }))
        })
}

/// 渲染一组时间列。
fn time_group(
    picker: &DateTimePicker,
    title: Option<&'static str>,
    endpoint: DateTimePickerRangeEndpoint,
    resolved: ResolvedDateTimePickerStyle,
    cx: &mut Context<DateTimePicker>,
) -> impl IntoElement {
    // 单值时间选择器不再渲染“时间”标题，避免弹层顶部出现重复语义；范围模式仍保留
    // “开始/结束”端点标题，帮助用户区分两组时分秒列。
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .when_some(title, |this, title| {
            this.child(
                div()
                    .text_color(resolved.muted_text)
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .child(title),
            )
        })
        .child(div().flex().gap(resolved.time_column_gap).children(
            ["时", "分", "秒"].into_iter().map(|label| {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(resolved.time_column_width)
                    .text_size(px(11.0))
                    .line_height(px(14.0))
                    .text_color(resolved.muted_text)
                    .child(label)
            }),
        ))
        .child(
            div()
                .flex()
                .gap(resolved.time_column_gap)
                .child(time_column(
                    picker,
                    TimeColumnKind::Hour,
                    (0..24).collect(),
                    endpoint,
                    resolved,
                    cx,
                ))
                .child(time_column(
                    picker,
                    TimeColumnKind::Minute,
                    stepped_values(picker.minute_step),
                    endpoint,
                    resolved,
                    cx,
                ))
                .child(time_column(
                    picker,
                    TimeColumnKind::Second,
                    stepped_values(picker.second_step),
                    endpoint,
                    resolved,
                    cx,
                )),
        )
}

/// 时间列类型。
#[derive(Clone, Copy)]
enum TimeColumnKind {
    /// 小时列。
    Hour,
    /// 分钟列。
    Minute,
    /// 秒列。
    Second,
}

/// 渲染小时、分钟或秒列。
fn time_column(
    picker: &DateTimePicker,
    kind: TimeColumnKind,
    values: Vec<u32>,
    endpoint: DateTimePickerRangeEndpoint,
    resolved: ResolvedDateTimePickerStyle,
    cx: &mut Context<DateTimePicker>,
) -> impl IntoElement {
    // 这里不能通过 `cx.entity().read(cx)` 重新读取 DateTimePicker。`time_column` 本身在
    // DateTimePicker::render 中执行，此时实体已经处于更新/渲染借用状态；再次读取会触发 GPUI
    // 的重入保护并在点击时间项后的刷新中崩溃。直接使用 render 调用链传入的 `picker` 快照即可。
    let selected_time =
        current_time_for_endpoint(picker.state.draft(), picker.state.value(), endpoint)
            .unwrap_or_else(default_time);
    let current = current_time_part(selected_time, kind);
    let values = Rc::new(values);
    let disabled_time = picker.disabled_time.clone();
    let interaction_disabled = picker.disabled || picker.readonly;
    let picker_entity = cx.entity();
    let selected_index = time_value_scroll_index(values.as_slice(), current).unwrap_or(0);
    let wheel_values = time_wheel_visible_indices(values.len(), selected_index);
    let wheel_values_for_scroll = values.clone();

    // 固定槽位 wheel 只渲染中心值上下各 3 项，不使用滚动容器和虚拟列表。滚轮事件只改变
    // DateTimePicker 草稿中的时间值，因此滚动期间不会触发 GPUI 通用列表的测量、裁剪和重排。
    div()
        .id(time_column_container_id(kind, endpoint))
        .w(resolved.time_column_width)
        .h(resolved.time_item_height * TIME_COLUMN_VISIBLE_ROWS)
        .rounded(px(6.0))
        .border_1()
        .border_color(resolved.popup_border)
        .bg(resolved.popup_background)
        .overflow_hidden()
        .on_scroll_wheel(cx.listener(move |picker, event, window, cx| {
            picker.on_time_wheel(
                kind,
                endpoint,
                wheel_values_for_scroll.clone(),
                event,
                window,
                cx,
            )
        }))
        .children(wheel_values.into_iter().filter_map(move |(index, offset)| {
            let value = *values.get(index)?;
            let next_time = build_time_from_part(selected_time, kind, value);
            let disabled = disabled_time
                .as_ref()
                .map(|predicate| predicate(next_time))
                .unwrap_or(false)
                || interaction_disabled;

            Some(time_cell_element(
                picker_entity.clone(),
                kind,
                endpoint,
                value,
                next_time,
                offset == 0,
                disabled,
                resolved,
                offset,
            ))
        }))
}

/// 渲染单个时间候选项。
///
/// 该 helper 只接收渲染所需的快照数据，避免虚拟列表闭包读取组件实体；事件通过
/// `window.listener_for` 回到原始实体，点击时才进入可变更新。
#[allow(clippy::too_many_arguments)]
fn time_cell_element(
    picker: Entity<DateTimePicker>,
    kind: TimeColumnKind,
    endpoint: DateTimePickerRangeEndpoint,
    value: u32,
    next_time: NaiveTime,
    selected: bool,
    disabled: bool,
    resolved: ResolvedDateTimePickerStyle,
    offset: i32,
) -> impl IntoElement {
    let distance = offset.unsigned_abs() as f32;
    let muted_opacity = (1.0 - distance * 0.18).max(0.42);

    div()
        .id((time_cell_id(kind, endpoint), value as usize))
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h(resolved.time_item_height)
        .px(px(4.0))
        .opacity(if selected { 1.0 } else { muted_opacity })
        .cursor(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .h(resolved.time_item_height - px(6.0))
                .rounded(px(5.0))
                .bg(if selected {
                    resolved.cell_selected
                } else {
                    resolved.popup_background
                })
                .text_color(if disabled {
                    resolved.disabled_text
                } else if selected {
                    resolved.cell_selected_text
                } else {
                    resolved.text
                })
                .text_size(resolved.font_size)
                .line_height(resolved.line_height)
                .child(SharedString::from(format!("{:02}", value))),
        )
        .when(!disabled, |this| {
            this.hover(move |style| style.bg(resolved.popup_background))
                .on_click({
                    let picker = picker.downgrade();
                    move |event, window, cx| {
                        let _ = picker.update(cx, |picker, cx| {
                            picker.on_time_click(next_time, endpoint, event, window, cx)
                        });
                    }
                })
        })
}

/// 创建图标按钮。
fn icon_button(
    id: &'static str,
    icon_name: LucideIcon,
    resolved: ResolvedDateTimePickerStyle,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(resolved.height * 0.74)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(move |style| style.bg(resolved.clear_button_background))
        .child(icon::lucide_icon(
            icon_name,
            resolved.muted_text,
            resolved.icon_size,
        ))
}

/// 创建文字按钮。
fn text_button(
    id: &'static str,
    text: &'static str,
    resolved: ResolvedDateTimePickerStyle,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .text_color(resolved.text)
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .cursor_pointer()
        .hover(move |style| style.bg(resolved.cell_hover))
        .child(text)
}

/// 渲染月份标题。
fn month_header_title(
    year: i32,
    month: u32,
    resolved: ResolvedDateTimePickerStyle,
) -> impl IntoElement {
    div()
        .text_color(resolved.text)
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .child(SharedString::from(format!("{} 年 {} 月", year, month)))
}

/// 渲染月份头部的等宽占位。
///
/// 双月面板只有最左侧和最右侧需要真实翻页按钮；中间侧用同尺寸占位保持月份标题居中。
fn month_header_spacer(resolved: ResolvedDateTimePickerStyle) -> gpui::Div {
    div().flex_none().size(resolved.height * 0.74)
}

/// 创建日期/时间面板切换按钮。
///
/// 分段按钮使用固定高度和等宽布局，避免点击切换时改变弹层宽度；选中态复用日期单元选中色，
/// 非选中态保持透明并只在 hover 时提示可点击。
fn panel_switch_button(
    id: &'static str,
    icon_name: LucideIcon,
    text: &'static str,
    panel: DateTimePickerPopupPanel,
    active_panel: DateTimePickerPopupPanel,
    resolved: ResolvedDateTimePickerStyle,
) -> gpui::Stateful<gpui::Div> {
    let selected = panel == active_panel;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .flex_1()
        .h(px(28.0))
        .rounded(px(5.0))
        .cursor_pointer()
        .bg(if selected {
            resolved.cell_selected
        } else {
            resolved.cell_active
        })
        .text_color(if selected {
            resolved.cell_selected_text
        } else {
            resolved.text
        })
        .text_size(resolved.font_size)
        .line_height(resolved.line_height)
        .hover(move |style| {
            if selected {
                style
            } else {
                style.bg(resolved.cell_hover)
            }
        })
        .child(icon::lucide_icon(
            icon_name,
            if selected {
                resolved.cell_selected_text
            } else {
                resolved.muted_text
            },
            resolved.icon_size,
        ))
        .child(text)
}

/// 创建输入框前缀类型图标插槽。
fn type_icon_slot(icon_name: LucideIcon, resolved: ResolvedDateTimePickerStyle) -> TextInputSlot {
    TextInputSlot::new(move || {
        icon::lucide_icon(icon_name, resolved.muted_text, resolved.icon_size).into_any_element()
    })
}

/// 创建输入框后缀清除按钮插槽。
fn clear_icon_slot(
    picker: WeakEntity<DateTimePicker>,
    resolved: ResolvedDateTimePickerStyle,
) -> TextInputSlot {
    TextInputSlot::new(move || {
        let picker = picker.clone();
        div()
            .id("xgpui-date-time-picker-input-clear")
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .size(px(20.0))
            .rounded(crate::foundation::radius::full())
            .cursor_pointer()
            .child(icon::lucide_icon(
                LucideIcon::X,
                resolved.muted_text,
                resolved.icon_size,
            ))
            .hover(move |style| style.bg(resolved.clear_button_background))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let mut focus_handle = None;
                let _ = picker.update(cx, |picker, cx| {
                    picker.cancel_outside_close();
                    picker.clear(cx);
                    focus_handle = Some(picker.focus_handle.clone());
                });
                if let Some(focus_handle) = focus_handle {
                    window.focus(&focus_handle);
                }
            })
            .into_any_element()
    })
}

/// 根据模式计算弹层内容宽度。
///
/// 弹层宽度按日历和时间面板自身需要计算，不再跟随输入框宽度；这样单日期选择不会出现
/// 大片空白，范围和日期时间联选仍有足够空间承载多面板内容。
fn popup_content_width(mode: DateTimePickerMode, resolved: ResolvedDateTimePickerStyle) -> Pixels {
    let calendar_width = calendar_grid_width(resolved);
    let time_group_width = time_group_width(resolved);
    let content_width = match mode {
        DateTimePickerMode::Date => calendar_width,
        DateTimePickerMode::DateRange => calendar_width * 2.0 + range_panel_gap(resolved),
        DateTimePickerMode::Time => time_group_width,
        DateTimePickerMode::TimeRange => time_group_width * 2.0 + range_panel_gap(resolved),
        DateTimePickerMode::DateTime => max_pixels(calendar_width, time_group_width),
        DateTimePickerMode::DateTimeRange => max_pixels(
            calendar_width * 2.0 + range_panel_gap(resolved),
            time_group_width * 2.0 + range_panel_gap(resolved),
        ),
    };

    content_width + resolved.popup_padding * 2.0
}

/// 计算单个月份日历网格宽度。
fn calendar_grid_width(resolved: ResolvedDateTimePickerStyle) -> Pixels {
    resolved.cell_size * 7.0 + px(DATE_CELL_COLUMN_GAP * 6.0)
}

/// 范围面板左右两组内容之间的间距。
///
/// 双月日历和起止时间列都需要比普通内部 padding 更明显的分隔，避免两个面板视觉上挤在一起。
fn range_panel_gap(resolved: ResolvedDateTimePickerStyle) -> Pixels {
    resolved.popup_padding * 2.0
}

/// 返回两个像素值里的较大者。
fn max_pixels(a: Pixels, b: Pixels) -> Pixels {
    if f32::from(a) >= f32::from(b) {
        a
    } else {
        b
    }
}

/// 当前模式是否需要日期/时间分段切换。
fn mode_uses_panel_switch(mode: DateTimePickerMode) -> bool {
    mode_has_date(mode) && mode_has_time(mode)
}

/// 弹层内容区是否需要在高度不足时滚动。
fn popup_body_can_scroll(mode: DateTimePickerMode) -> bool {
    matches!(
        mode,
        DateTimePickerMode::DateRange | DateTimePickerMode::DateTimeRange
    )
}

/// 返回打开弹层时默认展示的面板。
fn initial_popup_panel(mode: DateTimePickerMode) -> DateTimePickerPopupPanel {
    if mode_has_date(mode) {
        DateTimePickerPopupPanel::Date
    } else {
        DateTimePickerPopupPanel::Time
    }
}

/// 判断某个面板是否适用于当前模式。
fn panel_allowed(mode: DateTimePickerMode, panel: DateTimePickerPopupPanel) -> bool {
    match panel {
        DateTimePickerPopupPanel::Date => mode_has_date(mode),
        DateTimePickerPopupPanel::Time => mode_has_time(mode),
    }
}

/// 规范化内部活动面板，防止外部 set_mode 后遗留不适用于新模式的面板值。
fn normalized_popup_panel(
    mode: DateTimePickerMode,
    panel: DateTimePickerPopupPanel,
) -> DateTimePickerPopupPanel {
    if panel_allowed(mode, panel) {
        panel
    } else {
        initial_popup_panel(mode)
    }
}

/// 计算单个时间组宽度。
fn time_group_width(resolved: ResolvedDateTimePickerStyle) -> Pixels {
    resolved.time_column_width * 3.0 + resolved.time_column_gap * 2.0
}

/// 返回固定时间滚轮需要展示的候选索引。
///
/// 固定 wheel 不渲染完整列表，而是围绕当前候选显示上下若干项。这里使用环形索引，让
/// `23 -> 00`、`59 -> 00` 自然衔接，避免边界处出现空槽导致视觉跳动。
fn time_wheel_visible_indices(item_count: usize, selected_index: usize) -> Vec<(usize, i32)> {
    if item_count == 0 {
        return Vec::new();
    }

    (-TIME_WHEEL_RADIUS..=TIME_WHEEL_RADIUS)
        .map(|offset| {
            let index = (selected_index as i32 + offset).rem_euclid(item_count as i32) as usize;
            (index, offset)
        })
        .collect()
}

/// 根据滚轮步数找到下一个可用候选索引。
///
/// 禁用时间由业务 predicate 决定。滚轮交互需要跳过禁用候选，避免用户滚到一个看起来不可选、
/// 但草稿值已经变化的位置；如果整列都不可选，则保持当前值。
fn time_wheel_next_enabled_index(
    values: &[u32],
    current_index: usize,
    steps: i32,
    selected_time: NaiveTime,
    kind: TimeColumnKind,
    disabled_time: Option<&props::TimeDisabledPredicate>,
) -> Option<usize> {
    if values.is_empty() {
        return None;
    }
    if steps == 0 {
        return Some(current_index.min(values.len() - 1));
    }

    let direction = steps.signum();
    let mut index = current_index.min(values.len() - 1);
    for _ in 0..steps.unsigned_abs() {
        let mut found = None;
        for _ in 0..values.len() {
            index = (index as i32 + direction).rem_euclid(values.len() as i32) as usize;
            let value = values[index];
            if !time_candidate_disabled(disabled_time, selected_time, kind, value) {
                found = Some(index);
                break;
            }
        }
        index = found?;
    }

    Some(index)
}

/// 判断某个时间候选是否被禁用。
fn time_candidate_disabled(
    disabled_time: Option<&props::TimeDisabledPredicate>,
    selected_time: NaiveTime,
    kind: TimeColumnKind,
    value: u32,
) -> bool {
    let next_time = build_time_from_part(selected_time, kind, value);
    disabled_time
        .map(|predicate| predicate(next_time))
        .unwrap_or(false)
}

/// 返回时间值在候选列表中的定位索引。
fn time_value_scroll_index(values: &[u32], value: u32) -> Option<usize> {
    values
        .iter()
        .position(|candidate| *candidate == value)
        .or_else(|| {
            values
                .iter()
                .enumerate()
                .min_by_key(|(_, candidate)| candidate.abs_diff(value))
                .map(|(index, _)| index)
        })
}

/// 触发器图标。
fn trigger_icon(mode: DateTimePickerMode) -> LucideIcon {
    if mode_has_time(mode) && !mode_has_date(mode) {
        LucideIcon::Clock
    } else {
        LucideIcon::Calendar
    }
}

/// 模式是否包含日期面板。
fn mode_has_date(mode: DateTimePickerMode) -> bool {
    matches!(
        mode,
        DateTimePickerMode::Date
            | DateTimePickerMode::DateTime
            | DateTimePickerMode::DateRange
            | DateTimePickerMode::DateTimeRange
    )
}

/// 模式是否包含时间面板。
fn mode_has_time(mode: DateTimePickerMode) -> bool {
    matches!(
        mode,
        DateTimePickerMode::Time
            | DateTimePickerMode::DateTime
            | DateTimePickerMode::TimeRange
            | DateTimePickerMode::DateTimeRange
    )
}

/// 计算下一个月。
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// 当前日期是否被选中。
fn date_selected(
    draft: &DateTimePickerDraft,
    value: Option<&DateTimePickerValue>,
    date: NaiveDate,
) -> bool {
    match draft {
        DateTimePickerDraft::Value(DateTimePickerValue::Date(value)) => *value == date,
        DateTimePickerDraft::Value(DateTimePickerValue::DateTime(value)) => value.date() == date,
        DateTimePickerDraft::Value(DateTimePickerValue::DateRange(range)) => {
            range.start == date || range.end == date
        }
        DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(range)) => {
            range.start.date() == date || range.end.date() == date
        }
        DateTimePickerDraft::DateRangeStart(start) => *start == date,
        DateTimePickerDraft::DateTimeRangeStart(start) => start.date() == date,
        _ => value
            .map(|value| date_selected(&DateTimePickerDraft::Value(value.clone()), None, date))
            .unwrap_or(false),
    }
}

/// 当前日期是否在范围中间段。
fn date_in_range(
    draft: &DateTimePickerDraft,
    value: Option<&DateTimePickerValue>,
    date: NaiveDate,
) -> bool {
    match draft {
        DateTimePickerDraft::Value(DateTimePickerValue::DateRange(range)) => {
            date > range.start && date < range.end
        }
        DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(range)) => {
            date > range.start.date() && date < range.end.date()
        }
        _ => value
            .map(|value| date_in_range(&DateTimePickerDraft::Value(value.clone()), None, date))
            .unwrap_or(false),
    }
}

/// 返回某个范围端点当前时间。
fn current_time_for_endpoint(
    draft: &DateTimePickerDraft,
    value: Option<&DateTimePickerValue>,
    endpoint: DateTimePickerRangeEndpoint,
) -> Option<NaiveTime> {
    match draft {
        DateTimePickerDraft::Value(DateTimePickerValue::Time(value)) => Some(*value),
        DateTimePickerDraft::Value(DateTimePickerValue::DateTime(value)) => Some(value.time()),
        DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(range)) => match endpoint {
            DateTimePickerRangeEndpoint::Start => Some(range.start),
            DateTimePickerRangeEndpoint::End => Some(range.end),
        },
        DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(range)) => match endpoint {
            DateTimePickerRangeEndpoint::Start => Some(range.start.time()),
            DateTimePickerRangeEndpoint::End => Some(range.end.time()),
        },
        DateTimePickerDraft::TimeRangeStart(start) => Some(*start),
        DateTimePickerDraft::DateTimeRangeStart(start) => Some(start.time()),
        _ => value.and_then(|value| {
            current_time_for_endpoint(&DateTimePickerDraft::Value(value.clone()), None, endpoint)
        }),
    }
}

/// 根据当前列修改时间的一部分。
fn build_time_from_part(current: NaiveTime, kind: TimeColumnKind, value: u32) -> NaiveTime {
    match kind {
        TimeColumnKind::Hour => NaiveTime::from_hms_opt(value, current.minute(), current.second()),
        TimeColumnKind::Minute => NaiveTime::from_hms_opt(current.hour(), value, current.second()),
        TimeColumnKind::Second => NaiveTime::from_hms_opt(current.hour(), current.minute(), value),
    }
    .unwrap_or_else(default_time)
}

/// 读取时间列对应的当前值。
fn current_time_part(time: NaiveTime, kind: TimeColumnKind) -> u32 {
    match kind {
        TimeColumnKind::Hour => time.hour(),
        TimeColumnKind::Minute => time.minute(),
        TimeColumnKind::Second => time.second(),
    }
}

/// 时间列滚动容器 id。
fn time_column_container_id(
    kind: TimeColumnKind,
    endpoint: DateTimePickerRangeEndpoint,
) -> &'static str {
    match (kind, endpoint) {
        (TimeColumnKind::Hour, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-hour-column"
        }
        (TimeColumnKind::Minute, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-minute-column"
        }
        (TimeColumnKind::Second, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-second-column"
        }
        (TimeColumnKind::Hour, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-hour-column"
        }
        (TimeColumnKind::Minute, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-minute-column"
        }
        (TimeColumnKind::Second, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-second-column"
        }
    }
}

/// 时间项 id 前缀。
fn time_cell_id(kind: TimeColumnKind, endpoint: DateTimePickerRangeEndpoint) -> &'static str {
    match (kind, endpoint) {
        (TimeColumnKind::Hour, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-hour"
        }
        (TimeColumnKind::Minute, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-minute"
        }
        (TimeColumnKind::Second, DateTimePickerRangeEndpoint::Start) => {
            "xgpui-date-time-picker-start-second"
        }
        (TimeColumnKind::Hour, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-hour"
        }
        (TimeColumnKind::Minute, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-minute"
        }
        (TimeColumnKind::Second, DateTimePickerRangeEndpoint::End) => {
            "xgpui-date-time-picker-end-second"
        }
    }
}

/// 映射内部 TextInput 尺寸。
fn map_input_size(size: DateTimePickerSize) -> TextInputSize {
    match size {
        DateTimePickerSize::Small => TextInputSize::Small,
        DateTimePickerSize::Medium => TextInputSize::Medium,
        DateTimePickerSize::Large => TextInputSize::Large,
    }
}

/// 映射内部 TextInput 变体。
fn map_input_variant(variant: DateTimePickerVariant) -> TextInputVariant {
    match variant {
        DateTimePickerVariant::Outlined => TextInputVariant::Outlined,
        DateTimePickerVariant::Filled => TextInputVariant::Filled,
        DateTimePickerVariant::Ghost => TextInputVariant::Ghost,
    }
}

/// 映射内部 TextInput 状态。
fn map_input_status(status: DateTimePickerStatus) -> TextInputStatus {
    match status {
        DateTimePickerStatus::Default => TextInputStatus::Default,
        DateTimePickerStatus::Error => TextInputStatus::Error,
        DateTimePickerStatus::Warning => TextInputStatus::Warning,
        DateTimePickerStatus::Success => TextInputStatus::Success,
    }
}
