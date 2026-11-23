//! Overlays, tooltips, toasts, and date/time pickers page.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn overlays_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let trigger_row = elevated_card(
        flex_col((
            title_medium("弹窗、抽屉与提示"),
            flex_row((
                primary_button("基础弹窗", |s: &mut GalleryState| {
                    s.overlays.show_basic_dialog = true;
                }),
                secondary_button("原生警告", |s: &mut GalleryState| {
                    s.overlays.show_platform_alert = true;
                }),
                destructive_button("危险确认", |s: &mut GalleryState| {
                    s.overlays.show_destructive_dialog = true;
                })
                .leading_icon(icons::DELETE),
                secondary_button("底抽屉", |s: &mut GalleryState| {
                    s.overlays.show_bottom_sheet = true;
                }),
                secondary_button("侧抽屉", |s: &mut GalleryState| {
                    s.overlays.show_side_sheet = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((
                outlined_button("日期弹窗", |s: &mut GalleryState| {
                    s.overlays.show_date_dialog = true;
                })
                .leading_icon(icons::CALENDAR),
                outlined_button("时间弹窗", |s: &mut GalleryState| {
                    s.overlays.show_time_dialog = true;
                })
                .leading_icon(icons::SCHEDULE),
                outlined_button("轻提示", |s: &mut GalleryState| {
                    let theme_name = s.theme.language.label();
                    s.overlays.toasts.show(
                        format!("主题：{theme_name}"),
                        Some("关闭"),
                        ToastDuration::Short,
                        true,
                    );
                }),
                plain_tooltip(
                    "普通提示",
                    subtle_button("悬停提示", |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    }),
                ),
                rich_tooltip(
                    Some("富提示"),
                    "移出自动关闭。",
                    subtle_button("富提示", |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    }),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let pickers_row = flex_row((
        outlined_card(
            flex_col((
                title_medium(state.overlays.selected_date.format_iso()),
                date_picker(state.overlays.selected_date, |s: &mut GalleryState, d| {
                    s.overlays.selected_date = d;
                }),
            ))
            .gap(8.px()),
        ),
        outlined_card(
            flex_col((
                title_medium(state.overlays.selected_time.format_24h()),
                time_picker(state.overlays.selected_time, |s: &mut GalleryState, t| {
                    s.overlays.selected_time = t;
                }),
            ))
            .gap(8.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(16.px());

    flex_col((trigger_row, pickers_row))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
