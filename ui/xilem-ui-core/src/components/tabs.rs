//! Cross-theme primary, secondary, and scrollable tabs honoring [`TabIndicatorStyle`].

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::peniko::Color;
use masonry::properties::{Dimensions, Padding};
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    AnyFlexChild, CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row,
    label, portal, sized_box,
};

use crate::components::divider::horizontal_divider;
use crate::foundation::UiClassExt;
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, TabIndicatorStyle, TypeRole};

fn render_tabs<State: 'static, Action: 'static, F>(
    titles: Vec<ArcStr>,
    selected: usize,
    force_underline: Option<f64>,
    scrollable: bool,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let indicator = match force_underline {
            Some(h) => TabIndicatorStyle::Underline { height: h },
            None => spec.tab_indicator,
        };

        let items: Vec<AnyFlexChild<State, Action>> = titles
            .iter()
            .enumerate()
            .map(|(i, title)| {
                let active = i == selected;
                let cb = cb.clone();
                let ink = if active { p.accent } else { p.text_secondary };
                let lbl = spec
                    .typography
                    .style_label(TypeRole::TitleSmall, label(title.clone()))
                    .color(ink);

                match indicator {
                    TabIndicatorStyle::Underline { height } => {
                        let ind_color = if active { p.accent } else { Color::TRANSPARENT };
                        let bar = sized_box(label(""))
                            .dims(
                                Dimensions::height(height.px())
                                    .with_width(masonry::layout::Dim::Stretch),
                            )
                            .background_color(ind_color);
                        let btn =
                            button(lbl, move |s: &mut State| cb(s, i)).ui_class(classes::TAB_ITEM);
                        let col =
                            flex_col((btn, bar)).cross_axis_alignment(CrossAxisAlignment::Stretch);
                        if scrollable {
                            col.into_any_flex()
                        } else {
                            AnyFlexChild::Item(col.boxed().flex(1.0))
                        }
                    }
                    TabIndicatorStyle::ShortPill => {
                        let bar = if active {
                            sized_box(label(""))
                                .dims(Dimensions::fixed(24.px(), 3.px()))
                                .background_color(p.accent)
                                .corner_radius(999.px())
                        } else {
                            sized_box(label(""))
                                .dims(Dimensions::fixed(24.px(), 3.px()))
                                .background_color(Color::TRANSPARENT)
                                .corner_radius(999.px())
                        };
                        let btn = button(
                            flex_col((lbl, bar))
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .gap(4.px()),
                            move |s: &mut State| cb(s, i),
                        )
                        .ui_class(classes::TAB_ITEM);
                        if scrollable {
                            btn.into_any_flex()
                        } else {
                            AnyFlexChild::Item(btn.boxed().flex(1.0))
                        }
                    }
                    TabIndicatorStyle::SegmentedThumb => {
                        let bg = if active {
                            p.surface
                        } else {
                            Color::TRANSPARENT
                        };
                        let btn = button(
                            lbl.color(if active {
                                p.text_primary
                            } else {
                                p.text_secondary
                            }),
                            move |s: &mut State| cb(s, i),
                        )
                        .ui_class(classes::TAB_ITEM)
                        .background_color(bg)
                        .corner_radius((spec.metrics.button_radius - 1.0).max(4.0).px());
                        if scrollable {
                            btn.into_any_flex()
                        } else {
                            AnyFlexChild::Item(btn.boxed().flex(1.0))
                        }
                    }
                }
            })
            .collect();

        let strip = flex_row(items)
            .main_axis_alignment(MainAxisAlignment::Start)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px());

        let container: Box<AnyWidgetView<State, Action>> =
            if matches!(indicator, TabIndicatorStyle::SegmentedThumb) {
                sized_box(strip)
                    .padding(Padding::all(3.px()))
                    .background_color(p.surface_highest)
                    .corner_radius((spec.metrics.button_radius + 2.0).px())
                    .boxed()
            } else {
                flex_col((strip, horizontal_divider()))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .boxed()
            };

        let inner = if scrollable {
            portal(container).constrain_vertical(true).boxed()
        } else {
            container
        };
        sized_box(inner)
    })
}

/// Primary tab strip adapting to the active theme's [`TabIndicatorStyle`].
pub fn tabs<State: 'static, Action: 'static, T: Into<ArcStr> + Clone, F>(
    titles: &[T],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, T, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let list: Vec<ArcStr> = titles.iter().cloned().map(Into::into).collect();
    render_tabs(list, selected, None, false, on_select)
}

/// Secondary hairline-underline tab strip (`2px` indicator).
pub fn secondary_tabs<State: 'static, Action: 'static, T: Into<ArcStr> + Clone, F>(
    titles: &[T],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, T, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let list: Vec<ArcStr> = titles.iter().cloned().map(Into::into).collect();
    render_tabs(list, selected, Some(2.0), false, on_select)
}

/// Horizontally scrollable tab strip for many destinations.
pub fn scrollable_tabs<State: 'static, Action: 'static, T: Into<ArcStr> + Clone, F>(
    titles: &[T],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, T, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let list: Vec<ArcStr> = titles.iter().cloned().map(Into::into).collect();
    render_tabs(list, selected, None, true, on_select)
}
