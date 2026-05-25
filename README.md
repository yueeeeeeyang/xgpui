# xgpui

`xgpui` 是一个基于 `gpui` 的 Rust 基础 UI 组件库，目标是提供结构清晰、状态可同步、可组合并支持明暗皮肤的桌面 UI 组件。

当前版本处于早期建设阶段，已经提供基础主题能力、按钮组件 `Button`、单行文本输入组件 `TextInput`、多行文本输入组件 `Textarea`、单选下拉框组件 `Select` 和标准树组件 `Tree`。

## 特性

- 基于 `gpui 0.2.2` 和稳定 Rust 2021 edition。
- 提供 `xgpui::install(cx)` 统一安装主题状态、Lucide 图标字体和默认键盘绑定。
- 提供亮色、暗色两套基础主题，并可在运行时切换。
- 内置 Lucide 图标字体，组件内置图标使用成熟图标集而非临时手绘路径。
- 提供 `Button` 按钮组件，支持主按钮、次级按钮、描边按钮、幽灵按钮、链接按钮、危险色调、尺寸、禁用、加载、块级宽度、前后图标、纯图标按钮和键盘触发。
- 提供 `TextInput` 单行输入组件，支持普通文本、密码、数字、IME、选区、复制、剪切、粘贴、拖拽选中、清除、只读、禁用、状态样式和前后缀插槽。
- 提供 `Textarea` 多行输入组件，支持换行、软换行、IME、选区、复制、剪切、粘贴、拖拽选中、只读、禁用、最大长度、行数控制、内部滚动条、状态样式和 helper text。
- 提供 `Select` 单选下拉组件，支持本地搜索、键盘导航、锚定下拉面板、清除、禁用、状态样式、helper text、外部同步和明暗皮肤。
- 提供 `Tree` 标准树组件，支持展开折叠、单选/多选、级联复选半选、过滤、键盘导航、虚拟列表、禁用、状态样式、helper text 和外部同步。
- 通过 `xgpui::prelude::*` 重导出常用组件和配置类型。

## 安装

当前项目还未发布到 crates.io。可以先通过 git 依赖方式使用：

```toml
[dependencies]
xgpui = { git = "git@github.com:yueeeeeeyang/xgpui.git" }
gpui = "0.2.2"
```

如果在同一工作目录中开发，也可以使用 path 依赖：

```toml
[dependencies]
xgpui = { path = "../xgpui" }
gpui = "0.2.2"
```

## 初始化

应用启动时需要调用 `xgpui::install(cx)`。该函数会保证主题状态存在，安装 Lucide 图标字体，并幂等注册 `TextInput`、`Textarea`、`Select` 和 `Tree` 的默认键盘动作。

```rust
use gpui::Application;

fn main() {
    Application::new().run(|cx| {
        // 安装 xgpui 的应用级能力，包括主题状态和默认键盘绑定。
        xgpui::install(cx);
    });
}
```

## 主题

`xgpui` 内置亮色和暗色皮肤。组件样式会从 `foundation::theme` 读取 token，避免在组件内部硬编码单一皮肤颜色。

```rust
use xgpui::prelude::*;

// 切换到亮色皮肤。
set_theme_mode(cx, ThemeMode::Light);

// 切换到暗色皮肤。
set_theme_mode(cx, ThemeMode::Dark);
```

## Button

`Button` 是基础按钮组件，适合表单提交、工具栏操作和危险操作。组件以 `Entity` 形式创建，内部处理鼠标点击、`Enter` 和 `Space` 键盘触发；`disabled` 和 `loading` 状态不会触发 `on_click`，其中 `loading` 会展示圆圈加载动画。

```rust
use gpui::{Context, Entity};
use xgpui::prelude::*;

/// 示例父视图持有按钮实体，便于后续通过公开方法同步 loading 或 disabled 状态。
struct Toolbar {
    save: Entity<Button>,
}

impl Toolbar {
    /// 创建一个带 Lucide 前置图标的主按钮。
    fn new(cx: &mut Context<Self>) -> Self {
        let save = cx.new(|cx| {
            Button::new(
                cx,
                ButtonProps::default()
                    .label("保存")
                    .leading_icon(LucideIcon::Save)
                    .on_click(|| {
                        // 执行保存动作。
                    }),
            )
        });

        Self { save }
    }
}
```

