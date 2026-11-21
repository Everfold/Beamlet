//! Material Design 3 signature components (FAB, Extended FAB, Top/Bottom App Bars).

use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, label, sized_box,
};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui_core::foundation::{UiClassExt, ui_icon};
use xilem_ui_core::style_engine::{classes, with_spec};
use xilem_ui_core::tokens::{DesignSpec, TypeRole};

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

    const fn foreground(self, spec: &DesignSpec) -> xilem::Color {
        let p = &spec.palette;
        match self {
            Self::Primary => p.on_accent_container,
            Self::Surface => p.accent,
            Self::Secondary => p.on_secondary_container,
            Self::Tertiary => p.on_tertiary_container,
        }
    }
}

/// Material 3 `56×56` Floating Action Button.
pub fn fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> Box<AnyWidgetView<State, Action>>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let fg = color.foreground(spec);
        let cb = cb.clone();
        sized_box(
            button(ui_icon(svg_path, 24.0, fg), move |s: &mut State| cb(s))
                .ui_class(color.class_name()),
        )
        .dims(Dimensions::fixed(56.px(), 56.px()))
        .boxed()
    })
    .boxed()
}

/// Material 3 extended pill FAB combining icon + text label.
pub fn extended_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    text: impl Into<ArcStr>,
    color: FabColor,
    on_click: F,
) -> Box<AnyWidgetView<State, Action>>
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
        sized_box(
            button(
                flex_row((ui_icon(svg_path, 20.0, fg), lbl))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                move |s: &mut State| cb(s),
            )
            .ui_class(color.class_name()),
        )
        .dims(Dimensions::height(56.px()))
        .boxed()
    })
    .boxed()
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

/// Material 3 Top App Bar supporting small, center-aligned, medium, and large variants.
pub fn top_app_bar<State: 'static, Action: 'static>(
    variant: TopAppBarVariant,
    title: impl Into<ArcStr>,
) -> Box<AnyWidgetView<State, Action>> {
    let title: ArcStr = title.into();
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        match variant {
            TopAppBarVariant::Small => sized_box(
                flex_row((spec
                    .typography
                    .style_label(TypeRole::TitleLarge, label(title.clone()))
                    .color(p.text_primary)
                    .flex(1.0),))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .padding(Padding::from_vh(8.px(), 16.px())),
            )
            .dims(Dimensions::height(64.px()))
            .ui_class(classes::HEADER_BAR)
            .boxed(),
            TopAppBarVariant::CenterAligned => sized_box(
                flex_row((spec
                    .typography
                    .style_label(TypeRole::TitleLarge, label(title.clone()))
                    .color(p.text_primary),))
                .main_axis_alignment(MainAxisAlignment::Center)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .padding(Padding::from_vh(8.px(), 16.px())),
            )
            .dims(Dimensions::height(64.px()))
            .ui_class(classes::HEADER_BAR)
            .boxed(),
            TopAppBarVariant::Medium => sized_box(
                flex_col((spec
                    .typography
                    .style_label(TypeRole::HeadlineSmall, label(title.clone()))
                    .color(p.text_primary),))
                .main_axis_alignment(MainAxisAlignment::End)
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .padding(Padding::from_vh(18.px(), 16.px())),
            )
            .dims(Dimensions::height(112.px()))
            .ui_class(classes::HEADER_BAR)
            .boxed(),
            TopAppBarVariant::Large => sized_box(
                flex_col((spec
                    .typography
                    .style_label(TypeRole::HeadlineMedium, label(title.clone()))
                    .color(p.text_primary),))
                .main_axis_alignment(MainAxisAlignment::End)
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .padding(Padding::from_vh(24.px(), 16.px())),
            )
            .dims(Dimensions::height(152.px()))
            .ui_class(classes::HEADER_BAR)
            .boxed(),
        }
    })
    .boxed()
}

/// Center-aligned Material 3 top app bar.
pub fn center_aligned_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> Box<AnyWidgetView<State, Action>> {
    top_app_bar(TopAppBarVariant::CenterAligned, title)
}

/// Medium Material 3 top app bar.
pub fn medium_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> Box<AnyWidgetView<State, Action>> {
    top_app_bar(TopAppBarVariant::Medium, title)
}

/// Large Material 3 top app bar.
pub fn large_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> Box<AnyWidgetView<State, Action>> {
    top_app_bar(TopAppBarVariant::Large, title)
}

/// Material 3 Bottom App Bar docking secondary icon actions alongside an optional FAB.
pub fn bottom_app_bar<State: 'static, Action: 'static>(
    actions: Vec<Box<AnyWidgetView<State, Action>>>,
    trailing_fab: Option<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action> {
    sized_box(
        flex_row((
            flex_row(actions)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .flex(1.0),
            trailing_fab,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .padding(Padding::from_vh(8.px(), 16.px())),
    )
    .dims(Dimensions::height(80.px()))
    .ui_class(classes::BOTTOM_BAR)
}
