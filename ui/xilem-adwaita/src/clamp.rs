//! GNOME Libadwaita `AdwClamp` widget constraining maximum child content width while centering horizontally.

use std::marker::PhantomData;

use tracing::{Span, trace_span};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Size};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{LayoutSize, LenReq, Length};
use xilem::{Pod, ViewCtx, WidgetView};

const CLAMP_CHILD_ID: ViewId = ViewId::new(0x5549_3801);

/// Masonry container constraining its single child's width to `max_width` and centering it horizontally.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct AdwClamp {
    child: WidgetPod<dyn Widget>,
    max_width: f64,
}

impl AdwClamp {
    fn new(child: NewWidget<impl Widget + ?Sized>, max_width: f64) -> Self {
        Self {
            child: child.erased().to_pod(),
            max_width: max_width.max(1.0),
        }
    }

    fn set_max_width(this: &mut WidgetMut<'_, Self>, max_width: f64) {
        let next = max_width.max(1.0);
        if (this.widget.max_width - next).abs() > f64::EPSILON {
            this.widget.max_width = next;
            this.ctx.request_layout();
        }
    }

    /// Returns a mutable reference to the clamped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for AdwClamp {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        match axis {
            Axis::Horizontal => match len_req {
                LenReq::FitContent(space) => {
                    let capped_cross = cross_length;
                    let _ = ctx.compute_length(
                        &mut self.child,
                        LenReq::FitContent(Length::px(space.get().min(self.max_width))).into(),
                        LayoutSize::maybe(Axis::Vertical, capped_cross),
                        Axis::Horizontal,
                        capped_cross,
                    );
                    space
                }
                LenReq::MinContent | LenReq::MaxContent => {
                    let child_len = ctx.compute_length(
                        &mut self.child,
                        len_req.into(),
                        LayoutSize::maybe(Axis::Vertical, cross_length),
                        Axis::Horizontal,
                        cross_length,
                    );
                    Length::px(child_len.get().min(self.max_width))
                }
            },
            Axis::Vertical => {
                let capped_cross = cross_length.map(|w| Length::px(w.get().min(self.max_width)));
                ctx.compute_length(
                    &mut self.child,
                    len_req.into(),
                    LayoutSize::maybe(Axis::Horizontal, capped_cross),
                    Axis::Vertical,
                    capped_cross,
                )
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let clamped_w = size.width.min(self.max_width);
        let child_size = Size::new(clamped_w, size.height);
        ctx.run_layout(&mut self.child, child_size);
        let x = ((size.width - clamped_w) * 0.5).max(0.0);
        ctx.place_child(&mut self.child, Point::new(x, 0.0));
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
        trace_span!("AdwClamp", id = id.trace())
    }
}

/// View wrapper constructed by [`clamp`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct ClampView<V, State, Action> {
    max_width: f64,
    child: V,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

/// GNOME `AdwClamp`: limits `child`'s maximum width to `max_width` and centers it horizontally.
pub fn clamp<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    max_width: f64,
    child: V,
) -> ClampView<V, State, Action> {
    ClampView {
        max_width,
        child,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for ClampView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for ClampView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<AdwClamp>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, child_state) =
            ctx.with_id(CLAMP_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(AdwClamp::new(child_pod.new_widget, self.max_width));
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
        AdwClamp::set_max_width(&mut element, self.max_width);
        let mut child_dyn = AdwClamp::child_mut(&mut element);
        ctx.with_id(CLAMP_CHILD_ID, |ctx| {
            self.child.rebuild(
                &prev.child,
                view_state,
                ctx,
                child_dyn.downcast(),
                app_state,
            );
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child_dyn = AdwClamp::child_mut(&mut element);
        ctx.with_id(CLAMP_CHILD_ID, |ctx| {
            self.child.teardown(view_state, ctx, child_dyn.downcast());
        });
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(CLAMP_CHILD_ID) => {
                let mut child_dyn = AdwClamp::child_mut(&mut element);
                self.child
                    .message(view_state, message, child_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
