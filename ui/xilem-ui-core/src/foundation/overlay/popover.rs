//! Anchored interactive popover foundation: trigger rect capture and viewport-safe overlay placement.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, AccessEvent, ActionCtx, ChildrenIds, ErasedAction, EventCtx, LayoutCtx, MeasureCtx,
    NewWidget, NoAction, PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, Update,
    UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Rect, Size, Vec2};
use masonry::layout::{LayoutSize, LenDef, LenReq, Length, SizeDef};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use super::clamp_anchored_layer_origin;

const POPOVER_ANCHOR_CHILD_ID: ViewId = ViewId::new(0x5549_3440);
const POPOVER_PLACEMENT_CHILD_ID: ViewId = ViewId::new(0x5549_3441);

/// Window-space trigger rectangle used to position [`popover_surface`](crate::components::popover_surface).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverAnchorRect {
    /// Left coordinate in logical window space.
    pub x: f64,
    /// Top coordinate in logical window space.
    pub y: f64,
    /// Trigger width in logical pixels.
    pub width: f64,
    /// Trigger height in logical pixels.
    pub height: f64,
}

impl PopoverAnchorRect {
    /// Constructs a [`PopoverAnchorRect`] from a Kurbo `Rect`.
    #[must_use]
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            x: rect.x0,
            y: rect.y0,
            width: rect.width(),
            height: rect.height(),
        }
    }

    /// Converts back to a Kurbo `Rect`.
    #[must_use]
    pub fn to_rect(self) -> Rect {
        Rect::from_origin_size((self.x, self.y), (self.width, self.height))
    }
}

/// Internal widget action emitted when a [`PopoverAnchorWidget`] captures or clears its window bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopoverAnchorAction {
    /// Trigger interacted cleanly and reported its current window rectangle.
    Updated(PopoverAnchorRect),
    /// Trigger was stashed or unmounted; open popovers anchored to it should dismiss.
    Cleared,
}

/// Masonry container capturing window-space anchor coordinates for a popover trigger widget.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct PopoverAnchorWidget {
    child: WidgetPod<dyn Widget>,
    last_rect: Option<PopoverAnchorRect>,
}

impl PopoverAnchorWidget {
    fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: child.erased().to_pod(),
            last_rect: None,
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    fn current_window_rect(origin: Point, size: Size) -> Option<PopoverAnchorRect> {
        (size != Size::ZERO).then_some(PopoverAnchorRect::from_rect(Rect::from_origin_size(
            origin, size,
        )))
    }
}

impl Widget for PopoverAnchorWidget {
    type Action = PopoverAnchorAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if matches!(event, PointerEvent::Down(_)) && !ctx.is_stashed() {
            let box_rect = ctx.border_box();
            if let Some(rect) =
                Self::current_window_rect(ctx.to_window(box_rect.origin()), box_rect.size())
                && self.last_rect != Some(rect)
            {
                self.last_rect = Some(rect);
                ctx.submit_action::<PopoverAnchorAction>(PopoverAnchorAction::Updated(rect));
            }
        }
    }

    fn on_action(
        &mut self,
        ctx: &mut ActionCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _action: &ErasedAction,
        _source: WidgetId,
    ) {
        if !ctx.is_stashed() {
            let box_rect = ctx.border_box();
            if let Some(rect) =
                Self::current_window_rect(ctx.to_window(box_rect.origin()), box_rect.size())
                && self.last_rect != Some(rect)
            {
                self.last_rect = Some(rect);
                ctx.submit_action::<PopoverAnchorAction>(PopoverAnchorAction::Updated(rect));
            }
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
        if !ctx.is_stashed() {
            let box_rect = ctx.border_box();
            if let Some(rect) =
                Self::current_window_rect(ctx.to_window(box_rect.origin()), box_rect.size())
                && self.last_rect != Some(rect)
            {
                self.last_rect = Some(rect);
                ctx.submit_action::<PopoverAnchorAction>(PopoverAnchorAction::Updated(rect));
            }
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::StashedChanged(true)) && self.last_rect.take().is_some() {
            ctx.submit_action::<PopoverAnchorAction>(PopoverAnchorAction::Cleared);
        }
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
        trace_span!("PopoverAnchorWidget", id = id.trace())
    }
}

