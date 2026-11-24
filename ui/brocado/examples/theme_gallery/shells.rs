//! Responsive platform-native shells switching cleanly between wide sidebars/rails and compact bottom navigation.

use brocado::prelude::*;
use masonry::layout::AsUnit;
use masonry::properties::Padding;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, sized_box};

use crate::overlays::wrap_with_overlays;
use crate::pages;
use crate::state::{GalleryState, GalleryTab};

fn destinations() -> [NavDestination; 5] {
    GalleryTab::ALL.map(|tab| NavDestination::new(tab.title(), tab.icon()))
}

fn switcher_bar(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let accent_idx = UiAccent::PRESETS
        .iter()
        .position(|a| *a == state.theme.accent)
        .unwrap_or(0);
    let accent_labels = ["默认", "蓝", "青", "绿", "紫", "粉", "橙", "红"];
    let is_dark = state.theme.brightness == Brightness::Dark;
    let is_hc = state.theme.high_contrast;
    let is_rm = state.theme.reduced_motion;

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
    let hc_chip = filter_chip("高对比", is_hc, |s: &mut GalleryState| {
        s.theme.high_contrast = !s.theme.high_contrast;
    });
    let rm_chip = filter_chip("减动效", is_rm, |s: &mut GalleryState| {
        s.theme.reduced_motion = !s.theme.reduced_motion;
    });

    sized_box(
        wrap((sel(state.accent_menu_open), dark_chip, hc_chip, rm_chip))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
            .run_gap(6.px())
            .padding(Padding::from_vh(6.px(), 10.px())),
    )
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
    scroll_view(sized_box(page).padding(pad)).constrain_horizontal(true)
}

fn narrow_bottom_nav(state: &GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    let idx = state.active_tab.index();
    let dests = destinations();
    navigation_bar(&dests, idx, |s: &mut GalleryState, i| {
        s.active_tab = GalleryTab::from_index(i);
    })
    .boxed()
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
        active_page_body(state, Padding::all(8.px())).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let narrow = flex_col((narrow_main.flex(1.0), narrow_bottom_nav(state)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    wrap_with_overlays(state, breakpoint_bin(680.0, wide, narrow))
}

pub fn md3_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let rail = navigation_rail_with_fab(
        None::<brocado::components::button::UiButton<GalleryState, ()>>,
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| {
            s.active_tab = GalleryTab::from_index(i);
        },
    );
    responsive_shell(state, rail, false)
}
