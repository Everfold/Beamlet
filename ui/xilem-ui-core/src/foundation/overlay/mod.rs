//! Shared cross-theme overlay primitives: scrims, Escape-key handling, modal shells,
//! hover tooltips, and anchored popup layers.

pub(crate) mod focus_bridge;
pub mod modal;
pub mod popover;
pub mod popup_layer;
pub mod tooltip_host;

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget, PaintCtx, PointerEvent,
    PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Rect, Size, Vec2};
use masonry::layout::{LenReq, Length};
use masonry::properties::Dimensions;
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{button, label, sized_box};

use crate::foundation::UiClassExt as _;
use crate::style_engine::classes;

pub use modal::{
    ModalShellConfig, ModalShellState, modal_shell_build, modal_shell_message, modal_shell_rebuild,
    modal_shell_teardown,
};
pub use popover::{
    PopoverAnchorAction, PopoverAnchorRect, PopoverAnchorView, PopoverAnchorWidget,
    PopoverPlacementView, PopoverPlacementWidget, popover_anchor, popover_place,
};
pub use popup_layer::{
    AnchoredPopupHost, AnchoredPopupView, ContextMenuHost, ContextMenuView, PopupAction,
    PopupEntrySpec, PopupLayer, PopupLayerSpec,
};
pub use tooltip_host::{HoverTooltipHost, HoverTooltipView, TooltipContentSpec};

type ActionCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Computes a viewport-safe window origin for an anchored floating layer, flipping above or
/// aligning to the trigger's right edge when needed to stay inside `[margin, window_size - margin]`.
pub(crate) fn clamp_anchored_layer_origin(
    anchor: Rect,
    layer_size: Size,
    window_size: Size,
    offset: Vec2,
) -> Point {
    const MARGIN: f64 = 8.0;
    let max_x = (window_size.width - layer_size.width - MARGIN).max(MARGIN);
    let left_x = anchor.x0 + offset.x;
    let x = if left_x + layer_size.width <= window_size.width - MARGIN {
        left_x.max(MARGIN)
    } else {
        (anchor.x1 - layer_size.width).clamp(MARGIN, max_x)
    };

    let max_y = (window_size.height - layer_size.height - MARGIN).max(MARGIN);
    let below_y = anchor.y1 + offset.y;
    let above_y = anchor.y0 - layer_size.height - offset.y;
    let y = if below_y + layer_size.height <= window_size.height - MARGIN {
        below_y.max(MARGIN)
    } else if above_y >= MARGIN || anchor.y0 > (window_size.height - anchor.y1) {
        above_y.clamp(MARGIN, max_y)
    } else {
        below_y.clamp(MARGIN, max_y)
    };

    Point::new(x, y)
}

/// Full-viewport modal scrim backdrop blocking pointer pass-through and optionally dismissing.
pub fn ui_scrim<State: 'static, Action: 'static>(
    dismiss_on_click: bool,
    on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
) -> Box<AnyWidgetView<State, Action>> {
    if dismiss_on_click {
        button(label(""), move |state| on_dismiss(state))
            .ui_class(classes::SCRIM)
            .dims(Dimensions::STRETCH)
            .boxed()
    } else {
        sized_box(label(""))
            .ui_class(classes::SCRIM)
            .dims(Dimensions::STRETCH)
            .boxed()
    }
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
    trap_focus: bool,
    focus_ring: Vec<WidgetId>,
}

impl EscapeCatcher {
    fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: child.erased().to_pod(),
            trap_focus: false,
            focus_ring: Vec::new(),
        }
    }

    /// Creates a modal [`EscapeCatcher`] exposing `Role::Dialog` (`modal = true`) and trapping `Tab` focus.
    pub(crate) fn new_modal_with_focus_ring(
        child: NewWidget<impl Widget + ?Sized>,
        focus_ring: Vec<WidgetId>,
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            trap_focus: true,
            focus_ring,
        }
    }

    fn set_trap_focus(this: &mut WidgetMut<'_, Self>, trap: bool) {
        if this.widget.trap_focus != trap {
            this.widget.trap_focus = trap;
            this.ctx.request_accessibility_update();
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

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if !ctx.has_focus_target() && matches!(event, PointerEvent::Down(_) | PointerEvent::Move(_))
        {
            ctx.request_focus();
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(key_event) = event
            && !key_event.state.is_up()
        {
            if key_event.key == Key::Named(NamedKey::Escape) {
                ctx.set_handled();
                ctx.submit_action::<Self::Action>(EscapePressed);
            } else if self.trap_focus && key_event.key == Key::Named(NamedKey::Tab) {
                let backward = key_event.modifiers.shift();
                match focus_bridge::next_focus_target(
                    &self.focus_ring,
                    ctx.focus_target_id(),
                    backward,
                ) {
                    Some(target) => ctx.set_focus(target),
                    None => ctx.request_focus(),
                }
                ctx.set_handled();
            }
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
        if self.trap_focus {
            Role::Dialog
        } else {
            Role::GenericContainer
        }
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        if self.trap_focus {
            node.set_modal();
        }
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
        trap_focus: false,
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
    trap_focus: bool,
    on_escape: ActionCallback<State, Action>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> EscapeCatcherView<V, State, Action> {
    /// Enables modal `Role::Dialog` (`modal = true`) semantics and keyboard focus trapping.
    pub fn modal(mut self, trap_focus: bool) -> Self {
        self.trap_focus = trap_focus;
        self
    }
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
        let pod = ctx.with_action_widget(|ctx| {
            let widget = if self.trap_focus {
                EscapeCatcher::new_modal_with_focus_ring(child_pod.new_widget, Vec::new())
            } else {
                EscapeCatcher::new(child_pod.new_widget)
            };
            ctx.create_pod(widget)
        });
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
        EscapeCatcher::set_trap_focus(&mut element, self.trap_focus);
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
