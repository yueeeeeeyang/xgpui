//! `DateTimePicker` 组件示例。
//!
//! 该示例展示日期、时间、日期时间、范围选择、错误态、外部同步和明暗皮肤切换。

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, Entity, Hsla, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例窗口根视图。
struct DateTimePickerExample {
    date: Entity<DateTimePicker>,
    time: Entity<DateTimePicker>,
    datetime: Entity<DateTimePicker>,
    date_range: Entity<DateTimePicker>,
    time_range: Entity<DateTimePicker>,
    datetime_range: Entity<DateTimePicker>,
    error: Entity<DateTimePicker>,
    synced: Entity<DateTimePicker>,
}

impl DateTimePickerExample {
    /// 创建示例中使用的多个 DateTimePicker 实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let date = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::Date)
                    .placeholder("选择日期")
                    .helper_text(Some(SharedString::from("单日期选择，点击日期后立即提交"))),
            )
        });
        let time = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::Time)
                    .placeholder("选择时间")
                    .value(Some(DateTimePickerValue::Time(t(9, 30, 0))))
                    .helper_text(Some(SharedString::from(
                        "时间选择支持 0-59 分和 0-59 秒逐项选择，点击确认后提交",
                    ))),
            )
        });
        let datetime = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::DateTime)
                    .placeholder("选择日期时间")
                    .value(Some(DateTimePickerValue::DateTime(dt(
                        2026, 5, 26, 10, 0, 0,
                    ))))
                    .helper_text(Some(SharedString::from("日期和时间联动选择"))),
            )
        });
        let date_range = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::DateRange)
                    .placeholder("选择日期范围")
                    .value(Some(DateTimePickerValue::DateRange(DateTimeRange::new(
                        d(2026, 5, 1),
                        d(2026, 5, 7),
                    ))))
                    .helper_text(Some(SharedString::from("范围模式使用双月面板，确认后提交"))),
            )
        });
        let time_range = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::TimeRange)
                    .placeholder("选择时间范围")
                    .value(Some(DateTimePickerValue::TimeRange(DateTimeRange::new(
                        t(9, 0, 0),
                        t(18, 0, 0),
                    ))))
                    .helper_text(Some(SharedString::from("TimeRange 不表达跨天时间段"))),
            )
        });
        let datetime_range = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::DateTimeRange)
                    .placeholder("选择日期时间范围")
                    .value(Some(DateTimePickerValue::DateTimeRange(
                        DateTimeRange::new(dt(2026, 5, 26, 9, 0, 0), dt(2026, 5, 28, 18, 0, 0)),
                    )))
                    .helper_text(Some(SharedString::from("范围起止会自动规范化顺序"))),
            )
        });
        let error = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::Date)
                    .placeholder("不可选周末")
                    .status(DateTimePickerStatus::Error)
                    .disabled_date(|date| {
                        use chrono::Datelike;
                        let weekday = date.weekday().number_from_monday();
                        weekday >= 6
                    })
                    .helper_text(Some(SharedString::from("周末被 disabled_date 禁用"))),
            )
        });
        let synced = cx.new(|cx| {
            DateTimePicker::new(
                cx,
                DateTimePickerProps::default()
                    .mode(DateTimePickerMode::DateTime)
                    .placeholder("外部同步日期时间")
                    .clearable(true)
                    .helper_text(Some(SharedString::from("点击按钮会通过 set_value 写入"))),
            )
        });

        Self {
            date,
            time,
            datetime,
            date_range,
            time_range,
            datetime_range,
            error,
            synced,
        }
    }

    /// 从父组件外部同步日期时间值。
    fn set_synced_value(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.synced.update(cx, |picker, cx| {
            picker.set_value(
                Some(DateTimePickerValue::DateTime(dt(2026, 6, 1, 8, 30, 0))),
                cx,
            );
        });
    }

    /// 切换到亮色皮肤。
    fn use_light_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        set_theme_mode(cx, ThemeMode::Light);
    }

    /// 切换到暗色皮肤。
    fn use_dark_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        set_theme_mode(cx, ThemeMode::Dark);
    }
}

