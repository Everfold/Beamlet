//! Standalone Windows 11 / WinUI 3 Settings example using `xilem-fluent` directly without `xilem-ui`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};
use xilem::{AppState, EventLoop, WidgetView, Xilem};
use xilem_fluent::{
    Brightness, FluentAccent, FluentTheme, InfoBarSeverity, NavDestination, basic_dialog,
    body_small, breadcrumb_bar, command_bar, expander, fluent_window, headline_small,
    horizontal_divider, icons, info_badge, info_bar, navigation_view, number_box, outlined_button,
    primary_button, settings_card, settings_expander, subtle_button, switch,
};

struct SettingsApp {
    window_id: xilem::WindowId,
    running: bool,
    brightness: Brightness,
    accent: FluentAccent,
    nav_idx: usize,
    mica_enabled: bool,
    auto_updates: bool,
    expander_open: bool,
    custom_expander_open: bool,
    worker_threads: f64,
    show_banner: bool,
    show_reset_dialog: bool,
}

impl AppState for SettingsApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for SettingsApp {
    fn default() -> Self {
        Self {
            window_id: xilem::WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            accent: FluentAccent::WindowsBlue,
            nav_idx: 0,
            mica_enabled: true,
            auto_updates: true,
            expander_open: true,
            custom_expander_open: false,
            worker_threads: 12.0,
            show_banner: true,
            show_reset_dialog: false,
        }
    }
}

impl SettingsApp {
    fn current_theme(&self) -> FluentTheme {
        FluentTheme {
            brightness: self.brightness,
            accent: self.accent,
            ..FluentTheme::light()
        }
    }
}

fn app_view(state: &mut SettingsApp) -> impl WidgetView<SettingsApp> + use<> {
    let primary_nav = [
        NavDestination::new("System", icons::SETTINGS),
        NavDestination::new("Personalization", icons::PALETTE),
        NavDestination::new("Developer", icons::CODE),
    ];
    let footer_nav = [NavDestination::new("Updates", icons::REFRESH).with_badge("1")];

    let nav = navigation_view(
        Some("Windows Settings"),
        &primary_nav,
        &footer_nav,
        state.nav_idx,
        |s: &mut SettingsApp, i| s.nav_idx = i,
    );

    let page_title = match state.nav_idx {
        0 => "System",
        1 => "Personalization",
        2 => "Developer",
        _ => "Updates",
    };

    let crumbs = ["Settings", page_title, "Display & Performance"];
    let bc = breadcrumb_bar(&crumbs, |s: &mut SettingsApp, idx| {
        if idx == 0 {
            s.nav_idx = 0;
        }
    });

    let cmd = command_bar(
        vec![
            primary_button("Toggle Dark Mica", |s: &mut SettingsApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
            subtle_button("Cycle Accent", |s: &mut SettingsApp| {
                s.accent = match s.accent {
                    FluentAccent::WindowsBlue => FluentAccent::Teal,
                    FluentAccent::Teal => FluentAccent::Purple,
                    FluentAccent::Purple => FluentAccent::Green,
                    _ => FluentAccent::WindowsBlue,
                };
            })
            .boxed(),
        ],
        vec![
            outlined_button("Reset Defaults", |s: &mut SettingsApp| {
                s.show_reset_dialog = true;
            })
            .boxed(),
        ],
    );

    let banner_opt = state.show_banner.then(|| {
        info_bar(
            InfoBarSeverity::Informational,
            "Fluent 2 Shell Ready",
            "Demonstrating NavigationView footer pin, BreadcrumbBar, InfoBadge, and PrimaryLeading dialog buttons.",
            Some(Arc::new(|s: &mut SettingsApp| s.show_banner = false)),
        )
    });

    let body = portal(
        sized_box(
            flex_col((
                headline_small(page_title),
                cmd,
                banner_opt,
                settings_card(
                    Some(icons::PALETTE),
                    "Mica Backdrop Effect",
                    Some("Layered `#F3F3F3` Light / `#202020` Dark desktop materials"),
                    flex_row((
                        info_badge(InfoBarSeverity::Success, Some("Mica")),
                        switch(state.mica_enabled, |s: &mut SettingsApp, v| {
                            s.mica_enabled = v;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                ),
                settings_card(
                    Some(icons::TUNE),
                    "Background Build Workers",
                    Some("Clamped WinUI 3 NumberBox spin control"),
                    number_box(
                        state.worker_threads,
                        1.0,
                        64.0,
                        1.0,
                        |s: &mut SettingsApp, next| s.worker_threads = next,
                    ),
                ),
                settings_expander(
                    Some(icons::SETTINGS),
                    "Windows Update & Diagnostics",
                    Some("Auto-download security updates"),
                    state.expander_open,
                    |s: &mut SettingsApp| s.expander_open = !s.expander_open,
                    Some(switch(state.auto_updates, |s: &mut SettingsApp, v| {
                        s.auto_updates = v;
                    })),
                    vec![
                        body_small(
                            "WinUI 3 SettingsExpander reveals nested child rows dynamically.",
                        )
                        .boxed(),
                    ],
                ),
                expander(
                    "Experimental Build Options",
                    Some("Standalone WinUI 3 Expander wrapping custom view body"),
                    state.custom_expander_open,
                    |s: &mut SettingsApp| s.custom_expander_open = !s.custom_expander_open,
                    flex_row((
                        info_badge(InfoBarSeverity::Warning, Some("Preview")),
                        body_small("Custom nested diagnostics payload."),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(12.px()),
        )
        .padding(Padding::all(20.px())),
    )
    .constrain_horizontal(true);

    let main = flex_col((
        sized_box(bc).padding(Padding::from_vh(8.px(), 16.px())),
        horizontal_divider(),
        body.flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let shell = flex_row((nav, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);

    let reset_modal = state.show_reset_dialog.then(|| {
        basic_dialog(
            "Reset Fluent Settings?",
            "Windows Fluent places primary Confirm left of Cancel (`DialogActionOrder::PrimaryLeading`).",
            "Reset Now",
            |s: &mut SettingsApp| {
                s.brightness = Brightness::Light;
                s.accent = FluentAccent::WindowsBlue;
                s.worker_threads = 12.0;
                s.show_reset_dialog = false;
            },
            |s: &mut SettingsApp| s.show_reset_dialog = false,
        )
        .destructive(true)
        .dismiss_text("Cancel")
    });

    zstack((
        shell.alignment(UnitPoint::CENTER),
        reset_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = SettingsApp::default();
    let props = xilem_fluent::build_default_properties(&initial.current_theme().to_spec());
    let app = Xilem::new(initial, |state: &mut SettingsApp| {
        let theme = state.current_theme();
        let wid = state.window_id;
        std::iter::once(
            fluent_window(wid, "WinUI 3 Fluent Settings", &theme, app_view(state)).with_options(
                |opts| {
                    opts.on_close(|s: &mut SettingsApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(1080.0, 740.0))
                },
            ),
        )
    })
    .with_default_properties(props);
    app.run_in(EventLoop::with_user_event())
        .expect("fluent_settings event loop exited cleanly");
}
