//! 按钮组件。
//!
//! `Button` 提供前端 UI 框架中常见的按钮能力：变体、色调、尺寸、禁用、加载、块级宽度、
//! Lucide 前后图标、纯图标按钮、鼠标点击和键盘触发。组件以 Entity 形式实现，便于后续在内部
//! 增加更复杂的交互状态，而不会破坏外部 API。

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, point, px, Animation, AnimationExt, App, AppContext, Bounds, ClickEvent, Context,
    CursorStyle, Element, ElementId, FocusHandle, Focusable, GlobalElementId, Hsla,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseDownEvent,
    ParentElement, PathBuilder, Pixels, Point, Render, SharedString, StatefulInteractiveElement,
    Style, Styled, Window,
};

mod props;
mod style;

#[cfg(test)]
mod tests;

pub use props::{ButtonClickHandler, ButtonProps, ButtonSize, ButtonTone, ButtonVariant};
use style::{can_trigger, resolve_button_style, ResolvedButtonStyle};

use crate::foundation::{icon, theme};

/// loading 图标完成一轮动画的时长。
///
/// 该值保持在 0.8 秒，接近常见桌面 UI loading spinner 的节奏：足够明显，又不会显得过快。
const LOADING_SPINNER_DURATION: Duration = Duration::from_millis(800);

/// loading 圆环一段弧线占整圆的比例。
///
/// 这里选择约 72% 的圆周，保留明显缺口，让用户能感知到旋转方向，而不是看到一个静态圆圈。
const LOADING_SPINNER_SWEEP_RATIO: f32 = 0.72;

/// loading 圆环的默认线宽。
///
/// 线宽独立于按钮文字字号，避免小按钮中图标过粗，也避免大按钮中图标显得太轻。
const LOADING_SPINNER_STROKE_WIDTH: f32 = 2.0;

/// xgpui 按钮组件。
///
/// Button 保存创建参数中的视觉属性和回调，同时拥有自己的 `FocusHandle`。
/// 外部通常通过 `cx.new(|cx| Button::new(cx, props))` 创建实体，然后把实体作为元素渲染。
pub struct Button {
    /// 组件焦点句柄。
    focus_handle: FocusHandle,
    /// 按钮文案。纯图标按钮不显示该文案，但会用它作为 tooltip fallback。
    label: SharedString,
    /// 视觉变体。
    variant: ButtonVariant,
    /// 语义色调。
    tone: ButtonTone,
    /// 按钮尺寸。
    size: ButtonSize,
    /// 禁用状态。禁用按钮不可聚焦，也不会触发回调。
    disabled: bool,
    /// 加载状态。加载按钮可展示处理中视觉，但不会触发回调。
    loading: bool,
    /// 是否占满父容器宽度。
    block: bool,
    /// 是否使用纯图标布局。
    icon_only: bool,
    /// 前置 Lucide 图标。
    leading_icon: Option<icon::LucideIcon>,
    /// 后置 Lucide 图标。
    trailing_icon: Option<icon::LucideIcon>,
    /// tooltip 文案。
    tooltip: Option<SharedString>,
    /// 有效点击回调。
    on_click: Option<ButtonClickHandler>,
}

