//! `Button` 组件示例。
//!
//! 该示例展示按钮变体、危险色调、尺寸、禁用、加载、块级宽度、前后图标、纯图标按钮和明暗皮肤切换。

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, CursorStyle, Entity, Hsla,
    IntoElement, ParentElement, Render, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例窗口根视图。
struct ButtonExample {
    primary: Entity<Button>,
    secondary: Entity<Button>,
    outline: Entity<Button>,
    ghost: Entity<Button>,
    link: Entity<Button>,
    danger: Entity<Button>,
    small: Entity<Button>,
    medium: Entity<Button>,
    large: Entity<Button>,
    disabled: Entity<Button>,
    loading: Entity<Button>,
    block: Entity<Button>,
    leading_icon: Entity<Button>,
    trailing_icon: Entity<Button>,
    icon_only: Entity<Button>,
    /// 示例层保存 loading 按钮的外部状态，用于演示父组件通过公开方法同步子组件。
    loading_on: bool,
}

impl ButtonExample {
    /// 创建示例中使用的多个 Button 实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let primary = cx.new(|cx| Button::new(cx, ButtonProps::default().label("Primary")));
        let secondary = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Secondary")
                    .variant(ButtonVariant::Secondary),
            )
        });
        let outline = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Outline")
                    .variant(ButtonVariant::Outline),
            )
        });
        let ghost = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Ghost")
                    .variant(ButtonVariant::Ghost),
            )
        });
        let link = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Link")
                    .variant(ButtonVariant::Link),
            )
        });
        let danger = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("删除")
                    .tone(ButtonTone::Danger)
                    .leading_icon(LucideIcon::Trash2),
            )
        });
        let small = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Small")
                    .size(ButtonSize::Small),
            )
        });
        let medium = cx.new(|cx| Button::new(cx, ButtonProps::default().label("Medium")));
        let large = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Large")
                    .size(ButtonSize::Large),
            )
        });
        let disabled = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Disabled")
                    .variant(ButtonVariant::Secondary)
                    .disabled(true),
            )
        });
        let loading = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Loading")
                    .loading(true)
                    .leading_icon(LucideIcon::Save),
            )
        });
        let block = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("Block Button")
                    .variant(ButtonVariant::Outline)
                    .block(true),
            )
        });
        let leading_icon = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("保存")
                    .leading_icon(LucideIcon::Save),
            )
        });
        let trailing_icon = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("继续")
                    .variant(ButtonVariant::Secondary)
                    .trailing_icon(LucideIcon::ArrowRight),
            )
        });
        let icon_only = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("设置")
                    .variant(ButtonVariant::Ghost)
                    .icon_only(true)
                    .leading_icon(LucideIcon::Settings)
                    .tooltip("设置"),
            )
        });

        Self {
            primary,
            secondary,
            outline,
            ghost,
            link,
            danger,
            small,
            medium,
            large,
            disabled,
            loading,
            block,
            leading_icon,
            trailing_icon,
            icon_only,
            loading_on: true,
        }
    }

    /// 切换到亮色皮肤。
    fn use_light_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        set_theme_mode(cx, ThemeMode::Light);
    }

    /// 切换到暗色皮肤。
    fn use_dark_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        set_theme_mode(cx, ThemeMode::Dark);
    }

    /// 切换加载示例按钮的加载状态。
    fn toggle_loading(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.loading_on = !self.loading_on;
        let loading_on = self.loading_on;
        self.loading.update(cx, |button, cx| {
            button.set_loading(loading_on, cx);
        });
    }
}

impl Render for ButtonExample {
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
            .gap(px(18.0))
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
            .child(button_section(
                "按钮变体",
                row(vec![
                    self.primary.clone().into_any_element(),
                    self.secondary.clone().into_any_element(),
                    self.outline.clone().into_any_element(),
                    self.ghost.clone().into_any_element(),
                    self.link.clone().into_any_element(),
                ]),
                palette,
            ))
            .child(button_section(
                "危险色调",
                row(vec![self.danger.clone().into_any_element()]),
                palette,
            ))
            .child(button_section(
                "尺寸",
                row(vec![
                    self.small.clone().into_any_element(),
                    self.medium.clone().into_any_element(),
                    self.large.clone().into_any_element(),
                ]),
                palette,
            ))
            .child(button_section(
                "状态",
                row(vec![
                    self.disabled.clone().into_any_element(),
                    self.loading.clone().into_any_element(),
                    native_action_button("toggle-loading", "切换 Loading", palette)
                        .on_click(cx.listener(Self::toggle_loading))
                        .into_any_element(),
                ]),
                palette,
            ))
            .child(button_section(
                "图标",
                row(vec![
                    self.leading_icon.clone().into_any_element(),
                    self.trailing_icon.clone().into_any_element(),
                    self.icon_only.clone().into_any_element(),
                ]),
                palette,
            ))
            .child(button_section("块级按钮", self.block.clone(), palette))
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
            label: rgb(0xe2e8f0).into(),
            button_background: rgb(0x0f172a).into(),
            active_button_background: rgb(0x1e3a8a).into(),
            button_border: rgb(0x334155).into(),
            button_text: rgb(0xe2e8f0).into(),
        },
    }
}

/// 构造示例章节。
fn button_section(
    title: &'static str,
    content: impl IntoElement,
    palette: ExamplePalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(content)
}

/// 构造横向按钮排列。
fn row(children: Vec<gpui::AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .children(children)
}

/// 构造章节标题。
fn label(text: &'static str, palette: ExamplePalette) -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .line_height(px(20.0))
        .text_color(palette.label)
        .child(text)
}

/// 构造示例页面自己的皮肤切换按钮。
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
        .px(px(16.0))
        .h(px(34.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(palette.button_border)
        .bg(if active {
            palette.active_button_background
        } else {
            palette.button_background
        })
        .text_color(palette.button_text)
        .cursor(CursorStyle::PointingHand)
        .child(text)
}

/// 构造示例页面自己的操作按钮。
fn native_action_button(
    id: &'static str,
    text: &'static str,
    palette: ExamplePalette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px(px(12.0))
        .h(px(30.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(palette.button_border)
        .bg(palette.button_background)
        .text_color(palette.button_text)
        .cursor(CursorStyle::PointingHand)
        .child(text)
}

/// 示例入口。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(760.0), px(620.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(ButtonExample::new),
        )
        .expect("open button example window");
    });
}
