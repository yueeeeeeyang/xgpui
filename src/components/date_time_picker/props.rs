//! `DateTimePicker` 的公开参数、值类型和回调定义。
//!
//! 本模块只承载对外 API，不包含渲染细节。日期时间值使用 `chrono` 的 naive 类型，明确表示
//! 组件只处理本地表单输入，不承担时区、UTC 或夏令时转换。

use std::rc::Rc;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use gpui::{Keystroke, Pixels, SharedString};

/// 日期禁用判断器。
///
/// 返回 `true` 表示该日期不能被选择或通过手动输入提交。范围模式会分别校验 start 和 end。
pub type DateDisabledPredicate = Rc<dyn Fn(NaiveDate) -> bool>;

/// 时间禁用判断器。
///
/// 返回 `true` 表示该时间不能被选择或通过手动输入提交。DateTime 模式会同时检查日期和时间。
pub type TimeDisabledPredicate = Rc<dyn Fn(NaiveTime) -> bool>;

/// 值变化回调。
///
/// `None` 表示用户清空选择；受控同步方法不会触发该回调。
pub type DateTimePickerChangeHandler = Box<dyn FnMut(Option<DateTimePickerValue>)>;

/// 弹层打开状态变化回调。
pub type DateTimePickerOpenChangeHandler = Box<dyn FnMut(bool)>;

/// 聚焦或失焦回调。
pub type DateTimePickerFocusHandler = Box<dyn FnMut()>;

/// 键盘按下回调。
pub type DateTimePickerKeyDownHandler = Box<dyn FnMut(Keystroke)>;

/// 手动输入解析失败回调。
///
/// 参数是当前输入文本。错误文案由组件内部根据 `parse_error_text` 展示，业务层如果需要日志或额外
/// 提示，可以通过该回调拿到原始输入。
pub type DateTimePickerParseErrorHandler = Box<dyn FnMut(SharedString)>;

/// 日期时间范围值。
///
/// 结构在创建和提交时都会规范化为 `start <= end`。TimeRange 不表达跨天语义，只比较同一天内时间。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DateTimeRange<T> {
    /// 范围起点。
    pub start: T,
    /// 范围终点。
    pub end: T,
}

impl<T: Ord> DateTimeRange<T> {
    /// 创建一个有序范围。
    ///
    /// 如果调用方传入反向范围，组件会交换起止值，避免渲染和回调里出现不稳定顺序。
    pub fn new(start: T, end: T) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }
}

/// DateTimePicker 支持的选择模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimePickerMode {
    /// 单日期选择。
    Date,
    /// 单时间选择。
    Time,
    /// 日期时间选择。
    DateTime,
    /// 日期范围选择。
    DateRange,
    /// 时间范围选择。
    TimeRange,
    /// 日期时间范围选择。
    DateTimeRange,
}

impl Default for DateTimePickerMode {
    /// 默认使用日期模式，符合常见表单里最轻量的日期选择入口。
    fn default() -> Self {
        Self::Date
    }
}

/// DateTimePicker 对外值。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DateTimePickerValue {
    /// 单日期值。
    Date(NaiveDate),
    /// 单时间值。
    Time(NaiveTime),
    /// 单日期时间值。
    DateTime(NaiveDateTime),
    /// 日期范围值。
    DateRange(DateTimeRange<NaiveDate>),
    /// 时间范围值。
    TimeRange(DateTimeRange<NaiveTime>),
    /// 日期时间范围值。
    DateTimeRange(DateTimeRange<NaiveDateTime>),
}

impl DateTimePickerValue {
    /// 返回该值所属模式。
    pub fn mode(&self) -> DateTimePickerMode {
        match self {
            Self::Date(_) => DateTimePickerMode::Date,
            Self::Time(_) => DateTimePickerMode::Time,
            Self::DateTime(_) => DateTimePickerMode::DateTime,
            Self::DateRange(_) => DateTimePickerMode::DateRange,
            Self::TimeRange(_) => DateTimePickerMode::TimeRange,
            Self::DateTimeRange(_) => DateTimePickerMode::DateTimeRange,
        }
    }

    /// 判断值是否和指定模式匹配。
    pub(crate) fn matches_mode(&self, mode: DateTimePickerMode) -> bool {
        self.mode() == mode
    }
}

/// DateTimePicker 尺寸。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimePickerSize {
    /// 小尺寸，适合表格过滤和紧凑工具栏。
    Small,
    /// 默认尺寸。
    Medium,
    /// 大尺寸，适合强调型表单区域。
    Large,
}

