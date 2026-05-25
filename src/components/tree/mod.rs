//! 标准树组件。
//!
//! `Tree` 提供嵌套节点、展开/折叠、单选/多选、级联复选、过滤、键盘导航、
//! 虚拟列表、禁用、状态样式、helper text、受控同步和明暗皮肤。

use gpui::prelude::*;
use gpui::{
    actions, div, px, uniform_list, App, Context, CursorStyle, Entity, FocusHandle, Focusable,
    IntoElement, KeyBinding, KeyDownEvent, MouseDownEvent, ParentElement, Pixels, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle,
    Window,
};

use crate::foundation::icon::{self, LucideIcon};

mod props;
mod state;
mod style;

#[cfg(test)]
mod tests;

pub use props::{
    TreeCheckState, TreeNode, TreeProps, TreeSelectionMode, TreeSize, TreeStatus, TreeVariant,
};
use state::{TreeIndex, TreeState, TreeStateOutcome, VisibleTreeNode};
use style::{resolve_tree_style, ResolvedTreeStyle};

actions!(
    xgpui_tree,
    [
        MoveUp,
        MoveDown,
        FirstNode,
        LastNode,
        CollapseOrParent,
        ExpandOrChild,
        CommitSelection,
        ToggleCheckOrSelect,
        SelectAll,
    ]
);

/// 注册 `Tree` 默认键盘快捷键。
///
/// gpui 的键盘动作需要应用启动时注册。调用方通常不需要直接调用本函数，
/// 使用 `xgpui::install(cx)` 即可同时安装所有内置组件的默认快捷键。
pub fn register_tree_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, Some("Tree")),
        KeyBinding::new("down", MoveDown, Some("Tree")),
        KeyBinding::new("home", FirstNode, Some("Tree")),
        KeyBinding::new("end", LastNode, Some("Tree")),
        KeyBinding::new("left", CollapseOrParent, Some("Tree")),
        KeyBinding::new("right", ExpandOrChild, Some("Tree")),
        KeyBinding::new("enter", CommitSelection, Some("Tree")),
        KeyBinding::new("space", ToggleCheckOrSelect, Some("Tree")),
        KeyBinding::new("cmd-a", SelectAll, Some("Tree")),
        KeyBinding::new("ctrl-a", SelectAll, Some("Tree")),
    ]);
}

/// 标准树组件。
///
/// 组件内部维护展开、选中、复选、过滤和键盘活动项，同时通过 `set_*` 方法支持外部受控同步。
/// 受控同步默认不触发用户交互回调，避免父组件写回状态时形成回调循环。
pub struct Tree {
    focus_handle: FocusHandle,
    nodes: Vec<TreeNode>,
    /// 当前节点树的扁平索引。
    ///
    /// Tree 的渲染、键盘导航、过滤和复选级联都需要 DFS 顺序与 key 查找。如果每次 render
    /// 或每次键盘事件都重新构建索引，大节点树会在虚拟列表之外继续产生 O(n) 开销。
    /// 因此索引只在初始化和 `set_nodes` 时重建，其余路径复用缓存结果。
    index: TreeIndex,
    state: TreeState,
    selection_mode: TreeSelectionMode,
    checkable: bool,
    disabled: bool,
    required: bool,
    size: TreeSize,
    variant: TreeVariant,
    status: TreeStatus,
    helper_text: Option<SharedString>,
    empty_text: SharedString,
    max_height: Pixels,
    on_expand: Option<props::TreeExpandHandler>,
    on_select: Option<props::TreeSelectHandler>,
    on_check: Option<props::TreeCheckHandler>,
    on_filter_change: Option<props::TreeFilterChangeHandler>,
    on_focus: Option<props::TreeFocusHandler>,
    on_blur: Option<props::TreeFocusHandler>,
    on_key_down: Option<props::TreeKeyDownHandler>,
    scroll_handle: UniformListScrollHandle,
    is_focused: bool,
    /// 禁用态受控同步期间是否需要静默吸收下一次焦点变化。
    ///
    /// `set_disabled(true)` 是父组件写入的受控状态，不代表用户主动离开 Tree。
    /// 如果禁用前组件处于聚焦态，下一次 render 会因为 `disabled` 把交互焦点视为 false；
    /// 这里记录该变化来自受控同步，让 `sync_focus_callbacks` 只更新内部状态而不触发 `on_blur`。
    suppress_next_focus_callback: bool,
}

