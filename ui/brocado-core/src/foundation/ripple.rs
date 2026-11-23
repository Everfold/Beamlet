//! Material Design 3 clipped ink ripple wrapper ([`RippleLayer`] / [`ripple_layer`]).

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PointerButton,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, UpdateCtx, Widget, WidgetId,
    WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Circle, Point, Size};
use masonry::layout::{LenReq, Length};
use masonry::peniko::Color;
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::foundation::state_layer::with_opacity;
use crate::style_engine::current_spec_arc;
use crate::tokens::StateFeedbackMode;

const RIPPLE_CHILD_ID: ViewId = ViewId::new(0x5549_3610);
const RIPPLE_DURATION_SECS: f64 = 0.28;

#[derive(Clone, Copy, Debug, PartialEq)]
struct RippleConfig {
    enabled: bool,
    tint: Color,
    radius: f64,
}

/// Masonry single-child wrapper painting a clipped radial ink wave on primary pointer press.
///
/// Leaves `propagates_pointer_interaction()` enabled and never consumes pointer events so wrapped
/// controls (such as [`Button`](masonry::widgets::Button)) handle clicks normally.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct RippleLayer {
    child: WidgetPod<dyn Widget>,
    config: RippleConfig,
    origin: Point,
    progress: f64,
    active: bool,
}

impl RippleLayer {
    fn new(child: NewWidget<impl Widget + ?Sized>, config: RippleConfig) -> Self {
        Self {
            child: child.erased().to_pod(),
            config,
            origin: Point::ZERO,
            progress: 0.0,
            active: false,
        }
    }

    fn set_config(this: &mut WidgetMut<'_, Self>, config: RippleConfig) {
        if this.widget.config != config {
            this.widget.config = config;
            if !config.enabled {
                this.widget.active = false;
                this.widget.progress = 0.0;
            }
            this.ctx.request_post_paint();
        }
    }

    /// Returns a mutable reference to the wrapped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for RippleLayer {
    type Action = NoAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut masonry::core::EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if !self.config.enabled {
            return;
        }
        if let PointerEvent::Down(btn) = event
            && btn.button == Some(PointerButton::Primary)
        {
            self.origin = ctx.local_position(btn.state.position);
            self.progress = 0.0;
            self.active = true;
            ctx.request_anim_frame();
            ctx.request_post_paint();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if !self.active {
            return;
        }
        let dt = interval as f64 / 1_000_000_000.0;
        self.progress = (self.progress + dt / RIPPLE_DURATION_SECS).min(1.0);
        ctx.request_post_paint();
        if self.progress >= 1.0 {
            self.active = false;
        } else {
            ctx.request_anim_frame();
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

    fn post_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        if !self.active || !self.config.enabled {
            return;
        }
        let rect = ctx.border_box();
        if rect.width() <= 1.0 || rect.height() <= 1.0 {
            return;
        }
        let eased = 1.0 - (1.0 - self.progress).powi(2);
        let max_dx = (self.origin.x - rect.x0)
            .abs()
            .max((rect.x1 - self.origin.x).abs());
        let max_dy = (self.origin.y - rect.y0)
            .abs()
            .max((rect.y1 - self.origin.y).abs());
        let max_radius = max_dx.hypot(max_dy).max(1.0);
        let current_radius = max_radius * eased;
        let alpha = (0.16 * (1.0 - self.progress) as f32).clamp(0.0, 0.16);
        if alpha <= 0.001 || current_radius <= 0.2 {
            return;
        }

        let rrect = rect.to_rounded_rect(
            self.config
                .radius
                .max(0.0)
                .min(rect.width().min(rect.height()) / 2.0),
        );
        painter.push_fill_clip(rrect);
        painter
            .fill(
                Circle::new(self.origin, current_radius),
                with_opacity(self.config.tint, alpha),
            )
            .draw();
        painter.pop_clip();
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
        trace_span!("RippleLayer", id = id.trace())
    }
}

/// View wrapper constructed by [`ripple_layer`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct RippleLayerView<V, State, Action> {
    child: V,
    radius: Option<f64>,
    tint: Option<Color>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> RippleLayerView<V, State, Action> {
    /// Overrides the rounded-rectangle clip corner radius in logical pixels.
    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = Some(radius);
        self
    }

    /// Overrides the ripple ink wave color.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    fn resolve_config(&self, ctx: &ViewCtx) -> RippleConfig {
        let spec = current_spec_arc(ctx);
        RippleConfig {
            enabled: matches!(spec.feedback, StateFeedbackMode::TintedStateLayer { .. })
                && !spec.reduced_motion,
            tint: self.tint.unwrap_or(spec.palette.accent),
            radius: self.radius.unwrap_or(spec.metrics.button_radius),
        }
    }
}

/// Wraps `child` with a Material Design 3 clipped ink [`RippleLayer`] when [`StateFeedbackMode::TintedStateLayer`]
/// is active and `reduced_motion` is `false`.
pub fn ripple_layer<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> RippleLayerView<V, State, Action> {
    RippleLayerView {
        child,
        radius: None,
        tint: None,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for RippleLayerView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for RippleLayerView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<RippleLayer>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let config = self.resolve_config(ctx);
        let (child_pod, child_state) =
            ctx.with_id(RIPPLE_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(RippleLayer::new(child_pod.new_widget, config));
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
        let config = self.resolve_config(ctx);
        RippleLayer::set_config(&mut element, config);
        let mut child_dyn = RippleLayer::child_mut(&mut element);
        ctx.with_id(RIPPLE_CHILD_ID, |ctx| {
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
        let mut child_dyn = RippleLayer::child_mut(&mut element);
        ctx.with_id(RIPPLE_CHILD_ID, |ctx| {
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
            Some(RIPPLE_CHILD_ID) => {
                let mut child_dyn = RippleLayer::child_mut(&mut element);
                self.child
                    .message(view_state, message, child_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
