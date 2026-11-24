//! Gregorian `CalendarDate` picker and `TimeOfDay` selector (+ modal dialog wrappers).

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::{AsUnit, Dim};
use masonry::peniko::Color;
use masonry::properties::{Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, MainAxisAlignment, ZStackExt, button, flex_col, flex_row, label, portal,
    sized_box, zstack,
};

use crate::components::button::{primary_button, subtle_button};
use crate::components::dialog::dialog;
use crate::components::typography::ui_label;
use crate::foundation::{
    EscapeCatcher, UiClassExt, WrapWidget, adaptive_pair, icons, ui_icon, wrap,
};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, DialogActionOrder, TypeRole};

/// Proleptic Gregorian calendar date (`year`, `month` `1..=12`, `day` `1..=31`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    /// Four-digit year.
    pub year: i32,
    /// Month (`1..=12`).
    pub month: u8,
    /// Day of month (`1..=31`).
    pub day: u8,
}

impl CalendarDate {
    /// Creates a normalized [`CalendarDate`] clamped to valid month/day bounds.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        let m = month.clamp(1, 12);
        let max_d = days_in_month(year, m);
        Self {
            year,
            month: m,
            day: day.clamp(1, max_d),
        }
    }

    /// Returns the same day clamped to the previous calendar month.
    #[must_use]
    pub fn prev_month(self) -> Self {
        if self.month == 1 {
            Self::new(self.year - 1, 12, self.day)
        } else {
            Self::new(self.year, self.month - 1, self.day)
        }
    }

    /// Returns the same day clamped to the next calendar month.
    #[must_use]
    pub fn next_month(self) -> Self {
        if self.month == 12 {
            Self::new(self.year + 1, 1, self.day)
        } else {
            Self::new(self.year, self.month + 1, self.day)
        }
    }

    /// Formats the date cleanly as `YYYY-MM-DD`.
    #[must_use]
    pub fn format_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// 24-hour wall-clock time (`hour` `0..=23`, `minute` `0..=59`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    /// Hour (`0..=23`).
    pub hour: u8,
    /// Minute (`0..=59`).
    pub minute: u8,
}

