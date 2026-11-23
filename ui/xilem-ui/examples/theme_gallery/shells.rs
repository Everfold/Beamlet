//! Responsive platform-native shells switching cleanly between wide sidebars/rails and compact bottom navigation.

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, portal, sized_box};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui::prelude::*;

use crate::overlays::wrap_with_overlays;
use crate::pages;
use crate::state::{GalleryState, GalleryTab};

fn destinations() -> [NavDestination; 5] {
    GalleryTab::ALL.map(|tab| NavDestination::new(tab.title(), tab.icon()))
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
    let accent_labels = ["默认", "蓝", "青", "绿", "紫", "粉", "橙", "红"];
    let is_dark = state.theme.brightness == Brightness::Dark;
    let is_rm = state.theme.reduced_motion;

    let seg = |s_idx| {
        segmented_button(
            &["MD3", "Fluent", "HIG", "Adwaita"],
            s_idx,
            |s: &mut GalleryState, idx| {
                s.theme.language = DesignLanguage::ALL[idx];
            },
        )
    };
    let sel = |open: bool| {
        select(
            &accent_labels,
            accent_idx,
            open,
            |s: &mut GalleryState| s.accent_menu_open = !s.accent_menu_open,
            |s: &mut GalleryState, idx| {
                s.theme.accent = UiAccent::PRESETS[idx];
                s.accent_menu_open = false;
            },
            |s: &mut GalleryState| s.accent_menu_open = false,
        )
    };
    let dark_chip = filter_chip("深色", is_dark, |s: &mut GalleryState| {
        s.theme.toggle_brightness();
    });
    let rm_chip = filter_chip("减动效", is_rm, |s: &mut GalleryState| {
        s.theme.reduced_motion = !s.theme.reduced_motion;
    });

    let wide = sized_box(
        flex_row((
            seg(lang_idx),
            sel(state.accent_menu_open),
            dark_chip,
            rm_chip,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px())
        .padding(Padding::from_vh(8.px(), 16.px())),
    );

    let compact_dark = filter_chip("深色", is_dark, |s: &mut GalleryState| {
        s.theme.toggle_brightness();
    });
    let compact_rm = filter_chip("减动效", is_rm, |s: &mut GalleryState| {
        s.theme.reduced_motion = !s.theme.reduced_motion;
    });
    let compact = sized_box(
        flex_col((
            seg(lang_idx),
            flex_row((sel(state.accent_menu_open), compact_dark, compact_rm))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.px())
        .padding(Padding::from_vh(6.px(), 12.px())),
    );

    breakpoint_bin(520.0, wide, compact)
}

fn active_page_body(
    state: &mut GalleryState,
    pad: Padding,
) -> impl WidgetView<GalleryState> + use<> {
    let page: Box<AnyWidgetView<GalleryState>> = match state.active_tab {
        GalleryTab::Components => pages::components_page(state).boxed(),
        GalleryTab::Layout => pages::layout_page(state).boxed(),
        GalleryTab::Overlays => pages::overlays_page(state).boxed(),
        GalleryTab::Native => pages::native_page(state).boxed(),
        GalleryTab::Lab => pages::lab_page(state).boxed(),
    };
    portal(sized_box(page).padding(pad)).constrain_horizontal(true)
}

fn narrow_bottom_nav(state: &GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    let idx = state.active_tab.index();
    match state.theme.language {
        DesignLanguage::Adwaita => {
            let items: Vec<(&'static str, &'static str)> = GalleryTab::ALL
                .iter()
                .map(|t| (t.title(), t.icon()))
                .collect();
            view_switcher_bar(&items, idx, |s: &mut GalleryState, i| {
                s.active_tab = GalleryTab::from_index(i);
            })
            .boxed()
        }
        DesignLanguage::Md3 | DesignLanguage::Fluent | DesignLanguage::Hig => {
            let dests = destinations();
            navigation_bar(&dests, idx, |s: &mut GalleryState, i| {
                s.active_tab = GalleryTab::from_index(i);
            })
            .boxed()
        }
    }
}

fn responsive_shell<S: WidgetView<GalleryState> + 'static>(
    state: &mut GalleryState,
    sidebar: S,
    clamp_wide_content: bool,
) -> impl WidgetView<GalleryState> + use<S> {
    let wide_body: Box<AnyWidgetView<GalleryState>> = if clamp_wide_content {
        clamp(960.0, active_page_body(state, Padding::all(20.px()))).boxed()
    } else {
        active_page_body(state, Padding::all(20.px())).boxed()
    };
    let wide_main = flex_col((
        switcher_bar(state),
        horizontal_divider(),
        wide_body.flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let wide =
        flex_row((sidebar, wide_main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);

    let narrow_main = flex_col((
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state, Padding::all(12.px())).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let narrow = flex_col((narrow_main.flex(1.0), narrow_bottom_nav(state)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    wrap_with_overlays(state, breakpoint_bin(680.0, wide, narrow))
}

pub fn md3_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let rail = navigation_rail_with_fab(
        None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| {
            s.active_tab = GalleryTab::from_index(i);
        },
    );
    responsive_shell(state, rail, false)
}

pub fn fluent_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let nav = navigation_view_collapsible(
        Some("Fluent"),
        &dests[..4],
        &dests[4..],
        state.active_tab.index(),
        state.native.nav_collapsed,
        |s: &mut GalleryState| s.native.nav_collapsed = !s.native.nav_collapsed,
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    responsive_shell(state, nav, false)
}

pub fn hig_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let sections = [(
        "展示台",
        vec![
            ("组件", icons::EDIT),
            ("布局", icons::DOCK_LEFT),
            ("浮层", icons::MORE_HORIZ),
            ("平台", icons::PALETTE),
            ("性能", icons::TUNE),
        ],
    )];
    let sidebar = source_list_sidebar(
        &sections,
        (0, state.active_tab.index()),
        |s: &mut GalleryState, (_, row)| {
            s.active_tab = GalleryTab::from_index(row);
        },
    );
    responsive_shell(state, sidebar, false)
}

pub fn adwaita_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let sidebar = navigation_drawer(
        Some("Adwaita"),
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    responsive_shell(state, sidebar, true)
}