impl Button {
    /// 创建新的 `Button`。
    pub fn new(cx: &mut Context<Self>, props: ButtonProps) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: props.label,
            variant: props.variant,
            tone: props.tone,
            size: props.size,
            disabled: props.disabled,
            loading: props.loading,
            block: props.block,
            icon_only: props.icon_only,
            leading_icon: props.leading_icon,
            trailing_icon: props.trailing_icon,
            tooltip: props.tooltip,
            on_click: props.on_click,
        }
    }

    /// 返回按钮焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 设置加载状态。
    ///
    /// 该方法只改变按钮视觉和可触发状态，不主动调用点击回调。
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        if self.loading == loading {
            return;
        }
        self.loading = loading;
        cx.notify();
    }

    /// 设置禁用状态。
    ///
    /// 禁用后组件不会继续在渲染树上注册焦点追踪和点击处理，因此用户无法通过鼠标或键盘触发它。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        cx.notify();
    }

    /// 当前按钮是否允许触发业务点击。
    fn can_trigger(&self) -> bool {
        can_trigger(self.disabled, self.loading)
    }

    /// 返回当前渲染样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedButtonStyle {
        resolve_button_style(
            self.size,
            self.variant,
            self.tone,
            focused,
            self.disabled,
            self.loading,
            cx,
        )
    }

    /// 响应鼠标或 gpui 合成点击。
    fn on_button_click(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        if !self.can_trigger() {
            return;
        }
        self.emit_click(cx);
    }

    /// 响应按钮外部鼠标按下。
    ///
    /// gpui 不会因为点击空白区域自动清空当前焦点；Button 需要和输入框、下拉框一样主动监听
    /// 外部鼠标按下，这样用户点击页面空白处或其他非焦点元素时，按钮焦点边框会立即消失。
    fn on_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !should_blur_on_mouse_down_out(self.focus_handle.is_focused(window)) {
            return;
        }

        window.blur();
        cx.notify();
    }

    /// 响应键盘按下。
    ///
    /// Button 在组件内部处理 `Enter` 和 `Space`，因此不需要调用方通过 `xgpui::install`
    /// 注册额外 keybinding。触发后会停止事件继续传播，避免父级快捷键重复响应同一次按键。
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
            cx.stop_propagation();
            if self.can_trigger() {
                self.emit_click(cx);
            }
        }
    }

    /// 触发用户回调并通知 gpui 当前实体可能需要重绘。
    fn emit_click(&mut self, cx: &mut Context<Self>) {
        if let Some(on_click) = self.on_click.as_mut() {
            on_click();
        }
        cx.notify();
    }

    /// 解析当前应该展示的 tooltip 文案。
    ///
    /// 纯图标按钮如果没有显式 tooltip，会使用 label 作为 fallback；普通按钮只有显式设置
    /// `tooltip` 时才展示提示，避免普通文本按钮出现重复信息。
    fn tooltip_text(&self) -> Option<SharedString> {
        self.tooltip
            .clone()
            .or_else(|| self.icon_only.then(|| self.label.clone()))
            .filter(|text| !text.is_empty())
    }

    /// 返回纯图标按钮要显示的图标。
    ///
    /// 加载状态优先显示 loading 圆环；否则优先使用前置图标，再使用后置图标。
    /// 如果调用方开启 `icon_only` 但没有传入图标，组件会保持空内容并依赖 tooltip 表达语义。
    fn icon_only_icon(&self) -> Option<icon::LucideIcon> {
        if self.loading {
            Some(icon::LucideIcon::Loader2)
        } else {
            self.leading_icon.or(self.trailing_icon)
        }
    }
}

impl Render for Button {
    /// 渲染 Button。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let tooltip_text = self.tooltip_text();

        let mut button = div()
            .id("xgpui-button")
            .flex()
            .items_center()
            .justify_center()
            .h(resolved.height)
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
            .when_else(self.block, |this| this.w_full(), |this| this)
            .when_else(
                self.icon_only,
                |this| this.size(resolved.icon_only_size).px(px(0.0)),
                |this| this.min_w(resolved.min_width).px(resolved.padding_x),
            )
            .when_else(
                self.disabled,
                |this| this.cursor(CursorStyle::Arrow),
                |this| {
                    this.track_focus(&self.focus_handle)
                        .cursor(CursorStyle::PointingHand)
                        .hover(move |style| style.bg(resolved.hover_background))
                        .active(move |style| style.bg(resolved.active_background))
                        .on_click(cx.listener(Self::on_button_click))
                        .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
                        .on_key_down(cx.listener(Self::on_key_down))
                },
            );

        if self.icon_only {
            if let Some(button_icon) = self.icon_only_icon() {
                button = if self.loading {
                    button.child(loading_spinner_element(resolved))
                } else {
                    button.child(button_icon_element(button_icon, resolved))
                };
            }
        } else {
            if self.loading {
                button = button.child(loading_spinner_element(resolved));
            } else if let Some(leading_icon) = self.leading_icon {
                button = button.child(button_icon_element(leading_icon, resolved));
            }

            let label = div()
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(0.0))
                .overflow_hidden()
                .line_height(resolved.line_height)
                .when(resolved.underline, |this| this.underline())
                .child(self.label.clone());
            button = button.child(label);

            if let Some(trailing_icon) = self.trailing_icon {
                button = button.child(button_icon_element(trailing_icon, resolved));
            }
        }

        button.when_some(tooltip_text, |this, tooltip_text| {
            this.tooltip(move |_window, cx| {
                cx.new(|_| ButtonTooltip {
                    text: tooltip_text.clone(),
                })
                .into()
            })
        })
    }
}

impl Focusable for Button {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// 判断外部鼠标按下是否应该释放按钮焦点。
///
/// 该函数把失焦分支提取为纯逻辑，便于单元测试覆盖边界；真正的 `window.blur()` 仍只在组件事件中执行。
fn should_blur_on_mouse_down_out(focused: bool) -> bool {
    focused
}

/// 构造按钮内的 Lucide 图标元素。
fn button_icon_element(icon_name: icon::LucideIcon, resolved: ResolvedButtonStyle) -> gpui::Div {
    icon::lucide_icon(icon_name, resolved.text, resolved.icon_size)
}

/// 构造 loading spinner 元素。
///
/// spinner 的外层尺寸与普通图标一致，动画只更新内部圆环弧线的起始角度，不改变布局尺寸。
fn loading_spinner_element(resolved: ResolvedButtonStyle) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(resolved.icon_size)
        .child(
            CircleLoadingSpinnerElement::new(resolved.text, resolved.icon_size).with_animation(
                "xgpui-button-loading-spinner",
                Animation::new(LOADING_SPINNER_DURATION).repeat(),
                |mut spinner, delta| {
                    spinner.phase = normalized_spinner_phase(delta);
                    spinner
                },
            ),
        )
        .into_any_element()
}