impl Tree {
    /// 创建新的 `Tree`。
    pub fn new(cx: &mut Context<Self>, props: TreeProps) -> Self {
        let index = TreeIndex::new(&props.nodes);
        let state = TreeState::new(
            props.expanded_keys,
            props.selected_keys,
            props.checked_keys,
            props.filter_text,
            &index,
            props.selection_mode,
            props.checkable,
        );

        Self {
            focus_handle: cx.focus_handle(),
            nodes: props.nodes,
            index,
            state,
            selection_mode: props.selection_mode,
            checkable: props.checkable,
            disabled: props.disabled,
            required: props.required,
            size: props.size,
            variant: props.variant,
            status: props.status,
            helper_text: props.helper_text,
            empty_text: props.empty_text,
            max_height: props.max_height,
            on_expand: props.on_expand,
            on_select: props.on_select,
            on_check: props.on_check,
            on_filter_change: props.on_filter_change,
            on_focus: props.on_focus,
            on_blur: props.on_blur,
            on_key_down: props.on_key_down,
            scroll_handle: UniformListScrollHandle::new(),
            is_focused: false,
            suppress_next_focus_callback: false,
        }
    }

    /// 返回当前展开 key。
    pub fn expanded_keys(&self) -> &[SharedString] {
        self.state.expanded_keys()
    }

    /// 返回当前 selected key。
    pub fn selected_keys(&self) -> &[SharedString] {
        self.state.selected_keys()
    }

    /// 返回当前 checked key。
    pub fn checked_keys(&self) -> &[SharedString] {
        self.state.checked_keys()
    }

    /// 返回当前 half checked key。
    pub fn half_checked_keys(&self) -> Vec<SharedString> {
        self.state.half_checked_keys(&self.index, self.checkable)
    }

    /// 从外部同步节点树。
    ///
    /// 该方法保留当前展开、选中和复选 key；不存在于新节点树中的 key 不参与渲染和派生状态。
    /// 节点更新属于受控同步，不触发任何外部交互回调。
    pub fn set_nodes(&mut self, nodes: impl Into<Vec<TreeNode>>, cx: &mut Context<Self>) {
        let nodes = nodes.into();
        self.nodes = nodes;
        self.index = TreeIndex::new(&self.nodes);
        let outcome = self.state.sync_nodes_silent(&self.index, self.checkable);
        if outcome.should_notify() {
            self.scroll_active_into_view();
        }
        cx.notify();
    }

    /// 从外部同步展开 key。
    pub fn set_expanded_keys(
        &mut self,
        expanded_keys: impl Into<Vec<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let outcome =
            self.state
                .set_expanded_keys_silent(expanded_keys.into(), &self.index, self.checkable);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步 selected key。
    pub fn set_selected_keys(
        &mut self,
        selected_keys: impl Into<Vec<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let outcome = self
            .state
            .set_selected_keys_silent(selected_keys.into(), self.selection_mode);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步 checked key。
    pub fn set_checked_keys(
        &mut self,
        checked_keys: impl Into<Vec<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let outcome =
            self.state
                .set_checked_keys_silent(checked_keys.into(), &self.index, self.checkable);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步过滤文本。
    pub fn set_filter_text(
        &mut self,
        filter_text: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let outcome =
            self.state
                .set_filter_text_silent(filter_text.into(), &self.index, self.checkable);
        self.apply_outcome(outcome, false, cx);
    }

    /// 从外部同步禁用状态。
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }

        self.disabled = disabled;
        if disabled {
            self.suppress_next_focus_callback |= self.is_focused;
            self.is_focused = false;
        }
        cx.notify();
    }

    /// 从外部同步语义状态。
    pub fn set_status(&mut self, status: TreeStatus, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }

        self.status = status;
        cx.notify();
    }

