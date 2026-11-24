//! Single-choice and multi-choice segmented button rows returning concrete `Pod<widgets::Flex>`.

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::properties::LineBreaking;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt as _, button, flex_row};

use crate::components::typography::ui_label;
use crate::foundation::{UiClassExt, WrapWidget, clip_box, icons, ui_icon, wrap};
use crate::style_engine::{classes, with_theme};
use crate::theme::Theme;
use crate::tokens::TypeRole;

/// Single-selection segmented button bar (option labels, active index).
pub fn segmented_button<State: 'static, Action: 'static, F>(
    options: &[&str],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = WrapWidget> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let opts: Vec<ArcStr> = options.iter().map(|s| ArcStr::from(*s)).collect();
    let cb = Arc::new(on_select);
    with_theme(move |theme: &Theme, _| {
        let s = &theme.scheme;
        let btns: Vec<Box<AnyWidgetView<State, Action>>> = opts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    s.on_secondary_container
                } else {
                    s.on_surface
                };
                let ic = active.then(|| ui_icon(icons::CHECK, 15.0, fg));
                let lbl = clip_box(
                    ui_label(TypeRole::LabelLarge, text.clone())
                        .color(fg)
                        .line_breaking(LineBreaking::Clip),
                );
                let cls = if active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                button(
                    clip_box(
                        flex_row((ic, lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(6.px()),
                    ),
                    move |st: &mut State| cb(st, i),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();
        wrap(btns)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(4.px())
    })
}

/// Multi-choice toggleable segmented button bar (`(label, is_selected)` slice).
pub fn multi_choice_segmented_button<State: 'static, Action: 'static, F>(
    options: &[(&str, bool)],
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = WrapWidget> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let opts: Vec<(ArcStr, bool)> = options
        .iter()
        .map(|(s, on)| (ArcStr::from(*s), *on))
        .collect();
    let cb = Arc::new(on_toggle);
    with_theme(move |theme: &Theme, _| {
        let s = &theme.scheme;
        let btns: Vec<Box<AnyWidgetView<State, Action>>> = opts
            .iter()
            .enumerate()
            .map(|(i, (text, active))| {
                let cb = cb.clone();
                let fg = if *active {
                    s.on_secondary_container
                } else {
                    s.on_surface
                };
                let ic = active.then(|| ui_icon(icons::CHECK, 15.0, fg));
                let lbl = clip_box(
                    ui_label(TypeRole::LabelLarge, text.clone())
                        .color(fg)
                        .line_breaking(LineBreaking::Clip),
                );
                let cls = if *active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                button(
                    clip_box(
                        flex_row((ic, lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(6.px()),
                    ),
                    move |st: &mut State| cb(st, i),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();
        wrap(btns)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(4.px())
    })
}
