//! `DateTimePicker` 的纯状态管理。
//!
//! 本模块只处理日期时间解析、格式化、范围规范化、草稿值和约束校验，不依赖 gpui 渲染。
//! 这样复杂边界可以通过普通单元测试覆盖，渲染层只负责把状态映射成可点击元素。

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gpui::SharedString;

use super::props::{
    DateDisabledPredicate, DateTimePickerMode, DateTimePickerValue, DateTimeRange,
    TimeDisabledPredicate,
};

/// DateTimePicker 状态变更结果。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DateTimePickerStateOutcome {
    /// 对外值是否变化。
    pub value_changed: bool,
    /// 输入文本是否变化。
    pub input_changed: bool,
    /// 打开状态是否变化。
    pub open_changed: bool,
    /// 选择模式是否变化。
    pub mode_changed: bool,
    /// 草稿值是否变化。
    pub draft_changed: bool,
    /// 面板年月是否变化。
    pub view_changed: bool,
    /// 键盘活动日期是否变化。
    pub active_date_changed: bool,
    /// 解析错误展示状态是否变化。
    pub parse_error_changed: bool,
}

impl DateTimePickerStateOutcome {
    /// 判断本次状态变更是否需要刷新界面。
    pub fn should_notify(self) -> bool {
        self.value_changed
            || self.input_changed
            || self.open_changed
            || self.mode_changed
            || self.draft_changed
            || self.view_changed
            || self.active_date_changed
            || self.parse_error_changed
    }

    /// 合并多个连续状态变更结果。
    pub fn merge(self, other: Self) -> Self {
        Self {
            value_changed: self.value_changed || other.value_changed,
            input_changed: self.input_changed || other.input_changed,
            open_changed: self.open_changed || other.open_changed,
            mode_changed: self.mode_changed || other.mode_changed,
            draft_changed: self.draft_changed || other.draft_changed,
            view_changed: self.view_changed || other.view_changed,
            active_date_changed: self.active_date_changed || other.active_date_changed,
            parse_error_changed: self.parse_error_changed || other.parse_error_changed,
        }
    }
}

/// 日期时间格式配置。
///
/// 格式同时用于展示和手动输入解析。范围模式会把同一格式应用到 start/end 两端。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DateTimePickerFormats {
    /// 日期格式。
    pub date_format: SharedString,
    /// 时间格式。
    pub time_format: SharedString,
    /// 日期时间格式。
    pub datetime_format: SharedString,
    /// 范围分隔符。
    pub range_separator: SharedString,
}

/// 提交约束配置。
///
/// 约束只在解析、点击提交和确认草稿时生效。渲染层也会使用同一规则禁用不可选单元格，
/// 避免 UI 状态和最终提交校验不一致。
#[derive(Clone)]
pub struct DateTimePickerConstraints {
    /// 最小值。
    pub min: Option<DateTimePickerValue>,
    /// 最大值。
    pub max: Option<DateTimePickerValue>,
    /// 禁用日期规则。
    pub disabled_date: Option<DateDisabledPredicate>,
    /// 禁用时间规则。
    pub disabled_time: Option<TimeDisabledPredicate>,
}

/// 日期时间解析失败原因。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DateTimePickerParseError {
    /// 文本无法按当前格式解析。
    InvalidFormat,
    /// 文本能解析，但被 min/max 或禁用规则拦截。
    DisabledValue,
}

/// 草稿值。
///
/// RangeStart 表示用户已经点了范围第一端，但还没有点第二端；该状态不会暴露到对外 value。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DateTimePickerDraft {
    /// 没有草稿值。
    Empty,
    /// 完整草稿值，可在确认时提交。
    Value(DateTimePickerValue),
    /// 日期范围半选。
    DateRangeStart(NaiveDate),
    /// 时间范围半选。
    TimeRangeStart(NaiveTime),
    /// 日期时间范围半选。
    DateTimeRangeStart(NaiveDateTime),
}

impl DateTimePickerDraft {
    /// 从当前对外 value 创建草稿。
    fn from_value(value: &Option<DateTimePickerValue>) -> Self {
        value
            .clone()
            .map(Self::Value)
            .unwrap_or(DateTimePickerDraft::Empty)
    }
}

/// 时间范围端点。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateTimePickerRangeEndpoint {
    /// 起点。
    Start,
    /// 终点。
    End,
}

/// 组件核心状态。
#[derive(Clone, Debug)]
pub struct DateTimePickerState {
    mode: DateTimePickerMode,
    value: Option<DateTimePickerValue>,
    input_text: SharedString,
    is_open: bool,
    draft: DateTimePickerDraft,
    view_year: i32,
    view_month: u32,
    active_date: NaiveDate,
    parse_error: bool,
}

