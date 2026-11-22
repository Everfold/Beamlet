//! Built-in multi-theme scene catalog and snapshot/preview orchestration helpers.

use std::sync::Arc;

use anyhow::bail;
use image::RgbaImage;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, label, portal, sized_box, zstack,
};
use xilem::{AnyWidgetView, Color, WidgetView};
use xilem_ui::prelude::*;

use crate::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use crate::inspect::{
    ContrastIssue, LayoutIssue, LayoutNode, dump_layout_tree, lint_layout, lint_spec_contrast,
};

/// Metadata describing a built-in preview scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneInfo {
    /// CLI `--scene` identifier.
    pub id: &'static str,
    /// Human-readable description printed by `--list`.
    pub description: &'static str,
}

/// All registered preview scenes usable with `--scene`.
pub const ALL_SCENES: &[SceneInfo] = &[
    SceneInfo {
        id: "overview",
        description: "Buttons (incl. destructive/disabled), icon buttons, progress, tabs, chips",
    },
    SceneInfo {
        id: "inputs",
        description: "Text fields with FocusFrame, search bar, selection controls, sliders, select",
    },
    SceneInfo {
        id: "overlays",
        description: "Modal/destructive dialogs, bottom/side sheets, date/time pickers, toasts",
    },
    SceneInfo {
        id: "tokens",
        description: "DesignSpec 15-role typography ramp, status/surface swatches, metrics",
    },
    SceneInfo {
        id: "platform-native",
        description: "MD3, Windows Fluent, macOS HIG, and GNOME Adwaita signature widgets",
    },
    SceneInfo {
        id: "workspace",
        description: "Authentic 4-platform native desktop workspace shells + searchable workbench",
    },
];

/// Complete result of rendering one preview scene + theme combination.
#[derive(Clone, Debug)]
pub struct SceneOutput {
    /// Rasterized RGBA frame from `vello_cpu`.
    pub image: RgbaImage,
    /// Snapshot of the laid-out widget tree.
    pub tree: LayoutNode,
    /// Automated geometry lint findings.
    pub layout_issues: Vec<LayoutIssue>,
    /// Automated WCAG 2.1 color contrast findings on active `DesignSpec`.
    pub contrast_issues: Vec<ContrastIssue>,
}

/// Shared mutable state driving all built-in preview scenes.
#[derive(Clone, Debug)]
pub struct SceneState {
    /// Active multi-theme configuration.
    pub theme: UiTheme,
    /// Selected workspace tab index (`0..=5`).
    pub active_tab: usize,
    /// Shared click counter.
    pub click_count: u32,
    /// Accent menu state.
    pub accent_menu_open: bool,
    /// Primary text field value.
    pub text_value: String,
    /// Error-state text field value.
    pub email_value: String,
    /// Search bar value.
    pub search_query: String,
    /// Checkbox toggle.
    pub agree_terms: bool,
    /// Tri-state checkbox toggle.
    pub tri_state: ToggleableState,
    /// Switch toggle.
    pub switch_on: bool,
    /// Radio index.
    pub radio_choice: usize,
    /// Linear slider value.
    pub slider_val: f64,
    /// Dual-thumb range slider values.
    pub range_val: (f64, f64),
    /// Filter chip active flags.
    pub chip_flags: [bool; 3],
    /// Multi-choice segmented button flags.
    pub multi_seg_mask: [bool; 3],
    /// Removable tag chips.
    pub tags: Vec<String>,
    /// Primary tab strip index.
    pub selected_tab: usize,
    /// Secondary tab strip index.
    pub secondary_tab: usize,
    /// Select dropdown state.
    pub select_open: bool,
    /// Select dropdown index.
    pub select_index: usize,
    /// Selected calendar date.
    pub selected_date: CalendarDate,
    /// Selected time of day.
    pub selected_time: TimeOfDay,
    /// Basic dialog visibility.
    pub show_basic_dialog: bool,
    /// Destructive dialog visibility.
    pub show_destructive_dialog: bool,
    /// Bottom sheet visibility.
    pub show_bottom_sheet: bool,
    /// Side sheet visibility.
    pub show_side_sheet: bool,
    /// Date picker dialog visibility.
    pub show_date_dialog: bool,
    /// Time picker dialog visibility.
    pub show_time_dialog: bool,
    /// Toast queue state.
    pub toasts: ToastHostState,
    /// Fluent `InfoBar` visibility.
    pub info_bar_visible: bool,
    /// Fluent `SettingsExpander` state.
    pub expander_open: bool,
    /// Fluent `NumberBox` value.
    pub fluent_number_val: f64,
    /// HIG inspector visibility.
    pub show_hig_inspector: bool,
    /// HIG disclosure group visibility.
    pub hig_disclosure_open: bool,
    /// HIG stepper value.
    pub hig_stepper_val: i32,
    /// Adwaita banner visibility.
    pub banner_visible: bool,
    /// Adwaita expander row state.
    pub adw_expander_open: bool,
    /// Adwaita combo row open state.
    pub adw_combo_open: bool,
    /// Adwaita combo row selection.
    pub adw_combo_idx: usize,
    /// Adwaita entry row text.
    pub adw_entry_val: String,
    /// Selected workbench row.
    pub selected_row: usize,
}

