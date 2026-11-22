//! Multi-theme showcase gallery demonstrating shared state alongside distinct per-platform native layouts.

#![warn(clippy::print_stdout, clippy::print_stderr)]

mod pages;
mod state;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};
use xilem::{AnyWidgetView, AppState, EventLoop, WidgetView, Xilem};
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
        .padding(Padding::from_vh(8.px(), 16.px())),
    )
}

fn active_page_body(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let page: Box<AnyWidgetView<GalleryState>> = match state.active_tab {
        GalleryTab::Overview => pages::overview_page(state).boxed(),
        GalleryTab::Inputs => pages::inputs_page(state).boxed(),
        GalleryTab::Overlays => pages::overlays_page(state).boxed(),
        GalleryTab::PlatformNative => pages::platform_native_page(state).boxed(),
    };
    portal(sized_box(page).padding(Padding::all(20.px()))).constrain_horizontal(true)
}

fn wrap_with_overlays<V: WidgetView<GalleryState>>(
    state: &mut GalleryState,
    workspace: V,
    floating_action: Option<Box<AnyWidgetView<GalleryState>>>,
) -> impl WidgetView<GalleryState, Widget = widgets::ZStack> + use<V> {
    let dialog_overlay = state.show_basic_dialog.then(|| {
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
        .dismiss_text("Cancel")
    });

    let sheet_overlay = state.show_bottom_sheet.then(|| {
        bottom_sheet(
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
        )
    });

    let fab_layer = floating_action.map(|f| {
        sized_box(f)
            .padding(Padding::all(20.px()))
            .alignment(UnitPoint::BOTTOM_RIGHT)
    });
    let toast_layer = sized_box(toast_host(&state.toasts, |s: &mut GalleryState| {
        let _ = s.toasts.dismiss_current();
    }))
    .padding(Padding::all(16.px()))
    .alignment(UnitPoint::BOTTOM);

    zstack((
        workspace.alignment(UnitPoint::CENTER),
        fab_layer,
        toast_layer,
        sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        dialog_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn destinations() -> [NavDestination; 4] {
    [
        NavDestination::new("Overview", icons::HOME),
        NavDestination::new("Inputs", icons::EDIT),
        NavDestination::new("Overlays", icons::MORE_HORIZ),
        NavDestination::new("Platform Native", icons::PALETTE),
    ]
}

fn md3_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let rail = navigation_rail(
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| {
            s.active_tab = GalleryTab::from_index(i);
        },
    );
    let bar = top_app_bar_slots(
        TopAppBarVariant::Small,
        "Beamlet Material 3",
        None,
        vec![
            icon_button(icons::PALETTE, |s: &mut GalleryState| {
                s.theme.toggle_brightness()
            })
            .boxed(),
        ],
    );
    let main = flex_col((
        bar,
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((rail, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    let fab_view = extended_fab(
        icons::ADD,
        "New Action",
        FabColor::Primary,
        |s: &mut GalleryState| {
            s.click_count += 1;
            s.toasts.show(
                "Material 3 Extended FAB triggered",
                None::<&str>,
                ToastDuration::Short,
                true,
            );
        },
    )
    .boxed();
    wrap_with_overlays(state, workspace, Some(fab_view))
}

fn fluent_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let nav = navigation_view(
        Some("WinUI 3 Shell"),
        &dests[..3],
        &dests[3..],
        state.active_tab.index(),
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    let cmd = command_bar(
        vec![
            primary_button("New", |s: &mut GalleryState| s.click_count += 1).boxed(),
            subtle_button("Reset Counter", |s: &mut GalleryState| s.click_count = 0).boxed(),
        ],
        vec![
            outlined_button("Dialog", |s: &mut GalleryState| {
                s.show_basic_dialog = true;
            })
            .boxed(),
        ],
    );
    let notice = state.info_bar_visible.then(|| {
        info_bar(
            InfoBarSeverity::Informational,
            "Windows Fluent NavigationView + CommandBar",
            "Notice Settings pinned at the bottom of NavigationView & PrimaryLeading dialog buttons.",
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.info_bar_visible = false;
            })),
        )
    });
    let main = flex_col((
        switcher_bar(state),
        horizontal_divider(),
        sized_box(flex_col((cmd, notice)).gap(8.px())).padding(Padding::from_vh(8.px(), 16.px())),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((nav, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

fn hig_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let sections = [
        (
            "SHOWCASE",
            vec![
                ("Overview", icons::HOME),
                ("Inputs", icons::EDIT),
                ("Overlays", icons::MORE_HORIZ),
            ],
        ),
        ("PREFERENCES", vec![("Native Suites", icons::PALETTE)]),
    ];
    let selected = match state.active_tab {
        GalleryTab::PlatformNative => (1, 0),
        other => (0, other.index()),
    };
    let sidebar = source_list_sidebar(&sections, selected, |s: &mut GalleryState, (sec, row)| {
        let idx = if sec == 1 { 3 } else { row };
        s.active_tab = GalleryTab::from_index(idx);
    });
    let toolbar = macos_toolbar_with_center(
        vec![
            icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Overview;
            })
            .boxed(),
        ],
        search_bar(state.search_query.clone(), |s: &mut GalleryState, val| {
            s.search_query = val;
        })
        .on_clear(|s: &mut GalleryState| s.search_query.clear())
        .width(260.px()),
        vec![help_button(|s: &mut GalleryState| s.show_basic_dialog = true).boxed()],
    );
    let main = flex_col((
        toolbar,
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((sidebar, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

fn adwaita_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let sidebar = navigation_drawer(
        Some("Beamlet"),
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    let hdr = header_bar(
        "Beamlet Gallery",
        Some("GNOME Libadwaita Workspace"),
        vec![icon_button(icons::MENU, |s: &mut GalleryState| s.click_count += 1).boxed()],
        vec![
            primary_button("Dialog", |s: &mut GalleryState| {
                s.show_basic_dialog = true;
            })
            .boxed(),
        ],
    );
    let top_banner = state.banner_visible.then(|| {
        banner(
            "GNOME Adwaita SplitView + Centered HeaderBar",
            Some("Hide"),
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.banner_visible = false;
            })),
            None,
        )
    });
    let main = flex_col((
        hdr,
        top_banner,
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((sidebar, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

fn app_view(state: &mut GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    // Explicit developer-owned dispatch: shared state/pages, platform-authentic workspaces.
    match state.theme.language {
        DesignLanguage::Md3 => md3_workspace(state).boxed(),
        DesignLanguage::Fluent => fluent_workspace(state).boxed(),
        DesignLanguage::Hig => hig_workspace(state).boxed(),
        DesignLanguage::Adwaita => adwaita_workspace(state).boxed(),
    }
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
                .with_initial_inner_size(LogicalSize::new(1180.0, 800.0))
        }))
    })
    .with_default_properties(initial_props);
    app.run_in(EventLoop::with_user_event())
        .expect("theme_gallery event loop exited cleanly");
}