impl Default for DateTimePickerSize {
    /// 返回默认尺寸。
    fn default() -> Self {
        Self::Medium
    }
}

/// DateTimePicker 视觉变体。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimePickerVariant {
    /// 标准描边输入框。
    Outlined,
    /// 浅色填充输入框。
    Filled,
    /// 弱边界输入框。
    Ghost,
}

impl Default for DateTimePickerVariant {
    /// 返回默认视觉变体。
    fn default() -> Self {
        Self::Outlined
    }
}

/// DateTimePicker 语义状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimePickerStatus {
    /// 默认状态。
    Default,
    /// 错误状态。
    Error,
    /// 警告状态。
    Warning,
    /// 成功状态。
    Success,
}

impl Default for DateTimePickerStatus {
    /// 返回默认状态。
    fn default() -> Self {
        Self::Default
    }
}

/// DateTimePicker 创建参数。
///
/// 参数结构包含值、格式、约束、弹层行为和回调。组件创建后可通过 `set_*` 方法受控同步；
/// 同步方法不触发外部交互回调。
pub struct DateTimePickerProps {
    /// 当前选择模式。
    pub mode: DateTimePickerMode,
    /// 初始值。模式不匹配时会被静默规范化为空。
    pub value: Option<DateTimePickerValue>,
    /// 空值时显示的占位文本。
    pub placeholder: SharedString,
    /// 禁用状态。禁用后不能聚焦、输入、打开、清空或选择。
    pub disabled: bool,
    /// 只读状态。只读允许聚焦、复制和打开弹层查看，但不能修改值。
    pub readonly: bool,
    /// 是否标记为必填。
    pub required: bool,
    /// 是否显示清除按钮。
    pub clearable: bool,
    /// 最小可提交值。只有与当前模式同族的值参与比较，其他模式会被忽略。
    pub min: Option<DateTimePickerValue>,
    /// 最大可提交值。只有与当前模式同族的值参与比较，其他模式会被忽略。
    pub max: Option<DateTimePickerValue>,
    /// 禁用日期判断器。
    pub disabled_date: Option<DateDisabledPredicate>,
    /// 禁用时间判断器。
    pub disabled_time: Option<TimeDisabledPredicate>,
    /// 日期格式。
    pub date_format: SharedString,
    /// 时间格式。
    pub time_format: SharedString,
    /// 日期时间格式。
    pub datetime_format: SharedString,
    /// 范围输入中的起止分隔符。
    pub range_separator: SharedString,
    /// 分钟步长。
    pub minute_step: usize,
    /// 秒步长。
    pub second_step: usize,
    /// 尺寸。
    pub size: DateTimePickerSize,
    /// 视觉变体。
    pub variant: DateTimePickerVariant,
    /// 语义状态。
    pub status: DateTimePickerStatus,
    /// 辅助文本。
    pub helper_text: Option<SharedString>,
    /// 解析失败展示文案。
    pub parse_error_text: SharedString,
    /// 弹层最大高度。
    pub max_popup_height: Pixels,
    /// 值变化回调。
    pub on_change: Option<DateTimePickerChangeHandler>,
    /// 打开状态变化回调。
    pub on_open_change: Option<DateTimePickerOpenChangeHandler>,
    /// 聚焦回调。
    pub on_focus: Option<DateTimePickerFocusHandler>,
    /// 失焦回调。
    pub on_blur: Option<DateTimePickerFocusHandler>,
    /// 键盘按下回调。
    pub on_key_down: Option<DateTimePickerKeyDownHandler>,
    /// 手动输入解析失败回调。
    pub on_parse_error: Option<DateTimePickerParseErrorHandler>,
}

impl Default for DateTimePickerProps {
    /// 返回默认参数。
    fn default() -> Self {
        Self {
            mode: DateTimePickerMode::default(),
            value: None,
            placeholder: SharedString::from("请选择日期"),
            disabled: false,
            readonly: false,
            required: false,
            clearable: true,
            min: None,
            max: None,
            disabled_date: None,
            disabled_time: None,
            date_format: SharedString::from("%Y-%m-%d"),
            time_format: SharedString::from("%H:%M:%S"),
            datetime_format: SharedString::from("%Y-%m-%d %H:%M:%S"),
            range_separator: SharedString::from(" ~ "),
            minute_step: 1,
            second_step: 1,
            size: DateTimePickerSize::default(),
            variant: DateTimePickerVariant::default(),
            status: DateTimePickerStatus::default(),
            helper_text: None,
            parse_error_text: SharedString::from("日期时间格式不正确"),
            // 双月日期范围面板需要同时容纳分段切换、日历网格和底部确认栏。默认高度
            // 稍高于普通 Select 弹层，避免范围选择时 footer 被裁掉；调用方仍可按场景覆盖。
            max_popup_height: gpui::px(440.0),
            on_change: None,
            on_open_change: None,
            on_focus: None,
            on_blur: None,
            on_key_down: None,
            on_parse_error: None,
        }
    }
}

