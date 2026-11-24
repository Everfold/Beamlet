//! Single-child clipping and shrink-to-fit wrapper preventing single-line chrome overflow.

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Rect, Size};
use masonry::layout::{LayoutSize, LenDef, LenReq, Length, SizeDef};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

const CLIP_BOX_CHILD_ID: ViewId = ViewId::new(0x5549_3460);

/// Masonry container allowing its child to shrink down to `0px` and clipping to allocated bounds.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct ClipBoxWidget {
    child: WidgetPod<dyn Widget>,
    fill_available: bool,
}

impl ClipBoxWidget {
    fn new(child: NewWidget<impl Widget + ?Sized>, fill_available: bool) -> Self {
        Self {
            child: child.erased().to_pod(),
            fill_available,
        }
    }

    fn set_fill_available(this: &mut WidgetMut<'_, Self>, fill_available: bool) {
        if this.widget.fill_available != fill_available {
            this.widget.fill_available = fill_available;
            this.ctx.request_layout();
        }
    }

    /// Returns mutable access to the clipped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for ClipBoxWidget {
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
                LenReq::MinContent => Length::ZERO,
                LenReq::MaxContent => ctx.compute_length(
                    &mut self.child,
                    LenDef::MaxContent,
                    LayoutSize::maybe(Axis::Vertical, cross_length),
                    Axis::Horizontal,
                    cross_length,
                ),
                LenReq::FitContent(space) => {
                    if self.fill_available {
                        return space;
                    }
                    let natural = ctx.compute_length(
                        &mut self.child,
                        LenDef::MaxContent,
                        LayoutSize::maybe(Axis::Vertical, cross_length),
                        Axis::Horizontal,
                        cross_length,
                    );
                    natural.min(space)
                }
            },
            Axis::Vertical => {
                let eff_cross =
                    cross_length.or_else(|| ctx.context_size().length(Axis::Horizontal));
                ctx.compute_length(
                    &mut self.child,
                    len_req.into(),
                    LayoutSize::maybe(Axis::Horizontal, eff_cross),
                    Axis::Vertical,
                    eff_cross,
                )
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let clamped_space = Size::new(size.width.max(0.0), size.height.max(0.0));
        let child_def = if self.fill_available {
            SizeDef::new(
                LenDef::Fixed(Length::px(clamped_space.width)),
                LenDef::FitContent(Length::px(clamped_space.height)),
            )
        } else {
            SizeDef::fit(clamped_space)
        };
        let child_size = ctx.compute_size(&mut self.child, child_def, clamped_space.into());
        let bounded = Size::new(
            if self.fill_available {
                clamped_space.width
            } else {
                child_size.width.min(clamped_space.width)
            },
            child_size.height.min(clamped_space.height),
        );
        ctx.run_layout(&mut self.child, bounded);
        let dy = ((clamped_space.height - bounded.height) * 0.5).max(0.0);
        ctx.place_child(&mut self.child, Point::new(0.0, dy));
        ctx.set_clip_path(Rect::from_origin_size(Point::ORIGIN, clamped_space));
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
        trace_span!("ClipBoxWidget", id = id.trace())
    }
}

/// View wrapper constructed by [`clip_box`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct ClipBoxView<V, State, Action> {
    child: V,
    fill_available: bool,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Wraps `child` in a [`ClipBoxWidget`] that hugs natural width when space permits and shrinks + clips cleanly without overflowing tight chrome.
pub fn clip_box<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> ClipBoxView<V, State, Action> {
    ClipBoxView {
        child,
        fill_available: false,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ClipBoxView<V, State, Action> {
    /// Expands the child to fill allocated width (`true`) when used inside stretch slots/full-width rows,
    /// rather than hugging natural content width (`false`, default).
    pub fn fill_available(mut self, fill: bool) -> Self {
        self.fill_available = fill;
        self
    }
}

impl<V, State, Action> ViewMarker for ClipBoxView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for ClipBoxView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<ClipBoxWidget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, child_state) =
            ctx.with_id(CLIP_BOX_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(ClipBoxWidget::new(
            child_pod.new_widget,
            self.fill_available,
        ));
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
        ClipBoxWidget::set_fill_available(&mut element, self.fill_available);
        let mut child = ClipBoxWidget::child_mut(&mut element);
        ctx.with_id(CLIP_BOX_CHILD_ID, |ctx| {
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
        let mut child = ClipBoxWidget::child_mut(&mut element);
        ctx.with_id(CLIP_BOX_CHILD_ID, |ctx| {
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
        match message.take_first() {
            Some(CLIP_BOX_CHILD_ID) => {
                let mut child = ClipBoxWidget::child_mut(&mut element);
                self.child
                    .message(view_state, message, child.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