impl Render for DateTimePickerExample {
    /// 渲染示例界面。
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = theme_mode(cx);
        let palette = example_palette(mode);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(palette.background)
            .p(px(24.0))
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        theme_button("light-theme", "亮色", mode == ThemeMode::Light, palette)
                            .on_click(cx.listener(Self::use_light_theme)),
                    )
                    .child(
                        theme_button("dark-theme", "暗色", mode == ThemeMode::Dark, palette)
                            .on_click(cx.listener(Self::use_dark_theme)),
                    ),
            )
            .child(section("日期", self.date.clone(), palette))
            .child(section("时间", self.time.clone(), palette))
            .child(section("日期时间", self.datetime.clone(), palette))
            .child(section("日期范围", self.date_range.clone(), palette))
            .child(section("时间范围", self.time_range.clone(), palette))
            .child(section(
                "日期时间范围",
                self.datetime_range.clone(),
                palette,
            ))
            .child(section("禁用规则", self.error.clone(), palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(label("外部同步", palette))
                    .child(self.synced.clone())
                    .child(
                        div()
                            .id("set-synced-value")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(150.0))
                            .h(px(30.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(palette.button_border)
                            .bg(palette.button_background)
                            .text_color(palette.button_text)
                            .cursor_pointer()
                            .child("写入外部值")
                            .on_click(cx.listener(Self::set_synced_value)),
                    ),
            )
    }
}

/// 示例页面颜色。
#[derive(Clone, Copy)]
struct ExamplePalette {
    /// 示例窗口背景。
    background: Hsla,
    /// 标签文本颜色。
    label: Hsla,
    /// 普通按钮背景。
    button_background: Hsla,
    /// 当前选中按钮背景。
    active_button_background: Hsla,
    /// 按钮边框。
    button_border: Hsla,
    /// 按钮文本。
    button_text: Hsla,
}

/// 根据当前皮肤返回示例页面颜色。
fn example_palette(mode: ThemeMode) -> ExamplePalette {
    match mode {
        ThemeMode::Light => ExamplePalette {
            background: rgb(0xf8fafc).into(),
            label: rgb(0x334155).into(),
            button_background: rgb(0xffffff).into(),
            active_button_background: rgb(0xdbeafe).into(),
            button_border: rgb(0xcbd5e1).into(),
            button_text: rgb(0x0f172a).into(),
        },
        ThemeMode::Dark => ExamplePalette {
            background: rgb(0x020617).into(),
            label: rgb(0xcbd5e1).into(),
            button_background: rgb(0x0f172a).into(),
            active_button_background: rgb(0x1e3a8a).into(),
            button_border: rgb(0x334155).into(),
            button_text: rgb(0xe2e8f0).into(),
        },
    }
}

/// 渲染示例分组。
fn section(
    title: &'static str,
    picker: Entity<DateTimePicker>,
    palette: ExamplePalette,
) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(picker)
}

/// 渲染标签。
fn label(text: &'static str, palette: ExamplePalette) -> gpui::Div {
    div().text_color(palette.label).child(text)
}

/// 渲染皮肤切换按钮。
fn theme_button(
    id: &'static str,
    text: &'static str,
    active: bool,
    palette: ExamplePalette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(64.0))
        .h(px(30.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(palette.button_border)
        .bg(if active {
            palette.active_button_background
        } else {
            palette.button_background
        })
        .text_color(palette.button_text)
        .cursor_pointer()
        .child(text)
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

/// 示例入口。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(760.0), px(860.0)),
                    cx,
                ))),
                ..WindowOptions::default()
            },
            |_, cx| cx.new(DateTimePickerExample::new),
        )
        .expect("open date time picker example window");
    });
}