/// 将动画进度收敛到安全的圆环相位。
///
/// `delta` 通常由 gpui 以 `0.0..=1.0` 传入；这里保留边界处理，是为了让测试或未来手动调用
/// 不会因为越界浮点值导致圆环角度计算出现突变。
fn normalized_spinner_phase(delta: f32) -> f32 {
    delta.clamp(0.0, 0.999_999)
}

/// 根据动画相位计算圆环起始角度。
///
/// 起点从 12 点钟方向开始，符合常见 loading spinner 的视觉预期；随后按顺时针方向旋转。
fn spinner_start_angle(phase: f32) -> f32 {
    -std::f32::consts::FRAC_PI_2 + normalized_spinner_phase(phase) * std::f32::consts::TAU
}

/// 根据圆心、半径和角度计算圆环路径上的点。
fn spinner_point(center: Point<Pixels>, radius: Pixels, angle: f32) -> Point<Pixels> {
    point(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// Button loading 状态使用的圆圈加载元素。
///
/// gpui 0.2.2 的普通 `Div` 不提供直接旋转变换；因此这里用自定义元素绘制一段圆弧，
/// 再通过动画更新圆弧起始角度，得到真正的圆圈加载动画，同时避免引入额外 SVG 资源。
struct CircleLoadingSpinnerElement {
    /// 圆环颜色，来自按钮当前主题解析后的文本色，保证明暗皮肤一致。
    color: Hsla,
    /// 圆环布局尺寸，与按钮图标尺寸保持一致。
    size: Pixels,
    /// 当前动画相位，取值范围约为 `0.0..1.0`，表示圆弧绕圆心旋转一周的进度。
    phase: f32,
}

impl CircleLoadingSpinnerElement {
    /// 创建圆圈 loading 元素。
    fn new(color: Hsla, size: Pixels) -> Self {
        Self {
            color,
            size,
            phase: 0.0,
        }
    }
}

impl IntoElement for CircleLoadingSpinnerElement {
    type Element = Self;

    /// 将圆圈 loading 元素转换为 gpui element。
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CircleLoadingSpinnerElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    /// loading 圆环是按钮内部装饰元素，不需要稳定 id。
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// loading 圆环由组件内部生成，不暴露源码位置给 inspector。
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// 请求与按钮图标一致的正方形布局。
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = self.size.into();
        style.size.height = self.size.into();
        (window.request_layout(style, [], cx), ())
    }

    /// loading 圆环没有额外预绘制状态。
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

    /// 绘制一段绕圆心旋转的圆弧。
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
        paint_loading_spinner_arc(window, bounds, self.color, self.phase);
    }
}

/// 绘制 loading 圆环弧线。
///
/// 路径构建失败时直接跳过本帧绘制，避免装饰图标异常影响按钮主体布局和点击行为。
fn paint_loading_spinner_arc(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla, phase: f32) {
    let stroke_width = px(LOADING_SPINNER_STROKE_WIDTH);
    let radius = (bounds.size.width.min(bounds.size.height) - stroke_width) * 0.5;
    if radius <= Pixels::ZERO {
        return;
    }

    let start_angle = spinner_start_angle(phase);
    let sweep_angle = std::f32::consts::TAU * LOADING_SPINNER_SWEEP_RATIO;
    let end_angle = start_angle + sweep_angle;
    let center = bounds.center();
    let start = spinner_point(center, radius, start_angle);
    let end = spinner_point(center, radius, end_angle);

    let mut builder = PathBuilder::stroke(stroke_width);
    builder.move_to(start);
    builder.arc_to(
        point(radius, radius),
        px(0.0),
        sweep_angle > std::f32::consts::PI,
        true,
        end,
    );
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Button tooltip 视图。
///
/// 该视图只为 Button 内部 tooltip 服务，避免为了一个轻量提示引入额外组件依赖。
struct ButtonTooltip {
    /// tooltip 展示文本。
    text: SharedString,
}

impl Render for ButtonTooltip {
    /// 渲染 tooltip。
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button_theme = theme::button_theme(cx);

        div()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(button_theme.border)
            .bg(button_theme.secondary_background)
            .text_color(button_theme.text)
            .text_size(px(12.0))
            .line_height(px(16.0))
            .shadow_md()
            .child(self.text.clone())
    }
}
