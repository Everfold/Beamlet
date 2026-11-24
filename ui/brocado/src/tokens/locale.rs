//! Localization tokens for built-in calendar, time, dialog, and search controls.
//!
//! [`Theme`](crate::theme::Theme) defaults to [`UiLocale::english()`], while applications
//! can supply [`UiLocale::zh_cn()`] or customize any month, weekday, meridian, or button copy.

use masonry::core::ArcStr;

/// Configurable localized labels and calendar conventions for shared core components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiLocale {
    /// Localized month names indexed `0..12` (January .. December).
    pub months: [ArcStr; 12],
    /// Single-character or short weekday headers ordered Sunday (`0`) through Saturday (`6`).
    pub weekdays_sun_first: [ArcStr; 7],
    /// First column day of week (`0` = Sunday, `1` = Monday, ..., `6` = Saturday).
    pub first_day_of_week: u8,
    /// Morning / ante-meridiem label when [`use_24h`](Self::use_24h) is `false`.
    pub am_label: ArcStr,
    /// Afternoon / post-meridiem label when [`use_24h`](Self::use_24h) is `false`.
    pub pm_label: ArcStr,
    /// Uses 24-hour time selection (hiding AM/PM toggle buttons) when `true`.
    pub use_24h: bool,
    /// Default confirmation button label in [`date_picker_dialog`](crate::components::date_picker_dialog)
    /// and [`time_picker_dialog`](crate::components::time_picker_dialog).
    pub ok_label: ArcStr,
    /// Default cancel button label in [`date_picker_dialog`](crate::components::date_picker_dialog)
    /// and [`time_picker_dialog`](crate::components::time_picker_dialog).
    pub cancel_label: ArcStr,
    /// Default fallback placeholder used by [`search_bar`](crate::components::search_bar) when unset.
    pub search_placeholder: ArcStr,
}

impl UiLocale {
    /// Standard English US locale (`Sunday` week start, 12-hour AM/PM).
    #[must_use]
    pub fn english() -> Self {
        Self {
            months: [
                ArcStr::from("January"),
                ArcStr::from("February"),
                ArcStr::from("March"),
                ArcStr::from("April"),
                ArcStr::from("May"),
                ArcStr::from("June"),
                ArcStr::from("July"),
                ArcStr::from("August"),
                ArcStr::from("September"),
                ArcStr::from("October"),
                ArcStr::from("November"),
                ArcStr::from("December"),
            ],
            weekdays_sun_first: [
                ArcStr::from("S"),
                ArcStr::from("M"),
                ArcStr::from("T"),
                ArcStr::from("W"),
                ArcStr::from("T"),
                ArcStr::from("F"),
                ArcStr::from("S"),
            ],
            first_day_of_week: 0,
            am_label: ArcStr::from("AM"),
            pm_label: ArcStr::from("PM"),
            use_24h: false,
            ok_label: ArcStr::from("OK"),
            cancel_label: ArcStr::from("Cancel"),
            search_placeholder: ArcStr::from("Search..."),
        }
    }

    /// Simplified Chinese locale (`Monday` week start, 24-hour clock).
    #[must_use]
    pub fn zh_cn() -> Self {
        Self {
            months: [
                ArcStr::from("1月"),
                ArcStr::from("2月"),
                ArcStr::from("3月"),
                ArcStr::from("4月"),
                ArcStr::from("5月"),
                ArcStr::from("6月"),
                ArcStr::from("7月"),
                ArcStr::from("8月"),
                ArcStr::from("9月"),
                ArcStr::from("10月"),
                ArcStr::from("11月"),
                ArcStr::from("12月"),
            ],
            weekdays_sun_first: [
                ArcStr::from("日"),
                ArcStr::from("一"),
                ArcStr::from("二"),
                ArcStr::from("三"),
                ArcStr::from("四"),
                ArcStr::from("五"),
                ArcStr::from("六"),
            ],
            first_day_of_week: 1,
            am_label: ArcStr::from("上午"),
            pm_label: ArcStr::from("下午"),
            use_24h: true,
            ok_label: ArcStr::from("确定"),
            cancel_label: ArcStr::from("取消"),
            search_placeholder: ArcStr::from("搜索..."),
        }
    }

    /// Sets the first column day of the calendar week (`0` = Sunday .. `6` = Saturday).
    #[must_use]
    pub const fn with_first_day_of_week(mut self, day: u8) -> Self {
        self.first_day_of_week = day % 7;
        self
    }

    /// Configures 24-hour vs 12-hour time picker mode.
    #[must_use]
    pub const fn with_24h(mut self, use_24h: bool) -> Self {
        self.use_24h = use_24h;
        self
    }

    /// Overrides the default picker dialog confirmation and cancel button labels.
    #[must_use]
    pub fn with_ok_cancel(mut self, ok: impl Into<ArcStr>, cancel: impl Into<ArcStr>) -> Self {
        self.ok_label = ok.into();
        self.cancel_label = cancel.into();
        self
    }

    /// Overrides the default search bar placeholder copy.
    #[must_use]
    pub fn with_search_placeholder(mut self, text: impl Into<ArcStr>) -> Self {
        self.search_placeholder = text.into();
        self
    }
}

impl Default for UiLocale {
    fn default() -> Self {
        Self::english()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_presets_and_builders_configure_expected_defaults() {
        let en = UiLocale::english();
        assert_eq!(en.first_day_of_week, 0);
        assert!(!en.use_24h);
        assert_eq!(en.ok_label.as_ref(), "OK");
        assert_eq!(en.cancel_label.as_ref(), "Cancel");
        assert_eq!(en.search_placeholder.as_ref(), "Search...");

        let zh = UiLocale::zh_cn()
            .with_24h(false)
            .with_ok_cancel("应用", "返回")
            .with_search_placeholder("输入关键词");
        assert_eq!(zh.first_day_of_week, 1);
        assert!(!zh.use_24h);
        assert_eq!(zh.months[0].as_ref(), "1月");
        assert_eq!(zh.weekdays_sun_first[1].as_ref(), "一");
        assert_eq!(zh.ok_label.as_ref(), "应用");
        assert_eq!(zh.cancel_label.as_ref(), "返回");
        assert_eq!(zh.search_placeholder.as_ref(), "输入关键词");
    }
}
