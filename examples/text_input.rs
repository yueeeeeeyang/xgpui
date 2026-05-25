//! `TextInput` 组件示例。
//!
//! 该示例展示基础输入、禁用、只读、错误态、helper text、清除按钮、任意前后缀插槽和外部同步值。

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, Entity, Hsla, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例窗口根视图。
struct TextInputExample {
    basic: Entity<TextInput>,
    disabled: Entity<TextInput>,
    readonly: Entity<TextInput>,
    error: Entity<TextInput>,
    slotted: Entity<TextInput>,
    synced: Entity<TextInput>,
}

impl TextInputExample {
    /// 创建示例中使用的多个输入框实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let basic = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .placeholder("请输入用户名")
                    .clearable(true)
                    .helper_text(Some(SharedString::from("支持复制、粘贴、拖选和中文输入法"))),
            )
        });
        let disabled = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .value("禁用状态")
                    .disabled(true)
                    .helper_text(Some(SharedString::from("禁用后不能聚焦或编辑"))),
            )
        });
        let readonly = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .value("只读状态，可选择和复制")
                    .readonly(true)
                    .helper_text(Some(SharedString::from("只读允许选择和复制，但不能修改"))),
            )
        });
        let error = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .placeholder("请输入邮箱")
                    .status(TextInputStatus::Error)
                    .helper_text(Some(SharedString::from("邮箱格式不正确")))
                    .clearable(true),
            )
        });
        let slotted = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .placeholder("example.com")
                    .prefix(Some(TextInputSlot::new(|| {
                        div()
                            .text_color(rgb(0x64748b))
                            .child("https://")
                            .into_any_element()
                    })))
                    .suffix(Some(TextInputSlot::new(|| {
                        div()
                            .text_color(rgb(0x64748b))
                            .child(".com")
                            .into_any_element()
                    })))
                    .clearable(true)
                    .helper_text(Some(SharedString::from("前后缀是任意 gpui 元素插槽"))),
            )
        });
        let synced = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .value("外部可同步的值")
                    .clearable(true)
                    .max_length(Some(20))
                    .helper_text(Some(SharedString::from(
                        "点击按钮会通过 set_value 从外部同步",
                    ))),
            )
        });

        Self {
            basic,
            disabled,
            readonly,
            error,
            slotted,
            synced,
        }
    }

    /// 从父组件外部同步输入值。
    fn set_synced_value(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.synced.update(cx, |input, cx| {
            input.set_value("由父组件写入", cx);
            input.move_to_end(cx);
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

impl Render for TextInputExample {
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
            .child(section("基础输入", self.basic.clone(), palette))
            .child(section("禁用状态", self.disabled.clone(), palette))
            .child(section("只读状态", self.readonly.clone(), palette))
            .child(section("错误状态", self.error.clone(), palette))
            .child(section("前后缀插槽", self.slotted.clone(), palette))
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
    input: Entity<TextInput>,
    palette: ExamplePalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(input)
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

/// 示例入口。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);

        let bounds = Bounds::centered(None, size(px(560.0), px(660.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(TextInputExample::new),
        )
        .expect("text input example window should open");
    });
}
