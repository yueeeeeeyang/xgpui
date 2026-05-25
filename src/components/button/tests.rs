//! `Button` 纯逻辑测试。
//!
//! 这些测试聚焦默认参数、尺寸映射、交互开关和变体样式决策，不启动 gpui 窗口，
//! 从而让按钮核心规则在普通单元测试中保持快速和稳定。

use gpui::{px, rgb};

use crate::foundation::{radius, spacing, theme::ButtonTheme};

use super::{
    normalized_spinner_phase,
    props::{ButtonProps, ButtonSize, ButtonTone, ButtonVariant},
    should_blur_on_mouse_down_out, spinner_start_angle,
    style::{can_trigger, resolve_button_style_with_theme, size_tokens},
    LOADING_SPINNER_DURATION, LOADING_SPINNER_STROKE_WIDTH, LOADING_SPINNER_SWEEP_RATIO,
};

/// 构造测试用 Button 主题。
///
/// 测试主题使用差异明显的颜色，便于验证变体和禁用状态是否选择了正确 token。
fn sample_theme() -> ButtonTheme {
    ButtonTheme {
        primary_background: rgb(0x111111).into(),
        primary_hover_background: rgb(0x222222).into(),
        primary_active_background: rgb(0x333333).into(),
        primary_text: rgb(0xffffff).into(),
        danger_background: rgb(0xaa0000).into(),
        danger_hover_background: rgb(0xbb0000).into(),
        danger_active_background: rgb(0xcc0000).into(),
        danger_text: rgb(0xffffff).into(),
        secondary_background: rgb(0xeeeeee).into(),
        secondary_hover_background: rgb(0xdddddd).into(),
        secondary_active_background: rgb(0xcccccc).into(),
        text: rgb(0x101010).into(),
        muted_text: rgb(0x777777).into(),
        border: rgb(0x999999).into(),
        ghost_hover_background: rgb(0xf0f0f0).into(),
        ghost_active_background: rgb(0xe0e0e0).into(),
        danger_ghost_hover_background: rgb(0xffeeee).into(),
        danger_ghost_active_background: rgb(0xffdddd).into(),
        focus: rgb(0x0066ff).into(),
        disabled_background: rgb(0xf7f7f7).into(),
        disabled_border: rgb(0xeeeeee).into(),
        disabled_text: rgb(0xaaaaaa).into(),
        radius: radius::md(),
        gap: spacing::sm(),
    }
}

/// 默认参数应符合普通主按钮的预期。
#[test]
fn default_props_describe_primary_medium_button() {
    let props = ButtonProps::default();

    assert_eq!(props.label.as_ref(), "按钮");
    assert_eq!(props.variant, ButtonVariant::Primary);
    assert_eq!(props.tone, ButtonTone::Default);
    assert_eq!(props.size, ButtonSize::Medium);
    assert!(!props.disabled);
    assert!(!props.loading);
    assert!(!props.block);
    assert!(!props.icon_only);
}

/// 尺寸 token 应随尺寸增大而增大，纯图标按钮边长应等于按钮高度。
#[test]
fn size_tokens_scale_with_button_size() {
    let small = size_tokens(ButtonSize::Small);
    let medium = size_tokens(ButtonSize::Medium);
    let large = size_tokens(ButtonSize::Large);

    assert_eq!(small.icon_only_size, small.height);
    assert_eq!(medium.icon_only_size, medium.height);
    assert_eq!(large.icon_only_size, large.height);
    assert!(small.height < medium.height);
    assert!(medium.height < large.height);
}

/// 主按钮默认色调应使用主色填充和主按钮文本色。
#[test]
fn primary_default_style_uses_primary_tokens() {
    let theme = sample_theme();
    let style = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Primary,
        ButtonTone::Default,
        false,
        false,
        false,
        theme,
    );

    assert_eq!(style.background, theme.primary_background);
    assert_eq!(style.hover_background, theme.primary_hover_background);
    assert_eq!(style.active_background, theme.primary_active_background);
    assert_eq!(style.text, theme.primary_text);
    assert!(!style.underline);
}

