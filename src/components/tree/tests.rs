//! `Tree` 状态与公开同步方法测试。
//!
//! 状态测试聚焦展开、过滤、选择和复选级联；组件方法测试使用 gpui 测试上下文，
//! 确认受控同步方法不会意外触发用户交互回调。

use std::{cell::Cell, rc::Rc};

use gpui::{AppContext, SharedString, TestAppContext};

use super::{
    props::{TreeCheckState, TreeNode, TreeProps, TreeSelectionMode, TreeStatus},
    state::{TreeIndex, TreeState},
    Tree,
};

/// 构造测试使用的标准树。
fn sample_nodes() -> Vec<TreeNode> {
    vec![
        TreeNode::new("root", "Root").children(vec![
            TreeNode::new("a", "Alpha"),
            TreeNode::new("b", "Beta").children(vec![
                TreeNode::new("b1", "Beta One"),
                TreeNode::new("b2", "Beta Two").disabled(true),
            ]),
        ]),
        TreeNode::new("docs", "Docs").children(vec![TreeNode::new("guide", "Guide")]),
    ]
}

/// 便捷构造 SharedString。
fn s(value: &str) -> SharedString {
    SharedString::from(value.to_owned())
}

/// 嵌套节点应按 DFS 顺序扁平化，并保留深度和父子关系。
#[test]
fn index_flattens_nested_nodes() {
    let index = TreeIndex::new(&sample_nodes());
    let keys = index
        .records
        .iter()
        .map(|record| record.key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["root", "a", "b", "b1", "b2", "docs", "guide"]);
    assert_eq!(index.get(&s("b1")).unwrap().depth, 2);
    assert_eq!(index.get(&s("guide")).unwrap().parent, Some(s("docs")));
}

/// 展开 key 控制普通状态下的可见行。
#[test]
fn expanded_keys_control_visible_rows() {
    let index = TreeIndex::new(&sample_nodes());
    let mut state = TreeState::new(
        vec![s("root")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Single,
        false,
    );

    let visible = state.visible_nodes(&index, false);
    let keys = visible
        .iter()
        .map(|node| node.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["root", "a", "b", "docs"]);

    state.set_expanded_keys_silent(vec![s("root"), s("b")], &index, false);
    let visible = state.visible_nodes(&index, false);
    let keys = visible
        .iter()
        .map(|node| node.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["root", "a", "b", "b1", "b2", "docs"]);
}

/// 过滤会显示匹配节点和祖先路径，但不会修改真实 expanded key。
#[test]
fn filter_keeps_ancestor_path_without_mutating_expanded_keys() {
    let index = TreeIndex::new(&sample_nodes());
    let mut state = TreeState::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Single,
        false,
    );

    state.set_filter_text_silent(s("one"), &index, false);
    let visible = state.visible_nodes(&index, false);
    let keys = visible
        .iter()
        .map(|node| node.key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["root", "b", "b1"]);
    assert!(state.expanded_keys().is_empty());
}

