# xgpui

`xgpui` 是一个基于 `gpui` 的 Rust 基础 UI 组件库，目标是提供结构清晰、状态可同步、可组合并支持明暗皮肤的桌面 UI 组件。

当前版本处于早期建设阶段，已经提供基础主题能力、单行文本输入组件 `TextInput` 和单选下拉框组件 `Select`。

## 特性

- 基于 `gpui 0.2.2` 和稳定 Rust 2021 edition。
- 提供 `xgpui::install(cx)` 统一安装主题状态和默认键盘绑定。
- 提供亮色、暗色两套基础主题，并可在运行时切换。
- 提供 `TextInput` 单行输入组件，支持 IME、选区、复制、剪切、粘贴、拖拽选中、清除、只读、禁用、状态样式和前后缀插槽。
- 提供 `Select` 单选下拉组件，支持本地搜索、键盘导航、锚定下拉面板、清除、禁用、状态样式、helper text、外部同步和明暗皮肤。
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

应用启动时需要调用 `xgpui::install(cx)`。该函数会保证主题状态存在，并幂等注册 `TextInput` 和 `Select` 的默认键盘动作。

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

## TextInput

`TextInput` 是单行文本输入组件，适合表单、搜索框和设置项。组件内部维护完整编辑状态，同时提供 `set_value`、`clear`、`select_all` 和 `move_to_end` 等公开方法，便于外部同步。

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

## 示例

运行 `TextInput` 示例：

```bash
cargo run --example text_input
```

运行 `Select` 示例：

```bash
cargo run --example select
```

两个示例都包含亮色和暗色皮肤切换，用于验证组件在不同主题下的表现。

## 文档

静态 HTML 使用文档位于：

```text
docs/index.html
```

该文档包含组件用法、props 说明、公开方法、键盘行为、主题能力和当前边界。

## 开发检查

提交前建议运行：

```bash
cargo fmt --check
cargo test
cargo check --examples
```

这些命令分别检查代码格式、单元测试和示例编译状态。

## 当前边界

- `TextInput` 目前只实现单行输入，不包含 password、number 或 textarea。
- `Select` 目前只实现单选和本地搜索，不包含多选、远程加载、分组选项、虚拟列表或自定义 option 渲染。
- 项目优先使用稳定 Rust，不引入 nightly 特性。
- 新增或扩展组件时需要同步支持亮色和暗色皮肤。
