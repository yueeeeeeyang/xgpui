//! `Tree` 的纯状态管理。
//!
//! 本模块不依赖 gpui 窗口和渲染上下文，专门负责节点扁平化、展开、选中、
//! 复选级联、过滤和键盘活动项。把这些规则集中在状态层，可以用普通单元测试覆盖
//! Tree 的核心行为，并让渲染层只负责把状态结果映射为 gpui 元素。

use std::collections::{HashMap, HashSet};

use gpui::SharedString;

use crate::foundation::icon::LucideIcon;

use super::props::{TreeCheckState, TreeNode, TreeSelectionMode};

/// Tree 状态变更结果。
///
/// 渲染层根据这些标记决定是否触发外部回调和刷新界面。状态层不直接调用回调，
/// 从而保持纯逻辑可测试。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeStateOutcome {
    /// 展开集合是否变化。
    pub expanded_changed: bool,
    /// 选中集合是否变化。
    pub selected_changed: bool,
    /// 复选集合是否变化。
    pub checked_changed: bool,
    /// 过滤文本是否变化。
    pub filter_changed: bool,
    /// 键盘活动项是否变化。
    pub active_changed: bool,
}

impl TreeStateOutcome {
    /// 判断这次状态变更是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.expanded_changed
            || self.selected_changed
            || self.checked_changed
            || self.filter_changed
            || self.active_changed
    }
}

/// 扁平化后的节点记录。
///
/// 渲染和状态计算都使用该结构，避免在每次操作时递归扫描公开的嵌套节点。
#[derive(Clone, Debug)]
pub struct TreeNodeRecord {
    /// 节点稳定 key。
    pub key: SharedString,
    /// 节点展示文本。
    pub label: SharedString,
    /// 可选节点图标。
    pub icon: Option<LucideIcon>,
    /// 父节点 key，根节点为 `None`。
    pub parent: Option<SharedString>,
    /// 节点深度，根节点为 0。
    pub depth: usize,
    /// 节点是否禁用。
    pub disabled: bool,
    /// 节点是否可行选中。
    pub selectable: bool,
    /// 节点是否可复选。
    pub checkable: bool,
    /// 有效子节点 key。重复 key 子树会被过滤，因此这里只包含状态层接受的子节点。
    pub child_keys: Vec<SharedString>,
    /// 是否拥有有效子节点。
    pub has_children: bool,
}

/// Tree 节点索引。
///
/// `records` 保持 DFS 顺序，是所有对外 key 列表规范化和虚拟列表渲染的顺序来源；
/// `by_key` 只用于快速定位节点。
#[derive(Clone, Debug, Default)]
pub struct TreeIndex {
    /// DFS 顺序节点记录。
    pub records: Vec<TreeNodeRecord>,
    /// key 到 `records` 下标的映射。
    by_key: HashMap<SharedString, usize>,
}

impl TreeIndex {
    /// 从嵌套节点构建索引。
    pub fn new(nodes: &[TreeNode]) -> Self {
        let mut records = Vec::new();
        let mut seen = HashSet::new();

        for node in nodes {
            visit_node(node, None, 0, &mut seen, &mut records);
        }

        let by_key = records
            .iter()
            .enumerate()
            .map(|(index, record)| (record.key.clone(), index))
            .collect();

        Self { records, by_key }
    }

    /// 返回节点记录。
    pub fn get(&self, key: &SharedString) -> Option<&TreeNodeRecord> {
        self.by_key
            .get(key)
            .and_then(|index| self.records.get(*index))
    }

    /// 判断 key 是否属于有子节点的节点。
    pub fn has_children(&self, key: &SharedString) -> bool {
        self.get(key)
            .map(|record| record.has_children)
            .unwrap_or(false)
    }

    /// 判断节点在当前全局复选开关下是否可检查。
    fn is_checkable(&self, key: &SharedString, global_checkable: bool) -> bool {
        if !global_checkable {
            return false;
        }

        self.get(key)
            .map(|record| !record.disabled && record.checkable)
            .unwrap_or(false)
    }