/// Callback receiving `Some(anchor_rect)` when interacted with, or `None` if stashed/unmounted.
pub type PopoverAnchorCallback<State, Action> =
    Arc<dyn Fn(&mut State, Option<PopoverAnchorRect>) -> Action + Send + Sync>;

/// Xilem wrapper tracking a trigger's window-space rectangle for [`popover_surface`](crate::components::popover_surface).
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic trigger View and closure callback"
)]
pub struct PopoverAnchorView<TriggerV, State, Action> {
    trigger: TriggerV,
    on_anchor: PopoverAnchorCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Wraps `trigger` and reports its window-space [`PopoverAnchorRect`] to `on_anchor` before/during activation.
pub fn popover_anchor<State, Action, TriggerV, OnAnchor>(
    trigger: TriggerV,
    on_anchor: OnAnchor,
) -> PopoverAnchorView<TriggerV, State, Action>
where
    State: 'static,
    Action: 'static,
    TriggerV: WidgetView<State, Action>,
    OnAnchor: Fn(&mut State, Option<PopoverAnchorRect>) -> Action + Send + Sync + 'static,
{
    PopoverAnchorView {
        trigger,
        on_anchor: Arc::new(on_anchor),
        phantom: PhantomData,
    }
}

impl<TriggerV, State, Action> ViewMarker for PopoverAnchorView<TriggerV, State, Action> {}

impl<TriggerV, State, Action> View<State, Action, ViewCtx>
    for PopoverAnchorView<TriggerV, State, Action>
where
    TriggerV: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<PopoverAnchorWidget>;
    type ViewState = TriggerV::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (trigger_pod, st) = ctx.with_id(POPOVER_ANCHOR_CHILD_ID, |ctx| {
            self.trigger.build(ctx, app_state)
        });
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(PopoverAnchorWidget::new(trigger_pod.new_widget))
        });
        (pod, st)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let mut child = PopoverAnchorWidget::child_mut(&mut element);
        ctx.with_id(POPOVER_ANCHOR_CHILD_ID, |ctx| {
            self.trigger
                .rebuild(&prev.trigger, view_state, ctx, child.downcast(), app_state);
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child = PopoverAnchorWidget::child_mut(&mut element);
            ctx.with_id(POPOVER_ANCHOR_CHILD_ID, |ctx| {
                self.trigger.teardown(view_state, ctx, child.downcast());
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
            Some(POPOVER_ANCHOR_CHILD_ID) => {
                let mut child = PopoverAnchorWidget::child_mut(&mut element);
                self.trigger
                    .message(view_state, message, child.downcast(), app_state)
            }
            None => match message.take_message::<PopoverAnchorAction>() {
                Some(action) => match *action {
                    PopoverAnchorAction::Updated(rect) => {
                        MessageResult::Action((self.on_anchor)(app_state, Some(rect)))
                    }
                    PopoverAnchorAction::Cleared => {
                        MessageResult::Action((self.on_anchor)(app_state, None))
                    }
                },
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}

/// Overlay container placing a popover content widget adjacent to a [`PopoverAnchorRect`] inside viewport bounds.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct PopoverPlacementWidget {
    child: WidgetPod<dyn Widget>,
    anchor: PopoverAnchorRect,
    offset: Vec2,
    parent_window_origin: Point,
}

impl PopoverPlacementWidget {
    fn new(
        child: NewWidget<impl Widget + ?Sized>,
        anchor: PopoverAnchorRect,
        offset: Vec2,
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            anchor,
            offset,
            parent_window_origin: Point::ORIGIN,
        }
    }

    fn sync(this: &mut WidgetMut<'_, Self>, anchor: PopoverAnchorRect, offset: Vec2) {
        if this.widget.anchor != anchor || this.widget.offset != offset {
            this.widget.anchor = anchor;
            this.widget.offset = offset;
            this.ctx.request_layout();
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for PopoverPlacementWidget {
    type Action = NoAction;

    fn accepts_pointer_interaction(&self) -> bool {
        false
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _interval: u64,
    ) {
        let origin = ctx.to_window(Point::ORIGIN);
        if (origin.x - self.parent_window_origin.x).abs() > 0.5
            || (origin.y - self.parent_window_origin.y).abs() > 0.5
        {
            self.parent_window_origin = origin;
            ctx.request_layout();
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        _axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match len_req {
            LenReq::MinContent => Length::ZERO,
            LenReq::MaxContent => Length::px(1_000_000.0),
            LenReq::FitContent(space) => space,
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let max_size = Size::new((size.width - 16.0).max(0.0), (size.height - 16.0).max(0.0));
        // First measure intrinsic content size so compact popovers hug tightly, then fit-clamp on tight screens.
        let natural = ctx.compute_size(
            &mut self.child,
            SizeDef::new(LenDef::MaxContent, LenDef::MaxContent),
            LayoutSize::NONE,
        );
        let child_size = if natural.width > max_size.width || natural.height > max_size.height {
            ctx.compute_size(&mut self.child, SizeDef::fit(max_size), max_size.into())
        } else {
            natural
        };
        let clamped_size = Size::new(
            child_size.width.min(max_size.width),
            child_size.height.min(max_size.height),
        );
        let local_anchor = Rect::from_origin_size(
            Point::new(
                self.anchor.x - self.parent_window_origin.x,
                self.anchor.y - self.parent_window_origin.y,
            ),
            (self.anchor.width, self.anchor.height),
        );
        let placed_origin =
            clamp_anchored_layer_origin(local_anchor, clamped_size, size, self.offset);
        ctx.run_layout(&mut self.child, clamped_size);
        ctx.place_child(&mut self.child, placed_origin);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::Dialog
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
        trace_span!("PopoverPlacementWidget", id = id.trace())
    }
}

/// Positions `child` inside the viewport overlay adjacent to `anchor` with automatic edge clamping and flipping.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Holds generic child View")]
pub struct PopoverPlacementView<ChildV, State, Action> {
    child: ChildV,
    anchor: PopoverAnchorRect,
    offset: Vec2,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Positions `child` inside the overlay coordinate space relative to `anchor`.
pub fn popover_place<ChildV, State, Action>(
    anchor: PopoverAnchorRect,
    offset: Vec2,
    child: ChildV,
) -> PopoverPlacementView<ChildV, State, Action>
where
    State: 'static,
    Action: 'static,
    ChildV: WidgetView<State, Action>,
{
    PopoverPlacementView {
        child,
        anchor,
        offset,
        phantom: PhantomData,
    }
}

impl<ChildV, State, Action> ViewMarker for PopoverPlacementView<ChildV, State, Action> {}

impl<ChildV, State, Action> View<State, Action, ViewCtx>
    for PopoverPlacementView<ChildV, State, Action>
where
    ChildV: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<PopoverPlacementWidget>;
    type ViewState = ChildV::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, st) = ctx.with_id(POPOVER_PLACEMENT_CHILD_ID, |ctx| {
            self.child.build(ctx, app_state)
        });
        let pod = ctx.create_pod(PopoverPlacementWidget::new(
            child_pod.new_widget,
            self.anchor,
            self.offset,
        ));
        (pod, st)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        PopoverPlacementWidget::sync(&mut element, self.anchor, self.offset);
        let mut child = PopoverPlacementWidget::child_mut(&mut element);
        ctx.with_id(POPOVER_PLACEMENT_CHILD_ID, |ctx| {
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
        let mut child = PopoverPlacementWidget::child_mut(&mut element);
        ctx.with_id(POPOVER_PLACEMENT_CHILD_ID, |ctx| {
            self.child.teardown(view_state, ctx, child.downcast());
        });
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        if matches!(message.take_first(), Some(POPOVER_PLACEMENT_CHILD_ID)) {
            let mut child = PopoverPlacementWidget::child_mut(&mut element);
            self.child
                .message(view_state, message, child.downcast(), app_state)
        } else {
            MessageResult::Stale
        }
    }
}
