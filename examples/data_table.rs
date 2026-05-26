//! `DataTable` 组件示例。
//!
//! 该示例展示本地过滤、排序、分页、多选、禁用行、自定义操作列、外部同步和明暗皮肤切换。

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, CursorStyle, Entity, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions,
};
use xgpui::prelude::*;

/// 示例订单行。
#[derive(Clone)]
struct OrderRow {
    /// 稳定业务 id，作为 DataTable 的 row key。
    id: &'static str,
    /// 客户名称。
    customer: &'static str,
    /// 订单状态。
    status: &'static str,
    /// 金额展示文本。
    amount: &'static str,
    /// 行是否禁用。
    disabled: bool,
}

/// 示例窗口根视图。
struct DataTableExample {
    orders: Entity<DataTable<OrderRow>>,
    compact: Entity<DataTable<OrderRow>>,
}

impl DataTableExample {
    /// 创建示例中使用的两个 DataTable 实体。
    fn new(cx: &mut Context<Self>) -> Self {
        let orders = cx.new(|cx| {
            DataTable::new(
                cx,
                DataTableProps::new(|row: &OrderRow| SharedString::from(row.id))
                    .rows(order_rows())
                    .columns(order_columns())
                    .row_disabled(|row| row.disabled)
                    .selection_mode(DataTableSelectionMode::Multiple)
                    .page_size(4)
                    .page_size_options(vec![4, 8, 12])
                    .helper_text(Some(SharedString::from(
                        "支持搜索、点击表头排序、Select 切换每页条数、多选和操作列渲染",
                    ))),
            )
        });
        let compact = cx.new(|cx| {
            DataTable::new(
                cx,
                DataTableProps::new(|row: &OrderRow| SharedString::from(row.id))
                    .rows(order_rows())
                    .columns(order_columns())
                    .show_filter(false)
                    .selection_mode(DataTableSelectionMode::Single)
                    .size(DataTableSize::Small)
                    .variant(DataTableVariant::Filled)
                    .status(DataTableStatus::Warning)
                    .page_size(3)
                    .helper_text(Some(SharedString::from(
                        "紧凑单选表格，可作为嵌入式业务列表",
                    ))),
            )
        });

        Self { orders, compact }
    }

    /// 从父组件外部同步新的过滤词和选择状态。
    fn sync_orders(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.orders.update(cx, |table, cx| {
            table.set_filter_text("ready", cx);
            table.set_selected_row_keys(vec![SharedString::from("ord-1001")], cx);
            table.set_page(1, cx);
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

impl Render for DataTableExample {
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
                    )
                    .child(
                        theme_button("sync-orders", "同步", false, palette)
                            .w(px(72.0))
                            .on_click(cx.listener(Self::sync_orders)),
                    ),
            )
            .child(section("订单数据表格", self.orders.clone(), palette))
            .child(section("紧凑单选表格", self.compact.clone(), palette))
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
fn section(
    title: &'static str,
    table: Entity<DataTable<OrderRow>>,
    palette: ExamplePalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, palette))
        .child(table)
}

/// 渲染皮肤切换和同步按钮。
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

/// 示例列配置，最后一列为自定义操作列。
fn order_columns() -> Vec<DataTableColumn<OrderRow>> {
    vec![
        DataTableColumn::text("customer", "客户", |row: &OrderRow| {
            SharedString::from(row.customer)
        })
        .width(px(180.0)),
        DataTableColumn::text("status", "状态", |row: &OrderRow| {
            SharedString::from(row.status)
        })
        .width(px(120.0)),
        DataTableColumn::text("amount", "金额", |row: &OrderRow| {
            SharedString::from(row.amount)
        })
        .align(DataTableAlign::Right)
        .width(px(120.0)),
        DataTableColumn::actions(
            "actions",
            "操作",
            |ctx: DataTableCellContext<'_, OrderRow>| {
                let label = if ctx.selected { "已选中" } else { "查看" };
                let id = ctx.row_index;
                let row_id = ctx.row.id;
                div()
                    .id(("order-action", id))
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .cursor(if ctx.disabled {
                        CursorStyle::Arrow
                    } else {
                        CursorStyle::PointingHand
                    })
                    .child(label)
                    .when(!ctx.disabled, |this| {
                        this.on_click(move |_, _, _| {
                            println!("查看订单 {row_id}");
                        })
                    })
                    .into_any_element()
            },
        ),
    ]
}

/// 示例订单数据。
fn order_rows() -> Vec<OrderRow> {
    vec![
        OrderRow {
            id: "ord-1001",
            customer: "Acme",
            status: "Ready",
            amount: "$128.00",
            disabled: false,
        },
        OrderRow {
            id: "ord-1002",
            customer: "Globex",
            status: "Blocked",
            amount: "$89.50",
            disabled: true,
        },
        OrderRow {
            id: "ord-1003",
            customer: "Initech",
            status: "Ready",
            amount: "$310.20",
            disabled: false,
        },
        OrderRow {
            id: "ord-1004",
            customer: "Umbrella",
            status: "Done",
            amount: "$64.00",
            disabled: false,
        },
        OrderRow {
            id: "ord-1005",
            customer: "Stark",
            status: "Ready",
            amount: "$980.00",
            disabled: false,
        },
        OrderRow {
            id: "ord-1006",
            customer: "Wayne",
            status: "Done",
            amount: "$412.10",
            disabled: false,
        },
        OrderRow {
            id: "ord-1007",
            customer: "Hooli",
            status: "Ready",
            amount: "$72.30",
            disabled: false,
        },
    ]
}

/// 启动示例应用。
fn main() {
    Application::new().run(|cx: &mut App| {
        xgpui::install(cx);
        let bounds = Bounds::centered(None, size(px(860.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            |_, cx| cx.new(DataTableExample::new),
        )
        .expect("open data table example window");
    });
}
