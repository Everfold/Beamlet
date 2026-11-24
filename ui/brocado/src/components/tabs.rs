//! Material Design 3 primary, secondary, and scrollable tabs.

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::peniko::Color;
use masonry::properties::{Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    AnyFlexChild, CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row,
    label, sized_box,
};

use crate::components::divider::horizontal_divider;
use crate::components::typography::ui_label;
use crate::foundation::{ScrollBarVisibility, UiClassExt, clip_box, scroll_view};
use crate::style_engine::{classes, with_theme};
use crate::theme::Theme;
use crate::tokens::TypeRole;

fn render_tabs<State: 'static, Action: 'static, F>(
    titles: Vec<ArcStr>,
    selected: usize,
    indicator_height: f64,
    scrollable: bool,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_select);
    with_theme(move |theme: &Theme, _| {
        let s = &theme.scheme;

        let items: Vec<AnyFlexChild<State, Action>> = titles
            .iter()
            .enumerate()
            .map(|(i, title)| {
                let active = i == selected;
                let cb = cb.clone();
                let ink = if active {
                    s.primary
                } else {
                    s.on_surface_variant
                };
                let lbl = clip_box(
                    ui_label(TypeRole::TitleSmall, title.clone())
                        .color(ink)
                        .line_breaking(LineBreaking::Clip),
                );
                let narrow_pad = Padding::from_vh(8.px(), 4.px());

                let ind_color = if active {
                    s.primary
                } else {
                    Color::TRANSPARENT
                };
                let bar = sized_box(label(""))
                    .dims(
                        Dimensions::height(indicator_height.px())
                            .with_width(masonry::layout::Dim::Stretch),
                    )
                    .background_color(ind_color);
                let btn = button(lbl, move |st: &mut State| cb(st, i)).ui_class(classes::TAB_ITEM);
                let btn: Box<AnyWidgetView<State, Action>> = if !scrollable {
                    btn.padding(narrow_pad).boxed()
                } else {
                    btn.boxed()
                };
                let col = flex_col((btn, bar)).cross_axis_alignment(CrossAxisAlignment::Stretch);
                if scrollable {
                    col.into_any_flex()
                } else {
                    AnyFlexChild::Item(col.boxed().flex(1.0))
                }
            })
            .collect();

        let strip = flex_row(items)
            .main_axis_alignment(MainAxisAlignment::Start)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px());

        let container: Box<AnyWidgetView<State, Action>> = flex_col((strip, horizontal_divider()))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .boxed();

        let inner = if scrollable {
            scroll_view(container)
                .constrain_vertical(true)
                .scrollbar_visibility(ScrollBarVisibility::Never)
                .boxed()
        } else {
            container
        };
        sized_box(inner)
    })
}

/// Primary Material Design 3 tab strip (`3px` underline indicator).
pub fn tabs<State: 'static, Action: 'static, T: Into<ArcStr> + Clone, F>(
    titles: &[T],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, T, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let list: Vec<ArcStr> = titles.iter().cloned().map(Into::into).collect();
    render_tabs(list, selected, 3.0, false, on_select)
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
    render_tabs(list, selected, 2.0, false, on_select)
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
    render_tabs(list, selected, 3.0, true, on_select)
}