    /// 从外部同步辅助文本。
    pub fn set_helper_text(
        &mut self,
        helper_text: impl Into<Option<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        let helper_text = helper_text.into();
        if self.helper_text == helper_text {
            return;
        }

        self.helper_text = helper_text;
        cx.notify();
    }

    /// 清空 selected key。
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.clear_selection();
        self.apply_outcome(outcome, true, cx);
    }

    /// 清空 checked key。
    pub fn clear_checked(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.clear_checked();
        self.apply_outcome(outcome, true, cx);
    }

    /// 返回焦点句柄。
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 解析当前渲染样式。
    fn resolved_style(&self, focused: bool, cx: &App) -> ResolvedTreeStyle {
        resolve_tree_style(
            self.size,
            self.variant,
            self.status,
            focused,
            self.disabled,
            cx,
        )
    }

    /// 应用状态结果。
    fn apply_outcome(
        &mut self,
        outcome: TreeStateOutcome,
        emit_callbacks: bool,
        cx: &mut Context<Self>,
    ) {
        if emit_callbacks {
            if outcome.expanded_changed {
                self.emit_expand();
            }
            if outcome.selected_changed {
                self.emit_select();
            }
            if outcome.checked_changed {
                self.emit_check();
            }
            if outcome.filter_changed {
                self.emit_filter_change();
            }
        }

        if outcome.active_changed {
            self.scroll_active_into_view();
        }
        if outcome.should_notify() {
            cx.notify();
        }
    }

    /// 触发展开回调。
    fn emit_expand(&mut self) {
        if let Some(on_expand) = self.on_expand.as_mut() {
            on_expand(self.state.expanded_keys().to_vec());
        }
    }

    /// 触发选中回调。
    fn emit_select(&mut self) {
        if let Some(on_select) = self.on_select.as_mut() {
            on_select(self.state.selected_keys().to_vec());
        }
    }

    /// 触发复选回调。
    fn emit_check(&mut self) {
        let half_checked_keys = self.state.half_checked_keys(&self.index, self.checkable);
        if let Some(on_check) = self.on_check.as_mut() {
            on_check(self.state.checked_keys().to_vec(), half_checked_keys);
        }
    }

    /// 触发过滤文本回调。
    fn emit_filter_change(&mut self) {
        if let Some(on_filter_change) = self.on_filter_change.as_mut() {
            on_filter_change(self.state.filter_text().clone());
        }
    }

    /// 同步焦点状态并触发焦点回调。
    fn sync_focus_callbacks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        if focused == self.is_focused {
            if !focused && !self.disabled {
                self.suppress_next_focus_callback = false;
            }
            return;
        }

        self.is_focused = focused;
        if focused {
            if self.suppress_next_focus_callback {
                self.suppress_next_focus_callback = false;
            } else if let Some(on_focus) = self.on_focus.as_mut() {
                on_focus();
            }
        } else if self.suppress_next_focus_callback {
            self.suppress_next_focus_callback = false;
        } else if let Some(on_blur) = self.on_blur.as_mut() {
            on_blur();
        }
        cx.notify();
    }

    /// 将活动项滚入可见区域。
    fn scroll_active_into_view(&mut self) {
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        if let Some(active_key) = self.state.active_key() {
            if let Some(visible_index) = visible.iter().position(|node| &node.key == active_key) {
                self.scroll_handle
                    .scroll_to_item(visible_index, ScrollStrategy::Center);
            }
        }
    }

    /// 响应行点击。
    fn on_row_click(
        &mut self,
        key: SharedString,
        event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);

        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let modifiers = event.modifiers();
        let outcome = self.state.select_key(
            &key,
            &visible,
            self.selection_mode,
            modifiers.platform || modifiers.control,
            modifiers.shift,
        );
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应展开按钮点击。
    fn on_expander_click(
        &mut self,
        key: SharedString,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        let outcome = self
            .state
            .toggle_expanded(&key, &self.index, self.checkable);
        self.apply_outcome(outcome, true, cx);
    }

    /// 响应复选框点击。
    fn on_checkbox_click(
        &mut self,
        key: SharedString,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || !self.checkable {
            return;
        }
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        let outcome = self.state.toggle_checked(&key, &self.index, self.checkable);
        self.apply_outcome(outcome, true, cx);
    }

    /// 鼠标在 Tree 外按下时释放焦点。
    fn on_mouse_down_out(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus_handle.is_focused(window) {
            return;
        }

        window.blur();
        self.sync_focus_callbacks(window, cx);
    }

    /// 响应键盘按下事件。
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, _: &mut Context<Self>) {
        if let Some(on_key_down) = self.on_key_down.as_mut() {
            on_key_down(event.keystroke.clone());
        }
    }

    /// 向上移动活动项。
    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_by(-1, cx);
    }

    /// 向下移动活动项。
    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_active_by(1, cx);
    }

    /// 按指定步长移动活动项。
    fn move_active_by(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self.state.move_active_by(delta, &visible);
        self.apply_outcome(outcome, false, cx);
    }

    /// 移动到首个节点。
    fn first_node(&mut self, _: &FirstNode, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self.state.move_active_first(&visible);
        self.apply_outcome(outcome, false, cx);
    }

    /// 移动到最后一个节点。
    fn last_node(&mut self, _: &LastNode, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self.state.move_active_last(&visible);
        self.apply_outcome(outcome, false, cx);
    }

    /// 折叠当前节点或移动到父节点。
    fn collapse_or_parent(&mut self, _: &CollapseOrParent, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let outcome = self.state.collapse_or_parent(&self.index, self.checkable);
        self.apply_outcome(outcome, true, cx);
    }

    /// 展开当前节点或移动到第一个子节点。
    fn expand_or_child(&mut self, _: &ExpandOrChild, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self
            .state
            .expand_or_child(&self.index, &visible, self.checkable);
        self.apply_outcome(outcome, true, cx);
    }

    /// Enter 执行当前活动行选中。
    fn commit_selection(&mut self, _: &CommitSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.select_active(false, false, cx);
    }

    /// Space 优先切换复选框，未开启复选时执行行选中。
    fn toggle_check_or_select(
        &mut self,
        _: &ToggleCheckOrSelect,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let Some(active) = self.state.active_key().cloned() else {
            return;
        };
        if self.checkable {
            let outcome = self
                .state
                .toggle_checked(&active, &self.index, self.checkable);
            self.apply_outcome(outcome, true, cx);
        } else {
            self.select_active(false, false, cx);
        }
    }

    /// 选择当前活动行。
    fn select_active(&mut self, toggle: bool, range: bool, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let Some(active) = self.state.active_key().cloned() else {
            return;
        };
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self
            .state
            .select_key(&active, &visible, self.selection_mode, toggle, range);
        self.apply_outcome(outcome, true, cx);
    }

    /// 多选模式下选中所有可见可选节点。
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let outcome = self.state.select_all_visible(&visible, self.selection_mode);
        self.apply_outcome(outcome, true, cx);
    }
}

