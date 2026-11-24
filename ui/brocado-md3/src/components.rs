//! Material Design 3 signature components (FAB, Extended FAB, Top/Bottom App Bars) with concrete `WidgetView`s.

use std::sync::Arc;

use brocado_core::components::{
    NavDestination, headline_medium, headline_small, surface_box, title_large, vertical_divider,
};
use brocado_core::foundation::{UiClassExt, clip_box, ui_icon, wrap};
use brocado_core::style_engine::{classes, with_spec};
use brocado_core::tokens::{DesignSpec, ElevationLevel, TypeRole};
use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::properties::{Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, label, sized_box,
};

use brocado_core::TextAlign;

/// Material 3 FAB tonal color mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FabColor {
    /// Primary accent container (`FAB_PRIMARY`).
    #[default]
    Primary,
    /// Elevated surface container (`FAB_SURFACE`).
    Surface,
    /// Secondary container (`FAB_SECONDARY`).
    Secondary,
    /// Tertiary container (`FAB_TERTIARY`).
    Tertiary,
}

impl FabColor {
    const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => classes::FAB_PRIMARY,
            Self::Surface => classes::FAB_SURFACE,
            Self::Secondary => classes::FAB_SECONDARY,
            Self::Tertiary => classes::FAB_TERTIARY,
        }
    }

    const fn foreground(self, spec: &DesignSpec) -> masonry::peniko::Color {
        let p = &spec.palette;
        match self {
            Self::Primary => p.on_accent_container,
            Self::Surface => p.accent,
            Self::Secondary => p.on_secondary_container,
            Self::Tertiary => p.on_tertiary_container,
        }
    }
}

/// Material 3 FAB size tokens (`Small 40px`, `Medium 56px`, `Large 96px`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FabSize {
    /// Compact `40×40` FAB (`12px` radius, `20px` icon).
    Small,
    /// Standard `56×56` FAB (`16px` radius, `24px` icon).
    #[default]
    Medium,
    /// Prominent `96×96` FAB (`28px` radius, `36px` icon).
    Large,
}

impl FabSize {
    const fn box_px(self) -> f64 {
        match self {
            Self::Small => 40.0,
            Self::Medium => 56.0,
            Self::Large => 96.0,
        }
    }

    const fn icon_px(self) -> f64 {
        match self {
            Self::Small => 20.0,
            Self::Medium => 24.0,
            Self::Large => 36.0,
        }
    }

    const fn radius_px(self) -> f64 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 16.0,
            Self::Large => 28.0,
        }
    }
}

/// Material 3 Floating Action Button parameterized by [`FabSize`] and [`FabColor`], returning `Pod<widgets::Button>`.
pub fn sized_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    size: FabSize,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let fg = color.foreground(spec);
        let cb = cb.clone();
        button(
            ui_icon(svg_path, size.icon_px(), fg),
            move |s: &mut State| cb(s),
        )
        .ui_class(color.class_name())
        .corner_radius(size.radius_px().px())
        .dims(Dimensions::fixed(size.box_px().px(), size.box_px().px()))
    })
}

/// Material 3 `40×40` Small Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn small_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Small, color, on_click)
}

/// Material 3 `56×56` Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Medium, color, on_click)
}

/// Material 3 `96×96` Large Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn large_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Large, color, on_click)
}

/// Material 3 extended pill FAB combining icon + text label, returning `Pod<widgets::Button>` directly.
pub fn extended_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    text: impl Into<ArcStr>,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let text: ArcStr = text.into();
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let fg = color.foreground(spec);
        let cb = cb.clone();
        let lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, label(text.clone()))
            .color(fg);
        button(
            flex_row((ui_icon(svg_path, 20.0, fg), lbl))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px()),
            move |s: &mut State| cb(s),
        )
        .ui_class(color.class_name())
    })
}

/// Material 3 Top App Bar height and layout variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TopAppBarVariant {
    /// Standard `64px` small top app bar.
    #[default]
    Small,
    /// `64px` bar with centered title.
    CenterAligned,
    /// Two-row `112px` medium expanding top app bar.
    Medium,
    /// Two-row `152px` large expanding top app bar.
    Large,
}

