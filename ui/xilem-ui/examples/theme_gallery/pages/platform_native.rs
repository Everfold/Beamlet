//! Platform-native signature component showcase and open developer customization demo.

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, label, sized_box};
use xilem::{Color, WidgetView};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn platform_native_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let clicks = state.click_count;

    // Demonstrates open custom slots (`button_view`) and direct Xilem Style overrides
    // hitting concrete underlying Masonry widgets without Passthrough wrappers.
    let dev_control_card = outlined_card(
        flex_col((
            title_medium("Developer Control & Open Custom Slots"),
            body_small(
                "No Passthrough walls: chain .width()/.corner_radius()/.background_color() directly or pass arbitrary views.",
            ),
            flex_row((
                button_view(
                    ButtonVariant::Primary,
                    with_spec(|spec, _| {
                        let shortcut = spec.shortcut_convention.format_primary("S");
                        flex_row((
                            ui_icon(icons::CHECK, 16.0, spec.palette.on_accent),
                            label("Save Workspace").color(spec.palette.on_accent),
                            sized_box(
                                label(shortcut)
                                    .text_size(11.0)
                                    .color(spec.palette.on_accent),
                            )
                            .padding(Padding::from_vh(2.px(), 6.px()))
                            .border(spec.palette.on_accent, 1.px())
                            .corner_radius(4.px()),
                        ))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(8.px())
                    }),
                    |s: &mut GalleryState| s.click_count += 1,
                ),
                primary_button(format!("Wide Square Override ({clicks})"), |s: &mut GalleryState| {
                    s.click_count += 1;
                })
                .width(230.px())
                .corner_radius(3.px()),
                list_item_view(
                    flex_row((
                        ui_icon(icons::INFO, 16.0, Color::from_rgb8(0x4E, 0x8C, 0xE8)),
                        body_medium("Custom list_item_view slot").flex(1.0),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px()),
    )
    .corner_radius(18.px());

    let md3_section = filled_card(
        flex_col((
            title_medium("Material Design 3 Signature"),
            top_app_bar_slots(
                TopAppBarVariant::CenterAligned,
                "Centered MD3 TopAppBar",
                Some(icon_button(icons::MENU, |s: &mut GalleryState| s.click_count += 1).boxed()),
                vec![icon_button(icons::SEARCH, |s: &mut GalleryState| s.click_count += 1).boxed()],
            ),
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
            "Fluent Mica surfaces and reactive 2px bottom accent bars are active.",
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
            vec![
                command_bar(
                    vec![primary_button("Save", |s: &mut GalleryState| s.click_count += 1).boxed()],
                    vec![subtle_button("Reset", |s: &mut GalleryState| s.click_count = 0).boxed()],
                )
                .boxed(),
            ],
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let hig_section = flex_col((
        title_medium("macOS Apple HIG Signature"),
        macos_toolbar(
            "Inspector",
            vec![icon_button(icons::ARROW_BACK, |s: &mut GalleryState| s.click_count += 1).boxed()],
            vec![help_button(|s: &mut GalleryState| s.click_count += 1).boxed()],
        ),
        form_group(
            Some("SYSTEM APPEARANCE"),
            vec![
                form_row(
                    "Hardware Acceleration",
                    switch(state.native_switch_b, |s: &mut GalleryState, v| {
                        s.native_switch_b = v;
                    }),
                )
                .boxed(),
                form_row(
                    "Default Action",
                    primary_button("Apply", |s: &mut GalleryState| s.click_count += 1),
                )
                .boxed(),
            ],
            Some("Matches macOS compact 26px Aqua geometry & reactive 3px focus halos."),
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
                )
                .boxed(),
                clickable_action_row(
                    "Sync GNOME Account",
                    Some("Direct Button widget row returning Pod<widgets::Button>"),
                    Some(icons::SETTINGS),
                    None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
                    |s: &mut GalleryState| s.click_count += 1,
                )
                .boxed(),
            ],
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    flex_col((
        headline_small("Platform Native Suites & Customization"),
        dev_control_card,
        md3_section,
        fluent_section,
        hig_section,
        adwaita_section,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