/// 多选模式支持普通替换、切换和可见范围选择。
#[test]
fn multiple_selection_supports_replace_toggle_and_range() {
    let index = TreeIndex::new(&sample_nodes());
    let mut state = TreeState::new(
        vec![s("root"), s("b")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Multiple,
        false,
    );
    let visible = state.visible_nodes(&index, false);

    state.select_key(&s("a"), &visible, TreeSelectionMode::Multiple, false, false);
    assert_eq!(state.selected_keys(), &[s("a")]);

    state.select_key(&s("b1"), &visible, TreeSelectionMode::Multiple, true, false);
    assert_eq!(state.selected_keys(), &[s("a"), s("b1")]);

    state.select_key(&s("b"), &visible, TreeSelectionMode::Multiple, false, true);
    // Cmd/Ctrl 切换会把范围选择锚点更新到最近操作的 b1，因此 Shift 选择 b 时会覆盖为 b..b1。
    assert_eq!(state.selected_keys(), &[s("b"), s("b1")]);
}

/// 级联复选应返回完全选中和半选节点，禁用节点不参与计算。
#[test]
fn checked_cascade_derives_half_checked_keys() {
    let index = TreeIndex::new(&sample_nodes());
    let mut state = TreeState::new(
        vec![s("root"), s("b")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Single,
        true,
    );

    state.toggle_checked(&s("b1"), &index, true);

    // b2 被禁用并排除在可检查集合之外，所以 b 的所有可检查子孙都已选中，规范化后 b 也为 checked。
    assert_eq!(state.checked_keys(), &[s("b"), s("b1")]);
    assert_eq!(state.half_checked_keys(&index, true), vec![s("root")]);

    let b = state
        .visible_nodes(&index, true)
        .into_iter()
        .find(|node| node.key == s("b"))
        .unwrap();
    assert_eq!(b.check_state, TreeCheckState::Checked);
}

/// 键盘活动项移动应跳过禁用节点，避免 disabled 行继续接收 Enter/Space/Left/Right 操作。
#[test]
fn active_navigation_skips_disabled_visible_nodes() {
    let index = TreeIndex::new(&sample_nodes());
    let mut state = TreeState::new(
        vec![s("root"), s("b")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Single,
        false,
    );
    let visible = state.visible_nodes(&index, false);

    state.move_active_by(1, &visible);
    assert_eq!(state.active_key(), Some(&s("a")));
    state.move_active_by(1, &visible);
    assert_eq!(state.active_key(), Some(&s("b")));
    state.move_active_by(1, &visible);
    assert_eq!(state.active_key(), Some(&s("b1")));
    state.move_active_by(1, &visible);
    assert_eq!(state.active_key(), Some(&s("docs")));
}

/// Right 键在展开节点上应移动到第一个非禁用子孙，Left 键不应回退到禁用父节点。
#[test]
fn keyboard_parent_child_navigation_respects_disabled_nodes() {
    let index = TreeIndex::new(&[TreeNode::new("root", "Root").children(vec![
        TreeNode::new("disabled", "Disabled").disabled(true),
        TreeNode::new("enabled", "Enabled"),
    ])]);
    let mut state = TreeState::new(
        vec![s("root")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &index,
        TreeSelectionMode::Single,
        false,
    );
    let visible = state.visible_nodes(&index, false);
    state.expand_or_child(&index, &visible, false);
    assert_eq!(state.active_key(), Some(&s("enabled")));

    let disabled_parent_index = TreeIndex::new(&[TreeNode::new("parent", "Parent")
        .disabled(true)
        .children(vec![TreeNode::new("child", "Child")])]);
    let mut child_state = TreeState::new(
        vec![s("parent")],
        Vec::new(),
        Vec::new(),
        SharedString::default(),
        &disabled_parent_index,
        TreeSelectionMode::Single,
        false,
    );
    assert_eq!(child_state.active_key(), Some(&s("child")));
    child_state.collapse_or_parent(&disabled_parent_index, false);
    assert_eq!(child_state.active_key(), Some(&s("child")));
}

/// set_nodes 应保留旧 key，重复 key 不应导致 panic 或覆盖第一次出现的节点。
#[test]
fn set_nodes_semantics_keep_state_and_ignore_duplicate_keys() {
    let index = TreeIndex::new(&[
        TreeNode::new("same", "First"),
        TreeNode::new("same", "Second").children(vec![TreeNode::new("child", "Child")]),
    ]);
    assert_eq!(index.records.len(), 1);
    assert_eq!(index.records[0].label, s("First"));
}

/// 受控同步方法不应触发交互回调。
#[gpui::test]
fn controlled_setters_do_not_emit_callbacks(cx: &mut TestAppContext) {
    let expand_count = Rc::new(Cell::new(0));
    let select_count = Rc::new(Cell::new(0));
    let check_count = Rc::new(Cell::new(0));
    let filter_count = Rc::new(Cell::new(0));

    let expand_for_callback = expand_count.clone();
    let select_for_callback = select_count.clone();
    let check_for_callback = check_count.clone();
    let filter_for_callback = filter_count.clone();

    let tree = cx.new(|cx| {
        Tree::new(
            cx,
            TreeProps::default()
                .nodes(sample_nodes())
                .checkable(true)
                .on_expand(move |_| expand_for_callback.set(expand_for_callback.get() + 1))
                .on_select(move |_| select_for_callback.set(select_for_callback.get() + 1))
                .on_check(move |_, _| check_for_callback.set(check_for_callback.get() + 1))
                .on_filter_change(move |_| filter_for_callback.set(filter_for_callback.get() + 1)),
        )
    });

    tree.update(cx, |tree, cx| {
        tree.set_expanded_keys(vec![s("root")], cx);
        tree.set_selected_keys(vec![s("a")], cx);
        tree.set_checked_keys(vec![s("b1")], cx);
        tree.set_filter_text("beta", cx);
    });

    assert_eq!(expand_count.get(), 0);
    assert_eq!(select_count.get(), 0);
    assert_eq!(check_count.get(), 0);
    assert_eq!(filter_count.get(), 0);
}

/// set_nodes 应重建缓存索引，使后续受控 checked 同步基于新节点树计算级联状态。
#[gpui::test]
fn set_nodes_rebuilds_cached_index_for_controlled_sync(cx: &mut TestAppContext) {
    let tree = cx.new(|cx| {
        Tree::new(
            cx,
            TreeProps::default()
                .nodes(vec![TreeNode::new("old", "Old")])
                .checkable(true),
        )
    });

    tree.update(cx, |tree, cx| {
        tree.set_nodes(
            vec![TreeNode::new("new", "New").children(vec![TreeNode::new("child", "Child")])],
            cx,
        );
        tree.set_checked_keys(vec![s("new")], cx);

        assert_eq!(tree.checked_keys(), &[s("new"), s("child")]);
    });
}

/// 禁用状态下 clear 类方法应保持 no-op，避免禁用组件仍触发交互回调。
#[gpui::test]
fn disabled_tree_clear_methods_are_noop_and_silent(cx: &mut TestAppContext) {
    let select_count = Rc::new(Cell::new(0));
    let check_count = Rc::new(Cell::new(0));
    let select_for_callback = select_count.clone();
    let check_for_callback = check_count.clone();

    let tree = cx.new(|cx| {
        Tree::new(
            cx,
            TreeProps::default()
                .nodes(sample_nodes())
                .selected_keys(vec![s("a")])
                .checked_keys(vec![s("a")])
                .checkable(true)
                .disabled(true)
                .on_select(move |_| select_for_callback.set(select_for_callback.get() + 1))
                .on_check(move |_, _| check_for_callback.set(check_for_callback.get() + 1)),
        )
    });

    tree.update(cx, |tree, cx| {
        tree.clear_selection(cx);
        tree.clear_checked(cx);

        assert_eq!(tree.selected_keys(), &[s("a")]);
        assert_eq!(tree.checked_keys(), &[s("a")]);
    });
    assert_eq!(select_count.get(), 0);
    assert_eq!(check_count.get(), 0);
}

/// 禁用和展示类同步只改变组件输入，不应改变树状态集合。
#[gpui::test]
fn disabled_status_and_helper_sync_do_not_change_tree_state(cx: &mut TestAppContext) {
    let tree = cx.new(|cx| {
        Tree::new(
            cx,
            TreeProps::default()
                .nodes(sample_nodes())
                .expanded_keys(vec![s("root")])
                .selected_keys(vec![s("a")])
                .checked_keys(vec![s("a")])
                .checkable(true),
        )
    });

    tree.update(cx, |tree, cx| {
        let expanded = tree.expanded_keys().to_vec();
        let selected = tree.selected_keys().to_vec();
        let checked = tree.checked_keys().to_vec();

        tree.set_disabled(true, cx);
        tree.set_status(TreeStatus::Error, cx);
        tree.set_helper_text(Some(s("请选择节点")), cx);

        assert_eq!(tree.expanded_keys(), expanded.as_slice());
        assert_eq!(tree.selected_keys(), selected.as_slice());
        assert_eq!(tree.checked_keys(), checked.as_slice());
        assert!(tree.disabled);
        assert_eq!(tree.status, TreeStatus::Error);
        assert_eq!(tree.helper_text, Some(s("请选择节点")));
    });
}