    /// 返回指定节点下所有可检查子孙，包含节点自身。
    fn checkable_subtree_keys(
        &self,
        key: &SharedString,
        global_checkable: bool,
        output: &mut HashSet<SharedString>,
    ) {
        let Some(record) = self.get(key) else {
            return;
        };

        if self.is_checkable(key, global_checkable) {
            output.insert(key.clone());
        }

        for child_key in &record.child_keys {
            self.checkable_subtree_keys(child_key, global_checkable, output);
        }
    }
}

/// 可见行记录。
///
/// 该结构是状态层和渲染层之间的桥梁，已经包含渲染一行 Tree 所需的派生状态。
#[derive(Clone, Debug)]
pub struct VisibleTreeNode {
    /// 节点 key。
    pub key: SharedString,
    /// 节点展示文本。
    pub label: SharedString,
    /// 可选节点图标。
    pub icon: Option<LucideIcon>,
    /// 节点深度。
    pub depth: usize,
    /// 节点是否禁用。
    pub disabled: bool,
    /// 节点是否可选中。
    pub selectable: bool,
    /// 节点是否可复选。
    pub checkable: bool,
    /// 节点是否有子节点。
    pub has_children: bool,
    /// 节点当前是否展开。过滤态下祖先会临时展开，但不会写回 `expanded_keys`。
    pub expanded: bool,
    /// 节点复选框状态。
    pub check_state: TreeCheckState,
    /// 节点是否处于 selected 状态。
    pub selected: bool,
    /// 节点是否是键盘活动项。
    pub active: bool,
}

/// Tree 核心状态。
#[derive(Clone, Debug)]
pub struct TreeState {
    expanded_keys: Vec<SharedString>,
    selected_keys: Vec<SharedString>,
    checked_keys: Vec<SharedString>,
    filter_text: SharedString,
    active_key: Option<SharedString>,
    selection_anchor_key: Option<SharedString>,
}

impl TreeState {
    /// 创建新的 Tree 状态。
    pub fn new(
        expanded_keys: Vec<SharedString>,
        selected_keys: Vec<SharedString>,
        checked_keys: Vec<SharedString>,
        filter_text: SharedString,
        index: &TreeIndex,
        selection_mode: TreeSelectionMode,
        global_checkable: bool,
    ) -> Self {
        let mut state = Self {
            expanded_keys: dedupe_keys(expanded_keys),
            selected_keys: normalize_selected_keys(selected_keys, selection_mode),
            checked_keys: normalize_checked_keys(index, checked_keys, global_checkable).0,
            filter_text,
            active_key: None,
            selection_anchor_key: None,
        };
        state.sync_active_to_visible(index, global_checkable);
        state
    }

    /// 返回展开 key。
    pub fn expanded_keys(&self) -> &[SharedString] {
        &self.expanded_keys
    }

    /// 返回 selected key。
    pub fn selected_keys(&self) -> &[SharedString] {
        &self.selected_keys
    }

    /// 返回 checked key。
    pub fn checked_keys(&self) -> &[SharedString] {
        &self.checked_keys
    }

    /// 返回过滤文本。
    pub fn filter_text(&self) -> &SharedString {
        &self.filter_text
    }

    /// 返回当前活动节点 key。
    pub fn active_key(&self) -> Option<&SharedString> {
        self.active_key.as_ref()
    }