impl TimeOfDay {
    /// Creates a [`TimeOfDay`] wrapping hours modulo 24 and minutes modulo 60.
    #[must_use]
    pub const fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour % 24,
            minute: minute % 60,
        }
    }

    /// Increments hour by `delta` modulo 24.
    #[must_use]
    pub const fn add_hours(self, delta: i8) -> Self {
        let next = (self.hour as i16 + delta as i16).rem_euclid(24) as u8;
        Self::new(next, self.minute)
    }

    /// Increments minute by `delta` modulo 60.
    #[must_use]
    pub const fn add_minutes(self, delta: i8) -> Self {
        let next = (self.minute as i16 + delta as i16).rem_euclid(60) as u8;
        Self::new(self.hour, next)
    }

    /// Formats time as `HH:MM`.
    #[must_use]
    pub fn format_24h(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

/// Returns `true` for Gregorian leap years.
#[must_use]
pub const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in `(year, month)`.
#[must_use]
pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Sakamoto's weekday algorithm returning `0 = Sunday .. 6 = Saturday`.
#[must_use]
pub const fn weekday_sunday_zero(year: i32, month: u8, day: u8) -> u8 {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = (month.saturating_sub(1) as usize) % 12;
    (y + y / 4 - y / 100 + y / 400 + OFFSETS[m] + day as i32).rem_euclid(7) as u8
}

const fn calendar_leading_blank_days(year: i32, month: u8, first_day_of_week: u8) -> usize {
    let sun_wd = weekday_sunday_zero(year, month, 1) as usize;
    let start = (first_day_of_week % 7) as usize;
    (sun_wd + 7 - start) % 7
}

fn localized_month_label(months: &[ArcStr; 12], month: u8) -> &str {
    months
        .get(month.saturating_sub(1) as usize)
        .map_or("", ArcStr::as_ref)
}

/// Interactive docked Gregorian month-grid date picker.
pub fn date_picker<State: 'static, Action: 'static, F>(
    selected: CalendarDate,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    F: Fn(&mut State, CalendarDate) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let loc = &spec.locale;
        let prev_date = selected.prev_month();
        let next_date = selected.next_month();
        let cb_prev = cb.clone();
        let cb_next = cb.clone();

        let prev_btn = button(
            ui_icon(icons::CHEVRON_LEFT, 16.0, p.text_secondary),
            move |s: &mut State| cb_prev(s, prev_date),
        )
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(28.px(), 28.px()))
        .padding(Padding::all(0.px()));
        let next_btn = button(
            ui_icon(icons::CHEVRON_RIGHT, 16.0, p.text_secondary),
            move |s: &mut State| cb_next(s, next_date),
        )
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(28.px(), 28.px()))
        .padding(Padding::all(0.px()));

        let header = adaptive_pair(
            ui_label(
                TypeRole::TitleMedium,
                format!(
                    "{} {}",
                    localized_month_label(&loc.months, selected.month),
                    selected.year
                ),
            )
            .color(p.text_primary),
            flex_row((prev_btn, next_btn))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(2.px()),
        )
        .gap(4.px())
        .run_gap(6.px())
        .min_leading(48.0);

        let cell_sz = (f64::from(spec.typography.body_small.font_size) * 2.5)
            .ceil()
            .max(30.0);
        let wd_h = (f64::from(spec.typography.label_small.line_height) + 8.0)
            .ceil()
            .max(24.0);

        let start_day = (loc.first_day_of_week % 7) as usize;
        let weekday_row = flex_row(
            (0..7usize)
                .map(|offset| {
                    let w = loc.weekdays_sun_first[(start_day + offset) % 7].clone();
                    sized_box(zstack((ui_label(TypeRole::LabelSmall, w)
                        .line_height(masonry::parley::LineHeight::MetricsRelative(1.0))
                        .color(p.text_secondary)
                        .line_breaking(LineBreaking::Clip)
                        .alignment(masonry::layout::UnitPoint::CENTER),)))
                    .dims(Dimensions::fixed(cell_sz.px(), wd_h.px()))
                })
                .collect::<Vec<_>>(),
        )
        .gap(2.px());

        let first_wd =
            calendar_leading_blank_days(selected.year, selected.month, loc.first_day_of_week);
        let days = days_in_month(selected.year, selected.month) as usize;
        let mut cells: Vec<Option<u8>> = vec![None; first_wd];
        for d in 1..=days {
            cells.push(Some(d as u8));
        }
        while !cells.len().is_multiple_of(7) {
            cells.push(None);
        }

        let rows: Vec<Box<AnyWidgetView<State, Action>>> = cells
            .as_chunks::<7>()
            .0
            .iter()
            .map(|week| {
                let row_cells: Vec<Box<AnyWidgetView<State, Action>>> = week
                    .iter()
                    .map(|cell| match cell {
                        None => sized_box(label(""))
                            .dims(Dimensions::fixed(cell_sz.px(), cell_sz.px()))
                            .boxed(),
                        Some(d) => {
                            let d = *d;
                            let is_sel = d == selected.day;
                            let cb_d = cb.clone();
                            let target = CalendarDate::new(selected.year, selected.month, d);
                            let fg = if is_sel { p.on_accent } else { p.text_primary };
                            let cls = if is_sel {
                                classes::ICON_BTN_FILLED
                            } else {
                                classes::ICON_BTN_STD
                            };
                            sized_box(
                                button(
                                    ui_label(TypeRole::BodySmall, d.to_string())
                                        .color(fg)
                                        .line_breaking(LineBreaking::Clip),
                                    move |s: &mut State| cb_d(s, target),
                                )
                                .ui_class(cls)
                                .dims(Dimensions::fixed(cell_sz.px(), cell_sz.px()))
                                .padding(Padding::all(0.px())),
                            )
                            .dims(Dimensions::fixed(cell_sz.px(), cell_sz.px()))
                            .boxed()
                        }
                    })
                    .collect();
                flex_row(row_cells).gap(2.px()).boxed()
            })
            .collect();

        let grid = sized_box(
            flex_col((weekday_row, flex_col(rows).gap(4.px())))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(6.px())
                .padding(Padding::bottom(14.px())),
        )
        .dims(Dimensions::new(Dim::MaxContent, Dim::MaxContent));

        flex_col((header, portal(grid).constrain_vertical(true)))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(8.px())
    })
}