impl Focusable for Tree {
    /// 返回组件焦点句柄。
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Tree {
    /// 渲染 Tree 组件。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus_callbacks(window, cx);

        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let resolved = self.resolved_style(focused, cx);
        let helper_text = self.helper_text.clone();
        let required = self.required;
        let visible = self.state.visible_nodes(&self.index, self.checkable);
        let list_height = tree_list_height(visible.len(), resolved, self.max_height);
        let list = if visible.is_empty() {
            div()
                .id("xgpui-tree-empty")
                .flex()
                .items_center()
                .w_full()
                .h(list_height)
                .px(resolved.padding_x)
                .text_color(resolved.empty_text)
                .child(self.empty_text.clone())
                .into_any_element()
        } else {
            let tree = cx.entity();
            let item_count = visible.len();
            let scroll_handle = self.scroll_handle.clone();
            uniform_list("xgpui-tree-list", item_count, move |range, window, cx| {
                let tree_state = tree.read(cx);
                range
                    .filter_map(|visible_index| {
                        visible
                            .get(visible_index)
                            .cloned()
                            .map(|node| (visible_index, node))
                    })
                    .map(|(visible_index, node)| {
                        tree_row_element(
                            tree.clone(),
                            visible_index,
                            node,
                            tree_state.checkable,
                            resolved,
                            window,
                        )
                    })
                    .collect()
            })
            .w_full()
            .h(list_height)
            .track_scroll(scroll_handle)
            .into_any_element()
        };

        let tree_box = div()
            .id("xgpui-tree")
            .flex()
            .flex_col()
            .w_full()
            .h(list_height + resolved.padding_y * 2.0)
            .px(resolved.padding_x)
            .py(resolved.padding_y)
            .rounded(resolved.radius)
            .border_1()
            .border_color(resolved.border)
            .bg(resolved.background)
            .text_size(resolved.font_size)
            .line_height(resolved.line_height)
            .opacity(resolved.opacity)
            .overflow_hidden()
            .when_else(
                self.disabled,
                |this| this.cursor(CursorStyle::Arrow),
                |this| {
                    this.track_focus(&self.focus_handle)
                        .cursor(CursorStyle::Arrow)
                        .key_context("Tree")
                        .on_action(cx.listener(Self::move_up))
                        .on_action(cx.listener(Self::move_down))
                        .on_action(cx.listener(Self::first_node))
                        .on_action(cx.listener(Self::last_node))
                        .on_action(cx.listener(Self::collapse_or_parent))
                        .on_action(cx.listener(Self::expand_or_child))
                        .on_action(cx.listener(Self::commit_selection))
                        .on_action(cx.listener(Self::toggle_check_or_select))
                        .on_action(cx.listener(Self::select_all))
                        .on_key_down(cx.listener(Self::on_key_down))
                        .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
                },
            )
            .child(list);

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(tree_box)
            .when(required, |this| {
                this.child(
                    div()
                        .text_color(crate::foundation::color::danger_500())
                        .child("*"),
                )
            })
            .when_some(helper_text, |this, helper_text| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(resolved.helper)
                        .child(helper_text),
                )
            })
    }
}