    /// 静默同步展开 key。
    pub fn set_expanded_keys_silent(
        &mut self,
        expanded_keys: Vec<SharedString>,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let expanded_keys = dedupe_keys(expanded_keys);
        let expanded_changed = self.expanded_keys != expanded_keys;
        self.expanded_keys = expanded_keys;
        let active_changed = self.sync_active_to_visible(index, global_checkable);

        TreeStateOutcome {
            expanded_changed,
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 静默同步 selected key。
    pub fn set_selected_keys_silent(
        &mut self,
        selected_keys: Vec<SharedString>,
        selection_mode: TreeSelectionMode,
    ) -> TreeStateOutcome {
        let selected_keys = normalize_selected_keys(selected_keys, selection_mode);
        let selected_changed = self.selected_keys != selected_keys;
        self.selected_keys = selected_keys;

        TreeStateOutcome {
            selected_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 静默同步 checked key。
    pub fn set_checked_keys_silent(
        &mut self,
        checked_keys: Vec<SharedString>,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let checked_keys = normalize_checked_keys(index, checked_keys, global_checkable).0;
        let checked_changed = self.checked_keys != checked_keys;
        self.checked_keys = checked_keys;

        TreeStateOutcome {
            checked_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 静默同步过滤文本。
    pub fn set_filter_text_silent(
        &mut self,
        filter_text: SharedString,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let filter_changed = self.filter_text != filter_text;
        self.filter_text = filter_text;
        let active_changed = self.sync_active_to_visible(index, global_checkable);

        TreeStateOutcome {
            filter_changed,
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 节点树变化后同步活动项。
    ///
    /// 该方法不会清空展开、选择或复选 key。不存在于新节点树中的 key 会自然不参与可见行和派生状态。
    pub fn sync_nodes_silent(
        &mut self,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let active_changed = self.sync_active_to_visible(index, global_checkable);
        TreeStateOutcome {
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 切换节点展开状态。
    pub fn toggle_expanded(
        &mut self,
        key: &SharedString,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        if !index.has_children(key) {
            return TreeStateOutcome::default();
        }

        let mut expanded = self.expanded_set();
        let expanded_changed = if expanded.contains(key) {
            expanded.remove(key)
        } else {
            expanded.insert(key.clone())
        };
        if expanded_changed {
            self.expanded_keys = ordered_existing_keys(index, &expanded)
                .into_iter()
                .chain(
                    self.expanded_keys
                        .iter()
                        .filter(|key| index.get(key).is_none())
                        .cloned(),
                )
                .collect();
        }

        let active_changed = self.sync_active_to_visible(index, global_checkable);
        TreeStateOutcome {
            expanded_changed,
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 选择指定节点。
    pub fn select_key(
        &mut self,
        key: &SharedString,
        visible: &[VisibleTreeNode],
        selection_mode: TreeSelectionMode,
        toggle: bool,
        range: bool,
    ) -> TreeStateOutcome {
        if !visible_node_enabled(visible, key) {
            return TreeStateOutcome::default();
        }

        let active_changed = self.active_key.as_ref() != Some(key);
        self.active_key = Some(key.clone());

        if selection_mode == TreeSelectionMode::None || !visible_node_selectable(visible, key) {
            return TreeStateOutcome {
                active_changed,
                ..TreeStateOutcome::default()
            };
        }

        let before = self.selected_keys.clone();
        match selection_mode {
            TreeSelectionMode::None => {}
            TreeSelectionMode::Single => {
                self.selected_keys = vec![key.clone()];
                self.selection_anchor_key = Some(key.clone());
            }
            TreeSelectionMode::Multiple if range => {
                let anchor = self
                    .selection_anchor_key
                    .as_ref()
                    .filter(|anchor| visible_node_selectable(visible, anchor))
                    .cloned()
                    .unwrap_or_else(|| key.clone());
                self.selected_keys = visible_range_keys(visible, &anchor, key);
            }
            TreeSelectionMode::Multiple if toggle => {
                let mut selected = self.selected_set();
                if selected.contains(key) {
                    selected.remove(key);
                } else {
                    selected.insert(key.clone());
                }
                self.selected_keys = visible_selected_order(visible, &selected);
                self.selection_anchor_key = Some(key.clone());
            }
            TreeSelectionMode::Multiple => {
                self.selected_keys = vec![key.clone()];
                self.selection_anchor_key = Some(key.clone());
            }
        }

        TreeStateOutcome {
            selected_changed: self.selected_keys != before,
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 清空 selected 状态。
    pub fn clear_selection(&mut self) -> TreeStateOutcome {
        let selected_changed = !self.selected_keys.is_empty();
        self.selected_keys.clear();
        self.selection_anchor_key = None;
        TreeStateOutcome {
            selected_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 切换指定节点复选状态。
    pub fn toggle_checked(
        &mut self,
        key: &SharedString,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        if !index.is_checkable(key, global_checkable) {
            return TreeStateOutcome::default();
        }

        let before = self.checked_keys.clone();
        let (current_checked, _) =
            normalize_checked_keys(index, self.checked_keys.clone(), global_checkable);
        let current_state = checked_state_for_key(index, &current_checked, key, global_checkable);
        let mut next = current_checked.into_iter().collect::<HashSet<_>>();
        let mut subtree = HashSet::new();
        index.checkable_subtree_keys(key, global_checkable, &mut subtree);

        if current_state == TreeCheckState::Checked {
            for subtree_key in subtree {
                next.remove(&subtree_key);
            }
        } else {
            next.extend(subtree);
        }

        self.checked_keys =
            normalize_checked_keys(index, next.into_iter().collect(), global_checkable).0;

        TreeStateOutcome {
            checked_changed: self.checked_keys != before,
            ..TreeStateOutcome::default()
        }
    }

    /// 清空 checked 状态。
    pub fn clear_checked(&mut self) -> TreeStateOutcome {
        let checked_changed = !self.checked_keys.is_empty();
        self.checked_keys.clear();
        TreeStateOutcome {
            checked_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 移动活动项。
    pub fn move_active_by(
        &mut self,
        delta: isize,
        visible: &[VisibleTreeNode],
    ) -> TreeStateOutcome {
        if visible.is_empty() {
            return TreeStateOutcome::default();
        }

        let Some(current) =
            Self::enabled_visible_index_after_move(visible, self.active_key.as_ref(), delta)
        else {
            return self.clear_active();
        };

        self.set_active_to_visible_index(current, visible)
    }

    /// 清空活动项。
    fn clear_active(&mut self) -> TreeStateOutcome {
        let active_changed = self.active_key.take().is_some();
        TreeStateOutcome {
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 根据方向键位移计算下一个可作为活动项的可见节点下标。
    ///
    /// disabled 节点虽然可以继续展示，但不能成为键盘操作目标，因此这里先收集非禁用行，
    /// 再在这个压缩后的序列中移动，避免方向键把 active 状态落到禁用节点上。
    fn enabled_visible_index_after_move(
        visible: &[VisibleTreeNode],
        active_key: Option<&SharedString>,
        delta: isize,
    ) -> Option<usize> {
        let enabled_indices = visible
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (!node.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled_indices.is_empty() {
            return None;
        }

        let current = active_key
            .and_then(|key| {
                enabled_indices
                    .iter()
                    .position(|visible_index| &visible[*visible_index].key == key)
            })
            .unwrap_or(0);
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            (current + delta as usize).min(enabled_indices.len() - 1)
        };

        enabled_indices.get(next).copied()
    }

    /// 移动活动项到首个可见节点。
    pub fn move_active_first(&mut self, visible: &[VisibleTreeNode]) -> TreeStateOutcome {
        let Some(index) = first_enabled_visible_index(visible) else {
            return self.clear_active();
        };
        self.set_active_to_visible_index(index, visible)
    }

    /// 移动活动项到最后一个可见节点。
    pub fn move_active_last(&mut self, visible: &[VisibleTreeNode]) -> TreeStateOutcome {
        let Some(index) = last_enabled_visible_index(visible) else {
            return self.clear_active();
        };
        self.set_active_to_visible_index(index, visible)
    }

    /// 执行 Left 键语义。
    pub fn collapse_or_parent(
        &mut self,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let Some(active) = self.active_key.clone() else {
            return TreeStateOutcome::default();
        };
        let Some(active_record) = index.get(&active) else {
            return TreeStateOutcome::default();
        };
        if active_record.disabled {
            return TreeStateOutcome::default();
        }

        if active_record.has_children && self.expanded_set().contains(&active) {
            return self.toggle_expanded(&active, index, global_checkable);
        }

        let Some(parent) = index.get(&active).and_then(|record| record.parent.clone()) else {
            return TreeStateOutcome::default();
        };
        if index
            .get(&parent)
            .map(|record| record.disabled)
            .unwrap_or(true)
        {
            return TreeStateOutcome::default();
        }
        let active_changed = self.active_key.as_ref() != Some(&parent);
        self.active_key = Some(parent);
        TreeStateOutcome {
            active_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 执行 Right 键语义。
    pub fn expand_or_child(
        &mut self,
        index: &TreeIndex,
        visible: &[VisibleTreeNode],
        global_checkable: bool,
    ) -> TreeStateOutcome {
        let Some(active) = self.active_key.clone() else {
            return TreeStateOutcome::default();
        };
        let Some(active_record) = index.get(&active) else {
            return TreeStateOutcome::default();
        };
        if active_record.disabled || !active_record.has_children {
            return TreeStateOutcome::default();
        }

        if !self.expanded_set().contains(&active) && self.filter_text.is_empty() {
            return self.toggle_expanded(&active, index, global_checkable);
        }

        let Some(active_position) = visible.iter().position(|node| node.key == active) else {
            return TreeStateOutcome::default();
        };
        let Some(next_child) = first_enabled_descendant_after(visible, active_position) else {
            return TreeStateOutcome::default();
        };
        self.set_active_to_visible_index(next_child, visible)
    }

    /// 选中所有可见可选节点。
    pub fn select_all_visible(
        &mut self,
        visible: &[VisibleTreeNode],
        selection_mode: TreeSelectionMode,
    ) -> TreeStateOutcome {
        if selection_mode != TreeSelectionMode::Multiple {
            return TreeStateOutcome::default();
        }

        let selected = visible
            .iter()
            .filter(|node| !node.disabled && node.selectable)
            .map(|node| node.key.clone())
            .collect::<Vec<_>>();
        let selected_changed = self.selected_keys != selected;
        self.selected_keys = selected;
        TreeStateOutcome {
            selected_changed,
            ..TreeStateOutcome::default()
        }
    }

    /// 返回当前半选 key。
    pub fn half_checked_keys(
        &self,
        index: &TreeIndex,
        global_checkable: bool,
    ) -> Vec<SharedString> {
        normalize_checked_keys(index, self.checked_keys.clone(), global_checkable).1
    }

    /// 返回当前可见节点。
    pub fn visible_nodes(&self, index: &TreeIndex, global_checkable: bool) -> Vec<VisibleTreeNode> {
        let checked_states = checked_states(index, &self.checked_keys, global_checkable);
        let selected = self.selected_set();
        let expanded = self.expanded_set();
        let filter = normalized_filter(self.filter_text.as_str());
        let included = if filter.is_empty() {
            HashSet::new()
        } else {
            included_by_filter(index, &filter)
        };
        let mut visible = Vec::new();

        for record in index
            .records
            .iter()
            .filter(|record| record.parent.is_none())
        {
            collect_visible(
                record,
                index,
                &expanded,
                &included,
                &filter,
                &checked_states,
                &selected,
                self.active_key.as_ref(),
                global_checkable,
                &mut visible,
            );
        }

        visible
    }

    /// 返回当前展开集合。
    fn expanded_set(&self) -> HashSet<SharedString> {
        self.expanded_keys.iter().cloned().collect()
    }

    /// 返回当前 selected 集合。
    fn selected_set(&self) -> HashSet<SharedString> {
        self.selected_keys.iter().cloned().collect()
    }

    /// 保证活动项仍处于可见节点内。
    fn sync_active_to_visible(&mut self, index: &TreeIndex, global_checkable: bool) -> bool {
        let visible = self.visible_nodes(index, global_checkable);
        if visible.is_empty() {
            let changed = self.active_key.take().is_some();
            return changed;
        }

        if self.active_key.as_ref().is_some_and(|key| {
            visible
                .iter()
                .any(|node| &node.key == key && !node.disabled)
        }) {
            return false;
        }

        let next_active = visible
            .iter()
            .find(|node| !node.disabled)
            .map(|node| node.key.clone());
        let changed = self.active_key != next_active;
        self.active_key = next_active;
        changed
    }

    /// 把活动项移动到可见节点下标。
    fn set_active_to_visible_index(
        &mut self,
        index: usize,
        visible: &[VisibleTreeNode],
    ) -> TreeStateOutcome {
        let Some(node) = visible.get(index) else {
            return TreeStateOutcome::default();
        };
        if node.disabled {
            return TreeStateOutcome::default();
        }
        let active_changed = self.active_key.as_ref() != Some(&node.key);
        self.active_key = Some(node.key.clone());
        TreeStateOutcome {
            active_changed,
            ..TreeStateOutcome::default()
        }
    }
}

/// 递归访问节点并写入 DFS 记录。
fn visit_node(
    node: &TreeNode,
    parent: Option<SharedString>,
    depth: usize,
    seen: &mut HashSet<SharedString>,
    records: &mut Vec<TreeNodeRecord>,
) -> Option<SharedString> {
    if !seen.insert(node.key.clone()) {
        return None;
    }

    let key = node.key.clone();
    let index = records.len();
    records.push(TreeNodeRecord {
        key: key.clone(),
        label: node.label.clone(),
        icon: node.icon,
        parent,
        depth,
        disabled: node.disabled,
        selectable: node.selectable,
        checkable: node.checkable,
        child_keys: Vec::new(),
        has_children: false,
    });

    let mut child_keys = Vec::new();
    for child in &node.children {
        if let Some(child_key) = visit_node(child, Some(key.clone()), depth + 1, seen, records) {
            child_keys.push(child_key);
        }
    }

    records[index].has_children = !child_keys.is_empty();
    records[index].child_keys = child_keys;
    Some(key)
}

/// 对 key 列表去重并保留首次出现顺序。
fn dedupe_keys(keys: Vec<SharedString>) -> Vec<SharedString> {
    let mut seen = HashSet::new();
    keys.into_iter()
        .filter(|key| seen.insert(key.clone()))
        .collect()
}

/// 按选择模式规范化 selected key。
fn normalize_selected_keys(
    keys: Vec<SharedString>,
    selection_mode: TreeSelectionMode,
) -> Vec<SharedString> {
    match selection_mode {
        TreeSelectionMode::None => Vec::new(),
        TreeSelectionMode::Single => keys.into_iter().next().into_iter().collect(),
        TreeSelectionMode::Multiple => dedupe_keys(keys),
    }
}

/// 按 DFS 顺序输出现有节点中的 key，并保留尾部未知 key。
fn ordered_existing_keys(index: &TreeIndex, keys: &HashSet<SharedString>) -> Vec<SharedString> {
    index
        .records
        .iter()
        .filter(|record| keys.contains(&record.key))
        .map(|record| record.key.clone())
        .collect()
}

/// 规范化过滤文本。
fn normalized_filter(filter: &str) -> String {
    filter.trim().to_lowercase()
}

/// 计算过滤态下应显示的节点 key。
fn included_by_filter(index: &TreeIndex, filter: &str) -> HashSet<SharedString> {
    let mut included = HashSet::new();
    for record in index.records.iter().rev() {
        let matched = record.label.as_str().to_lowercase().contains(filter);
        let child_included = record
            .child_keys
            .iter()
            .any(|child_key| included.contains(child_key));
        if matched || child_included {
            included.insert(record.key.clone());
        }
    }
    included
}

/// 递归收集可见节点。
#[allow(clippy::too_many_arguments)]
fn collect_visible(
    record: &TreeNodeRecord,
    index: &TreeIndex,
    expanded: &HashSet<SharedString>,
    included: &HashSet<SharedString>,
    filter: &str,
    checked_states: &HashMap<SharedString, TreeCheckState>,
    selected: &HashSet<SharedString>,
    active_key: Option<&SharedString>,
    global_checkable: bool,
    output: &mut Vec<VisibleTreeNode>,
) {
    let filtering = !filter.is_empty();
    if filtering && !included.contains(&record.key) {
        return;
    }

    let expanded_now = if filtering {
        record
            .child_keys
            .iter()
            .any(|child_key| included.contains(child_key))
    } else {
        expanded.contains(&record.key)
    };
    let checkable = global_checkable && !record.disabled && record.checkable;

    output.push(VisibleTreeNode {
        key: record.key.clone(),
        label: record.label.clone(),
        icon: record.icon,
        depth: record.depth,
        disabled: record.disabled,
        selectable: record.selectable,
        checkable,
        has_children: record.has_children,
        expanded: expanded_now,
        check_state: checked_states
            .get(&record.key)
            .copied()
            .unwrap_or(TreeCheckState::Unchecked),
        selected: selected.contains(&record.key),
        active: active_key == Some(&record.key),
    });

    if expanded_now {
        for child_key in &record.child_keys {
            if let Some(child) = index.get(child_key) {
                collect_visible(
                    child,
                    index,
                    expanded,
                    included,
                    filter,
                    checked_states,
                    selected,
                    active_key,
                    global_checkable,
                    output,
                );
            }
        }
    }
}

/// 判断可见节点是否可选中。
fn visible_node_selectable(visible: &[VisibleTreeNode], key: &SharedString) -> bool {
    visible
        .iter()
        .find(|node| &node.key == key)
        .map(|node| !node.disabled && node.selectable)
        .unwrap_or(false)
}

/// 判断 key 是否对应一个非禁用可见节点。
///
/// active 状态表示当前键盘操作目标，而禁用节点不应接收键盘操作；因此选择、移动和左右键逻辑
/// 都通过这个辅助函数过滤 disabled 节点。
fn visible_node_enabled(visible: &[VisibleTreeNode], key: &SharedString) -> bool {
    visible
        .iter()
        .find(|node| &node.key == key)
        .map(|node| !node.disabled)
        .unwrap_or(false)
}

/// 返回第一个可作为 active 的可见节点下标。
fn first_enabled_visible_index(visible: &[VisibleTreeNode]) -> Option<usize> {
    visible.iter().position(|node| !node.disabled)
}

/// 返回最后一个可作为 active 的可见节点下标。
fn last_enabled_visible_index(visible: &[VisibleTreeNode]) -> Option<usize> {
    visible.iter().rposition(|node| !node.disabled)
}

/// 返回当前节点之后的第一个非禁用可见子孙下标。
///
/// Right 键在已展开节点上应该移动到“第一个可操作子节点”。如果最近的可见子节点被禁用，
/// 直接把 active 移过去会让后续 Enter/Space/Left/Right 行为与禁用语义冲突，因此这里在当前
/// 子树范围内继续寻找第一个非禁用后代。
fn first_enabled_descendant_after(
    visible: &[VisibleTreeNode],
    active_position: usize,
) -> Option<usize> {
    let active_depth = visible.get(active_position)?.depth;
    visible
        .iter()
        .enumerate()
        .skip(active_position + 1)
        .take_while(|(_, node)| node.depth > active_depth)
        .find_map(|(index, node)| (!node.disabled).then_some(index))
}

/// 返回两个可见节点之间的可选 key。
fn visible_range_keys(
    visible: &[VisibleTreeNode],
    start_key: &SharedString,
    end_key: &SharedString,
) -> Vec<SharedString> {
    let Some(start) = visible.iter().position(|node| &node.key == start_key) else {
        return Vec::new();
    };
    let Some(end) = visible.iter().position(|node| &node.key == end_key) else {
        return Vec::new();
    };
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };

    visible[start..=end]
        .iter()
        .filter(|node| !node.disabled && node.selectable)
        .map(|node| node.key.clone())
        .collect()
}

/// 按可见顺序输出 selected key。
fn visible_selected_order(
    visible: &[VisibleTreeNode],
    selected: &HashSet<SharedString>,
) -> Vec<SharedString> {
    visible
        .iter()
        .filter(|node| selected.contains(&node.key) && !node.disabled && node.selectable)
        .map(|node| node.key.clone())
        .collect()
}

/// 根据 raw checked key 扩展到所有可检查子孙。
fn expanded_checked_set(
    index: &TreeIndex,
    keys: Vec<SharedString>,
    global_checkable: bool,
) -> HashSet<SharedString> {
    let mut checked = HashSet::new();
    for key in keys {
        if index.is_checkable(&key, global_checkable) {
            index.checkable_subtree_keys(&key, global_checkable, &mut checked);
        }
    }
    checked
}

/// 规范化 checked key，并返回半选 key。
fn normalize_checked_keys(
    index: &TreeIndex,
    keys: Vec<SharedString>,
    global_checkable: bool,
) -> (Vec<SharedString>, Vec<SharedString>) {
    let states = checked_states_from_set(
        index,
        &expanded_checked_set(index, keys, global_checkable),
        global_checkable,
    );
    let mut checked = Vec::new();
    let mut half_checked = Vec::new();

    for record in &index.records {
        if !index.is_checkable(&record.key, global_checkable) {
            continue;
        }
        match states
            .get(&record.key)
            .copied()
            .unwrap_or(TreeCheckState::Unchecked)
        {
            TreeCheckState::Checked => checked.push(record.key.clone()),
            TreeCheckState::Indeterminate => half_checked.push(record.key.clone()),
            TreeCheckState::Unchecked => {}
        }
    }

    (checked, half_checked)
}

/// 计算当前 checked key 对应的所有节点复选状态。
fn checked_states(
    index: &TreeIndex,
    checked_keys: &[SharedString],
    global_checkable: bool,
) -> HashMap<SharedString, TreeCheckState> {
    checked_states_from_set(
        index,
        &expanded_checked_set(index, checked_keys.to_vec(), global_checkable),
        global_checkable,
    )
}

/// 返回指定 key 的复选状态。
fn checked_state_for_key(
    index: &TreeIndex,
    checked_keys: &[SharedString],
    key: &SharedString,
    global_checkable: bool,
) -> TreeCheckState {
    checked_states(index, checked_keys, global_checkable)
        .get(key)
        .copied()
        .unwrap_or(TreeCheckState::Unchecked)
}

/// 从已展开的 checked 集合计算每个节点的复选状态。
fn checked_states_from_set(
    index: &TreeIndex,
    checked: &HashSet<SharedString>,
    global_checkable: bool,
) -> HashMap<SharedString, TreeCheckState> {
    let mut states = HashMap::new();
    for record in index
        .records
        .iter()
        .filter(|record| record.parent.is_none())
    {
        compute_check_state(record, index, checked, global_checkable, &mut states);
    }
    states
}

/// 递归计算复选状态。
///
/// 返回值中的 bool 表示该子树是否包含可检查节点。不可检查子树不会影响父节点半选状态。
fn compute_check_state(
    record: &TreeNodeRecord,
    index: &TreeIndex,
    checked: &HashSet<SharedString>,
    global_checkable: bool,
    states: &mut HashMap<SharedString, TreeCheckState>,
) -> (TreeCheckState, bool) {
    let mut child_states = Vec::new();
    for child_key in &record.child_keys {
        if let Some(child) = index.get(child_key) {
            let (state, relevant) =
                compute_check_state(child, index, checked, global_checkable, states);
            if relevant {
                child_states.push(state);
            }
        }
    }

    let self_checkable = index.is_checkable(&record.key, global_checkable);
    let state = if child_states.is_empty() {
        if self_checkable && checked.contains(&record.key) {
            TreeCheckState::Checked
        } else {
            TreeCheckState::Unchecked
        }
    } else if child_states
        .iter()
        .all(|state| *state == TreeCheckState::Checked)
    {
        TreeCheckState::Checked
    } else if child_states
        .iter()
        .all(|state| *state == TreeCheckState::Unchecked)
        && !(self_checkable && checked.contains(&record.key))
    {
        TreeCheckState::Unchecked
    } else {
        TreeCheckState::Indeterminate
    };

    states.insert(record.key.clone(), state);
    (state, self_checkable || !child_states.is_empty())
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    /// 创建状态层测试使用的简单树。
    fn sample_nodes() -> Vec<TreeNode> {
        vec![TreeNode::new("root", "Root").children(vec![
            TreeNode::new("a", "Alpha"),
            TreeNode::new("b", "Beta").children(vec![
                TreeNode::new("b1", "Beta 1"),
                TreeNode::new("b2", "Beta 2").disabled(true),
            ]),
        ])]
    }

    /// TreeIndex 应按 DFS 顺序扁平化节点并记录父子关系。
    #[test]
    fn index_flattens_nodes_in_dfs_order() {
        let index = TreeIndex::new(&sample_nodes());
        let keys = index
            .records
            .iter()
            .map(|record| record.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(keys, vec!["root", "a", "b", "b1", "b2"]);
        assert_eq!(index.get(&SharedString::from("b1")).unwrap().depth, 2);
        assert_eq!(
            index.get(&SharedString::from("b1")).unwrap().parent,
            Some(SharedString::from("b"))
        );
    }

    /// 重复 key 应只保留第一次出现的节点。
    #[test]
    fn duplicate_keys_keep_first_node() {
        let index = TreeIndex::new(&[
            TreeNode::new("same", "First"),
            TreeNode::new("same", "Second").children(vec![TreeNode::new("child", "Child")]),
        ]);

        assert_eq!(index.records.len(), 1);
        assert_eq!(index.records[0].label, SharedString::from("First"));
    }
}
