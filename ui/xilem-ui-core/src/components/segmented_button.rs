//! Single-choice and multi-choice segmented button rows returning concrete `Pod<widgets::Flex>`.

use std::sync::Arc;

use xilem::AnyWidgetView;
use xilem::WidgetView;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, button, flex_row, label};

use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, TypeRole};

/// Single-selection segmented button bar (option labels, active index).
pub fn segmented_button<State: 'static, Action: 'static, F>(
    options: &[&str],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let opts: Vec<ArcStr> = options.iter().map(|s| ArcStr::from(*s)).collect();
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let btns: Vec<Box<AnyWidgetView<State, Action>>> = opts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.on_secondary_container
                } else {
                    p.text_primary
                };
                let ic = active.then(|| ui_icon(icons::CHECK, 15.0, fg));
                let lbl = spec
                    .typography
                    .style_label(TypeRole::LabelLarge, label(text.clone()))
                    .color(fg);
                let cls = if active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                button(
                    flex_row((ic, lbl))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(6.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();
        flex_row(btns)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
    })
}

/// Multi-choice toggleable segmented button bar (`(label, is_selected)` slice).
pub fn multi_choice_segmented_button<State: 'static, Action: 'static, F>(
    options: &[(&str, bool)],
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let opts: Vec<(ArcStr, bool)> = options
        .iter()
        .map(|(s, on)| (ArcStr::from(*s), *on))
        .collect();
    let cb = Arc::new(on_toggle);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let btns: Vec<Box<AnyWidgetView<State, Action>>> = opts
            .iter()
            .enumerate()
            .map(|(i, (text, active))| {
                let cb = cb.clone();
                let fg = if *active {
                    p.on_secondary_container
                } else {
                    p.text_primary
                };
                let ic = active.then(|| ui_icon(icons::CHECK, 15.0, fg));
                let lbl = spec
                    .typography
                    .style_label(TypeRole::LabelLarge, label(text.clone()))
                    .color(fg);
                let cls = if *active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                button(
                    flex_row((ic, lbl))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(6.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();
        flex_row(btns)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
    })
}
