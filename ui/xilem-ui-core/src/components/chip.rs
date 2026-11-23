//! Assist, filter, input, and suggestion chips returning concrete `Pod<widgets::SizedBox>`.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::properties::{Dimensions, Padding};
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, button, flex_row, label, sized_box};

use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::{DesignSpec, TypeRole};

const CHIP_CHILD_VIEW_ID: ViewId = ViewId::new(0x5549_3710);

/// Callback type for chip clicks and close actions.
pub type ChipCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChipKind {
    Assist,
    Filter,
    Input,
    Suggestion,
}

/// Multi-theme interactive chip view exposing `Pod<widgets::SizedBox>` directly.
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

    fn build_inner(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
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

        if let Some(on_rem) = &self.on_remove {
            let on_rem = on_rem.clone();
            let inner_h = (spec.metrics.chip_height - 4.0).max(20.0);
            let main_chip = button(
                flex_row((lead, lbl))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                move |state: &mut State| on_click(state),
            )
            .disabled(self.disabled)
            .ui_class(classes::BTN_SUBTLE)
            .padding(Padding::from_vh(2.px(), 6.px()))
            .dims(Dimensions::height(inner_h.px()));

            let btn_sz = (spec.metrics.chip_height - 6.0).clamp(18.0, 24.0);
            let close_btn = button(ui_icon(icons::CLOSE, 13.0, fg), move |state: &mut State| {
                on_rem(state)
            })
            .disabled(self.disabled)
            .ui_class(classes::ICON_BTN_STD)
            .dims(Dimensions::fixed(btn_sz.px(), btn_sz.px()));

            let (chip_bg, chip_border) = match self.kind {
                ChipKind::Filter if self.selected => (p.secondary_container, None),
                ChipKind::Filter => (
                    masonry::peniko::Color::TRANSPARENT,
                    Some(spec.content_color(p.border_strong, self.disabled)),
                ),
                ChipKind::Input => (
                    p.surface_container,
                    Some(spec.content_color(p.border, self.disabled)),
                ),
                ChipKind::Assist | ChipKind::Suggestion => (
                    masonry::peniko::Color::TRANSPARENT,
                    Some(spec.content_color(p.border, self.disabled)),
                ),
            };
            let (border_color, border_w) = match chip_border {
                Some(c) => (c, 1.px()),
                None => (masonry::peniko::Color::TRANSPARENT, 0.px()),
            };
            sized_box(
                flex_row((main_chip, close_btn))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(2.px()),
            )
            .background_color(chip_bg)
            .border(border_color, border_w)
            .corner_radius(spec.metrics.chip_radius.px())
            .padding(Padding::from_vh(0.px(), 4.px()))
            .dims(Dimensions::height(spec.metrics.chip_height.px()))
            .boxed()
        } else {
            button(
                flex_row((lead, lbl))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                move |state: &mut State| on_click(state),
            )
            .disabled(self.disabled)
            .ui_class(cls)
            .boxed()
        }
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiChip<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiChip<State, Action> {
    type Element = Pod<widgets::SizedBox>;
    type ViewState = (
        Box<AnyWidgetView<State, Action>>,
        <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let inner = self.build_inner(&spec);
        let (child_pod, child_state) =
            ctx.with_id(CHIP_CHILD_VIEW_ID, |ctx| inner.build(ctx, app_state));
        let mut pod = ctx.create_pod(widgets::SizedBox::new(child_pod.new_widget.erased()));
        pod.new_widget.classes.insert(classes::CHIP_ROW.to_string());
        (pod, (inner, child_state))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_inner = self.build_inner(&spec);
        let mut child_dyn =
            widgets::SizedBox::child_mut(&mut element).expect("UiChip SizedBox always has child");
        ctx.with_id(CHIP_CHILD_VIEW_ID, |ctx| {
            next_inner.rebuild(
                prev_inner,
                child_state,
                ctx,
                child_dyn.downcast(),
                app_state,
            );
        });
        *prev_inner = next_inner;
    }

    fn teardown(
        &self,
        (inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child_dyn =
            widgets::SizedBox::child_mut(&mut element).expect("UiChip SizedBox always has child");
        ctx.with_id(CHIP_CHILD_VIEW_ID, |ctx| {
            inner.teardown(child_state, ctx, child_dyn.downcast());
        });
    }

    fn message(
        &self,
        (inner, child_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(CHIP_CHILD_VIEW_ID) => {
                let mut child_dyn = widgets::SizedBox::child_mut(&mut element)
                    .expect("UiChip SizedBox always has child");
                inner.message(child_state, message, child_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}

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

/// Assist chip triggering a contextual action.
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

/// Input token chip with click action and optional `.on_remove(...)` close affordance.
pub fn input_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Input, text, false, on_click)
}

/// Suggestion chip offering quick prompts or choices.
pub fn suggestion_chip<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiChip<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_chip(ChipKind::Suggestion, text, false, on_click)
}