常用配置包括 `ButtonVariant::{Primary, Secondary, Outline, Ghost, Link}`、`ButtonTone::{Default, Danger}`、`ButtonSize::{Small, Medium, Large}`、`block`、`icon_only`、`leading_icon`、`trailing_icon` 和 `tooltip`。纯图标按钮会隐藏 `label`，但会把 `label` 作为 tooltip 的默认文案。

## TextInput

`TextInput` 是单行文本输入组件，适合表单、密码、金额和设置项。组件内部维护完整编辑状态，同时提供 `set_value`、`clear`、`select_all` 和 `move_to_end` 等公开方法，便于外部同步。

```rust
use gpui::{Context, Entity, SharedString};
use xgpui::prelude::*;

/// 示例父视图持有输入框实体，便于在事件中通过实体方法同步值。
struct FormView {
    username: Entity<TextInput>,
}

impl FormView {
    /// 创建一个可清除并带 helper text 的输入框。
    fn new(cx: &mut Context<Self>) -> Self {
        let username = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .placeholder("请输入用户名")
                    .clearable(true)
                    .helper_text(Some(SharedString::from("支持中文输入法、复制、粘贴和拖选"))),
            )
        });

        Self { username }
    }
}
```

`TextInputType` 可配置输入类型。密码类型默认显示掩码，并提供眼睛图标切换可见性；数字类型仍保存为字符串，只限制输入形态，不做数值解析或格式化。

```rust
use gpui::{Context, Entity};
use xgpui::prelude::*;

/// 示例父视图持有密码和数字输入框实体。
struct AccountView {
    password: Entity<TextInput>,
    amount: Entity<TextInput>,
}

impl AccountView {
    /// 创建密码输入框和数字输入框。
    fn new(cx: &mut Context<Self>) -> Self {
        let password = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .input_type(TextInputType::Password)
                    .placeholder("请输入密码")
                    .clearable(true),
            )
        });

        let amount = cx.new(|cx| {
            TextInput::new(
                cx,
                TextInputProps::default()
                    .input_type(TextInputType::Number)
                    .placeholder("请输入金额"),
            )
        });

        Self { password, amount }
    }
}
```

## Textarea

`Textarea` 是标准多行文本输入组件，适合备注、描述、正文片段和需要保留换行的表单字段。组件内部维护多行编辑状态，同时提供 `set_value`、`set_disabled`、`set_readonly`、`set_status`、`set_helper_text`、`clear` 和 `select_all` 等公开方法，便于父组件做受控同步。

```rust
use gpui::{Context, Entity, SharedString};
use xgpui::prelude::*;

/// 示例父视图持有 textarea 实体，便于在事件中同步状态。
struct FeedbackView {
    comment: Entity<Textarea>,
}

impl FeedbackView {
    /// 创建一个带最大长度和 helper text 的多行输入框。
    fn new(cx: &mut Context<Self>) -> Self {
        let comment = cx.new(|cx| {
            Textarea::new(
                cx,
                TextareaProps::default()
                    .placeholder("请输入反馈内容")
                    .rows(4)
                    .max_rows(Some(8))
                    .max_length(Some(500))
                    .helper_text(Some(SharedString::from("支持换行、中文输入法和内部滚动"))),
            )
        });

        Self { comment }
    }
}
```

`Enter` 和 `Shift+Enter` 会插入换行；`Cmd+Enter` / `Ctrl+Enter` 触发 `on_submit`，不插入换行。`max_length` 按 Unicode 字素簇计数，emoji、组合字符和换行都按用户可感知字符处理。

## Select

`Select` 是单选下拉框组件，适合在有限选项中选择一个值。第一版只支持单选和本地文本搜索，不包含多选、远程加载、分组选项或自定义 option 渲染。

