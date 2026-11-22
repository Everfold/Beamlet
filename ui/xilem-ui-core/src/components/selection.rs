//! Selection controls: checkbox, tri-state checkbox, switch, radio button/group, and slider.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Dimensions;
use xilem::masonry::widgets::{self, RadioButtonSelected};
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, button, checkbox as raw_checkbox, flex_col, flex_row, label,
    radio_group as raw_radio_group, sized_box, slider as raw_slider, switch as raw_switch,
};
use xilem::{Pod, ViewCtx, WidgetView};

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
) -> impl WidgetView<State, Action, Widget = widgets::Checkbox>
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
        view.ui_class_if(checked, classes::CHECKED)
    })
}

/// Tri-state checkbox cycling `Off -> On -> Off` (or `Indeterminate -> On`).
pub fn tri_state_checkbox<State: 'static, Action: 'static, F>(
    label_text: impl Into<ArcStr>,
    state: ToggleableState,
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
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
    })
}

/// Spec-styled toggle switch.
pub fn switch<State: 'static, Action: 'static, F>(
    on: bool,
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = widgets::Switch>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    raw_switch(on, on_toggle)
}

/// Callback type for [`UiRadio`].
pub type RadioCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Concrete radio option view exposing `Pod<widgets::RadioButton>` uniformly for [`radio_group`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiRadio<State, Action> {
    text: ArcStr,
    selected: bool,
    disabled: bool,
    on_select: RadioCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiRadio<State, Action> {
    /// Disables radio selection when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, Action> ViewMarker for UiRadio<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiRadio<State, Action> {
    type Element = Pod<widgets::RadioButton>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod =
                ctx.create_pod(widgets::RadioButton::new(self.selected, self.text.clone()));
            pod.new_widget.options.disabled = self.disabled;
            if self.selected {
                pod.new_widget.classes.insert(classes::CHECKED.to_string());
            }
            pod
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if prev.text != self.text {
            widgets::RadioButton::set_text(&mut element, self.text.clone());
        }
        if prev.selected != self.selected {
            widgets::RadioButton::set_checked(&mut element, self.selected);
            if self.selected {
                element.ctx.add_class(classes::CHECKED);
            } else {
                element.ctx.remove_class(classes::CHECKED);
            }
        }
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_message::<RadioButtonSelected>() {
            Some(_) => MessageResult::Action((self.on_select)(app_state)),
            None => MessageResult::Stale,
        }
    }
}

/// Spec-styled radio button option returning a uniform [`UiRadio`] concrete view.
pub fn radio<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    selected: bool,
    on_select: F,
) -> UiRadio<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiRadio {
        text: text.into(),
        selected,
        disabled: false,
        on_select: Arc::new(on_select),
        phantom: PhantomData,
    }
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