/// Material 3 Top App Bar with open `leading` and `trailing` custom view slots.
pub fn top_app_bar_slots<State: 'static, Action: 'static>(
    variant: TopAppBarVariant,
    title: impl Into<ArcStr>,
    leading: Option<Box<AnyWidgetView<State, Action>>>,
    trailing: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let title: ArcStr = title.into();
    let trail_row = (!trailing.is_empty()).then(|| {
        flex_row(trailing)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
    });

    let (height, body): (f64, Box<AnyWidgetView<State, Action>>) = match variant {
        TopAppBarVariant::Small => (
            64.0,
            flex_row((
                leading,
                clip_box(title_large(title).line_breaking(LineBreaking::Clip))
                    .fill_available(true)
                    .flex(1.0),
                trail_row,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .padding(Padding::from_vh(8.px(), 6.px()))
            .boxed(),
        ),
        TopAppBarVariant::CenterAligned => (
            64.0,
            flex_row((
                leading,
                clip_box(
                    title_large(title)
                        .text_alignment(TextAlign::Center)
                        .line_breaking(LineBreaking::Clip),
                )
                .fill_available(true)
                .flex(1.0),
                trail_row,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .padding(Padding::from_vh(8.px(), 6.px()))
            .boxed(),
        ),
        TopAppBarVariant::Medium => {
            let nav_row = flex_row((leading, sized_box(label("")).flex(1.0), trail_row))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px());
            (
                112.0,
                flex_col((
                    nav_row,
                    clip_box(headline_small(title).line_breaking(LineBreaking::Clip)),
                ))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .padding(Padding::from_vh(14.px(), 12.px()))
                .boxed(),
            )
        }
        TopAppBarVariant::Large => {
            let nav_row = flex_row((leading, sized_box(label("")).flex(1.0), trail_row))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px());
            (
                152.0,
                flex_col((
                    nav_row,
                    clip_box(headline_medium(title).line_breaking(LineBreaking::Clip)),
                ))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .padding(Padding::from_vh(18.px(), 12.px()))
                .boxed(),
            )
        }
    };

    sized_box(body)
        .dims(Dimensions::height(height.px()))
        .ui_class(classes::HEADER_BAR)
}

/// Material 3 Top App Bar supporting small, center-aligned, medium, and large variants.
pub fn top_app_bar<State: 'static, Action: 'static>(
    variant: TopAppBarVariant,
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar_slots(variant, title, None, Vec::new())
}

/// Center-aligned Material 3 top app bar.
pub fn center_aligned_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::CenterAligned, title)
}

/// Medium Material 3 top app bar.
pub fn medium_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::Medium, title)
}

/// Large Material 3 top app bar.
pub fn large_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::Large, title)
}

/// Material 3 Bottom App Bar docking secondary icon actions alongside an optional FAB.
pub fn bottom_app_bar<State: 'static, Action: 'static>(
    actions: Vec<Box<AnyWidgetView<State, Action>>>,
    trailing_fab: Option<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(
        flex_row((
            wrap(actions)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .run_gap(4.px())
                .flex(1.0),
            trailing_fab,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(6.px())
        .padding(Padding::from_vh(10.px(), 10.px())),
    )
    .ui_class(classes::BOTTOM_BAR)
}

/// Material 3 Navigation Rail hosting an optional top header/FAB slot above pill navigation destinations.
pub fn navigation_rail_with_fab<State: 'static, Action: 'static, H, F>(
    header: Option<H>,
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, H, F>
where
    H: WidgetView<State, Action>,
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let dests = destinations.to_vec();
    let cb = Arc::new(on_select);
    let nav_items = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let items: Vec<Box<AnyWidgetView<State, Action>>> = dests
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.on_secondary_container
                } else {
                    p.text_secondary
                };
                let pill_bg = if active {
                    p.secondary_container
                } else {
                    masonry::peniko::Color::TRANSPARENT
                };
                let pill = sized_box(ui_icon(d.icon, 18.0, fg))
                    .padding(Padding::from_vh(5.px(), 14.px()))
                    .background_color(pill_bg)
                    .corner_radius(spec.metrics.nav_pill_radius.px());
                let txt = spec
                    .typography
                    .style_label(TypeRole::LabelSmall, label(d.label.clone()))
                    .color(if active {
                        p.text_primary
                    } else {
                        p.text_secondary
                    });
                button(
                    flex_col((pill, txt))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(classes::TAB_ITEM)
                .boxed()
            })
            .collect();
        flex_col(items)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
    });

    flex_row((
        surface_box(
            ElevationLevel::Level1,
            flex_col((header, nav_items))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(14.px())
                .padding(Padding::from_vh(14.px(), 6.px())),
        )
        .radius(0.0)
        .padding(0.0)
        .dims(Dimensions::STRETCH.with_width(80.px())),
        vertical_divider(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// Material 3 Expressive pill-shaped floating toolbar hosting an action row with `Level3` elevation.
pub fn floating_toolbar<State: 'static, Action: 'static>(
    actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    surface_box(
        ElevationLevel::Level3,
        wrap(actions)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(6.px())
            .run_gap(6.px())
            .padding(Padding::from_vh(4.px(), 8.px())),
    )
    .radius(28.0)
    .padding(8.0)
}
