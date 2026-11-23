//! Overlays, tooltips, toasts, and date/time pickers page.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn overlays_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let basic_btn = |lbl| {
        primary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_basic_dialog = true;
        })
    };
    let native_alert_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_platform_alert = true;
        })
    };
    let delete_btn = |lbl| {
        destructive_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_destructive_dialog = true;
        })
        .leading_icon(icons::DELETE)
    };
    let bottom_sheet_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_bottom_sheet = true;
        })
    };
    let side_sheet_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_side_sheet = true;
        })
    };
    let date_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_date_dialog = true;
        })
        .leading_icon(icons::CALENDAR)
    };
    let time_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_time_dialog = true;
        })
        .leading_icon(icons::SCHEDULE)
    };
    let toast_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            let theme_name = s.theme.language.label();
            s.overlays.toasts.show(
                format!("主题：{theme_name}"),
                Some("关闭"),
                ToastDuration::Short,
                true,
            );
        })
    };
    let plain_tip_btn = || {
        plain_tooltip(
            "普通提示",
            subtle_button("悬停提示", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
        )
    };
    let rich_tip_btn = || {
        rich_tooltip(
            Some("富提示"),
            "移出自动关闭。",
            subtle_button("富提示", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
        )
    };

    let wide_triggers = flex_col((
        flex_row((
            basic_btn("基础弹窗"),
            native_alert_btn("原生警告"),
            delete_btn("危险确认"),
            bottom_sheet_btn("底抽屉"),
            side_sheet_btn("侧抽屉"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px()),
        flex_row((
            date_btn("日期弹窗"),
            time_btn("时间弹窗"),
            toast_btn("轻提示"),
            plain_tip_btn(),
            rich_tip_btn(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(12.px());

    let narrow_triggers = flex_col((
        flex_row((
            basic_btn("基础弹窗"),
            native_alert_btn("原生警告"),
            delete_btn("危险确认"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        flex_row((
            bottom_sheet_btn("底抽屉"),
            side_sheet_btn("侧抽屉"),
            toast_btn("轻提示"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        flex_row((date_btn("日期弹窗"), time_btn("时间弹窗")))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
        flex_row((plain_tip_btn(), rich_tip_btn()))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(8.px());

    let trigger_row = elevated_card(
        flex_col((
            title_medium("弹窗、抽屉与提示"),
            breakpoint_bin(520.0, wide_triggers, narrow_triggers),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let date_card = |d: CalendarDate| {
        outlined_card(
            flex_col((
                title_medium(d.format_iso()),
                date_picker(d, |s: &mut GalleryState, next| {
                    s.overlays.selected_date = next;
                }),
            ))
            .gap(8.px()),
        )
    };
    let time_card = |t: TimeOfDay| {
        outlined_card(
            flex_col((
                title_medium(t.format_24h()),
                time_picker(t, |s: &mut GalleryState, next| {
                    s.overlays.selected_time = next;
                }),
            ))
            .gap(8.px()),
        )
    };

    let pickers_row = breakpoint_bin(
        640.0,
        flex_row((
            date_card(state.overlays.selected_date),
            time_card(state.overlays.selected_time),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(16.px()),
        flex_col((
            date_card(state.overlays.selected_date),
            time_card(state.overlays.selected_time),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((trigger_row, pickers_row))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
