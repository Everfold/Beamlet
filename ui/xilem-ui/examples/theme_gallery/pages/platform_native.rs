//! Platform-native signature component showcase and open developer customization demo.

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, label, sized_box};
use xilem::{Color, WidgetView};
use xilem_ui::prelude::*;

use crate::state::{GalleryState, GalleryTab};

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
                small_fab(icons::EDIT, FabColor::Surface, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                fab(icons::ADD, FabColor::Primary, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                large_fab(icons::SEND, FabColor::Tertiary, |s: &mut GalleryState| {
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
            floating_toolbar(vec![
                icon_button(icons::ARROW_BACK, |s: &mut GalleryState| s.click_count += 1).boxed(),
                icon_button(icons::COPY, |s: &mut GalleryState| s.click_count += 1).boxed(),
                icon_button(icons::SHARE, |s: &mut GalleryState| s.click_count += 1).boxed(),
                primary_button("Publish", |s: &mut GalleryState| s.click_count += 1).boxed(),
            ]),
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

    let fluent_crumbs = ["Settings", "Personalization", "Colors & Mica", "Accent"];
    let fluent_depth = (state.fluent_breadcrumb_depth + 1).clamp(1, fluent_crumbs.len());

    let fluent_section = flex_col((
        title_medium("Windows 11 Fluent / WinUI 3 Signature"),
        breadcrumb_bar(
            &fluent_crumbs[..fluent_depth],
            |s: &mut GalleryState, idx| {
                s.fluent_breadcrumb_depth = idx;
            },
        ),
        fluent_info,
        settings_card(
            Some(icons::PALETTE),
            "Mica Backdrop Material",
            Some("Adapts base fill between #F3F3F3 Light and #202020 Dark"),
            flex_row((
                info_badge(InfoBarSeverity::Success, Some("Active")),
                switch(state.native_switch_a, |s: &mut GalleryState, v| {
                    s.native_switch_a = v;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ),
        settings_card(
            Some(icons::TUNE),
            "WinUI 3 NumberBox",
            Some("Spin step control clamped to [0, 100]"),
            number_box(
                state.fluent_number_val,
                0.0,
                100.0,
                4.0,
                |s: &mut GalleryState, v| s.fluent_number_val = v,
            ),
        ),
        expander(
            "WinUI 3 General Expander",
            Some("Wraps any custom nested view"),
            state.fluent_expander_open,
            |s: &mut GalleryState| s.fluent_expander_open = !s.fluent_expander_open,
            flex_row((
                info_badge(InfoBarSeverity::Warning, Some("Beta")),
                body_medium("Custom nested WinUI 3 Expander body with live InfoBadge."),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
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

    let hig_paths = [
        ("Macintosh HD", icons::FOLDER),
        ("Projects", icons::FOLDER),
        ("Beamlet", icons::CODE),
        ("UI", icons::DESCRIPTION),
    ];
    let hig_depth = (state.hig_path_depth + 1).clamp(1, hig_paths.len());

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
                    "Step Density",
                    stepper(
                        "Columns",
                        state.hig_stepper_val,
                        1,
                        16,
                        1,
                        |s: &mut GalleryState, v| s.hig_stepper_val = v,
                    ),
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
        disclosure_group(
            "Inspector Metadata (DisclosureGroup)",
            state.hig_disclosure_open,
            |s: &mut GalleryState| s.hig_disclosure_open = !s.hig_disclosure_open,
            body_medium("Matches NSDisclosureButton / SwiftUI DisclosureGroup indentation."),
        ),
        path_control(&hig_paths[..hig_depth], |s: &mut GalleryState, idx| {
            s.hig_path_depth = idx;
        }),
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
        header_bar_with_title(
            view_switcher(
                &[("Overview", icons::HOME), ("Native", icons::PALETTE)],
                0,
                |s: &mut GalleryState, idx| s.active_tab = GalleryTab::from_index(idx),
            ),
            vec![],
            vec![],
        ),
        adwaita_banner,
        clamp(
            620.0,
            preferences_group(
                Some("Desktop Behavior"),
                Some("AdwClamp + AdwPreferencesGroup + AdwEntryRow + AdwComboRow + AdwExpanderRow"),
                vec![
                    entry_row(
                        "Application Name",
                        state.adw_entry_val.clone(),
                        |s: &mut GalleryState, v| s.adw_entry_val = v,
                    )
                    .boxed(),
                    combo_row(
                        "Rendering Backend",
                        Some("AdwComboRow inline dropdown"),
                        &["Vello WGPU", "Vello CPU", "Software"],
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
                    switch_row(
                        "Enable Flat Overlays",
                        Some("Uses 7% hover / 14% active alpha veils"),
                        state.native_switch_a,
                        |s: &mut GalleryState, v| s.native_switch_a = v,
                    )
                    .boxed(),
                    expander_row(
                        "Developer Sub-settings",
                        Some("AdwExpanderRow indented child list"),
                        Some(icons::CODE),
                        state.adw_expander_open,
                        |s: &mut GalleryState| s.adw_expander_open = !s.adw_expander_open,
                        vec![
                            clickable_action_row(
                                "Sync GNOME Account",
                                Some("Direct Button widget row returning Pod<widgets::Button>"),
                                Some(icons::SETTINGS),
                                None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
                                |s: &mut GalleryState| s.click_count += 1,
                            )
                            .boxed(),
                        ],
                    )
                    .boxed(),
                ],
            ),
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
