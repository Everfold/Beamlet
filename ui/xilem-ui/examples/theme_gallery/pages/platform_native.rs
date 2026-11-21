//! Platform-native signature component showcase across MD3, WinUI 3, macOS HIG, and Adwaita.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn platform_native_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let md3_section = filled_card(
        flex_col((
            title_medium("Material Design 3 Signature"),
            center_aligned_top_app_bar("Centered MD3 TopAppBar"),
            flex_row((
                fab(icons::ADD, FabColor::Primary, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                extended_fab(
                    icons::EDIT,
                    "Compose",
                    FabColor::Secondary,
                    |s: &mut GalleryState| s.click_count += 1,
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let fluent_info = state.info_bar_visible.then(|| {
        info_bar(
            InfoBarSeverity::Success,
            "WinUI 3 InfoBar",
            "Fluent Mica surfaces and 2px bottom input accent bars are active.",
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.info_bar_visible = false;
            })),
        )
    });

    let fluent_section = flex_col((
        title_medium("Windows 11 Fluent / WinUI 3 Signature"),
        fluent_info,
        settings_card(
            Some(icons::PALETTE),
            "Mica Backdrop Material",
            Some("Adapts base fill between #F3F3F3 Light and #202020 Dark"),
            switch(state.native_switch_a, |s: &mut GalleryState, v| {
                s.native_switch_a = v;
            }),
        ),
        settings_expander(
            Some(icons::SETTINGS),
            "Advanced Fluent Options",
            Some("CommandBar + expandable child rows"),
            state.expander_open,
            |s: &mut GalleryState| s.expander_open = !s.expander_open,
            None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
            vec![command_bar(
                vec![primary_button("Save", |s: &mut GalleryState| s.click_count += 1).boxed()],
                vec![subtle_button("Reset", |s: &mut GalleryState| s.click_count = 0).boxed()],
            )],
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let hig_section = flex_col((
        title_medium("macOS Apple HIG Signature"),
        macos_toolbar(
            "Inspector",
            vec![icon_button(icons::ARROW_BACK, |s: &mut GalleryState| s.click_count += 1).boxed()],
            vec![help_button(|s: &mut GalleryState| s.click_count += 1)],
        ),
        form_group(
            Some("SYSTEM APPEARANCE"),
            vec![
                form_row(
                    "Hardware Acceleration",
                    switch(state.native_switch_b, |s: &mut GalleryState, v| {
                        s.native_switch_b = v;
                    }),
                ),
                form_row(
                    "Default Action",
                    primary_button("Apply", |s: &mut GalleryState| s.click_count += 1),
                ),
            ],
            Some("Matches macOS compact 26px Aqua geometry & 3px focus halos."),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let adwaita_banner = state.banner_visible.then(|| {
        banner(
            "Libadwaita AdwBanner — Flat Alpha Overlays Enabled",
            Some("Dismiss"),
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.banner_visible = false;
            })),
            None,
        )
    });

    let adwaita_section = flex_col((
        title_medium("GNOME Libadwaita Signature"),
        header_bar("Preferences", Some("AdwHeaderBar 46px"), vec![], vec![]),
        adwaita_banner,
        preferences_group(
            Some("Desktop Behavior"),
            Some("AdwPreferencesGroup + AdwSwitchRow + AdwActionRow"),
            vec![
                switch_row(
                    "Enable Flat Overlays",
                    Some("Uses 7% hover / 14% active alpha veils"),
                    state.native_switch_a,
                    |s: &mut GalleryState, v| s.native_switch_a = v,
                ),
                action_row(
                    "Sync GNOME Account",
                    Some("Triggers row activation callback"),
                    Some(icons::SETTINGS),
                    None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
                    Some(std::sync::Arc::new(|s: &mut GalleryState| {
                        s.click_count += 1
                    })),
                ),
            ],
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    flex_col((
        headline_small("Platform-Specific Signature Components"),
        md3_section,
        fluent_section,
        hig_section,
        adwaita_section,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(18.px())
}
