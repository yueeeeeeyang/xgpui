//! `Select` 组件示例。
//!
//! 该示例展示基础单选、搜索、禁用、错误态、清除按钮、空结果、外部同步值和明暗皮肤切换。

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, Entity, Hsla, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例窗口根视图。
struct SelectExample {
    basic: Entity<Select>,
    searchable: Entity<Select>,
    disabled: Entity<Select>,
    error: Entity<Select>,
    clearable: Entity<Select>,
    empty: Entity<Select>,
    synced: Entity<Select>,
}

impl SelectExample {
    /// 创建示例中使用的多个 Select 实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let basic = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .placeholder("请选择城市")
                    .options(city_options())
                    .helper_text(Some(SharedString::from("基础单选，下拉后可用方向键导航"))),
            )
        });
        let searchable = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .placeholder("搜索语言")
                    .options(language_options())
                    .searchable(true)
                    .search_placeholder("输入关键词")
                    .helper_text(Some(SharedString::from(
                        "打开后直接在选择框中输入字符按 label 过滤",
                    ))),
            )
        });
        let disabled = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .value(Some(SharedString::from("disabled")))
                    .options(vec![SelectOption::new("disabled", "禁用状态")])
                    .disabled(true)
                    .helper_text(Some(SharedString::from("禁用后不能聚焦、打开或清除"))),
            )
        });
        let error = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .placeholder("请选择支付方式")
                    .options(payment_options())
                    .status(SelectStatus::Error)
                    .helper_text(Some(SharedString::from("支付方式不能为空"))),
            )
        });
        let clearable = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .value(Some(SharedString::from("apple")))
                    .placeholder("请选择水果")
                    .options(fruit_options())
                    .clearable(true)
                    .helper_text(Some(SharedString::from("有值且非禁用时显示清除按钮"))),
            )
        });
        let empty = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .placeholder("暂无可选项")
                    .options(Vec::new())
                    .empty_text("没有匹配的选项")
                    .helper_text(Some(SharedString::from("空选项列表会展示 empty_text"))),
            )
        });
        let synced = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .value(Some(SharedString::from("tea")))
                    .placeholder("外部同步值")
                    .options(drink_options())
                    .clearable(true)
                    .helper_text(Some(SharedString::from(
                        "点击按钮会通过 set_value 从父组件写入",
                    ))),
            )
        });

        Self {
            basic,
            searchable,
            disabled,
            error,
            clearable,
            empty,
            synced,
        }
    }

    /// 从父组件外部同步 Select 值。
    fn set_synced_value(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.synced.update(cx, |select, cx| {
            select.set_value(Some(SharedString::from("coffee")), cx);
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

impl Render for SelectExample {
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
            .child(section("基础选择", self.basic.clone(), palette))
            .child(section("搜索选择", self.searchable.clone(), palette))
            .child(section("禁用状态", self.disabled.clone(), palette))
            .child(section("错误状态", self.error.clone(), palette))
            .child(section("可清除", self.clearable.clone(), palette))
            .child(section("空选项", self.empty.clone(), palette))
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
                            .w(px(120.0))
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

/// 示例页面在当前皮肤下使用的外层颜色。
///
/// 这些颜色只用于示例壳层，组件自身颜色仍由 xgpui 主题系统解析。
#[derive(Clone, Copy)]
struct ExamplePalette {
    /// 示例窗口背景色。
    background: Hsla,
    /// 示例标签文本颜色。
    label: Hsla,
    /// 普通按钮背景色。
    button_background: Hsla,
    /// 当前选中皮肤按钮背景色。
    active_button_background: Hsla,
    /// 按钮边框颜色。
    button_border: Hsla,
    /// 按钮文本颜色。
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

/// 创建带标签的示例区块。
fn section(
    title: &'static str,
    select: Entity<Select>,
    palette: ExamplePalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(select)
}

/// 创建区块标签。
fn label(title: &'static str, palette: ExamplePalette) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(palette.label)
        .child(title)
}

/// 创建皮肤切换按钮。
fn theme_button(
    id: &'static str,
    title: &'static str,
    active: bool,
    palette: ExamplePalette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.0))
        .px(px(14.0))
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
        .child(title)
}

/// 示例城市选项。
fn city_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("beijing", "北京"),
        SelectOption::new("shanghai", "上海"),
        SelectOption::new("shenzhen", "深圳"),
        SelectOption::new("hangzhou", "杭州"),
    ]
}

/// 示例语言选项。
fn language_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("rust", "Rust"),
        SelectOption::new("typescript", "TypeScript"),
        SelectOption::new("swift", "Swift"),
        SelectOption::new("go", "Go"),
        SelectOption::new("disabled", "Disabled option").disabled(true),
    ]
}

/// 示例支付方式选项。
fn payment_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("card", "银行卡"),
        SelectOption::new("wallet", "钱包"),
        SelectOption::new("transfer", "银行转账"),
    ]
}

/// 示例水果选项。
fn fruit_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("apple", "苹果"),
        SelectOption::new("orange", "橙子"),
        SelectOption::new("banana", "香蕉"),
    ]
}

/// 示例饮品选项。
fn drink_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("tea", "茶"),
        SelectOption::new("coffee", "咖啡"),
        SelectOption::new("water", "水"),
    ]
}

/// 示例入口。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);

        let bounds = Bounds::centered(None, size(px(560.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            |_, cx| cx.new(SelectExample::new),
        )
        .expect("open select example window");
    });
}