fn picker_dialog_actions<State: 'static, Action: 'static, FOke, FDis>(
    on_confirm: FOke,
    on_dismiss: FDis,
) -> impl WidgetView<State, Action, Widget = WrapWidget>
where
    FOke: Fn(&mut State) -> Action + Send + Sync + 'static,
    FDis: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let confirm_cb = Arc::new(on_confirm);
    let dismiss_cb = Arc::new(on_dismiss);
    with_spec(move |spec: &DesignSpec, _| {
        let ok = confirm_cb.clone();
        let dis = dismiss_cb.clone();
        let cancel_btn = subtle_button(spec.locale.cancel_label.clone(), move |s| dis(s)).boxed();
        let ok_btn = primary_button(spec.locale.ok_label.clone(), move |s| ok(s)).boxed();
        let btns = match spec.dialog_action_order {
            DialogActionOrder::PrimaryLeading => vec![ok_btn, cancel_btn],
            DialogActionOrder::PrimaryTrailing => vec![cancel_btn, ok_btn],
        };
        wrap(btns)
            .main_axis_alignment(MainAxisAlignment::End)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
            .run_gap(8.px())
    })
}

/// Modal dialog wrapping [`date_picker`] with Cancel / OK footer actions ordered per platform convention.
pub fn date_picker_dialog<State: 'static, Action: 'static, FSel, FOke, FDis>(
    selected: CalendarDate,
    on_select: FSel,
    on_confirm: FOke,
    on_dismiss: FDis,
) -> impl WidgetView<State, Action, Widget = EscapeCatcher>
where
    FSel: Fn(&mut State, CalendarDate) -> Action + Send + Sync + 'static,
    FOke: Fn(&mut State) -> Action + Send + Sync + 'static,
    FDis: Fn(&mut State) -> Action + Send + Sync + 'static + Clone,
{
    let picker = date_picker(selected, on_select);
    let actions = picker_dialog_actions(on_confirm, on_dismiss.clone());

    let content = flex_col((picker, actions))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px());
    dialog(content, on_dismiss)
        .min_width(240.0)
        .max_width(560.0)
}

