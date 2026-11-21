//! Selection controls: checkbox, tri-state checkbox, switch, radio button/group, and slider.

use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, button, checkbox as raw_checkbox, flex_col, flex_row, label,
    radio_button as raw_radio, radio_group as raw_radio_group, sized_box, slider as raw_slider,
    switch as raw_switch,
};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, TypeRole};

/// Logical state of a tri-state checkbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToggleableState {
    /// Unchecked (`Off`).
    #[default]
    Off,
    /// Checked (`On`).
    On,
    /// Partially selected (`Indeterminate`).
    Indeterminate,
}

/// Callback type for [`checkbox`] / [`switch`].
pub type BoolCallback<State, Action> = Arc<dyn Fn(&mut State, bool) -> Action + Send + Sync>;

/// Spec-styled checkbox with optional label.
pub fn checkbox<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    checked: bool,
    on_toggle: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    let text = text.into();
    let cb = Arc::new(on_toggle);
    with_spec(move |spec: &DesignSpec, _state| {
        let on_t = cb.clone();
        let style = spec.typography.get(TypeRole::BodyMedium);
        let mut view = raw_checkbox(text.clone(), checked, move |state: &mut State, v: bool| {
            on_t(state, v)
        })
        .text_size(style.font_size)
        .weight(style.weight);
        if let Some(font) = spec.typography.font_family() {
            view = view.font(font.clone());
        }
        view.ui_class_if(checked, classes::CHECKED).boxed()
    })
}

/// Tri-state checkbox cycling `Off -> On -> Off` (or `Indeterminate -> On`).
pub fn tri_state_checkbox<State: 'static, Action: 'static, F>(
    label_text: impl Into<ArcStr>,
    state: ToggleableState,
    on_toggle: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State, ToggleableState) -> Action + Send + Sync + 'static,
{
    let label_text = label_text.into();
    let cb = Arc::new(on_toggle);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let next = match state {
            ToggleableState::Off | ToggleableState::Indeterminate => ToggleableState::On,
            ToggleableState::On => ToggleableState::Off,
        };
        let (box_bg, border_c, icon_opt) = match state {
            ToggleableState::Off => (xilem::Color::TRANSPARENT, p.border_strong, None),
            ToggleableState::On => (p.accent, p.accent, Some(icons::CHECK)),
            ToggleableState::Indeterminate => {
                (p.accent, p.accent, Some(icons::INDETERMINATE_MINUS))
            }
        };
        let box_view = sized_box(
            icon_opt
                .map(|path| ui_icon(path, 14.0, p.on_accent))
                .unwrap_or_else(|| ui_icon(icons::CHECK, 0.0, xilem::Color::TRANSPARENT)),
        )
        .dims(Dimensions::fixed(18.px(), 18.px()))
        .background_color(box_bg)
        .border(border_c, 2.px())
        .corner_radius(spec.metrics.checkbox_radius.px());

        let text_view = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(label_text.clone()))
            .color(p.text_primary);
        let cb = cb.clone();
        button(
            flex_row((box_view, text_view))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
            move |s: &mut State| cb(s, next),
        )
        .ui_class(classes::MENU_ITEM)
        .boxed()
    })
}

/// Spec-styled toggle switch.
pub fn switch<State: 'static, Action: 'static, F>(
    on: bool,
    on_toggle: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    raw_switch(on, on_toggle)
}

/// Spec-styled radio button option.
pub fn radio<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    selected: bool,
    on_select: F,
) -> Box<AnyWidgetView<State, Action>>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let text = text.into();
    let cb = Arc::new(on_select);
    with_spec(move |_spec: &DesignSpec, _| {
        let cb = cb.clone();
        raw_radio(text.clone(), selected, move |s: &mut State| cb(s))
            .ui_class_if(selected, classes::CHECKED)
            .boxed()
    })
    .boxed()
}

/// Vertical or horizontal radio group container with accessible semantics.
pub fn radio_group<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    items: Vec<V>,
) -> impl WidgetView<State, Action> {
    raw_radio_group(
        flex_col(items)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(6.px()),
    )
}

/// Spec-styled continuous or stepped slider (`min..=max`).
pub fn slider<State: 'static, Action: 'static, F>(
    min: f64,
    max: f64,
    value: f64,
    step: Option<f64>,
    on_change: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State, f64) -> Action + Send + Sync + 'static,
{
    let s = raw_slider(min, max, value, on_change);
    if let Some(step) = step {
        s.step(step)
    } else {
        s
    }
}
