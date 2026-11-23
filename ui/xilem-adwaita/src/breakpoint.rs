//! Responsive `AdwBreakpointBin` container switching cleanly between wide and narrow layouts.

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size};
use masonry::layout::{LayoutSize, LenReq, Length};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

const WIDE_CHILD_ID: ViewId = ViewId::new(0x5549_3810);
const NARROW_CHILD_ID: ViewId = ViewId::new(0x5549_3811);

/// Masonry container automatically toggling visibility between `wide` and `narrow` subtrees
/// based on whether allocated width drops below `breakpoint_px`.
#[expect(
    missing_debug_implementations,
    reason = "Holds erased Masonry WidgetPods"
)]
pub struct AdwBreakpointBin {
    wide: WidgetPod<dyn Widget>,
    narrow: WidgetPod<dyn Widget>,
    breakpoint_px: f64,
    is_narrow: bool,
}

impl AdwBreakpointBin {
    fn new(
        wide: NewWidget<impl Widget + ?Sized>,
        narrow: NewWidget<impl Widget + ?Sized>,
        breakpoint_px: f64,
    ) -> Self {
        Self {
            wide: wide.erased().to_pod(),
            narrow: narrow.erased().to_pod(),
            breakpoint_px: breakpoint_px.max(1.0),
            is_narrow: false,
        }
    }

    fn set_breakpoint(this: &mut WidgetMut<'_, Self>, breakpoint_px: f64) {
        let next = breakpoint_px.max(1.0);
        if (this.widget.breakpoint_px - next).abs() > f64::EPSILON {
            this.widget.breakpoint_px = next;
            this.ctx.request_layout();
        }
    }

    fn wide_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.wide)
    }

    fn narrow_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.narrow)
    }
}

impl Widget for AdwBreakpointBin {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.wide);
        ctx.register_child(&mut self.narrow);
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let use_narrow = match (axis, len_req) {
            (Axis::Horizontal, LenReq::MinContent) => true,
            (Axis::Horizontal, LenReq::FitContent(space)) => space.get() < self.breakpoint_px,
            (Axis::Horizontal, LenReq::MaxContent) => ctx
                .context_size()
                .length(Axis::Horizontal)
                .map_or(self.is_narrow, |w| w.get() < self.breakpoint_px),
            (Axis::Vertical, _) => cross_length
                .or_else(|| ctx.context_size().length(Axis::Horizontal))
                .map_or(self.is_narrow, |w| w.get() < self.breakpoint_px),
        };
        let active = if use_narrow {
            &mut self.narrow
        } else {
            &mut self.wide
        };
        match (axis, len_req) {
            (Axis::Horizontal, LenReq::FitContent(space)) => {
                let _ = ctx.compute_length(
                    active,
                    len_req.into(),
                    LayoutSize::maybe(Axis::Vertical, cross_length),
                    Axis::Horizontal,
                    cross_length,
                );
                space
            }
            _ => ctx.compute_length(
                active,
                len_req.into(),
                LayoutSize::maybe(axis.cross(), cross_length),
                axis,
                cross_length,
            ),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let narrow = size.width < self.breakpoint_px;
        self.is_narrow = narrow;
        ctx.set_stashed(&mut self.wide, narrow);
        ctx.set_stashed(&mut self.narrow, !narrow);

        let active = if narrow {
            &mut self.narrow
        } else {
            &mut self.wide
        };
        ctx.run_layout(active, size);
        ctx.place_child(active, Point::ORIGIN);
        ctx.derive_baselines(active);
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
        ChildrenIds::from_slice(&[self.wide.id(), self.narrow.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("AdwBreakpointBin", id = id.trace())
    }
}

/// View wrapper constructed by [`breakpoint_bin`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct BreakpointBin<W, N, State, Action> {
    breakpoint_px: f64,
    wide: W,
    narrow: N,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

/// GNOME `AdwBreakpointBin`: displays `wide` when width `>= breakpoint_px`, otherwise switches to `narrow`.
pub fn breakpoint_bin<State: 'static, Action: 'static, W, N>(
    breakpoint_px: f64,
    wide: W,
    narrow: N,
) -> BreakpointBin<W, N, State, Action>
where
    W: WidgetView<State, Action>,
    N: WidgetView<State, Action>,
{
    BreakpointBin {
        breakpoint_px,
        wide,
        narrow,
        phantom: PhantomData,
    }
}

impl<W, N, State, Action> ViewMarker for BreakpointBin<W, N, State, Action> {}

impl<W, N, State, Action> View<State, Action, ViewCtx> for BreakpointBin<W, N, State, Action>
where
    W: WidgetView<State, Action>,
    N: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<AdwBreakpointBin>;
    type ViewState = (W::ViewState, N::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (wide_pod, wide_state) =
            ctx.with_id(WIDE_CHILD_ID, |ctx| self.wide.build(ctx, app_state));
        let (narrow_pod, narrow_state) =
            ctx.with_id(NARROW_CHILD_ID, |ctx| self.narrow.build(ctx, app_state));
        let pod = ctx.create_pod(AdwBreakpointBin::new(
            wide_pod.new_widget,
            narrow_pod.new_widget,
            self.breakpoint_px,
        ));
        (pod, (wide_state, narrow_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (wide_state, narrow_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        AdwBreakpointBin::set_breakpoint(&mut element, self.breakpoint_px);
        {
            let mut wide_dyn = AdwBreakpointBin::wide_mut(&mut element);
            ctx.with_id(WIDE_CHILD_ID, |ctx| {
                self.wide
                    .rebuild(&prev.wide, wide_state, ctx, wide_dyn.downcast(), app_state);
            });
        }
        let mut narrow_dyn = AdwBreakpointBin::narrow_mut(&mut element);
        ctx.with_id(NARROW_CHILD_ID, |ctx| {
            self.narrow.rebuild(
                &prev.narrow,
                narrow_state,
                ctx,
                narrow_dyn.downcast(),
                app_state,
            );
        });
    }

    fn teardown(
        &self,
        (wide_state, narrow_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut wide_dyn = AdwBreakpointBin::wide_mut(&mut element);
            ctx.with_id(WIDE_CHILD_ID, |ctx| {
                self.wide.teardown(wide_state, ctx, wide_dyn.downcast());
            });
        }
        let mut narrow_dyn = AdwBreakpointBin::narrow_mut(&mut element);
        ctx.with_id(NARROW_CHILD_ID, |ctx| {
            self.narrow
                .teardown(narrow_state, ctx, narrow_dyn.downcast());
        });
    }

    fn message(
        &self,
        (wide_state, narrow_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(WIDE_CHILD_ID) => {
                let mut wide_dyn = AdwBreakpointBin::wide_mut(&mut element);
                self.wide
                    .message(wide_state, message, wide_dyn.downcast(), app_state)
            }
            Some(NARROW_CHILD_ID) => {
                let mut narrow_dyn = AdwBreakpointBin::narrow_mut(&mut element);
                self.narrow
                    .message(narrow_state, message, narrow_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
