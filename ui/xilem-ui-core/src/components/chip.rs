//! Assist, filter, input, and suggestion chips driven by [`DesignSpec`].

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, button, flex_row, label, sized_box};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Callback type for chip clicks and close actions.
pub type ChipCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChipKind {
    Assist,
    Filter,
    Input,
    Suggestion,
}

/// Multi-theme interactive chip view.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiChip<State, Action> {
    kind: ChipKind,
    text: ArcStr,
    selected: bool,
    disabled: bool,
    leading_icon: Option<&'static str>,
    on_click: ChipCallback<State, Action>,
    on_remove: Option<ChipCallback<State, Action>>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiChip<State, Action> {
    /// Sets a leading SVG icon.
    pub fn leading_icon(mut self, svg_path: &'static str) -> Self {
        self.leading_icon = Some(svg_path);
        self
    }

    /// Disables chip interactions when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Attaches a trailing remove/close callback (primarily for input chips).
    pub fn on_remove<F>(mut self, on_remove: F) -> Self
    where
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        self.on_remove = Some(Arc::new(on_remove));
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let (cls, active_fg) = match self.kind {
            ChipKind::Assist => (classes::CHIP_ASSIST, p.text_primary),
            ChipKind::Filter => {
                if self.selected {
                    (classes::CHIP_FILTER_SELECTED, p.on_secondary_container)
                } else {
                    (classes::CHIP_FILTER, p.text_primary)
                }
            }
            ChipKind::Input => (classes::CHIP_INPUT, p.text_primary),
            ChipKind::Suggestion => (classes::CHIP_SUGGESTION, p.text_secondary),
        };
        let fg = spec.content_color(active_fg, self.disabled);
        let lead = match (self.kind, self.selected, self.leading_icon) {
            (ChipKind::Filter, true, _) => Some(ui_icon(icons::CHECK, 16.0, fg)),
            (_, _, Some(icon)) => Some(ui_icon(icon, 16.0, fg)),
            _ => None,
        };
        let lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, label(self.text.clone()))
            .color(fg);
        let on_click = self.on_click.clone();

        let main_chip = button(
            flex_row((lead, lbl))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px()),
            move |state: &mut State| on_click(state),
        )
        .disabled(self.disabled)
        .ui_class(cls);

        if let Some(on_rem) = &self.on_remove {
            let on_rem = on_rem.clone();
            let close_btn = button(ui_icon(icons::CLOSE, 14.0, fg), move |state: &mut State| {
                on_rem(state)
            })
            .disabled(self.disabled)
            .ui_class(classes::ICON_BTN_STD);
            sized_box(
                flex_row((main_chip, close_btn))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(2.px()),
            )
            .dims(Dimensions::height(spec.metrics.chip_height.px()))
            .boxed()
        } else {
            sized_box(main_chip)
                .dims(Dimensions::height(spec.metrics.chip_height.px()))
                .boxed()
        }
    }
}

crate::impl_spec_view!(UiChip<State, Action>);

fn make_chip<State, Action, F>(
    kind: ChipKind,
    text: impl Into<ArcStr>,
    selected: bool,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiChip {
        kind,
        text: text.into(),
        selected,
        disabled: false,
        leading_icon: None,
        on_click: Arc::new(on_click),
        on_remove: None,
        phantom: PhantomData,
    }
}

/// Assist chip triggering an contextual action.
pub fn assist_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Assist, text, false, on_click)
}

/// Toggleable filter chip displaying a checkmark when `selected` is `true`.
pub fn filter_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    selected: bool,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Filter, text, selected, on_click)
}

/// Input chip (often paired with `.on_remove(...)`).
pub fn input_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Input, text, false, on_click)
}

/// Suggestion chip presenting recommended options.
pub fn suggestion_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Suggestion, text, false, on_click)
}