/// Interactive numeric 24-hour / 12-hour stepper time selector.
pub fn time_picker<State: 'static, Action: 'static, F>(
    selected: TimeOfDay,
    on_change: F,
) -> impl WidgetView<State, Action, Widget = WrapWidget>
where
    F: Fn(&mut State, TimeOfDay) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_change);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let is_pm = selected.hour >= 12;
        let h_plus = cb.clone();
        let h_minus = cb.clone();
        let m_plus = cb.clone();
        let m_minus = cb.clone();
        let am_cb = cb.clone();
        let pm_cb = cb.clone();

        let headline = spec.typography.headline_small;
        let box_w = (f64::from(headline.font_size) * 2.08).round().max(50.0);
        let box_h = (f64::from(headline.line_height) + 14.0).round().max(46.0);
        let step_h = (box_h * 0.6).round().max(28.0);
        let step_w = (step_h + 4.0).round().max(32.0);

        let step_btn =
            |path: &'static str, on_click: Arc<dyn Fn(&mut State) -> Action + Send + Sync>| {
                button(
                    ui_icon(path, 18.0, p.text_secondary),
                    move |s: &mut State| on_click(s),
                )
                .ui_class(classes::ICON_BTN_STD)
                .dims(Dimensions::fixed(step_w.px(), step_h.px()))
                .padding(Padding::all(0.px()))
            };

        let hour_col = flex_col((
            step_btn(
                icons::ARROW_DROP_UP,
                Arc::new(move |s| h_plus(s, selected.add_hours(1))),
            ),
            sized_box(zstack((ui_label(
                TypeRole::HeadlineSmall,
                format!("{:02}", selected.hour),
            )
            .color(p.on_accent_container)
            .line_breaking(LineBreaking::Clip)
            .alignment(masonry::layout::UnitPoint::CENTER),)))
            .dims(Dimensions::fixed(box_w.px(), box_h.px()))
            .background_color(p.accent_container)
            .corner_radius(spec.metrics.card_radius.px()),
            step_btn(
                icons::ARROW_DROP_DOWN,
                Arc::new(move |s| h_minus(s, selected.add_hours(-1))),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(4.px());

        let colon = ui_label(TypeRole::HeadlineSmall, ":")
            .color(p.text_primary)
            .line_breaking(LineBreaking::Clip);

        let min_col = flex_col((
            step_btn(
                icons::ARROW_DROP_UP,
                Arc::new(move |s| m_plus(s, selected.add_minutes(5))),
            ),
            sized_box(zstack((ui_label(
                TypeRole::HeadlineSmall,
                format!("{:02}", selected.minute),
            )
            .color(p.text_primary)
            .line_breaking(LineBreaking::Clip)
            .alignment(masonry::layout::UnitPoint::CENTER),)))
            .dims(Dimensions::fixed(box_w.px(), box_h.px()))
            .background_color(p.surface_highest)
            .corner_radius(spec.metrics.card_radius.px()),
            step_btn(
                icons::ARROW_DROP_DOWN,
                Arc::new(move |s| m_minus(s, selected.add_minutes(-5))),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(4.px());

        let am_pm_col = (!spec.locale.use_24h).then(|| {
            let am_btn = button(
                ui_label(TypeRole::LabelMedium, spec.locale.am_label.clone())
                    .color(if !is_pm {
                        p.on_tertiary_container
                    } else {
                        p.text_secondary
                    })
                    .line_breaking(LineBreaking::Clip),
                move |s: &mut State| am_cb(s, TimeOfDay::new(selected.hour % 12, selected.minute)),
            )
            .ui_class(if !is_pm {
                classes::SEG_BTN_SELECTED
            } else {
                classes::SEG_BTN
            });

            let pm_btn = button(
                ui_label(TypeRole::LabelMedium, spec.locale.pm_label.clone())
                    .color(if is_pm {
                        p.on_tertiary_container
                    } else {
                        p.text_secondary
                    })
                    .line_breaking(LineBreaking::Clip),
                move |s: &mut State| {
                    pm_cb(
                        s,
                        TimeOfDay::new((selected.hour % 12) + 12, selected.minute),
                    )
                },
            )
            .ui_class(if is_pm {
                classes::SEG_BTN_SELECTED
            } else {
                classes::SEG_BTN
            });

            flex_col((am_btn, pm_btn)).gap(6.px())
        });
        let _ = Color::TRANSPARENT;

        let mut parts: Vec<Box<AnyWidgetView<State, Action>>> =
            vec![hour_col.boxed(), colon.boxed(), min_col.boxed()];
        if let Some(col) = am_pm_col {
            parts.push(col.boxed());
        }
        wrap(parts)
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
            .run_gap(8.px())
    })
}

/// Modal dialog wrapping [`time_picker`] with Cancel / OK footer actions ordered per platform convention.
pub fn time_picker_dialog<State: 'static, Action: 'static, FSel, FOke, FDis>(
    selected: TimeOfDay,
    on_change: FSel,
    on_confirm: FOke,
    on_dismiss: FDis,
) -> impl WidgetView<State, Action, Widget = EscapeCatcher>
where
    FSel: Fn(&mut State, TimeOfDay) -> Action + Send + Sync + 'static,
    FOke: Fn(&mut State) -> Action + Send + Sync + 'static,
    FDis: Fn(&mut State) -> Action + Send + Sync + 'static + Clone,
{
    let picker = time_picker(selected, on_change);
    let actions = picker_dialog_actions(on_confirm, on_dismiss.clone());

    let content = flex_col((picker, actions))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px());
    dialog(content, on_dismiss)
        .min_width(240.0)
        .max_width(560.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_handles_leap_years_and_month_rollover() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2100));
        assert!(is_leap_year(2000));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);

        let mar31 = CalendarDate::new(2025, 3, 31);
        let feb = mar31.prev_month();
        assert_eq!(feb, CalendarDate::new(2025, 2, 28));

        let jan = CalendarDate::new(2025, 1, 15).prev_month();
        assert_eq!(jan, CalendarDate::new(2024, 12, 15));
        assert_eq!(jan.next_month(), CalendarDate::new(2025, 1, 15));
    }

    #[test]
    fn weekday_matches_known_dates() {
        // 2024-01-01 was Monday (1), 2024-02-29 was Thursday (4)
        assert_eq!(weekday_sunday_zero(2024, 1, 1), 1);
        assert_eq!(weekday_sunday_zero(2024, 2, 29), 4);
    }

    #[test]
    fn time_of_day_wraps_cleanly() {
        let t = TimeOfDay::new(23, 55);
        assert_eq!(t.add_hours(2), TimeOfDay::new(1, 55));
        assert_eq!(t.add_hours(-25), TimeOfDay::new(22, 55));
        assert_eq!(t.add_minutes(10), TimeOfDay::new(23, 5));
    }

    #[test]
    fn calendar_leading_blank_days_respects_first_day_of_week() {
        // 2024-01-01 was Monday (weekday_sunday_zero == 1)
        assert_eq!(calendar_leading_blank_days(2024, 1, 0), 1);
        assert_eq!(calendar_leading_blank_days(2024, 1, 1), 0);

        // 2024-09-01 was Sunday (weekday_sunday_zero == 0)
        assert_eq!(calendar_leading_blank_days(2024, 9, 0), 0);
        assert_eq!(calendar_leading_blank_days(2024, 9, 1), 6);
    }
}