```rust
use gpui::{Context, Entity, SharedString};
use xgpui::prelude::*;

/// 示例父视图持有 Select 实体，便于通过 set_value 从外部同步当前值。
struct SettingsView {
    city: Entity<Select>,
}

impl SettingsView {
    /// 创建一个支持搜索、清除和 helper text 的城市选择框。
    fn new(cx: &mut Context<Self>) -> Self {
        let city = cx.new(|cx| {
            Select::new(
                cx,
                SelectProps::default()
                    .placeholder("请选择城市")
                    .searchable(true)
                    .clearable(true)
                    .helper_text(Some(SharedString::from("打开后可直接输入关键词过滤")))
                    .options(vec![
                        SelectOption::new("beijing", "北京"),
                        SelectOption::new("shanghai", "上海"),
                        SelectOption::new("shenzhen", "深圳"),
                    ]),
            )
        });

        Self { city }
    }
}
```

## Tree

`Tree` 是标准树组件，适合文件树、权限树、导航树和分层资源选择。组件内部维护展开、选中、复选、过滤和键盘活动项，同时提供 `set_nodes`、`set_expanded_keys`、`set_selected_keys`、`set_checked_keys`、`set_filter_text`、`set_disabled`、`set_status` 和 `set_helper_text` 等受控同步方法。

```rust
use gpui::{Context, Entity, SharedString};
use xgpui::prelude::*;

/// 示例父视图持有 Tree 实体，便于从外部同步选择状态。
struct PermissionView {
    tree: Entity<Tree>,
}

impl PermissionView {
    /// 创建一个级联复选权限树。
    fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| {
            Tree::new(
                cx,
                TreeProps::default()
                    .nodes(vec![TreeNode::new("admin", "后台管理").children(vec![
                        TreeNode::new("user.read", "查看用户"),
                        TreeNode::new("user.write", "编辑用户"),
                    ])])
                    .expanded_keys(vec![SharedString::from("admin")])
                    .checkable(true)
                    .helper_text(Some(SharedString::from("父子节点会自动计算 checked 和半选状态"))),
            )
        });

        Self { tree }
    }
}
```

Tree 的 selected 状态和 checked 状态互相独立。过滤使用 `filter_text`，默认按 label 大小写不敏感匹配并展示祖先路径；第一版不内置搜索输入框，可配合 `TextInput` 使用。

## 示例

运行 `Button` 示例：

```bash
cargo run --example button
```

运行 `TextInput` 示例：

```bash
cargo run --example text_input
```

运行 `Textarea` 示例：

```bash
cargo run --example textarea
```

运行 `Select` 示例：

```bash
cargo run --example select
```

运行 `Tree` 示例：

```bash
cargo run --example tree
```

组件示例都包含亮色和暗色皮肤切换，用于验证组件在不同主题下的表现。

## 文档

静态 HTML 使用文档位于 `docs` 目录，并已按组件拆分：

```text
docs/index.html
docs/button.html
docs/text_input.html
docs/textarea.html
docs/select.html
docs/tree.html
```

`docs/index.html` 保留项目接入、主题、示例入口和实现边界；组件级用法、props、公开方法和键盘行为分别维护在对应组件页面中。

## 开发检查

提交前建议运行：

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo check --examples
```

这些命令分别检查代码格式、单元测试、Clippy 警告和示例编译状态。

## 当前边界

- `TextInput` 目前只实现单行输入，包含普通文本、密码和数字类型，不包含 email 或 search。
- `Textarea` 目前实现标准多行文本输入，不包含右下角拖拽 resize、密码/数字类型或前后缀 slot。
- `Select` 目前只实现单选和本地搜索，不包含多选、远程加载、分组选项或自定义 option 渲染。
- `Tree` 目前实现标准树能力，不包含拖拽排序、异步加载、远程搜索、右键菜单、节点编辑或自定义节点渲染。
- `Button` 目前实现单按钮能力，不包含按钮组、异步状态管理或任意元素插槽。
- 项目优先使用稳定 Rust，不引入 nightly 特性。
- 新增或扩展组件时需要同步支持亮色和暗色皮肤。