impl DateTimePickerProps {
    /// 设置选择模式。
    pub fn mode(mut self, mode: DateTimePickerMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置初始值。
    pub fn value(mut self, value: impl Into<Option<DateTimePickerValue>>) -> Self {
        self.value = value.into();
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置只读状态。
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// 设置必填语义。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置是否可清除。
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    /// 设置最小可提交值。
    pub fn min(mut self, min: impl Into<Option<DateTimePickerValue>>) -> Self {
        self.min = min.into();
        self
    }

    /// 设置最大可提交值。
    pub fn max(mut self, max: impl Into<Option<DateTimePickerValue>>) -> Self {
        self.max = max.into();
        self
    }

    /// 设置禁用日期判断器。
    pub fn disabled_date(mut self, predicate: impl Fn(NaiveDate) -> bool + 'static) -> Self {
        self.disabled_date = Some(Rc::new(predicate));
        self
    }

    /// 设置禁用时间判断器。
    pub fn disabled_time(mut self, predicate: impl Fn(NaiveTime) -> bool + 'static) -> Self {
        self.disabled_time = Some(Rc::new(predicate));
        self
    }

    /// 设置日期格式。
    pub fn date_format(mut self, format: impl Into<SharedString>) -> Self {
        self.date_format = format.into();
        self
    }

    /// 设置时间格式。
    pub fn time_format(mut self, format: impl Into<SharedString>) -> Self {
        self.time_format = format.into();
        self
    }

    /// 设置日期时间格式。
    pub fn datetime_format(mut self, format: impl Into<SharedString>) -> Self {
        self.datetime_format = format.into();
        self
    }

    /// 设置范围分隔符。
    pub fn range_separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.range_separator = separator.into();
        self
    }

    /// 设置分钟步长。
    pub fn minute_step(mut self, step: usize) -> Self {
        self.minute_step = step;
        self
    }

    /// 设置秒步长。
    pub fn second_step(mut self, step: usize) -> Self {
        self.second_step = step;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: DateTimePickerSize) -> Self {
        self.size = size;
        self
    }

    /// 设置视觉变体。
    pub fn variant(mut self, variant: DateTimePickerVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置语义状态。
    pub fn status(mut self, status: DateTimePickerStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置辅助文本。
    pub fn helper_text(mut self, helper_text: impl Into<Option<SharedString>>) -> Self {
        self.helper_text = helper_text.into();
        self
    }

    /// 设置解析错误文案。
    pub fn parse_error_text(mut self, parse_error_text: impl Into<SharedString>) -> Self {
        self.parse_error_text = parse_error_text.into();
        self
    }

    /// 设置弹层最大高度。
    pub fn max_popup_height(mut self, max_popup_height: Pixels) -> Self {
        self.max_popup_height = max_popup_height;
        self
    }

    /// 设置值变化回调。
    pub fn on_change(mut self, handler: impl FnMut(Option<DateTimePickerValue>) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// 设置打开状态变化回调。
    pub fn on_open_change(mut self, handler: impl FnMut(bool) + 'static) -> Self {
        self.on_open_change = Some(Box::new(handler));
        self
    }

    /// 设置聚焦回调。
    pub fn on_focus(mut self, handler: impl FnMut() + 'static) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }

    /// 设置失焦回调。
    pub fn on_blur(mut self, handler: impl FnMut() + 'static) -> Self {
        self.on_blur = Some(Box::new(handler));
        self
    }

    /// 设置键盘回调。
    pub fn on_key_down(mut self, handler: impl FnMut(Keystroke) + 'static) -> Self {
        self.on_key_down = Some(Box::new(handler));
        self
    }

    /// 设置解析失败回调。
    pub fn on_parse_error(mut self, handler: impl FnMut(SharedString) + 'static) -> Self {
        self.on_parse_error = Some(Box::new(handler));
        self
    }
}
