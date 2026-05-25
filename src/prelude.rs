//! 常用类型的预导入模块。
//!
//! 用户可以通过 `use xgpui::prelude::*;` 引入最常用的组件和配置类型，
//! 避免在业务代码中反复书写较长的模块路径。

pub use crate::components::button::{Button, ButtonProps, ButtonSize, ButtonTone, ButtonVariant};
pub use crate::components::select::{
    Select, SelectOption, SelectProps, SelectSize, SelectStatus, SelectVariant,
};
pub use crate::components::text_input::{
    TextInput, TextInputProps, TextInputSize, TextInputSlot, TextInputStatus, TextInputType,
    TextInputVariant,
};
pub use crate::components::textarea::{
    Textarea, TextareaProps, TextareaSize, TextareaStatus, TextareaVariant,
};
pub use crate::foundation::icon::LucideIcon;
pub use crate::{set_theme_mode, theme_mode, ThemeMode};