impl DateTimePickerState {
    /// 创建状态。
    pub fn new(
        mode: DateTimePickerMode,
        value: Option<DateTimePickerValue>,
        formats: &DateTimePickerFormats,
    ) -> Self {
        let value = normalize_value_for_mode(value, mode);
        let (view_year, view_month) = view_month_from_value(value.as_ref());
        let active_date = active_date_from_value(value.as_ref());
        Self {
            mode,
            input_text: format_optional_value(value.as_ref(), mode, formats),
            draft: DateTimePickerDraft::from_value(&value),
            value,
            is_open: false,
            view_year,
            view_month,
            active_date,
            parse_error: false,
        }
    }

    /// 返回模式。
    pub fn mode(&self) -> DateTimePickerMode {
        self.mode
    }

    /// 返回当前对外值。
    pub fn value(&self) -> Option<&DateTimePickerValue> {
        self.value.as_ref()
    }

    /// 克隆当前对外值。
    pub fn value_cloned(&self) -> Option<DateTimePickerValue> {
        self.value.clone()
    }

    /// 返回输入文本。
    pub fn input_text(&self) -> &SharedString {
        &self.input_text
    }

    /// 返回打开状态。
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// 返回草稿值。
    pub fn draft(&self) -> &DateTimePickerDraft {
        &self.draft
    }

    /// 返回当前面板年份。
    pub fn view_year(&self) -> i32 {
        self.view_year
    }

    /// 返回当前面板月份。
    pub fn view_month(&self) -> u32 {
        self.view_month
    }

    /// 返回键盘活动日期。
    ///
    /// 活动日期用于弹层打开后的方向键、Home/End 和 Enter 操作。它不等同于已提交值；
    /// 用户移动活动日期时只改变可视焦点，只有按 Enter 或点击日期时才会进入草稿/提交流程。
    pub fn active_date(&self) -> NaiveDate {
        self.active_date
    }

    /// 返回是否存在解析错误。
    pub fn has_parse_error(&self) -> bool {
        self.parse_error
    }

