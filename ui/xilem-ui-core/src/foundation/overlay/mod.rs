//! Shared cross-theme overlay primitives: scrims, Escape-key handling, modal shells,
//! hover tooltips, and anchored popup layers.

pub mod modal;
pub mod popup_layer;
pub mod tooltip_host;

use std::marker::PhantomData;
use std::sync::Arc;

use tracing::{Span, trace_span};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Size};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::keyboard::{Key, NamedKey};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget, PaintCtx, PropertiesMut,
    PropertiesRef, RegisterCtx, TextEvent, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{LenReq, Length};
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::{button, label, sized_box};
use xilem::{Pod, ViewCtx, WidgetView};

use crate::foundation::UiClassExt as _;
use crate::style_engine::classes;

pub use modal::{
    ModalShellConfig, ModalShellState, modal_shell_build, modal_shell_message, modal_shell_rebuild,
    modal_shell_teardown,
};
pub use popup_layer::{
    AnchoredPopupHost, AnchoredPopupView, PopupAction, PopupEntrySpec, PopupLayer, PopupLayerSpec,
};
pub use tooltip_host::{HoverTooltipHost, HoverTooltipView, TooltipContentSpec};

type ActionCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Full-viewport modal scrim backdrop blocking pointer pass-through and optionally dismissing.
pub fn ui_scrim<State: 'static, Action: Default + 'static>(
    dismiss_on_click: bool,
    on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
) -> impl WidgetView<State, Action> + use<State, Action> {
    button(label(""), move |state| {
        if dismiss_on_click {
            on_dismiss(state)
        } else {
            Action::default()
        }
    })
    .ui_class(classes::SCRIM)
    .dims(Dimensions::STRETCH)
}

/// Action emitted by [`EscapeCatcher`] when the `Escape` key is pressed while focus is inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EscapePressed;

const ESCAPE_CHILD_ID: ViewId = ViewId::new(0x5549_3401);

/// Transparent single-child focus container intercepting `Escape` key presses for modal dismissal.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct EscapeCatcher {
    child: WidgetPod<dyn Widget>,
}

impl EscapeCatcher {
    fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: child.erased().to_pod(),
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for EscapeCatcher {
    type Action = EscapePressed;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(key_event) = event
            && !key_event.state.is_up()
            && key_event.key == Key::Named(NamedKey::Escape)
        {
            ctx.set_handled();
            ctx.submit_action::<Self::Action>(EscapePressed);
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        _len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        ctx.redirect_measurement(&mut self.child, axis, cross_length)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.child, size);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.derive_baselines(&self.child);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("EscapeCatcher", id = id.trace())
    }
}

/// Wraps `child` so pressing `Escape` anywhere within its subtree invokes `on_escape`.
pub fn escape_catcher<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
    on_escape: impl Fn(&mut State) -> Action + Send + Sync + 'static,
) -> EscapeCatcherView<V, State, Action> {
    EscapeCatcherView {
        child,
        on_escape: Arc::new(on_escape),
        phantom: PhantomData,
    }
}

/// View wrapper constructed by [`escape_catcher`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic child View and callback trait object"
)]
pub struct EscapeCatcherView<V, State, Action> {
    child: V,
    on_escape: ActionCallback<State, Action>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> ViewMarker for EscapeCatcherView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for EscapeCatcherView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<EscapeCatcher>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, child_state) =
            ctx.with_id(ESCAPE_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod =
            ctx.with_action_widget(|ctx| ctx.create_pod(EscapeCatcher::new(child_pod.new_widget)));
        (pod, child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let mut child = EscapeCatcher::child_mut(&mut element);
        ctx.with_id(ESCAPE_CHILD_ID, |ctx| {
            self.child
                .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child = EscapeCatcher::child_mut(&mut element);
            ctx.with_id(ESCAPE_CHILD_ID, |ctx| {
                self.child.teardown(view_state, ctx, child.downcast());
            });
        }
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(ESCAPE_CHILD_ID) => {
                let mut child = EscapeCatcher::child_mut(&mut element);
                self.child
                    .message(view_state, message, child.downcast(), app_state)
            }
            None => match message.take_message::<EscapePressed>() {
                Some(_) => MessageResult::Action((self.on_escape)(app_state)),
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}

/// Spacer utility creating an inert zero-size box when an optional overlay slot is empty.
#[must_use]
pub fn empty_slot<State: 'static, Action: 'static>() -> impl WidgetView<State, Action> {
    sized_box(label("")).dims(Dimensions::fixed(Length::ZERO, Length::ZERO))
}
