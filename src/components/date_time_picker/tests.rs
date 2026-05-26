//! `DateTimePicker` 状态和组件公开同步方法测试。
//!
//! 状态测试覆盖格式化、解析、范围规范化和约束判断；组件测试确认受控 `set_*` 方法不会把
//! 父组件同步误报成用户交互回调。

use std::{cell::Cell, rc::Rc};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gpui::{AppContext, SharedString, TestAppContext};

use crate::components::text_input::TextInputStatus;

use super::{
    props::{DateTimePickerValue, DateTimeRange, TimeDisabledPredicate},
    state::{
        date_allowed, parse_optional_value, stepped_values, DateTimePickerConstraints,
        DateTimePickerFormats, DateTimePickerState,
    },
    time_value_scroll_index, time_wheel_next_enabled_index, time_wheel_visible_indices,
    DateTimePicker, DateTimePickerMode, DateTimePickerPopupPanel, DateTimePickerProps,
    DateTimePickerStatus, TimeColumnKind,
};

/// 构造 SharedString。
fn s(value: &str) -> SharedString {
    SharedString::from(value.to_owned())
}

/// 构造日期。
fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// 构造时间。
fn t(hour: u32, minute: u32, second: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, second).unwrap()
}

/// 构造日期时间。
fn dt(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> NaiveDateTime {
    NaiveDateTime::new(d(year, month, day), t(hour, minute, second))
}

/// 标准测试格式。
fn formats() -> DateTimePickerFormats {
    DateTimePickerFormats {
        date_format: s("%Y-%m-%d"),
        time_format: s("%H:%M:%S"),
        datetime_format: s("%Y-%m-%d %H:%M:%S"),
        range_separator: s(" ~ "),
    }
}

/// 无额外禁用规则的约束。
fn no_constraints() -> DateTimePickerConstraints {
    DateTimePickerConstraints {
        min: None,
        max: None,
        disabled_date: None,
        disabled_time: None,
    }
}

/// 日期、时间、日期时间和范围文本都应按当前模式严格解析。
#[test]
fn parses_single_and_range_values() {
    assert_eq!(
        parse_optional_value("2026-05-26", DateTimePickerMode::Date, &formats()).unwrap(),
        Some(DateTimePickerValue::Date(d(2026, 5, 26)))
    );
    assert_eq!(
        parse_optional_value("09:30:05", DateTimePickerMode::Time, &formats()).unwrap(),
        Some(DateTimePickerValue::Time(t(9, 30, 5)))
    );
    assert_eq!(
        parse_optional_value(
            "2026-05-26 09:30:05",
            DateTimePickerMode::DateTime,
            &formats(),
        )
        .unwrap(),
        Some(DateTimePickerValue::DateTime(dt(2026, 5, 26, 9, 30, 5)))
    );
    assert_eq!(
        parse_optional_value(
            "2026-05-28 ~ 2026-05-26",
            DateTimePickerMode::DateRange,
            &formats(),
        )
        .unwrap(),
        Some(DateTimePickerValue::DateRange(DateTimeRange::new(
            d(2026, 5, 26),
            d(2026, 5, 28),
        )))
    );
}

/// 闰年日期可以解析，非闰年非法日期会失败。
#[test]
fn leap_day_parsing_respects_calendar_rules() {
    assert!(parse_optional_value("2024-02-29", DateTimePickerMode::Date, &formats()).is_ok());
    assert!(parse_optional_value("2025-02-29", DateTimePickerMode::Date, &formats()).is_err());
}

/// min/max 和 disabled_date 应共同拦截不可提交日期。
#[test]
fn constraints_block_out_of_range_or_disabled_dates() {
    let constraints = DateTimePickerConstraints {
        min: Some(DateTimePickerValue::Date(d(2026, 5, 10))),
        max: Some(DateTimePickerValue::Date(d(2026, 5, 20))),
        disabled_date: Some(Rc::new(|date| date == d(2026, 5, 16))),
        disabled_time: None,
    };

    assert!(!date_allowed(d(2026, 5, 9), constraints.clone()));
    assert!(date_allowed(d(2026, 5, 12), constraints.clone()));
    assert!(!date_allowed(d(2026, 5, 16), constraints));
}

/// 分钟和秒步长为 0 或超过 60 时应回退到 1，避免空时间列。
#[test]
fn time_step_values_are_normalized() {
    assert_eq!(stepped_values(15), vec![0, 15, 30, 45]);
    assert_eq!(stepped_values(0).len(), 60);
    assert_eq!(stepped_values(99).len(), 60);
}

/// 时间列打开时需要定位到当前值；当当前值不在步长候选中时，应选择最近候选项。
#[test]
fn time_scroll_index_uses_nearest_step_value() {
    let values = stepped_values(5);
    assert_eq!(time_value_scroll_index(&values, 30), Some(6));
    assert_eq!(time_value_scroll_index(&values, 32), Some(6));
    assert_eq!(time_value_scroll_index(&values, 33), Some(7));
    assert_eq!(time_value_scroll_index(&[], 10), None);
}

/// 固定时间滚轮只渲染当前值附近的槽位，并在边界处环绕展示，避免滚动到 00/59 时出现空白槽。
#[test]
fn time_wheel_visible_indices_wrap_around_edges() {
    assert_eq!(
        time_wheel_visible_indices(24, 0),
        vec![(21, -3), (22, -2), (23, -1), (0, 0), (1, 1), (2, 2), (3, 3)]
    );
    assert_eq!(
        time_wheel_visible_indices(24, 23),
        vec![
            (20, -3),
            (21, -2),
            (22, -1),
            (23, 0),
            (0, 1),
            (1, 2),
            (2, 3)
        ]
    );
    assert!(time_wheel_visible_indices(0, 0).is_empty());
}

/// 滚轮切换时间时应跳过业务禁用的候选值，避免草稿停在 UI 上不可点击的时间项。
#[test]
fn time_wheel_next_enabled_index_skips_disabled_candidates() {
    let values = vec![0, 1, 2, 3, 4];
    let disabled: TimeDisabledPredicate = Rc::new(|time| matches!(time.minute(), 1 | 2 | 4));

    assert_eq!(
        time_wheel_next_enabled_index(
            &values,
            0,
            1,
            t(9, 0, 0),
            TimeColumnKind::Minute,
            Some(&disabled),
        ),
        Some(3)
    );
    assert_eq!(
        time_wheel_next_enabled_index(
            &values,
            3,
            1,
            t(9, 3, 0),
            TimeColumnKind::Minute,
            Some(&disabled),
        ),
        Some(0)
    );
}

/// mode 和 value 不匹配时应静默规范化为空值，不 panic。
#[test]
fn mismatched_mode_value_is_normalized_to_none() {
    let state = DateTimePickerState::new(
        DateTimePickerMode::Time,
        Some(DateTimePickerValue::Date(d(2026, 5, 26))),
        &formats(),
    );

    assert!(state.value().is_none());
    assert_eq!(state.input_text(), &SharedString::default());
}

/// 方向键语义移动键盘活动日期，并在跨月时同步面板月份。
#[test]
fn active_date_navigation_tracks_visible_month() {
    let mut state = DateTimePickerState::new(
        DateTimePickerMode::Date,
        Some(DateTimePickerValue::Date(d(2026, 5, 31))),
        &formats(),
    );

    let outcome = state.move_active_date(1);
    assert!(outcome.active_date_changed);
    assert!(outcome.view_changed);
    assert_eq!(state.active_date(), d(2026, 6, 1));
    assert_eq!(state.view_year(), 2026);
    assert_eq!(state.view_month(), 6);

    let outcome = state.move_active_to_month_end();
    assert!(outcome.active_date_changed);
    assert_eq!(state.active_date(), d(2026, 6, 30));
}

/// 双月日期范围面板中，点击右侧月份的日期不应自动推进面板月份。
#[test]
fn range_date_selection_keeps_visible_right_month_in_place() {
    let mut state = DateTimePickerState::new(
        DateTimePickerMode::DateRange,
        Some(DateTimePickerValue::DateRange(DateTimeRange::new(
            d(2026, 5, 10),
            d(2026, 6, 20),
        ))),
        &formats(),
    );

    let outcome = state
        .select_date(d(2026, 6, 15), &formats(), no_constraints())
        .unwrap();

    assert!(!outcome.view_changed);
    assert_eq!(state.view_year(), 2026);
    assert_eq!(state.view_month(), 5);
}

/// DateTimeRange 选择完整日期范围后仍停留在日期面板，时间选择由用户通过分段按钮显式进入。
#[gpui::test]
fn datetime_range_keeps_date_panel_after_complete_date_range(cx: &mut TestAppContext) {
    let picker = cx.new(|cx| {
        DateTimePicker::new(
            cx,
            DateTimePickerProps::default().mode(DateTimePickerMode::DateTimeRange),
        )
    });

    picker.update(cx, |picker, cx| {
        picker.open(cx);
        assert_eq!(picker.active_panel, DateTimePickerPopupPanel::Date);

        picker.select_date_interaction(d(2026, 5, 26), cx);
        assert_eq!(picker.active_panel, DateTimePickerPopupPanel::Date);

        picker.select_date_interaction(d(2026, 5, 28), cx);
        assert_eq!(picker.active_panel, DateTimePickerPopupPanel::Date);
    });
}

/// 受控同步方法不触发 on_change 或 on_open_change。
#[gpui::test]
fn controlled_setters_do_not_emit_callbacks(cx: &mut TestAppContext) {
    let changes = Rc::new(Cell::new(0));
    let opens = Rc::new(Cell::new(0));
    let changes_for_callback = changes.clone();
    let opens_for_callback = opens.clone();

    let picker = cx.new(|cx| {
        DateTimePicker::new(
            cx,
            DateTimePickerProps::default()
                .on_change(move |_| changes_for_callback.set(changes_for_callback.get() + 1))
                .on_open_change(move |_| opens_for_callback.set(opens_for_callback.get() + 1)),
        )
    });

    picker.update(cx, |picker, cx| {
        picker.set_value(Some(DateTimePickerValue::Date(d(2026, 5, 26))), cx);
        picker.set_mode(DateTimePickerMode::DateTime, cx);
        picker.set_disabled(true, cx);
        picker.set_readonly(true, cx);
        picker.set_status(DateTimePickerStatus::Warning, cx);
        picker.set_helper_text(Some(s("请选择日期时间")), cx);
        picker.set_min(
            Some(DateTimePickerValue::DateTime(dt(2026, 1, 1, 0, 0, 0))),
            cx,
        );
        picker.set_max(
            Some(DateTimePickerValue::DateTime(dt(2026, 12, 31, 23, 59, 59))),
            cx,
        );
    });

    assert_eq!(changes.get(), 0);
    assert_eq!(opens.get(), 0);
}

/// 无效手动输入提交后不改变 value，并记录解析错误状态。
#[gpui::test]
fn invalid_manual_input_keeps_value_and_marks_parse_error(cx: &mut TestAppContext) {
    let parse_errors = Rc::new(Cell::new(0));
    let parse_errors_for_callback = parse_errors.clone();
    let picker = cx.new(|cx| {
        DateTimePicker::new(
            cx,
            DateTimePickerProps::default()
                .value(Some(DateTimePickerValue::Date(d(2026, 5, 26))))
                .on_parse_error(move |_| {
                    parse_errors_for_callback.set(parse_errors_for_callback.get() + 1);
                }),
        )
    });

    picker.update(cx, |picker, cx| {
        picker.state.set_input_text_silent(s("bad-date"));
        picker.commit_input_from_text(cx);
        assert_eq!(
            picker.value(),
            Some(&DateTimePickerValue::Date(d(2026, 5, 26)))
        );
        assert!(picker.state.has_parse_error());
    });
    assert_eq!(parse_errors.get(), 1);
}

/// 弹层确认失败时应同步内部输入框错误态，避免 helper text 变红但输入框边框仍保持旧状态。
#[gpui::test]
fn failed_popup_confirm_marks_inner_input_error(cx: &mut TestAppContext) {
    let parse_errors = Rc::new(Cell::new(0));
    let parse_errors_for_callback = parse_errors.clone();
    let picker = cx.new(|cx| {
        DateTimePicker::new(
            cx,
            DateTimePickerProps::default()
                .mode(DateTimePickerMode::DateTime)
                .min(Some(DateTimePickerValue::DateTime(dt(
                    2026, 5, 26, 12, 0, 0,
                ))))
                .on_parse_error(move |_| {
                    parse_errors_for_callback.set(parse_errors_for_callback.get() + 1);
                }),
        )
    });

    picker.update(cx, |picker, cx| {
        picker.open(cx);
        picker.select_date_interaction(d(2026, 5, 26), cx);
        picker.confirm_draft_interaction(cx);

        assert!(picker.state.has_parse_error());
        assert_eq!(picker.input.read(cx).status(), TextInputStatus::Error);
        assert!(picker.is_open());
    });
    assert_eq!(parse_errors.get(), 1);
}
