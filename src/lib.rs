//! `xgpui` 是基于 `gpui` 的 Rust 基础 UI 组件库。
//!
//! 这个 crate 目前提供组件所需的基础主题 token、`Button` 按钮组件、
//! `TextInput` 单行文本输入组件、`Textarea` 多行文本输入组件、`Select` 单选下拉框组件
//! 和 `Tree` 标准树组件。
//! 公共模块保持分层导出，避免把组件实现、主题定义和工具逻辑混在同一个文件中。

/// 基础设计 token 和主题能力。
pub mod foundation;

/// 对外可直接使用的 UI 组件集合。
pub mod components;

/// 常用公共类型的集中导出入口。
pub mod prelude;

pub use foundation::theme::{set_theme_mode, theme_mode, ThemeMode};

/// xgpui 在 `gpui::App` 上的安装标记。
///
/// 该全局状态只用于保证 `install` 幂等，避免调用方重复初始化时把默认快捷键绑定重复写入
/// gpui 的全局 keymap。
struct XgpuiInstallState;

impl gpui::Global for XgpuiInstallState {}

/// 安装 xgpui 的默认应用级能力。
///
/// 当前安装内容包括默认主题状态、Lucide 图标字体、`TextInput`、`Textarea`、`Select` 和 `Tree`
/// 默认键盘绑定。
/// `Button` 的 `Enter` / `Space` 键盘触发由组件内部处理，不需要额外注册全局快捷键。
/// 调用方应在 `Application::run` 的初始化闭包中调用一次本函数；函数内部会记录安装状态，
/// 因此重复调用不会重复注册快捷键，也不会覆盖调用方已经设置的皮肤模式。
pub fn install(cx: &mut gpui::App) {
    foundation::theme::ensure_theme(cx);

    if cx.has_global::<XgpuiInstallState>() {
        return;
    }

    foundation::icon::install_icon_fonts(cx);
    components::text_input::register_text_input_key_bindings(cx);
    components::textarea::register_textarea_key_bindings(cx);
    components::select::register_select_key_bindings(cx);
    components::tree::register_tree_key_bindings(cx);
    cx.set_global(XgpuiInstallState);
}