/// 危险描边按钮应使用危险语义边框和危险文本色。
#[test]
fn outline_danger_style_uses_danger_semantics() {
    let theme = sample_theme();
    let style = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Outline,
        ButtonTone::Danger,
        false,
        false,
        false,
        theme,
    );

    assert_eq!(style.border, theme.danger_background);
    assert_eq!(style.text, theme.danger_background);
    assert_eq!(style.hover_background, theme.danger_ghost_hover_background);
}

/// 聚焦状态应只替换边框为焦点色，不改变高度，避免布局跳动。
#[test]
fn focused_style_uses_focus_border_without_size_change() {
    let theme = sample_theme();
    let normal = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Outline,
        ButtonTone::Default,
        false,
        false,
        false,
        theme,
    );
    let focused = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Outline,
        ButtonTone::Default,
        true,
        false,
        false,
        theme,
    );

    assert_eq!(focused.border, theme.focus);
    assert_eq!(focused.height, normal.height);
    assert_eq!(focused.padding_x, normal.padding_x);
}

/// 禁用状态应使用禁用 token，并关闭触发能力。
#[test]
fn disabled_style_and_interaction_are_suppressed() {
    let theme = sample_theme();
    let style = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Primary,
        ButtonTone::Default,
        true,
        true,
        false,
        theme,
    );

    assert_eq!(style.background, theme.disabled_background);
    assert_eq!(style.border, theme.disabled_border);
    assert_eq!(style.text, theme.disabled_text);
    assert!(!can_trigger(true, false));
}

/// 加载状态不使用禁用 token，但同样不能触发点击。
#[test]
fn loading_keeps_visual_style_but_blocks_trigger() {
    let theme = sample_theme();
    let style = resolve_button_style_with_theme(
        ButtonSize::Medium,
        ButtonVariant::Primary,
        ButtonTone::Default,
        false,
        false,
        true,
        theme,
    );

    assert_eq!(style.background, theme.primary_background);
    assert!(style.opacity < 1.0);
    assert!(!can_trigger(false, true));
}

/// 外部鼠标按下只应在按钮已经聚焦时触发失焦。
#[test]
fn mouse_down_out_blurs_only_when_button_is_focused() {
    assert!(should_blur_on_mouse_down_out(true));
    assert!(!should_blur_on_mouse_down_out(false));
}

/// loading spinner 应保持明确的循环动画节奏。
#[test]
fn loading_spinner_uses_expected_animation_duration() {
    assert_eq!(LOADING_SPINNER_DURATION.as_millis(), 800);
}

/// loading 圆圈动画应把外部进度收敛到安全相位。
#[test]
fn loading_spinner_phase_is_clamped_for_circle_animation() {
    assert_eq!(normalized_spinner_phase(-1.0), 0.0);
    assert_eq!(normalized_spinner_phase(0.5), 0.5);
    assert!(normalized_spinner_phase(1.0) < 1.0);
}

/// loading 圆圈动画应随相位推进起始角度。
#[test]
fn loading_spinner_start_angle_advances_with_phase() {
    let start = spinner_start_angle(0.0);
    let half_turn = spinner_start_angle(0.5);

    assert!((start + std::f32::consts::FRAC_PI_2).abs() < f32::EPSILON);
    assert!((half_turn - (start + std::f32::consts::PI)).abs() < 0.0001);
}

/// loading 圆圈应使用带缺口的圆弧和固定细线宽，避免看起来像静态圆点或粗体图标。
#[test]
fn loading_spinner_uses_partial_circle_arc_tokens() {
    assert!(LOADING_SPINNER_SWEEP_RATIO > 0.5);
    assert!(LOADING_SPINNER_SWEEP_RATIO < 1.0);
    assert_eq!(LOADING_SPINNER_STROKE_WIDTH, 2.0);
}

/// Link 变体应启用下划线并保持最小视觉边界。
#[test]
fn link_variant_uses_text_style() {
    let theme = sample_theme();
    let style = resolve_button_style_with_theme(
        ButtonSize::Small,
        ButtonVariant::Link,
        ButtonTone::Default,
        false,
        false,
        false,
        theme,
    );

    assert!(style.underline);
    assert_eq!(style.border, gpui::rgba(0x0000_0000).into());
    assert_eq!(style.min_width, px(64.0));
}