/// 渲染单个树节点行。
fn tree_row_element(
    tree: Entity<Tree>,
    visible_index: usize,
    node: VisibleTreeNode,
    show_checkbox: bool,
    resolved: ResolvedTreeStyle,
    window: &mut Window,
) -> impl IntoElement {
    let key_for_row = node.key.clone();
    let text_color = if node.disabled {
        resolved.disabled_text
    } else if node.selected {
        resolved.row_selected_text
    } else {
        resolved.text
    };
    let background = if node.active {
        resolved.row_active
    } else if node.selected {
        resolved.row_selected
    } else {
        crate::foundation::color::transparent()
    };

    div()
        .id(("xgpui-tree-row", visible_index))
        .flex()
        .items_center()
        .w_full()
        .h(resolved.row_height)
        .pl(resolved.indent * node.depth as f32)
        .pr(resolved.padding_x)
        .gap(px(4.0))
        .bg(background)
        .text_color(text_color)
        .opacity(if node.disabled { 0.58 } else { 1.0 })
        .cursor(if node.disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .when(!node.disabled, |this| {
            this.hover(move |style| style.bg(resolved.row_hover))
                .on_click(window.listener_for(&tree, move |this, event, window, cx| {
                    this.on_row_click(key_for_row.clone(), event, window, cx)
                }))
        })
        .child(expander_element(
            tree.clone(),
            visible_index,
            node.clone(),
            resolved,
            window,
        ))
        .when(show_checkbox, |this| {
            this.child(checkbox_element(
                tree.clone(),
                visible_index,
                node.clone(),
                resolved,
                window,
            ))
        })
        .when_some(node.icon, |this, icon| {
            this.child(icon::lucide_icon(
                icon,
                resolved.muted_text,
                resolved.icon_size,
            ))
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(node.label),
        )
}

/// 渲染展开/折叠按钮。
fn expander_element(
    tree: Entity<Tree>,
    visible_index: usize,
    node: VisibleTreeNode,
    resolved: ResolvedTreeStyle,
    window: &mut Window,
) -> impl IntoElement {
    if !node.has_children {
        return div()
            .flex_none()
            .size(resolved.icon_size)
            .into_any_element();
    }

    let key = node.key.clone();
    let icon = if node.expanded {
        LucideIcon::ChevronDown
    } else {
        LucideIcon::ChevronRight
    };

    div()
        .id(("xgpui-tree-expander", visible_index))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(resolved.icon_size)
        .rounded(px(4.0))
        .cursor(if node.disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .child(icon::lucide_icon(
            icon,
            resolved.muted_text,
            resolved.icon_size,
        ))
        .when(!node.disabled, |this| {
            this.hover(move |style| style.bg(resolved.row_hover))
                .on_click(window.listener_for(&tree, move |this, event, window, cx| {
                    this.on_expander_click(key.clone(), event, window, cx)
                }))
        })
        .into_any_element()
}

/// 渲染复选框。
fn checkbox_element(
    tree: Entity<Tree>,
    visible_index: usize,
    node: VisibleTreeNode,
    resolved: ResolvedTreeStyle,
    window: &mut Window,
) -> impl IntoElement {
    if !node.checkable {
        // 节点级 checkable=false 表示该节点不显示也不参与复选；这里保留同宽占位，
        // 避免同级节点因缺少复选框列而出现文本错位。
        return div()
            .flex_none()
            .size(resolved.checkbox_size)
            .into_any_element();
    }

    let key = node.key.clone();
    let checked = node.check_state == TreeCheckState::Checked;
    let indeterminate = node.check_state == TreeCheckState::Indeterminate;
    let background = if checked || indeterminate {
        resolved.checkbox_checked_background
    } else {
        resolved.checkbox_background
    };
    let icon = if checked {
        Some(LucideIcon::Check)
    } else if indeterminate {
        Some(LucideIcon::Minus)
    } else {
        None
    };

    div()
        .id(("xgpui-tree-checkbox", visible_index))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(resolved.checkbox_size)
        .rounded(px(4.0))
        .border_1()
        .border_color(resolved.checkbox_border)
        .bg(background)
        .cursor(if node.disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .when_some(icon, |this, icon| {
            this.child(icon::lucide_icon(
                icon,
                resolved.checkbox_checked_text,
                resolved.checkbox_size * 0.82,
            ))
        })
        .when(!node.disabled, |this| {
            this.on_click(window.listener_for(&tree, move |this, event, window, cx| {
                this.on_checkbox_click(key.clone(), event, window, cx)
            }))
        })
        .into_any_element()
}

/// 根据可见节点数量计算虚拟列表高度。
fn tree_list_height(
    visible_count: usize,
    resolved: ResolvedTreeStyle,
    max_height: Pixels,
) -> Pixels {
    let rows = visible_count.max(1);
    (resolved.row_height * rows as f32)
        .min(max_height)
        .max(resolved.row_height)
}
