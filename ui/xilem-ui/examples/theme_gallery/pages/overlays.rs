//! Overlays, tooltips, modal dialogs/sheets, toasts, and date/time pickers page.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn overlays_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let trigger_row = elevated_card(
        flex_col((
            title_medium("Modals, Tooltips & Toast Queue"),
            flex_row((
                primary_button("Open Basic Dialog", |s: &mut GalleryState| {
                    s.show_basic_dialog = true;
                }),
                secondary_button("Open Bottom Sheet", |s: &mut GalleryState| {
                    s.show_bottom_sheet = true;
                }),
                outlined_button("Queue Toast", |s: &mut GalleryState| {
                    let theme_name = s.theme.language.label();
                    s.toasts.show(
                        format!("Theme active: {theme_name}"),
                        Some("OK"),
                        ToastDuration::Short,
                        true,
                    );
                }),
                plain_tooltip(
                    "Passive hover tooltip backed by LayerStack",
                    subtle_button("Hover Plain Tip", |s: &mut GalleryState| s.click_count += 1),
                ),
                rich_tooltip(
                    Some("Rich Tooltip"),
                    "Hover surfaces automatically tear down on mouse leave.",
                    subtle_button("Hover Rich Tip", |s: &mut GalleryState| s.click_count += 1),
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
                title_medium(format!(
                    "Date Picker ({})",
                    state.selected_date.format_iso()
                )),
                date_picker(state.selected_date, |s: &mut GalleryState, d| {
                    s.selected_date = d;
                }),
            ))
            .gap(8.px()),
        ),
        outlined_card(
            flex_col((
                title_medium(format!(
                    "Time Picker ({})",
                    state.selected_time.format_24h()
                )),
                time_picker(state.selected_time, |s: &mut GalleryState, t| {
                    s.selected_time = t;
                }),
            ))
            .gap(8.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(16.px());

    flex_col((
        headline_small("Layered Overlays & Pickers"),
        trigger_row,
        pickers_row,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
