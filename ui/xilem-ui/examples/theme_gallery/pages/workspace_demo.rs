//! Realistic filterable project workbench page combining custom slots, drawers, destructive confirmation, and platform settings.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn workspace_demo_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let query = state.search_query.trim().to_ascii_lowercase();
    let accent_color = state.theme.to_spec().palette.accent;
    let total = state.workspace_items.len();
    let selected = state.selected_workspace_idx.min(total.saturating_sub(1));

    let rows: Vec<_> = state
        .workspace_items
        .iter()
        .enumerate()
        .filter(|(_, (title, desc, _))| {
            query.is_empty()
                || title.to_ascii_lowercase().contains(&query)
                || desc.to_ascii_lowercase().contains(&query)
        })
        .map(|(idx, (title, desc, status))| {
            let active = idx == selected;
            let row_content = flex_row((
                ui_icon(
                    if active { icons::STAR } else { icons::FOLDER },
                    18.0,
                    if active {
                        accent_color
                    } else {
                        xilem::Color::from_rgb8(0x78, 0x7E, 0x88)
                    },
                ),
                flex_col((title_small(*title), body_small(*desc)))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(2.px())
                    .flex(1.0),
                info_badge(
                    if active {
                        InfoBarSeverity::Success
                    } else {
                        InfoBarSeverity::Informational
                    },
                    Some(*status),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px());

            clickable_list_item_view(row_content, active, move |s: &mut GalleryState| {
                s.selected_workspace_idx = idx;
            })
            .boxed()
        })
        .collect();

    let filter_bar = flex_row((
        search_bar(state.search_query.clone(), |s: &mut GalleryState, val| {
            s.search_query = val;
        })
        .on_clear(|s: &mut GalleryState| s.search_query.clear())
        .flex(1.0),
        secondary_button("Inspect Drawer", |s: &mut GalleryState| {
            s.show_side_sheet = true;
        })
        .leading_icon(icons::DOCK_LEFT),
        destructive_button("Remove Selected", |s: &mut GalleryState| {
            s.show_destructive_dialog = true;
        })
        .leading_icon(icons::DELETE)
        .disabled(state.workspace_items.is_empty()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(10.px());

    let list_section = if rows.is_empty() {
        status_page(
            icons::SEARCH,
            "No matching targets",
            "Clear the search filter or add a new workspace item.",
            Some((
                "Clear Search",
                std::sync::Arc::new(|s: &mut GalleryState| s.search_query.clear()),
            )),
        )
        .boxed()
    } else {
        grouped_card(
            flex_col(rows)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(2.px()),
        )
        .boxed()
    };

    // Developer explicitly chooses native settings container per design language.
    let platform_settings = match state.theme.language {
        DesignLanguage::Md3 => filled_card(
            flex_col((
                title_medium("Material 3 Workspace Actions"),
                body_small("Combines M3 Floating Toolbar & Small/Large FAB ergonomics."),
                floating_toolbar(vec![
                    icon_button(icons::REFRESH, |s: &mut GalleryState| s.click_count += 1).boxed(),
                    icon_button(icons::COPY, |s: &mut GalleryState| s.click_count += 1).boxed(),
                    icon_button(icons::SHARE, |s: &mut GalleryState| s.click_count += 1).boxed(),
                    primary_button("Deploy Build", |s: &mut GalleryState| s.click_count += 1)
                        .leading_icon(icons::SEND)
                        .boxed(),
                ]),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(10.px()),
        )
        .boxed(),
        DesignLanguage::Fluent => settings_card(
            Some(icons::TERMINAL),
            "Concurrent Worker Threads",
            Some("Configured via WinUI 3 NumberBox inside SettingsCard"),
            number_box(
                state.fluent_number_val,
                1.0,
                64.0,
                2.0,
                |s: &mut GalleryState, next| s.fluent_number_val = next,
            ),
        )
        .boxed(),
        DesignLanguage::Hig => form_group(
            Some("MACOS BUILD SETTINGS"),
            vec![
                form_row(
                    "Build Workers",
                    stepper(
                        "Threads",
                        state.hig_stepper_val,
                        1,
                        32,
                        1,
                        |s: &mut GalleryState, next| s.hig_stepper_val = next,
                    ),
                )
                .boxed(),
                form_row(
                    "Toggle Inspector",
                    switch(state.show_hig_inspector, |s: &mut GalleryState, v| {
                        s.show_hig_inspector = v;
                    }),
                )
                .boxed(),
            ],
            Some("macOS Form rows inside Aqua grouped card."),
        )
        .boxed(),
        DesignLanguage::Adwaita => preferences_group(
            Some("Libadwaita Project Settings"),
            Some("Uses AdwComboRow, AdwEntryRow, and AdwExpanderRow"),
            vec![
                entry_row(
                    "Workspace Slug",
                    state.adw_entry_val.clone(),
                    |s: &mut GalleryState, txt| s.adw_entry_val = txt,
                )
                .boxed(),
                combo_row(
                    "Release Profile",
                    Some("Select build optimization target"),
                    &["Debug", "Release", "RelWithDebInfo"],
                    state.adw_combo_idx,
                    state.adw_combo_open,
                    |s: &mut GalleryState| s.adw_combo_open = !s.adw_combo_open,
                    |s: &mut GalleryState, idx| {
                        s.adw_combo_idx = idx;
                        s.adw_combo_open = false;
                    },
                    |s: &mut GalleryState| s.adw_combo_open = false,
                )
                .boxed(),
            ],
        )
        .boxed(),
    };

    flex_col((
        headline_small("Project & Release Workbench"),
        body_small("Demonstrates shared searchable state driving native drawers, destructive confirmation, and platform-specific settings."),
        filter_bar,
        list_section,
        platform_settings,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
}
