//! Multi-theme showcase gallery demonstrating live switching between Material 3,
//! Windows Fluent, macOS HIG, and GNOME Adwaita.

#![warn(clippy::print_stdout, clippy::print_stderr)]

mod pages;
mod state;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, portal, sized_box};
use xilem::{AppState, EventLoop, WidgetView, Xilem};
use xilem_ui::prelude::*;

use state::{GalleryState, GalleryTab};

impl AppState for GalleryState {
    fn keep_running(&self) -> bool {
        self.running
    }
}

fn switcher_bar(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let lang_idx = DesignLanguage::ALL
        .iter()
        .position(|l| *l == state.theme.language)
        .unwrap_or(0);
    let accent_idx = UiAccent::PRESETS
        .iter()
        .position(|a| *a == state.theme.accent)
        .unwrap_or(0);
    let lang_titles = ["Material 3", "Windows Fluent", "macOS HIG", "GNOME Adwaita"];
    let accent_labels = [
        "Default", "Blue", "Teal", "Green", "Purple", "Pink", "Orange", "Red",
    ];
    let is_dark = state.theme.brightness == Brightness::Dark;
    let is_hc = state.theme.high_contrast;

    sized_box(
        flex_row((
            segmented_button(&lang_titles, lang_idx, |s: &mut GalleryState, idx| {
                s.theme.language = DesignLanguage::ALL[idx];
            }),
            select(
                &accent_labels,
                accent_idx,
                state.accent_menu_open,
                |s: &mut GalleryState| s.accent_menu_open = !s.accent_menu_open,
                |s: &mut GalleryState, idx| {
                    s.theme.accent = UiAccent::PRESETS[idx];
                    s.accent_menu_open = false;
                },
                |s: &mut GalleryState| s.accent_menu_open = false,
            ),
            filter_chip("Dark Mode", is_dark, |s: &mut GalleryState| {
                s.theme.toggle_brightness();
            }),
            filter_chip("High Contrast", is_hc, |s: &mut GalleryState| {
                s.theme.high_contrast = !s.theme.high_contrast;
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
        .padding(Padding::from_vh(10.px(), 16.px())),
    )
}

fn app_view(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let tab_idx = state.active_tab.index();
    let destinations = [
        NavDestination::new("Overview", icons::HOME),
        NavDestination::new("Inputs", icons::EDIT),
        NavDestination::new("Overlays", icons::MORE_HORIZ),
        NavDestination::new("Platform Native", icons::PALETTE),
    ];

    let active_page = match state.active_tab {
        GalleryTab::Overview => pages::overview_page(state).boxed(),
        GalleryTab::Inputs => pages::inputs_page(state).boxed(),
        GalleryTab::Overlays => pages::overlays_page(state).boxed(),
        GalleryTab::PlatformNative => pages::platform_native_page(state).boxed(),
    };

    let nav_drawer = navigation_drawer(
        Some(state.theme.language.label()),
        &destinations,
        tab_idx,
        |s: &mut GalleryState, idx| s.active_tab = GalleryTab::from_index(idx),
    );

    let main_area = flex_col((
        switcher_bar(state),
        horizontal_divider(),
        portal(sized_box(active_page).padding(Padding::all(20.px())))
            .constrain_horizontal(true)
            .flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let body = flex_row((nav_drawer, main_area.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let dialog_overlay = if state.show_basic_dialog {
        Some(
            basic_dialog(
                "Reset theme preferences?",
                "This restores the host OS default design language and accent preset.",
                "Reset",
                |s: &mut GalleryState| {
                    s.theme = UiTheme::native_light();
                    s.show_basic_dialog = false;
                },
                |s: &mut GalleryState| s.show_basic_dialog = false,
            )
            .icon(icons::PALETTE)
            .dismiss_text("Cancel"),
        )
    } else {
        None
    };

    let sheet_overlay = if state.show_bottom_sheet {
        Some(bottom_sheet(
            flex_col((
                headline_small("Bottom Sheet"),
                body_medium("Shared modal sheet animated with SlideTransition + EscapeCatcher."),
                primary_button("Close Sheet", |s: &mut GalleryState| {
                    s.show_bottom_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.show_bottom_sheet = false,
        ))
    } else {
        None
    };

    scaffold(body)
        .top_bar(top_bar(
            format!("Beamlet Gallery — {}", state.theme.language.label()),
            None,
            None,
        ))
        .toast(toast_host(&state.toasts, |s: &mut GalleryState| {
            let _ = s.toasts.dismiss_current();
        }))
        .sheet(sheet_overlay)
        .dialog(dialog_overlay)
        .build()
}

fn main() {
    let initial = GalleryState::default();
    let initial_props = initial.theme.default_properties();
    let app = Xilem::new(initial, |state: &mut GalleryState| {
        let wid = state.window_id;
        let theme = state.theme.clone();
        let title = format!(
            "Beamlet Multi-Theme UI Gallery ({})",
            theme.language.label()
        );
        let view = app_view(state);
        std::iter::once(ui_window(wid, title, &theme, view).with_options(|opts| {
            opts.on_close(|s: &mut GalleryState| s.running = false)
                .with_initial_inner_size(LogicalSize::new(1120.0, 760.0))
        }))
    })
    .with_default_properties(initial_props);
    app.run_in(EventLoop::with_user_event())
        .expect("theme_gallery event loop exited cleanly");
}
