//! `Tree` 组件示例。
//!
//! 该示例展示展开折叠、单选、多选、级联复选、过滤、禁用、错误态、外部同步和明暗皮肤切换。

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, Entity, Hsla, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例窗口根视图。
struct TreeExample {
    basic: Entity<Tree>,
    multiple: Entity<Tree>,
    checkable: Entity<Tree>,
    filtered: Entity<Tree>,
    disabled: Entity<Tree>,
    error: Entity<Tree>,
    synced: Entity<Tree>,
}

impl TreeExample {
    /// 创建示例中使用的多个 Tree 实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let basic = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(project_nodes())
                    .expanded_keys(vec![SharedString::from("src")])
                    .helper_text(Some(SharedString::from(
                        "基础单选树，方向键可移动活动项，左右键展开折叠",
                    ))),
            )
        });
        let multiple = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(project_nodes())
                    .expanded_keys(vec![SharedString::from("src")])
                    .selection_mode(TreeSelectionMode::Multiple)
                    .helper_text(Some(SharedString::from(
                        "多选模式支持普通点击替换、Cmd/Ctrl 点击切换和 Shift 范围选择",
                    ))),
            )
        });
        let checkable = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(permission_nodes())
                    .expanded_keys(vec![SharedString::from("admin")])
                    .checkable(true)
                    .checked_keys(vec![SharedString::from("user.read")])
                    .helper_text(Some(SharedString::from(
                        "复选框使用父子级联，并自动计算半选状态",
                    ))),
            )
        });
        let filtered = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(project_nodes())
                    .filter_text("rs")
                    .helper_text(Some(SharedString::from(
                        "过滤时显示匹配节点和祖先路径，不改写真正 expanded_keys",
                    ))),
            )
        });
        let disabled = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(project_nodes())
                    .expanded_keys(vec![SharedString::from("src")])
                    .disabled(true)
                    .helper_text(Some(SharedString::from("禁用后不能聚焦或交互"))),
            )
        });
        let error = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(permission_nodes())
                    .checkable(true)
                    .status(TreeStatus::Error)
                    .required(true)
                    .helper_text(Some(SharedString::from("至少需要选择一个权限节点"))),
            )
        });
        let synced = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(project_nodes())
                    .expanded_keys(vec![SharedString::from("src")])
                    .selected_keys(vec![SharedString::from("src/lib.rs")])
                    .helper_text(Some(SharedString::from(
                        "点击按钮会通过 set_selected_keys 从父组件写入",
                    ))),
            )
        });

        Self {
            basic,
            multiple,
            checkable,
            filtered,
            disabled,
            error,
            synced,
        }
    }

    /// 从父组件外部同步 selected key。
    fn set_synced_selection(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.synced.update(cx, |tree, cx| {
            tree.set_expanded_keys(vec![SharedString::from("src")], cx);
            tree.set_selected_keys(vec![SharedString::from("src/components/tree")], cx);
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

impl Render for TreeExample {
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
            .gap(px(14.0))
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
            .child(section("基础树", self.basic.clone(), palette))
            .child(section("多选树", self.multiple.clone(), palette))
            .child(section("级联复选", self.checkable.clone(), palette))
            .child(section("过滤结果", self.filtered.clone(), palette))
            .child(section("禁用状态", self.disabled.clone(), palette))
            .child(section("错误状态", self.error.clone(), palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(label("外部同步", palette))
                    .child(self.synced.clone())
                    .child(
                        div()
                            .id("set-synced-selection")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(154.0))
                            .h(px(30.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(palette.button_border)
                            .bg(palette.button_background)
                            .text_color(palette.button_text)
                            .cursor_pointer()
                            .child("写入外部选择")
                            .on_click(cx.listener(Self::set_synced_selection)),
                    ),
            )
    }
}

/// 示例页面在当前皮肤下使用的外层颜色。
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

/// 渲染示例区块标题。
fn label(text: impl Into<SharedString>, palette: ExamplePalette) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(palette.label)
        .child(text.into())
}

/// 渲染一个示例区块。
fn section(title: &'static str, tree: Entity<Tree>, palette: ExamplePalette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(tree)
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

/// 示例项目树节点。
fn project_nodes() -> Vec<TreeNode> {
    vec![
        TreeNode::new("src", "src")
            .icon(LucideIcon::Folder)
            .children(vec![
                TreeNode::new("src/lib.rs", "lib.rs").icon(LucideIcon::FileCode),
                TreeNode::new("src/prelude.rs", "prelude.rs").icon(LucideIcon::FileCode),
                TreeNode::new("src/components/tree", "components/tree")
                    .icon(LucideIcon::Folder)
                    .children(vec![
                        TreeNode::new("src/components/tree/mod.rs", "mod.rs")
                            .icon(LucideIcon::FileCode),
                        TreeNode::new("src/components/tree/state.rs", "state.rs")
                            .icon(LucideIcon::FileCode),
                    ]),
            ]),
        TreeNode::new("examples", "examples")
            .icon(LucideIcon::Folder)
            .children(vec![
                TreeNode::new("examples/tree.rs", "tree.rs").icon(LucideIcon::FileCode)
            ]),
        TreeNode::new("README.md", "README.md").icon(LucideIcon::FileText),
    ]
}

/// 示例权限树节点。
fn permission_nodes() -> Vec<TreeNode> {
    vec![TreeNode::new("admin", "后台管理").children(vec![
        TreeNode::new("user", "用户管理").children(vec![
            TreeNode::new("user.read", "查看用户"),
            TreeNode::new("user.write", "编辑用户"),
            TreeNode::new("user.delete", "删除用户").disabled(true),
        ]),
        TreeNode::new("audit", "审计日志").children(vec![
            TreeNode::new("audit.read", "查看日志"),
            TreeNode::new("audit.export", "导出日志").checkable(false),
        ]),
    ])]
}

/// 启动示例应用。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);
        let bounds = Bounds::centered(None, size(px(720.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            |_, cx| cx.new(TreeExample::new),
        )
        .expect("open tree example window");
    });
}