impl SceneState {
    /// Creates default state initialized with `theme`.
    #[must_use]
    pub fn with_theme(theme: UiTheme) -> Self {
        Self {
            theme,
            active_tab: 0,
            click_count: 3,
            accent_menu_open: false,
            text_value: "Beamlet UI System".into(),
            email_value: "invalid-email".into(),
            search_query: String::new(),
            agree_terms: true,
            tri_state: ToggleableState::Indeterminate,
            switch_on: true,
            radio_choice: 0,
            slider_val: 0.45,
            range_val: (25.0, 75.0),
            chip_flags: [true, false, true],
            multi_seg_mask: [true, false, true],
            tags: vec![
                "MD3".into(),
                "WinUI3".into(),
                "Aqua".into(),
                "Adwaita".into(),
            ],
            selected_tab: 0,
            secondary_tab: 1,
            select_open: false,
            select_index: 1,
            selected_date: CalendarDate::new(2026, 11, 22),
            selected_time: TimeOfDay::new(14, 30),
            show_basic_dialog: false,
            show_destructive_dialog: false,
            show_bottom_sheet: false,
            show_side_sheet: false,
            show_date_dialog: false,
            show_time_dialog: false,
            toasts: ToastHostState::new(),
            info_bar_visible: true,
            expander_open: true,
            fluent_number_val: 24.0,
            show_hig_inspector: true,
            hig_disclosure_open: true,
            hig_stepper_val: 8,
            banner_visible: true,
            adw_expander_open: true,
            adw_combo_open: false,
            adw_combo_idx: 0,
            adw_entry_val: "beamlet-workspace".into(),
            selected_row: 0,
        }
    }
}