    /// 静默同步值。
    pub fn set_value_silent(
        &mut self,
        value: Option<DateTimePickerValue>,
        formats: &DateTimePickerFormats,
    ) -> DateTimePickerStateOutcome {
        let value = normalize_value_for_mode(value, self.mode);
        let input_text = format_optional_value(value.as_ref(), self.mode, formats);
        let value_changed = self.value != value;
        let input_changed = self.input_text != input_text;
        let parse_error_changed = self.parse_error;
        self.value = value;
        self.input_text = input_text;
        self.draft = DateTimePickerDraft::from_value(&self.value);
        self.parse_error = false;
        let (year, month) = view_month_from_value(self.value.as_ref());
        let view_changed = self.view_year != year || self.view_month != month;
        self.view_year = year;
        self.view_month = month;
        let next_active_date = active_date_from_value(self.value.as_ref());
        let active_date_changed = self.active_date != next_active_date;
        self.active_date = next_active_date;

        DateTimePickerStateOutcome {
            value_changed,
            input_changed,
            draft_changed: value_changed,
            view_changed,
            active_date_changed,
            parse_error_changed,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 静默同步模式。
    pub fn set_mode_silent(
        &mut self,
        mode: DateTimePickerMode,
        formats: &DateTimePickerFormats,
    ) -> DateTimePickerStateOutcome {
        let before_value = self.value.clone();
        let before_input = self.input_text.clone();
        let before_mode = self.mode;
        self.mode = mode;
        self.value = normalize_value_for_mode(self.value.clone(), mode);
        self.input_text = format_optional_value(self.value.as_ref(), self.mode, formats);
        self.draft = DateTimePickerDraft::from_value(&self.value);
        let next_active_date = active_date_from_value(self.value.as_ref());
        let active_date_changed = self.active_date != next_active_date;
        self.active_date = next_active_date;
        let parse_error_changed = self.parse_error;
        self.parse_error = false;

        DateTimePickerStateOutcome {
            value_changed: self.value != before_value,
            input_changed: self.input_text != before_input,
            mode_changed: self.mode != before_mode,
            draft_changed: true,
            active_date_changed,
            parse_error_changed,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 静默同步输入文本。
    pub fn set_input_text_silent(
        &mut self,
        input_text: SharedString,
    ) -> DateTimePickerStateOutcome {
        let input_changed = self.input_text != input_text;
        self.input_text = input_text;
        DateTimePickerStateOutcome {
            input_changed,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 打开弹层并初始化草稿。
    pub fn open(&mut self) -> DateTimePickerStateOutcome {
        let open_changed = !self.is_open;
        self.is_open = true;
        self.draft = DateTimePickerDraft::from_value(&self.value);
        let next_active_date = active_date_from_value(self.value.as_ref());
        let active_date_changed = self.active_date != next_active_date;
        self.active_date = next_active_date;
        self.sync_view_to_active_date();
        DateTimePickerStateOutcome {
            open_changed,
            draft_changed: true,
            active_date_changed,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 关闭弹层，并决定是否丢弃草稿。
    pub fn close_discard(&mut self, formats: &DateTimePickerFormats) -> DateTimePickerStateOutcome {
        let open_changed = self.is_open;
        let before_input = self.input_text.clone();
        self.is_open = false;
        self.draft = DateTimePickerDraft::from_value(&self.value);
        self.input_text = format_optional_value(self.value.as_ref(), self.mode, formats);
        DateTimePickerStateOutcome {
            open_changed,
            draft_changed: true,
            input_changed: self.input_text != before_input,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 切换打开状态。
    pub fn toggle(&mut self, formats: &DateTimePickerFormats) -> DateTimePickerStateOutcome {
        if self.is_open {
            self.close_discard(formats)
        } else {
            self.open()
        }
    }

    /// 清空值和输入文本。
    pub fn clear(&mut self) -> DateTimePickerStateOutcome {
        let value_changed = self.value.is_some();
        let input_changed = !self.input_text.is_empty();
        let parse_error_changed = self.parse_error;
        self.value = None;
        self.input_text = SharedString::default();
        self.draft = DateTimePickerDraft::Empty;
        self.parse_error = false;
        let next_active_date = today();
        let active_date_changed = self.active_date != next_active_date;
        self.active_date = next_active_date;
        self.sync_view_to_active_date();
        DateTimePickerStateOutcome {
            value_changed,
            input_changed,
            draft_changed: true,
            active_date_changed,
            parse_error_changed,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 解析并提交当前输入文本。
    pub fn commit_input(
        &mut self,
        formats: &DateTimePickerFormats,
        constraints: DateTimePickerConstraints,
    ) -> Result<DateTimePickerStateOutcome, DateTimePickerParseError> {
        let parsed = match parse_optional_value(self.input_text.as_str(), self.mode, formats) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.parse_error = true;
                return Err(error);
            }
        };
        if let Some(value) = parsed.as_ref() {
            if !value_allowed(value, constraints) {
                // 禁用规则属于解析后的提交校验失败。这里只记录错误状态并返回统一错误类型，
                // 具体 UI 刷新由组件层根据错误结果触发，避免状态层为了失败路径构造无效 outcome。
                self.parse_error = true;
                return Err(DateTimePickerParseError::DisabledValue);
            }
        }

        let before = self.value.clone();
        let before_input = self.input_text.clone();
        self.value = parsed;
        self.input_text = format_optional_value(self.value.as_ref(), self.mode, formats);
        self.draft = DateTimePickerDraft::from_value(&self.value);
        let parse_error_changed = self.parse_error;
        self.parse_error = false;
        let active_date_changed = self.update_active_date_from_value();
        Ok(DateTimePickerStateOutcome {
            value_changed: self.value != before,
            input_changed: self.input_text != before_input,
            draft_changed: true,
            active_date_changed,
            parse_error_changed,
            ..DateTimePickerStateOutcome::default()
        })
    }

    /// 用户点击日期。
    ///
    /// Date 模式会立即提交；其他模式只更新草稿，等待确认按钮提交。
    pub fn select_date(
        &mut self,
        date: NaiveDate,
        formats: &DateTimePickerFormats,
        constraints: DateTimePickerConstraints,
    ) -> Result<DateTimePickerStateOutcome, DateTimePickerParseError> {
        let before_draft = self.draft.clone();
        let before_active = self.active_date;
        let before_view = (self.view_year, self.view_month);
        self.active_date = date;
        // 范围模式会同时渲染当前月和右侧下一个月。点击右侧月份中的日期时，该日期已经在
        // 当前双月视图内，不应把 view_month 推进到右侧月份；否则用户刚选完右侧日期，面板会
        // 自动跳到“右侧月 + 再下一个月”。只有被点日期不在当前可见月份范围内时才同步面板。
        if !self.active_date_visible_in_current_panel() {
            self.sync_view_to_active_date();
        }
        let focus_outcome = DateTimePickerStateOutcome {
            active_date_changed: self.active_date != before_active,
            view_changed: (self.view_year, self.view_month) != before_view,
            ..DateTimePickerStateOutcome::default()
        };
        match self.mode {
            DateTimePickerMode::Date => {
                let value = DateTimePickerValue::Date(date);
                self.commit_value(value, formats, constraints)
                    .map(|outcome| outcome.merge(focus_outcome))
            }
            DateTimePickerMode::DateTime => {
                let time = self
                    .draft_datetime()
                    .or_else(|| self.value_datetime())
                    .map(|dt| dt.time())
                    .unwrap_or_else(default_time);
                self.draft = DateTimePickerDraft::Value(DateTimePickerValue::DateTime(
                    NaiveDateTime::new(date, time),
                ));
                Ok(DateTimePickerStateOutcome {
                    draft_changed: self.draft != before_draft,
                    ..focus_outcome
                })
            }
            DateTimePickerMode::DateRange => {
                self.draft = match self.draft {
                    DateTimePickerDraft::DateRangeStart(start) => DateTimePickerDraft::Value(
                        DateTimePickerValue::DateRange(DateTimeRange::new(start, date)),
                    ),
                    _ => DateTimePickerDraft::DateRangeStart(date),
                };
                Ok(DateTimePickerStateOutcome {
                    draft_changed: self.draft != before_draft,
                    ..focus_outcome
                })
            }
            DateTimePickerMode::DateTimeRange => {
                self.draft = match self.draft {
                    DateTimePickerDraft::DateTimeRangeStart(start) => {
                        let end = NaiveDateTime::new(date, end_of_day_time());
                        DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(
                            DateTimeRange::new(start, end),
                        ))
                    }
                    _ => DateTimePickerDraft::DateTimeRangeStart(NaiveDateTime::new(
                        date,
                        NaiveTime::MIN,
                    )),
                };
                Ok(DateTimePickerStateOutcome {
                    draft_changed: self.draft != before_draft,
                    ..focus_outcome
                })
            }
            DateTimePickerMode::Time | DateTimePickerMode::TimeRange => {
                Ok(DateTimePickerStateOutcome::default())
            }
        }
    }

    /// 用户选择时间。
    pub fn select_time(
        &mut self,
        time: NaiveTime,
        endpoint: DateTimePickerRangeEndpoint,
    ) -> DateTimePickerStateOutcome {
        let before_draft = self.draft.clone();
        match self.mode {
            DateTimePickerMode::Time => {
                self.draft = DateTimePickerDraft::Value(DateTimePickerValue::Time(time));
            }
            DateTimePickerMode::DateTime => {
                let date = self
                    .draft_datetime()
                    .or_else(|| self.value_datetime())
                    .map(|dt| dt.date())
                    .unwrap_or_else(today);
                self.draft = DateTimePickerDraft::Value(DateTimePickerValue::DateTime(
                    NaiveDateTime::new(date, time),
                ));
            }
            DateTimePickerMode::TimeRange => {
                self.draft = match (&self.draft, endpoint) {
                    (
                        DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(range)),
                        DateTimePickerRangeEndpoint::Start,
                    ) => DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(
                        DateTimeRange::new(time, range.end),
                    )),
                    (
                        DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(range)),
                        DateTimePickerRangeEndpoint::End,
                    ) => DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(
                        DateTimeRange::new(range.start, time),
                    )),
                    (
                        DateTimePickerDraft::TimeRangeStart(start),
                        DateTimePickerRangeEndpoint::End,
                    ) => DateTimePickerDraft::Value(DateTimePickerValue::TimeRange(
                        DateTimeRange::new(*start, time),
                    )),
                    _ => DateTimePickerDraft::TimeRangeStart(time),
                };
            }
            DateTimePickerMode::DateTimeRange => {
                self.update_datetime_range_time(time, endpoint);
            }
            DateTimePickerMode::Date | DateTimePickerMode::DateRange => {}
        }

        DateTimePickerStateOutcome {
            draft_changed: self.draft != before_draft,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 按天移动键盘活动日期。
    ///
    /// 这里不跳过禁用日期，因为禁用规则可能由业务动态决定；活动日期可以落在禁用项上，
    /// 但 Enter 提交时仍会被同一套约束拦截。
    pub fn move_active_date(&mut self, delta_days: i64) -> DateTimePickerStateOutcome {
        let before_date = self.active_date;
        let before_view = (self.view_year, self.view_month);
        if let Some(next) = self
            .active_date
            .checked_add_signed(Duration::days(delta_days))
        {
            self.active_date = next;
            self.sync_view_to_active_date();
        }

        DateTimePickerStateOutcome {
            active_date_changed: self.active_date != before_date,
            view_changed: (self.view_year, self.view_month) != before_view,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 把键盘活动日期移动到当前面板月份首日。
    pub fn move_active_to_month_start(&mut self) -> DateTimePickerStateOutcome {
        let before_date = self.active_date;
        self.active_date =
            NaiveDate::from_ymd_opt(self.view_year, self.view_month, 1).unwrap_or_else(today);
        DateTimePickerStateOutcome {
            active_date_changed: self.active_date != before_date,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 把键盘活动日期移动到当前面板月份末日。
    pub fn move_active_to_month_end(&mut self) -> DateTimePickerStateOutcome {
        let before_date = self.active_date;
        let day = days_in_month(self.view_year, self.view_month);
        self.active_date =
            NaiveDate::from_ymd_opt(self.view_year, self.view_month, day).unwrap_or_else(today);
        DateTimePickerStateOutcome {
            active_date_changed: self.active_date != before_date,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 确认当前草稿。
    pub fn confirm_draft(
        &mut self,
        formats: &DateTimePickerFormats,
        constraints: DateTimePickerConstraints,
    ) -> Result<DateTimePickerStateOutcome, DateTimePickerParseError> {
        let DateTimePickerDraft::Value(value) = self.draft.clone() else {
            return Ok(DateTimePickerStateOutcome::default());
        };
        self.commit_value(value, formats, constraints)
    }

    /// 移动面板月份。
    pub fn move_month(&mut self, delta: i32) -> DateTimePickerStateOutcome {
        let before = (self.view_year, self.view_month);
        let before_active = self.active_date;
        let total = self.view_year * 12 + self.view_month as i32 - 1 + delta;
        self.view_year = total.div_euclid(12);
        self.view_month = total.rem_euclid(12) as u32 + 1;
        let day = self
            .active_date
            .day()
            .min(days_in_month(self.view_year, self.view_month));
        self.active_date = NaiveDate::from_ymd_opt(self.view_year, self.view_month, day)
            .unwrap_or(self.active_date);
        DateTimePickerStateOutcome {
            view_changed: before != (self.view_year, self.view_month),
            active_date_changed: self.active_date != before_active,
            ..DateTimePickerStateOutcome::default()
        }
    }

    /// 提交完整值。
    fn commit_value(
        &mut self,
        value: DateTimePickerValue,
        formats: &DateTimePickerFormats,
        constraints: DateTimePickerConstraints,
    ) -> Result<DateTimePickerStateOutcome, DateTimePickerParseError> {
        if !value_allowed(&value, constraints) {
            // 弹层确认同样复用 min/max 和禁用规则。失败时不修改已提交值，只留下
            // parse_error 标记供渲染层展示错误语义。
            self.parse_error = true;
            return Err(DateTimePickerParseError::DisabledValue);
        }
        let value = normalize_value_for_mode(Some(value), self.mode);
        let before = self.value.clone();
        let before_input = self.input_text.clone();
        self.value = value;
        self.input_text = format_optional_value(self.value.as_ref(), self.mode, formats);
        self.draft = DateTimePickerDraft::from_value(&self.value);
        let parse_error_changed = self.parse_error;
        self.parse_error = false;
        let active_date_changed = self.update_active_date_from_value();
        Ok(DateTimePickerStateOutcome {
            value_changed: self.value != before,
            input_changed: self.input_text != before_input,
            draft_changed: true,
            active_date_changed,
            parse_error_changed,
            ..DateTimePickerStateOutcome::default()
        })
    }

    /// 返回当前草稿日期时间。
    fn draft_datetime(&self) -> Option<NaiveDateTime> {
        match &self.draft {
            DateTimePickerDraft::Value(DateTimePickerValue::DateTime(value)) => Some(*value),
            _ => None,
        }
    }

    /// 返回当前对外日期时间值。
    fn value_datetime(&self) -> Option<NaiveDateTime> {
        match &self.value {
            Some(DateTimePickerValue::DateTime(value)) => Some(*value),
            _ => None,
        }
    }

    /// 更新 DateTimeRange 草稿里的时间部分。
    fn update_datetime_range_time(
        &mut self,
        time: NaiveTime,
        endpoint: DateTimePickerRangeEndpoint,
    ) {
        self.draft = match (&self.draft, endpoint) {
            (
                DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(range)),
                DateTimePickerRangeEndpoint::Start,
            ) => DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(
                DateTimeRange::new(NaiveDateTime::new(range.start.date(), time), range.end),
            )),
            (
                DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(range)),
                DateTimePickerRangeEndpoint::End,
            ) => DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(
                DateTimeRange::new(range.start, NaiveDateTime::new(range.end.date(), time)),
            )),
            (DateTimePickerDraft::DateTimeRangeStart(start), DateTimePickerRangeEndpoint::End) => {
                DateTimePickerDraft::Value(DateTimePickerValue::DateTimeRange(DateTimeRange::new(
                    *start,
                    NaiveDateTime::new(start.date(), time),
                )))
            }
            _ => DateTimePickerDraft::DateTimeRangeStart(NaiveDateTime::new(today(), time)),
        };
    }

    /// 让活动日期跟随当前对外值。
    ///
    /// 文本提交、弹层确认和受控值同步都会重建对外值。此时活动日期也需要同步到新值，
    /// 否则用户再次打开弹层后方向键会从旧月份继续移动。
    fn update_active_date_from_value(&mut self) -> bool {
        let next_active_date = active_date_from_value(self.value.as_ref());
        let changed = self.active_date != next_active_date;
        self.active_date = next_active_date;
        self.sync_view_to_active_date();
        changed
    }

    /// 面板年月跟随活动日期。
    fn sync_view_to_active_date(&mut self) {
        self.view_year = self.active_date.year();
        self.view_month = self.active_date.month();
    }

    /// 判断活动日期是否已经处在当前可见日期面板内。
    ///
    /// 单月模式只包含当前 `view_year/view_month`；日期范围和日期时间范围使用双月面板，
    /// 因此当前月和下一个月都视为可见范围。该判断只服务鼠标点击日期后的面板同步，键盘导航
    /// 仍继续通过 `sync_view_to_active_date` 保证活动日期可见。
    fn active_date_visible_in_current_panel(&self) -> bool {
        let active_month = (self.active_date.year(), self.active_date.month());
        let current_month = (self.view_year, self.view_month);
        if active_month == current_month {
            return true;
        }

        if matches!(
            self.mode,
            DateTimePickerMode::DateRange | DateTimePickerMode::DateTimeRange
        ) {
            let (next_year, next_month) = next_month(self.view_year, self.view_month);
            active_month == (next_year, next_month)
        } else {
            false
        }
    }
}

/// 格式化可选值。
pub fn format_optional_value(
    value: Option<&DateTimePickerValue>,
    mode: DateTimePickerMode,
    formats: &DateTimePickerFormats,
) -> SharedString {
    value
        .filter(|value| value.matches_mode(mode))
        .map(|value| format_value(value, formats))
        .unwrap_or_default()
}

/// 格式化完整值。
pub fn format_value(value: &DateTimePickerValue, formats: &DateTimePickerFormats) -> SharedString {
    let text = match value {
        DateTimePickerValue::Date(value) => value.format(formats.date_format.as_str()).to_string(),
        DateTimePickerValue::Time(value) => value.format(formats.time_format.as_str()).to_string(),
        DateTimePickerValue::DateTime(value) => {
            value.format(formats.datetime_format.as_str()).to_string()
        }
        DateTimePickerValue::DateRange(range) => format!(
            "{}{}{}",
            range.start.format(formats.date_format.as_str()),
            formats.range_separator,
            range.end.format(formats.date_format.as_str())
        ),
        DateTimePickerValue::TimeRange(range) => format!(
            "{}{}{}",
            range.start.format(formats.time_format.as_str()),
            formats.range_separator,
            range.end.format(formats.time_format.as_str())
        ),
        DateTimePickerValue::DateTimeRange(range) => format!(
            "{}{}{}",
            range.start.format(formats.datetime_format.as_str()),
            formats.range_separator,
            range.end.format(formats.datetime_format.as_str())
        ),
    };
    SharedString::from(text)
}

/// 解析可选值。
pub fn parse_optional_value(
    text: &str,
    mode: DateTimePickerMode,
    formats: &DateTimePickerFormats,
) -> Result<Option<DateTimePickerValue>, DateTimePickerParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    parse_value(text, mode, formats).map(Some)
}

/// 按模式解析完整值。
pub fn parse_value(
    text: &str,
    mode: DateTimePickerMode,
    formats: &DateTimePickerFormats,
) -> Result<DateTimePickerValue, DateTimePickerParseError> {
    match mode {
        DateTimePickerMode::Date => parse_date(text, formats).map(DateTimePickerValue::Date),
        DateTimePickerMode::Time => parse_time(text, formats).map(DateTimePickerValue::Time),
        DateTimePickerMode::DateTime => {
            parse_datetime(text, formats).map(DateTimePickerValue::DateTime)
        }
        DateTimePickerMode::DateRange => {
            let (start, end) = split_range(text, formats)?;
            Ok(DateTimePickerValue::DateRange(DateTimeRange::new(
                parse_date(start, formats)?,
                parse_date(end, formats)?,
            )))
        }
        DateTimePickerMode::TimeRange => {
            let (start, end) = split_range(text, formats)?;
            Ok(DateTimePickerValue::TimeRange(DateTimeRange::new(
                parse_time(start, formats)?,
                parse_time(end, formats)?,
            )))
        }
        DateTimePickerMode::DateTimeRange => {
            let (start, end) = split_range(text, formats)?;
            Ok(DateTimePickerValue::DateTimeRange(DateTimeRange::new(
                parse_datetime(start, formats)?,
                parse_datetime(end, formats)?,
            )))
        }
    }
}

/// 判断值是否满足约束。
pub fn value_allowed(value: &DateTimePickerValue, constraints: DateTimePickerConstraints) -> bool {
    match value {
        DateTimePickerValue::Date(value) => date_allowed(*value, constraints),
        DateTimePickerValue::Time(value) => time_allowed(*value, constraints),
        DateTimePickerValue::DateTime(value) => datetime_allowed(*value, constraints),
        DateTimePickerValue::DateRange(range) => {
            date_allowed(range.start, constraints.clone()) && date_allowed(range.end, constraints)
        }
        DateTimePickerValue::TimeRange(range) => {
            time_allowed(range.start, constraints.clone()) && time_allowed(range.end, constraints)
        }
        DateTimePickerValue::DateTimeRange(range) => {
            datetime_allowed(range.start, constraints.clone())
                && datetime_allowed(range.end, constraints)
        }
    }
}

/// 判断日期是否满足日期类约束。
pub fn date_allowed(date: NaiveDate, constraints: DateTimePickerConstraints) -> bool {
    if constraints
        .disabled_date
        .as_ref()
        .map(|predicate| predicate(date))
        .unwrap_or(false)
    {
        return false;
    }
    if let Some(min) = min_date_bound(constraints.min.as_ref()) {
        if date < min {
            return false;
        }
    }
    if let Some(max) = max_date_bound(constraints.max.as_ref()) {
        if date > max {
            return false;
        }
    }
    true
}

/// 判断时间是否满足时间类约束。
pub fn time_allowed(time: NaiveTime, constraints: DateTimePickerConstraints) -> bool {
    if constraints
        .disabled_time
        .as_ref()
        .map(|predicate| predicate(time))
        .unwrap_or(false)
    {
        return false;
    }
    if let Some(min) = min_time_bound(constraints.min.as_ref()) {
        if time < min {
            return false;
        }
    }
    if let Some(max) = max_time_bound(constraints.max.as_ref()) {
        if time > max {
            return false;
        }
    }
    true
}

/// 判断日期时间是否满足日期、时间和日期时间约束。
pub fn datetime_allowed(datetime: NaiveDateTime, constraints: DateTimePickerConstraints) -> bool {
    if !date_allowed(datetime.date(), constraints.clone()) {
        return false;
    }
    if !time_allowed(datetime.time(), constraints.clone()) {
        return false;
    }
    if let Some(min) = min_datetime_bound(constraints.min.as_ref()) {
        if datetime < min {
            return false;
        }
    }
    if let Some(max) = max_datetime_bound(constraints.max.as_ref()) {
        if datetime > max {
            return false;
        }
    }
    true
}

/// 生成步长规范化后的分钟或秒候选。
pub fn stepped_values(step: usize) -> Vec<u32> {
    let step = normalized_step(step);
    (0..60).step_by(step).map(|value| value as u32).collect()
}

/// 规范化分钟/秒步长。
pub fn normalized_step(step: usize) -> usize {
    if step == 0 || step > 60 {
        1
    } else {
        step
    }
}

/// 返回指定年月的日历网格起点。
pub fn calendar_grid_start(year: i32, month: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month.clamp(1, 12), 1)
        .expect("valid month should create first day");
    let weekday_offset = first.weekday().num_days_from_monday() as i64;
    first - chrono::Duration::days(weekday_offset)
}

/// 返回指定年月的 42 个日历格日期。
pub fn calendar_days(year: i32, month: u32) -> Vec<NaiveDate> {
    let start = calendar_grid_start(year, month);
    (0..42)
        .map(|offset| start + chrono::Duration::days(offset))
        .collect()
}

/// 返回今天。
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// 返回当前时间并去掉纳秒。
pub fn now_time() -> NaiveTime {
    Local::now()
        .time()
        .with_nanosecond(0)
        .unwrap_or_else(default_time)
}

/// 默认时间。
pub fn default_time() -> NaiveTime {
    NaiveTime::from_hms_opt(0, 0, 0).expect("00:00:00 should be valid")
}

/// 一天结束时间。
pub fn end_of_day_time() -> NaiveTime {
    NaiveTime::from_hms_opt(23, 59, 59).expect("23:59:59 should be valid")
}

/// 返回指定年月的下一个月。
fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month >= 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// 按模式规范化值。
fn normalize_value_for_mode(
    value: Option<DateTimePickerValue>,
    mode: DateTimePickerMode,
) -> Option<DateTimePickerValue> {
    value
        .filter(|value| value.matches_mode(mode))
        .map(|value| match value {
            DateTimePickerValue::DateRange(range) => {
                DateTimePickerValue::DateRange(DateTimeRange::new(range.start, range.end))
            }
            DateTimePickerValue::TimeRange(range) => {
                DateTimePickerValue::TimeRange(DateTimeRange::new(range.start, range.end))
            }
            DateTimePickerValue::DateTimeRange(range) => {
                DateTimePickerValue::DateTimeRange(DateTimeRange::new(range.start, range.end))
            }
            value => value,
        })
}

/// 根据当前值推导日历初始月份。
fn view_month_from_value(value: Option<&DateTimePickerValue>) -> (i32, u32) {
    let date = match value {
        Some(DateTimePickerValue::Date(value)) => *value,
        Some(DateTimePickerValue::DateTime(value)) => value.date(),
        Some(DateTimePickerValue::DateRange(range)) => range.start,
        Some(DateTimePickerValue::DateTimeRange(range)) => range.start.date(),
        _ => today(),
    };
    (date.year(), date.month())
}

/// 根据当前值推导键盘活动日期。
fn active_date_from_value(value: Option<&DateTimePickerValue>) -> NaiveDate {
    match value {
        Some(DateTimePickerValue::Date(value)) => *value,
        Some(DateTimePickerValue::DateTime(value)) => value.date(),
        Some(DateTimePickerValue::DateRange(range)) => range.start,
        Some(DateTimePickerValue::DateTimeRange(range)) => range.start.date(),
        _ => today(),
    }
}

/// 返回指定年月的天数。
fn days_in_month(year: i32, month: u32) -> u32 {
    let month = month.clamp(1, 12);
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let Some(next_first) = NaiveDate::from_ymd_opt(next_year, next_month, 1) else {
        return 31;
    };
    (next_first - Duration::days(1)).day()
}

/// 解析日期。
fn parse_date(
    text: &str,
    formats: &DateTimePickerFormats,
) -> Result<NaiveDate, DateTimePickerParseError> {
    NaiveDate::parse_from_str(text.trim(), formats.date_format.as_str())
        .map_err(|_| DateTimePickerParseError::InvalidFormat)
}

/// 解析时间。
fn parse_time(
    text: &str,
    formats: &DateTimePickerFormats,
) -> Result<NaiveTime, DateTimePickerParseError> {
    NaiveTime::parse_from_str(text.trim(), formats.time_format.as_str())
        .map_err(|_| DateTimePickerParseError::InvalidFormat)
}

/// 解析日期时间。
fn parse_datetime(
    text: &str,
    formats: &DateTimePickerFormats,
) -> Result<NaiveDateTime, DateTimePickerParseError> {
    NaiveDateTime::parse_from_str(text.trim(), formats.datetime_format.as_str())
        .map_err(|_| DateTimePickerParseError::InvalidFormat)
}

/// 拆分范围输入。
fn split_range<'a>(
    text: &'a str,
    formats: &DateTimePickerFormats,
) -> Result<(&'a str, &'a str), DateTimePickerParseError> {
    let separator = formats.range_separator.as_str();
    let mut parts = text.split(separator);
    let start = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or(DateTimePickerParseError::InvalidFormat)?;
    let end = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or(DateTimePickerParseError::InvalidFormat)?;
    if parts.next().is_some() {
        return Err(DateTimePickerParseError::InvalidFormat);
    }
    Ok((start, end))
}

/// 最小日期边界。
fn min_date_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveDate> {
    match value {
        Some(DateTimePickerValue::Date(value)) => Some(*value),
        Some(DateTimePickerValue::DateRange(range)) => Some(range.start),
        Some(DateTimePickerValue::DateTime(value)) => Some(value.date()),
        Some(DateTimePickerValue::DateTimeRange(range)) => Some(range.start.date()),
        _ => None,
    }
}

/// 最大日期边界。
fn max_date_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveDate> {
    match value {
        Some(DateTimePickerValue::Date(value)) => Some(*value),
        Some(DateTimePickerValue::DateRange(range)) => Some(range.end),
        Some(DateTimePickerValue::DateTime(value)) => Some(value.date()),
        Some(DateTimePickerValue::DateTimeRange(range)) => Some(range.end.date()),
        _ => None,
    }
}

/// 最小时间边界。
fn min_time_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveTime> {
    match value {
        Some(DateTimePickerValue::Time(value)) => Some(*value),
        Some(DateTimePickerValue::TimeRange(range)) => Some(range.start),
        _ => None,
    }
}

/// 最大时间边界。
fn max_time_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveTime> {
    match value {
        Some(DateTimePickerValue::Time(value)) => Some(*value),
        Some(DateTimePickerValue::TimeRange(range)) => Some(range.end),
        _ => None,
    }
}

/// 最小日期时间边界。
fn min_datetime_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveDateTime> {
    match value {
        Some(DateTimePickerValue::DateTime(value)) => Some(*value),
        Some(DateTimePickerValue::DateTimeRange(range)) => Some(range.start),
        _ => None,
    }
}

/// 最大日期时间边界。
fn max_datetime_bound(value: Option<&DateTimePickerValue>) -> Option<NaiveDateTime> {
    match value {
        Some(DateTimePickerValue::DateTime(value)) => Some(*value),
        Some(DateTimePickerValue::DateTimeRange(range)) => Some(range.end),
        _ => None,
    }
}
