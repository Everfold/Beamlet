//! Standalone GNOME Libadwaita Preferences & Status example using `xilem-adwaita` directly without `xilem-ui`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, portal, sized_box, zstack};
use xilem::{AppState, EventLoop, WidgetView, Xilem};
use xilem_adwaita::{
    AdwaitaAccent, AdwaitaTheme, Brightness, action_row, adwaita_window, banner, basic_dialog,
    clamp, clickable_action_row, combo_row, destructive_button, entry_row, expander_row,
    header_bar_with_title, icon_button, icons, preferences_group, status_page, switch_row,
    view_switcher,
};

struct PrefsApp {
    window_id: xilem::WindowId,
    running: bool,
    brightness: Brightness,
    accent: AdwaitaAccent,
    active_view: usize,
    banner_open: bool,
    flat_overlays: bool,
    telemetry_opt_in: bool,
    expander_open: bool,
    combo_open: bool,
    combo_idx: usize,
    app_title: String,
    show_wipe_dialog: bool,
}

impl AppState for PrefsApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for PrefsApp {
    fn default() -> Self {
        Self {
            window_id: xilem::WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            accent: AdwaitaAccent::Blue,
            active_view: 0,
            banner_open: true,
            flat_overlays: true,
            telemetry_opt_in: false,
            expander_open: true,
            combo_open: false,
            combo_idx: 0,
            app_title: "Beamlet Adwaita Studio".into(),
            show_wipe_dialog: false,
        }
    }
}

impl PrefsApp {
    fn current_theme(&self) -> AdwaitaTheme {
        AdwaitaTheme {
            brightness: self.brightness,
            accent: self.accent,
            ..AdwaitaTheme::light()
        }
    }
}

fn app_view(state: &mut PrefsApp) -> impl WidgetView<PrefsApp> + use<> {
    let hdr = header_bar_with_title(
        view_switcher(
            &[("Preferences", icons::TUNE), ("Status", icons::EXPLORE)],
            state.active_view,
            |s: &mut PrefsApp, idx| s.active_view = idx,
        ),
        vec![
            icon_button(icons::PALETTE, |s: &mut PrefsApp| {
                s.accent = match s.accent {
                    AdwaitaAccent::Blue => AdwaitaAccent::Teal,
                    AdwaitaAccent::Teal => AdwaitaAccent::Green,
                    AdwaitaAccent::Green => AdwaitaAccent::Purple,
                    _ => AdwaitaAccent::Blue,
                };
            })
            .boxed(),
        ],
        vec![
            icon_button(icons::DARK_MODE, |s: &mut PrefsApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
        ],
    );

    let top_banner = state.banner_open.then(|| {
        banner(
            "AdwClamp limits boxed-list readability width to 640px automatically.",
            Some("Switch to Status"),
            Some(Arc::new(|s: &mut PrefsApp| s.active_view = 1)),
            Some(Arc::new(|s: &mut PrefsApp| s.banner_open = false)),
        )
    });

    let page = if state.active_view == 0 {
        clamp(
            640.0,
            flex_col((
                preferences_group(
                    Some("Identity & Rendering"),
                    Some("AdwEntryRow, AdwComboRow, and AdwSwitchRow inside boxed-list."),
                    vec![
                        entry_row(
                            "Workspace Display Name",
                            state.app_title.clone(),
                            |s: &mut PrefsApp, txt| s.app_title = txt,
                        )
                        .boxed(),
                        combo_row(
                            "GNOME Accent Preset",
                            Some("Select one of GNOME 47 system accents"),
                            &["Blue", "Teal", "Green", "Purple"],
                            state.combo_idx,
                            state.combo_open,
                            |s: &mut PrefsApp| s.combo_open = !s.combo_open,
                            |s: &mut PrefsApp, idx| {
                                s.combo_idx = idx;
                                s.accent = match idx {
                                    0 => AdwaitaAccent::Blue,
                                    1 => AdwaitaAccent::Teal,
                                    2 => AdwaitaAccent::Green,
                                    _ => AdwaitaAccent::Purple,
                                };
                                s.combo_open = false;
                            },
                            |s: &mut PrefsApp| s.combo_open = false,
                        )
                        .boxed(),
                        switch_row(
                            "Alpha Overlay Feedback",
                            Some("Flat 7% hover / 14% active Adwaita state veils"),
                            state.flat_overlays,
                            |s: &mut PrefsApp, v| s.flat_overlays = v,
                        )
                        .boxed(),
                    ],
                ),
                preferences_group(
                    Some("Advanced"),
                    None,
                    vec![
                        expander_row(
                            "Diagnostic Data & Logs",
                            Some("AdwExpanderRow with nested AdwSwitchRow & AdwActionRow"),
                            Some(icons::INFO),
                            state.expander_open,
                            |s: &mut PrefsApp| s.expander_open = !s.expander_open,
                            vec![
                                switch_row(
                                    "Share Anonymous Metrics",
                                    None::<&str>,
                                    state.telemetry_opt_in,
                                    |s: &mut PrefsApp, v| s.telemetry_opt_in = v,
                                )
                                .boxed(),
                                clickable_action_row(
                                    "Export Support Bundle",
                                    Some("Clickable Pod<widgets::Button> action row"),
                                    Some(icons::DOWNLOAD),
                                    None::<xilem_adwaita::UiButton<PrefsApp, ()>>,
                                    |s: &mut PrefsApp| s.banner_open = true,
                                )
                                .boxed(),
                            ],
                        )
                        .boxed(),
                        action_row(
                            "Danger Zone",
                            Some("Demonstrates destructive_button inside AdwActionRow"),
                            Some(icons::WARNING),
                            Some(destructive_button("Reset App", |s: &mut PrefsApp| {
                                s.show_wipe_dialog = true;
                            })),
                            None,
                        )
                        .boxed(),
                    ],
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(18.px()),
        )
        .boxed()
    } else {
        clamp(
            640.0,
            status_page(
                icons::CHECK_CIRCLE,
                "All Systems Operational",
                "Standalone GNOME Libadwaita shell powered by xilem-adwaita.",
                Some((
                    "Return to Preferences",
                    Arc::new(|s: &mut PrefsApp| s.active_view = 0),
                )),
            ),
        )
        .boxed()
    };

    let body = portal(sized_box(page).padding(Padding::all(24.px()))).constrain_horizontal(true);

    let main_col = flex_col((hdr, top_banner, body.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let wipe_modal = state.show_wipe_dialog.then(|| {
        basic_dialog(
            "Reset Adwaita Preferences?",
            "Restores workspace display name and default GNOME Blue accent.",
            "Reset All",
            |s: &mut PrefsApp| {
                s.app_title = "Beamlet Adwaita Studio".into();
                s.accent = AdwaitaAccent::Blue;
                s.combo_idx = 0;
                s.show_wipe_dialog = false;
            },
            |s: &mut PrefsApp| s.show_wipe_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("Cancel")
    });

    zstack((
        main_col.alignment(UnitPoint::CENTER),
        wipe_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = PrefsApp::default();
    let props = xilem_adwaita::build_default_properties(&initial.current_theme().to_spec());
    let app = Xilem::new(initial, |state: &mut PrefsApp| {
        let theme = state.current_theme();
        let wid = state.window_id;
        std::iter::once(
            adwaita_window(wid, "GNOME Libadwaita Preferences", &theme, app_view(state))
                .with_options(|opts| {
                    opts.on_close(|s: &mut PrefsApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(980.0, 740.0))
                }),
        )
    })
    .with_default_properties(props);
    app.run_in(EventLoop::with_user_event())
        .expect("adwaita_preferences event loop exited cleanly");
}