fn render_overview_page(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let clicks = state.click_count;
    let prog = state.slider_val;

    let buttons_card = elevated_card(
        flex_col((
            title_medium("Buttons & Icon Actions"),
            flex_row((
                primary_button("Primary", |s: &mut SceneState| s.click_count += 1)
                    .leading_icon(icons::ADD),
                secondary_button("Secondary", |s: &mut SceneState| s.click_count += 1),
                elevated_button("Elevated", |s: &mut SceneState| s.click_count += 1),
                outlined_button("Outlined", |s: &mut SceneState| s.click_count += 1),
                subtle_button("Subtle", |s: &mut SceneState| s.click_count += 1),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
            flex_row((
                destructive_button("Delete", |s: &mut SceneState| {
                    s.show_destructive_dialog = true;
                })
                .leading_icon(icons::DELETE),
                primary_button("Disabled", |s: &mut SceneState| s.click_count += 1).disabled(true),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
            flex_row((
                icon_button(icons::FAVORITE, |s: &mut SceneState| s.click_count += 1),
                filled_icon_button(icons::BOOKMARK, |s: &mut SceneState| s.click_count += 1),
                tonal_icon_button(icons::SETTINGS, |s: &mut SceneState| s.click_count += 1),
                outlined_icon_button(icons::SEARCH, |s: &mut SceneState| s.click_count += 1),
                with_badge(
                    icon_button(icons::NOTIFICATIONS, |s: &mut SceneState| {
                        s.click_count += 1
                    }),
                    Some(large_badge(format!("{clicks}"))),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let progress_card = outlined_card(
        flex_col((
            title_medium("Progress & Tonal Surfaces"),
            body_small(format!(
                "Action count: {clicks} · Progress: {:.0}%",
                prog * 100.0
            )),
            linear_progress(Some(prog)),
            flex_row((
                determinate_circular_progress(prog),
                circular_progress(),
                small_circular_progress(),
                surface_box(
                    ElevationLevel::Level2,
                    body_medium("Elevated Level 2 Surface"),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let tag_chips: Vec<Box<AnyWidgetView<SceneState>>> = state
        .tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            input_chip(tag.clone(), |s: &mut SceneState| s.click_count += 1)
                .on_remove(move |s: &mut SceneState| {
                    if i < s.tags.len() {
                        s.tags.remove(i);
                    }
                })
                .boxed()
        })
        .collect();

    let chips_card = filled_card(
        flex_col((
            title_medium("Tabs, Segmented Controls & Chips"),
            tabs(
                &["General", "Appearance", "Shortcuts"],
                state.selected_tab,
                |s: &mut SceneState, idx| s.selected_tab = idx,
            ),
            secondary_tabs(
                &["Overview", "Spec", "Changelog"],
                state.secondary_tab,
                |s: &mut SceneState, idx| s.secondary_tab = idx,
            ),
            multi_choice_segmented_button(
                &[
                    ("Bold", state.multi_seg_mask[0]),
                    ("Italic", state.multi_seg_mask[1]),
                    ("Code", state.multi_seg_mask[2]),
                ],
                |s: &mut SceneState, idx| s.multi_seg_mask[idx] = !s.multi_seg_mask[idx],
            ),
            flex_row((
                assist_chip("Assist", |s: &mut SceneState| s.click_count += 1)
                    .leading_icon(icons::PALETTE),
                filter_chip("Filter A", state.chip_flags[0], |s: &mut SceneState| {
                    s.chip_flags[0] = !s.chip_flags[0];
                }),
                suggestion_chip("Suggestion", |s: &mut SceneState| s.click_count += 1),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
            flex_row(tag_chips)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    flex_col((
        headline_small("Shared Cross-Theme Component Matrix"),
        buttons_card,
        progress_card,
        chips_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
    .boxed()
}

fn render_inputs_page(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let choice = state.radio_choice;
    let range = state.range_val;

    let text_card = outlined_card(
        flex_col((
            title_medium("Text Inputs (Reactive FocusFrame)"),
            search_bar(state.search_query.clone(), |s: &mut SceneState, v| {
                s.search_query = v;
            })
            .on_clear(|s: &mut SceneState| s.search_query.clear()),
            text_field(state.text_value.clone(), |s: &mut SceneState, v| {
                s.text_value = v;
            })
            .label("Workspace Name")
            .prefix_text("https://")
            .max_chars(48)
            .supporting_text("Compare MD3 Outline vs Fluent Bottom Accent vs HIG Halo"),
            filled_text_field(state.email_value.clone(), |s: &mut SceneState, v| {
                s.email_value = v;
            })
            .label("Validation Demo")
            .error_text("Enter a valid email address"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let selection_card = elevated_card(
        flex_col((
            title_medium("Selection, Sliders & Dropdown"),
            flex_row((
                checkbox(
                    "Accept License",
                    state.agree_terms,
                    |s: &mut SceneState, v| {
                        s.agree_terms = v;
                    },
                ),
                tri_state_checkbox("Group Sync", state.tri_state, |s: &mut SceneState, v| {
                    s.tri_state = v;
                }),
                switch(state.switch_on, |s: &mut SceneState, v| s.switch_on = v),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px()),
            radio_group(vec![
                radio("Standard Density", choice == 0, |s: &mut SceneState| {
                    s.radio_choice = 0;
                }),
                radio("Compact Desktop", choice == 1, |s: &mut SceneState| {
                    s.radio_choice = 1;
                }),
            ]),
            slider(0.0, 1.0, state.slider_val, None, |s: &mut SceneState, v| {
                s.slider_val = v;
            }),
            range_slider(0.0, 100.0, range, |s: &mut SceneState, v| s.range_val = v),
            select(
                &["Auto Detect", "Force Light", "Force Dark"],
                state.select_index,
                state.select_open,
                |s: &mut SceneState| s.select_open = !s.select_open,
                |s: &mut SceneState, i| {
                    s.select_index = i;
                    s.select_open = false;
                },
                |s: &mut SceneState| s.select_open = false,
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    flex_col((
        headline_small("Text Fields & Selection Controls"),
        text_card,
        selection_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
    .boxed()
}

fn render_overlays_page(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let trigger_card = elevated_card(
        flex_col((
            title_medium("Modals, Sheets, Pickers & Tooltips"),
            flex_row((
                primary_button("Open Basic Dialog", |s: &mut SceneState| {
                    s.show_basic_dialog = true;
                }),
                destructive_button("Destructive Dialog", |s: &mut SceneState| {
                    s.show_destructive_dialog = true;
                })
                .leading_icon(icons::DELETE),
                secondary_button("Bottom Sheet", |s: &mut SceneState| {
                    s.show_bottom_sheet = true;
                }),
                secondary_button("Side Sheet", |s: &mut SceneState| {
                    s.show_side_sheet = true;
                }),
                outlined_button("Date Dialog", |s: &mut SceneState| {
                    s.show_date_dialog = true;
                }),
                outlined_button("Time Dialog", |s: &mut SceneState| {
                    s.show_time_dialog = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px()),
    );

    let pickers = flex_row((
        outlined_card(
            flex_col((
                title_medium(format!(
                    "Date Picker ({})",
                    state.selected_date.format_iso()
                )),
                date_picker(state.selected_date, |s: &mut SceneState, d| {
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
                time_picker(state.selected_time, |s: &mut SceneState, t| {
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
        trigger_card,
        pickers,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
    .boxed()
}

fn swatch<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    name: &'static str,
    fill: Color,
    ink: Color,
) -> Box<AnyWidgetView<State, Action>> {
    sized_box(
        spec.typography
            .style_label(TypeRole::LabelSmall, label(name))
            .color(ink),
    )
    .padding(Padding::from_vh(8.px(), 10.px()))
    .background_color(fill)
    .border(spec.palette.border, 1.px())
    .corner_radius(6.px())
    .dims(Dimensions::fixed(155.px(), 38.px()))
    .boxed()
}

fn render_tokens_page() -> Box<AnyWidgetView<SceneState>> {
    let swatches = with_spec(|spec: &DesignSpec, _| {
        let p = &spec.palette;
        flex_col((
            flex_row((
                swatch(spec, "accent", p.accent, p.on_accent),
                swatch(
                    spec,
                    "accent_container",
                    p.accent_container,
                    p.on_accent_container,
                ),
                swatch(spec, "secondary", p.secondary, p.on_secondary),
                swatch(
                    spec,
                    "secondary_container",
                    p.secondary_container,
                    p.on_secondary_container,
                ),
            ))
            .gap(8.px()),
            flex_row((
                swatch(spec, "success", p.success, p.on_success),
                swatch(spec, "warning", p.warning, p.on_warning),
                swatch(spec, "error", p.error, p.on_error),
                swatch(spec, "window_bg", p.window_bg, p.text_primary),
            ))
            .gap(8.px()),
            flex_row((
                swatch(spec, "surface_low", p.surface_low, p.text_primary),
                swatch(spec, "surface", p.surface, p.text_primary),
                swatch(spec, "surface_high", p.surface_high, p.text_primary),
                swatch(spec, "header_bar_bg", p.header_bar_bg, p.text_primary),
            ))
            .gap(8.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(8.px())
    });

    let ramp = outlined_card(
        flex_col((
            title_medium("15-Role Typographic Ramp"),
            display_small("Display Small"),
            headline_medium("Headline Medium"),
            title_large("Title Large"),
            title_medium("Title Medium"),
            body_large("Body Large — Primary reading copy"),
            body_medium("Body Medium — Compact desktop row copy"),
            label_large("Label Large — Button & Tab Caption"),
            label_small("Label Small — Shortcut Hint"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.px()),
    );

    flex_col((
        headline_small("DesignSpec Tokens & Semantics"),
        filled_card(swatches),
        ramp,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
    .boxed()
}

fn render_platform_native_page(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let md3_card = filled_card(
        flex_col((
            title_medium("Material Design 3 Signature"),
            flex_row((
                small_fab(icons::EDIT, FabColor::Surface, |s: &mut SceneState| {
                    s.click_count += 1
                }),
                fab(icons::ADD, FabColor::Primary, |s: &mut SceneState| {
                    s.click_count += 1
                }),
                large_fab(icons::SEND, FabColor::Tertiary, |s: &mut SceneState| {
                    s.click_count += 1
                }),
                extended_fab(
                    icons::EDIT,
                    "Compose",
                    FabColor::Secondary,
                    |s: &mut SceneState| {
                        s.click_count += 1;
                    },
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
            floating_toolbar(vec![
                icon_button(icons::COPY, |s: &mut SceneState| s.click_count += 1).boxed(),
                icon_button(icons::SHARE, |s: &mut SceneState| s.click_count += 1).boxed(),
                primary_button("Publish", |s: &mut SceneState| s.click_count += 1).boxed(),
            ]),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px()),
    );

    let fluent_card = flex_col((
        title_medium("Windows Fluent / WinUI 3"),
        breadcrumb_bar(
            &["Settings", "Personalization", "Colors"],
            |_: &mut SceneState, _| {},
        ),
        settings_card(
            Some(icons::TUNE),
            "WinUI 3 NumberBox + InfoBadge",
            Some("Clamped spin control inside SettingsCard"),
            flex_row((
                info_badge(InfoBarSeverity::Success, Some("Ready")),
                number_box(
                    state.fluent_number_val,
                    0.0,
                    100.0,
                    4.0,
                    |s: &mut SceneState, v| {
                        s.fluent_number_val = v;
                    },
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let hig_card = form_group(
        Some("MACOS HIG"),
        vec![
            form_row(
                "Workers",
                stepper(
                    "Count",
                    state.hig_stepper_val,
                    1,
                    16,
                    1,
                    |s: &mut SceneState, v| {
                        s.hig_stepper_val = v;
                    },
                ),
            )
            .boxed(),
            disclosure_group(
                "Details",
                state.hig_disclosure_open,
                |s: &mut SceneState| s.hig_disclosure_open = !s.hig_disclosure_open,
                body_small("Disclosed Aqua sub-row."),
            )
            .boxed(),
        ],
        None,
    );

    let adw_card = clamp(
        620.0,
        preferences_group(
            Some("GNOME Libadwaita"),
            Some("AdwClamp + AdwEntryRow + AdwComboRow + AdwExpanderRow"),
            vec![
                entry_row(
                    "App Slug",
                    state.adw_entry_val.clone(),
                    |s: &mut SceneState, v| {
                        s.adw_entry_val = v;
                    },
                )
                .boxed(),
                combo_row(
                    "Backend",
                    None::<&str>,
                    &["Vello WGPU", "Vello CPU"],
                    state.adw_combo_idx,
                    state.adw_combo_open,
                    |s: &mut SceneState| s.adw_combo_open = !s.adw_combo_open,
                    |s: &mut SceneState, i| {
                        s.adw_combo_idx = i;
                        s.adw_combo_open = false;
                    },
                    |s: &mut SceneState| s.adw_combo_open = false,
                )
                .boxed(),
                expander_row(
                    "More Options",
                    None::<&str>,
                    Some(icons::CODE),
                    state.adw_expander_open,
                    |s: &mut SceneState| s.adw_expander_open = !s.adw_expander_open,
                    vec![
                        switch_row(
                            "Flat Alpha",
                            None::<&str>,
                            state.switch_on,
                            |s: &mut SceneState, v| {
                                s.switch_on = v;
                            },
                        )
                        .boxed(),
                    ],
                )
                .boxed(),
            ],
        ),
    );

    flex_col((
        headline_small("Platform Native Suites"),
        md3_card,
        fluent_card,
        hig_card,
        adw_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
    .boxed()
}

fn wrap_overlays<V: WidgetView<SceneState>>(
    state: &mut SceneState,
    base: V,
    dock: Option<Box<AnyWidgetView<SceneState>>>,
) -> Box<AnyWidgetView<SceneState>> {
    let basic_dlg = state.show_basic_dialog.then(|| {
        basic_dialog(
            "Reset theme preferences?",
            "Restores the default light preset cleanly.",
            "Reset",
            |s: &mut SceneState| s.show_basic_dialog = false,
            |s: &mut SceneState| s.show_basic_dialog = false,
        )
        .icon(icons::PALETTE)
        .dismiss_text("Cancel")
    });
    let destructive_dlg = state.show_destructive_dialog.then(|| {
        basic_dialog(
            "Delete selected target?",
            "Demonstrates error-styled destructive confirmation.",
            "Delete",
            |s: &mut SceneState| s.show_destructive_dialog = false,
            |s: &mut SceneState| s.show_destructive_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("Cancel")
    });
    let sheet = state.show_bottom_sheet.then(|| {
        bottom_sheet(
            flex_col((
                headline_small("Bottom Sheet"),
                body_medium("Modal bottom sheet animated with SlideTransition."),
                primary_button("Close Sheet", |s: &mut SceneState| {
                    s.show_bottom_sheet = false;
                }),
            ))
            .gap(10.px()),
            |s: &mut SceneState| s.show_bottom_sheet = false,
        )
    });
    let drawer = state.show_side_sheet.then(|| {
        side_sheet(
            flex_col((
                headline_small("Side Sheet Drawer"),
                body_medium("Trailing inspector sheet backed by modal_shell."),
                primary_button("Close Drawer", |s: &mut SceneState| {
                    s.show_side_sheet = false;
                }),
            ))
            .gap(10.px()),
            |s: &mut SceneState| s.show_side_sheet = false,
        )
        .extent(320.0)
    });
    let date_dlg = state.show_date_dialog.then(|| {
        date_picker_dialog(
            state.selected_date,
            |s: &mut SceneState, d| s.selected_date = d,
            |s: &mut SceneState| s.show_date_dialog = false,
            |s: &mut SceneState| s.show_date_dialog = false,
        )
    });
    let time_dlg = state.show_time_dialog.then(|| {
        time_picker_dialog(
            state.selected_time,
            |s: &mut SceneState, t| s.selected_time = t,
            |s: &mut SceneState| s.show_time_dialog = false,
            |s: &mut SceneState| s.show_time_dialog = false,
        )
    });

    let dock_layer = dock.map(|d| {
        sized_box(d)
            .padding(Padding::all(18.px()))
            .alignment(UnitPoint::BOTTOM_RIGHT)
    });

    zstack((
        base.alignment(UnitPoint::CENTER),
        dock_layer,
        sheet.map(|v| v.alignment(UnitPoint::CENTER)),
        drawer.map(|v| v.alignment(UnitPoint::CENTER)),
        date_dlg.map(|v| v.alignment(UnitPoint::CENTER)),
        time_dlg.map(|v| v.alignment(UnitPoint::CENTER)),
        basic_dlg.map(|v| v.alignment(UnitPoint::CENTER)),
        destructive_dlg.map(|v| v.alignment(UnitPoint::CENTER)),
    ))
    .boxed()
}

fn workspace_body(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let items = [
        (
            "Core Design Engine",
            "FocusFrame & PropertyStack defaults",
            "Active",
        ),
        (
            "Windows Mica Shell",
            "NavigationView + BreadcrumbBar",
            "Synced",
        ),
        (
            "macOS Aqua Inspector",
            "Source List + 38px Toolbar",
            "Ready",
        ),
        (
            "GNOME Adwaita Boxed",
            "AdwClamp + EntryRow + ComboRow",
            "Updated",
        ),
    ];
    let selected = state.selected_row;
    let rows: Vec<Box<AnyWidgetView<SceneState>>> = items
        .iter()
        .enumerate()
        .map(|(i, (title, desc, badge))| {
            let active = i == selected;
            clickable_list_item_view(
                flex_row((
                    ui_icon(
                        if active { icons::STAR } else { icons::FOLDER },
                        18.0,
                        state.theme.to_spec().palette.accent,
                    ),
                    flex_col((title_small(*title), body_small(*desc)))
                        .cross_axis_alignment(CrossAxisAlignment::Start)
                        .flex(1.0),
                    info_badge(
                        if active {
                            InfoBarSeverity::Success
                        } else {
                            InfoBarSeverity::Informational
                        },
                        Some(*badge),
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px()),
                active,
                move |s: &mut SceneState| s.selected_row = i,
            )
            .boxed()
        })
        .collect();

    let card = grouped_card(
        flex_col(rows)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(2.px()),
    );

    let overview_bit = render_overview_page(state);
    portal(
        sized_box(
            flex_col((card, overview_bit))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(16.px()),
        )
        .padding(Padding {
            left: 18.px(),
            top: 18.px(),
            right: 24.px(),
            bottom: 18.px(),
        }),
    )
    .constrain_horizontal(true)
    .boxed()
}

fn render_workspace_scene(state: &mut SceneState) -> Box<AnyWidgetView<SceneState>> {
    let dests = [
        NavDestination::new("Overview", icons::HOME),
        NavDestination::new("Inputs", icons::EDIT),
        NavDestination::new("Tokens", icons::TUNE),
        NavDestination::new("Native", icons::PALETTE),
    ];

    match state.theme.language {
        DesignLanguage::Md3 => {
            let rail = navigation_rail_with_fab(
                Some(small_fab(
                    icons::ADD,
                    FabColor::Tertiary,
                    |s: &mut SceneState| {
                        s.click_count += 1;
                    },
                )),
                &dests,
                state.active_tab,
                |s: &mut SceneState, i| s.active_tab = i,
            );
            let bar = top_app_bar_slots(
                TopAppBarVariant::Small,
                "Beamlet Material 3",
                None,
                vec![
                    icon_button(icons::PALETTE, |s: &mut SceneState| {
                        s.theme.toggle_brightness();
                    })
                    .boxed(),
                ],
            );
            let main = flex_col((bar, horizontal_divider(), workspace_body(state).flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);
            let dock = flex_row((
                floating_toolbar(vec![
                    icon_button(icons::EDIT, |s: &mut SceneState| s.click_count += 1).boxed(),
                    icon_button(icons::TUNE, |s: &mut SceneState| s.click_count += 1).boxed(),
                ]),
                extended_fab(
                    icons::ADD,
                    "New Action",
                    FabColor::Primary,
                    |s: &mut SceneState| {
                        s.click_count += 1;
                    },
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px())
            .dims(Dimensions::MAX)
            .boxed();
            wrap_overlays(
                state,
                flex_row((rail, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch),
                Some(dock),
            )
        }
        DesignLanguage::Fluent => {
            let nav = navigation_view(
                Some("WinUI 3 Shell"),
                &dests[..3],
                &dests[3..],
                state.active_tab,
                |s: &mut SceneState, i| s.active_tab = i,
            );
            let bc = breadcrumb_bar(
                &["Beamlet", "Gallery", "Overview"],
                |_: &mut SceneState, _| {},
            );
            let cmd = command_bar(
                vec![primary_button("New", |s: &mut SceneState| s.click_count += 1).boxed()],
                vec![
                    outlined_button("Dialog", |s: &mut SceneState| s.show_basic_dialog = true)
                        .boxed(),
                ],
            );
            let notice = state.info_bar_visible.then(|| {
                info_bar(
                    InfoBarSeverity::Informational,
                    "Fluent Mica Shell",
                    "NavigationView with footer pin + BreadcrumbBar + CommandBar.",
                    Some(Arc::new(|s: &mut SceneState| s.info_bar_visible = false)),
                )
            });
            let main = flex_col((
                sized_box(flex_col((bc, cmd, notice)).gap(8.px()))
                    .padding(Padding::from_vh(10.px(), 16.px())),
                horizontal_divider(),
                workspace_body(state).flex(1.0),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch);
            wrap_overlays(
                state,
                flex_row((nav, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch),
                None,
            )
        }
        DesignLanguage::Hig => {
            let sections = [
                (
                    "SHOWCASE",
                    vec![
                        ("Overview", icons::HOME),
                        ("Inputs", icons::EDIT),
                        ("Tokens", icons::TUNE),
                    ],
                ),
                ("SYSTEM", vec![("Native Suites", icons::PALETTE)]),
            ];
            let sidebar = source_list_sidebar(
                &sections,
                (0, state.active_tab.min(2)),
                |s: &mut SceneState, r| {
                    s.active_tab = r.1;
                },
            );
            let toolbar = macos_toolbar_with_center(
                vec![icon_button(icons::ARROW_BACK, |s: &mut SceneState| s.active_tab = 0).boxed()],
                search_bar(state.search_query.clone(), |s: &mut SceneState, v| {
                    s.search_query = v;
                })
                .width(240.px()),
                vec![
                    icon_button(icons::DOCK_LEFT, |s: &mut SceneState| {
                        s.show_hig_inspector = !s.show_hig_inspector;
                    })
                    .boxed(),
                    help_button(|s: &mut SceneState| s.show_basic_dialog = true).boxed(),
                ],
            );
            let inspector = state.show_hig_inspector.then(|| {
                inspector_panel(
                    "Attributes",
                    vec![
                        form_group(
                            Some("METRICS"),
                            vec![
                                form_row("Clicks", label_medium(format!("{}", state.click_count)))
                                    .boxed(),
                            ],
                            None,
                        )
                        .boxed(),
                    ],
                )
            });
            let main = flex_col((
                toolbar,
                workspace_body(state).flex(1.0),
                path_control(
                    &[
                        ("Macintosh HD", icons::FOLDER),
                        ("Beamlet", icons::CODE),
                        ("Gallery", icons::DESCRIPTION),
                    ],
                    |_: &mut SceneState, _| {},
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch);
            wrap_overlays(
                state,
                flex_row((sidebar, main.flex(1.0), inspector))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                None,
            )
        }
        DesignLanguage::Adwaita => {
            let sidebar = navigation_drawer(
                Some("Beamlet"),
                &dests,
                state.active_tab,
                |s: &mut SceneState, i| {
                    s.active_tab = i;
                },
            );
            let hdr = header_bar_with_title(
                view_switcher(
                    &[("Overview", icons::HOME), ("Native", icons::PALETTE)],
                    0,
                    |s: &mut SceneState, i| s.active_tab = i,
                ),
                vec![icon_button(icons::MENU, |s: &mut SceneState| s.click_count += 1).boxed()],
                vec![
                    primary_button("Dialog", |s: &mut SceneState| s.show_basic_dialog = true)
                        .boxed(),
                ],
            );
            let bnr = state.banner_visible.then(|| {
                banner(
                    "GNOME Adwaita SplitView + AdwClamp(880px)",
                    Some("Hide"),
                    Some(Arc::new(|s: &mut SceneState| s.banner_visible = false)),
                    None,
                )
            });
            let main = flex_col((hdr, bnr, clamp(880.0, workspace_body(state)).flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);
            wrap_overlays(
                state,
                flex_row((sidebar, main.flex(1.0)))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                None,
            )
        }
    }
}

/// Builds a standalone [`PreviewHarness`] initialized to `scene_id` and `theme`.
pub fn build_scene_harness(
    scene_id: &str,
    theme: UiTheme,
    config: PreviewConfig,
) -> anyhow::Result<PreviewHarness<SceneState>> {
    let logic: fn(&mut SceneState) -> Box<AnyWidgetView<SceneState>> = match scene_id {
        "overview" => |s| {
            let body = portal(sized_box(render_overview_page(s)).padding(Padding::all(20.px())))
                .constrain_horizontal(true);
            wrap_overlays(s, body, None)
        },
        "inputs" => |s| {
            let body = portal(sized_box(render_inputs_page(s)).padding(Padding::all(20.px())))
                .constrain_horizontal(true);
            wrap_overlays(s, body, None)
        },
        "overlays" => |s| {
            let body = portal(sized_box(render_overlays_page(s)).padding(Padding::all(20.px())))
                .constrain_horizontal(true);
            wrap_overlays(s, body, None)
        },
        "tokens" => |s| {
            let body = portal(sized_box(render_tokens_page()).padding(Padding::all(20.px())))
                .constrain_horizontal(true);
            wrap_overlays(s, body, None)
        },
        "platform-native" => |s| {
            let body =
                portal(sized_box(render_platform_native_page(s)).padding(Padding::all(20.px())))
                    .constrain_horizontal(true);
            wrap_overlays(s, body, None)
        },
        "workspace" => render_workspace_scene,
        other => bail!("unknown preview scene '{other}'; run --list to see available scenes"),
    };

    Ok(PreviewHarness::new(
        SceneState::with_theme(theme),
        |s| s.theme.clone(),
        logic,
        config,
    ))
}

/// Renders `scene_id` under `theme`, replays `steps`, settles mount transitions (unless an explicit
/// `AnimateMs` step was passed), rasterizes pixels, dumps geometry, and runs layout + contrast lints.
pub fn run_scene_preview(
    scene_id: &str,
    theme: UiTheme,
    config: PreviewConfig,
    steps: &[PreviewStep],
) -> anyhow::Result<SceneOutput> {
    let mut harness = build_scene_harness(scene_id, theme, config)?;
    harness.run_steps(steps)?;
    if !steps.iter().any(|s| matches!(s, PreviewStep::AnimateMs(_))) {
        harness.settle_animations();
    }
    let image = harness.render_image();
    let tree = dump_layout_tree(&mut harness);
    let layout_issues = lint_layout(&tree, harness.viewport());
    let contrast_issues = lint_spec_contrast(harness.current_spec());
    Ok(SceneOutput {
        image,
        tree,
        layout_issues,
        contrast_issues,
    })
}
