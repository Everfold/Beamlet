//! Composited VSync slide-and-fade transitions driven by Masonry animation frames.

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, ComposeCtx, EventCtx, LayoutCtx, MeasureCtx, NewWidget, NoAction,
    PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget,
    WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::{Composite, GroupRef, Painter};
use masonry::kurbo::{Axis, Point, Size, Vec2};
use masonry::layout::{LayoutSize, LenDef, LenReq, Length};
use masonry::peniko;
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::tokens::{MotionDuration, MotionEasing};

fn opt_layout_size(width: Option<Length>, height: Option<Length>) -> LayoutSize {
    match (width, height) {
        (Some(w), Some(h)) => LayoutSize::new(w, h),
        (Some(w), None) => LayoutSize::one(Axis::Horizontal, w),
        (None, Some(h)) => LayoutSize::one(Axis::Vertical, h),
        (None, None) => LayoutSize::NONE,
    }
}

/// Masonry wrapper widget animating child position and opacity on mount, key change, or exit.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct SlideTransition {
    child: WidgetPod<dyn Widget>,
    offset: Vec2,
    duration_ns: u64,
    elapsed_ns: u64,
    exiting: bool,
    easing: MotionEasing,
    clamp_axes: (bool, bool),
    preferred_max: (Option<f64>, Option<f64>),
}

impl SlideTransition {
    /// Creates a new [`SlideTransition`] starting from `offset` and easing over `duration_ms`.
    #[must_use]
    pub fn new(
        child: NewWidget<impl Widget + ?Sized>,
        offset: Vec2,
        duration_ms: u64,
        easing: MotionEasing,
    ) -> Self {
        Self::new_with_limits(
            child,
            offset,
            duration_ms,
            easing,
            (false, false),
            (None, None),
        )
    }

    /// Creates a [`SlideTransition`] that caps child extent against available context size per axis.
    #[must_use]
    pub(crate) fn new_with_limits(
        child: NewWidget<impl Widget + ?Sized>,
        offset: Vec2,
        duration_ms: u64,
        easing: MotionEasing,
        clamp_axes: (bool, bool),
        preferred_max: (Option<f64>, Option<f64>),
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            offset,
            duration_ns: duration_ms.saturating_mul(1_000_000),
            elapsed_ns: 0,
            exiting: false,
            easing,
            clamp_axes,
            preferred_max,
        }
    }

    /// Updates optional per-axis maximum card extents (used by modal dialogs/sheets).
    pub(crate) fn set_limits(
        this: &mut WidgetMut<'_, Self>,
        clamp_axes: (bool, bool),
        preferred_max: (Option<f64>, Option<f64>),
    ) {
        if this.widget.clamp_axes != clamp_axes || this.widget.preferred_max != preferred_max {
            this.widget.clamp_axes = clamp_axes;
            this.widget.preferred_max = preferred_max;
            this.ctx.request_layout();
        }
    }

    fn effective_axis_cap(avail: Option<Length>, clamp: bool, pref: Option<f64>) -> Option<f64> {
        if !clamp {
            return match (avail, pref) {
                (Some(a), Some(p)) => Some(p.min(a.get().max(0.0))),
                (_, p) => p,
            };
        }
        avail.map_or(pref, |a| {
            let av = a.get().max(0.0);
            let margin = if av <= 200.0 { 12.0 } else { 24.0 };
            let cap = (av - margin).max(0.0);
            Some(pref.map_or(cap, |p| p.min(cap)))
        })
    }

    /// Returns mutable access to the wrapped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    /// Re-triggers the enter animation from `offset` over `duration_ms`.
    pub fn restart(
        this: &mut WidgetMut<'_, Self>,
        offset: Vec2,
        duration_ms: u64,
        easing: MotionEasing,
    ) {
        this.widget.offset = offset;
        this.widget.duration_ns = duration_ms.saturating_mul(1_000_000);
        this.widget.elapsed_ns = 0;
        this.widget.exiting = false;
        this.widget.easing = easing;
        this.ctx.request_anim_frame();
        this.ctx.request_compose();
        this.ctx.request_paint_only();
        this.ctx.request_post_paint();
    }

    /// Starts a reverse slide-out & fade-out exit animation toward `offset`.
    pub fn start_exit(this: &mut WidgetMut<'_, Self>, offset: Vec2, duration_ms: u64) {
        this.widget.offset = offset;
        this.widget.duration_ns = duration_ms.saturating_mul(1_000_000);
        this.widget.elapsed_ns = 0;
        this.widget.exiting = true;
        this.widget.easing = MotionEasing::EmphasizedAccelerate;
        this.ctx.request_anim_frame();
        this.ctx.request_compose();
        this.ctx.request_paint_only();
        this.ctx.request_post_paint();
    }

    fn current_eased(&self) -> f64 {
        if self.duration_ns == 0 {
            return 1.0;
        }
        let raw = (self.elapsed_ns as f64 / self.duration_ns as f64).clamp(0.0, 1.0);
        self.easing.sample(raw)
    }

    fn current_translation(&self) -> Vec2 {
        let p = self.current_eased();
        if self.exiting {
            self.offset * p
        } else {
            self.offset * (1.0 - p)
        }
    }

    fn current_alpha(&self) -> f32 {
        let p = self.current_eased() as f32;
        if self.exiting { 1.0 - p } else { p }.clamp(0.0, 1.0)
    }
}

impl Widget for SlideTransition {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) && self.elapsed_ns < self.duration_ns {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.elapsed_ns < self.duration_ns {
            self.elapsed_ns = (self.elapsed_ns + interval).min(self.duration_ns);
            ctx.request_compose();
            ctx.request_paint_only();
            ctx.request_post_paint();
            if self.elapsed_ns < self.duration_ns {
                ctx.request_anim_frame();
            }
        }
    }

    fn on_pointer_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &PointerEvent,
    ) {
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let cs = ctx.context_size();
        if !self.clamp_axes.0
            && !self.clamp_axes.1
            && self.preferred_max.0.is_none()
            && self.preferred_max.1.is_none()
        {
            return ctx.redirect_measurement(&mut self.child, axis, cross_length);
        }

        let avail_w = match (axis, len_req) {
            (Axis::Horizontal, LenReq::FitContent(s)) => Some(s),
            (Axis::Horizontal, _) => cs.length(Axis::Horizontal),
            (Axis::Vertical, _) => cross_length.or_else(|| cs.length(Axis::Horizontal)),
        };
        let avail_h = match (axis, len_req) {
            (Axis::Vertical, LenReq::FitContent(s)) => Some(s),
            (Axis::Vertical, _) => cs.length(Axis::Vertical),
            (Axis::Horizontal, _) => cross_length.or_else(|| cs.length(Axis::Vertical)),
        };

        let cap_w = Self::effective_axis_cap(avail_w, self.clamp_axes.0, self.preferred_max.0);
        let cap_h = Self::effective_axis_cap(avail_h, self.clamp_axes.1, self.preferred_max.1);

        match axis {
            Axis::Horizontal => {
                if matches!(len_req, LenReq::MinContent) && (self.clamp_axes.0 || cap_w.is_some()) {
                    return Length::ZERO;
                }
                let child_auto = if let Some(w) = cap_w {
                    if self.preferred_max.0.is_some() {
                        LenDef::Fixed(Length::px(w))
                    } else {
                        LenDef::FitContent(Length::px(w))
                    }
                } else {
                    len_req.into()
                };
                let eff_cross = cap_h.map(Length::px).or(cross_length);
                let child_ctx = opt_layout_size(cap_w.map(Length::px), eff_cross);
                let measured = ctx.compute_length(
                    &mut self.child,
                    child_auto,
                    child_ctx,
                    Axis::Horizontal,
                    eff_cross,
                );
                if let Some(w) = cap_w {
                    if self.preferred_max.0.is_some() {
                        Length::px(w)
                    } else {
                        Length::px(measured.get().min(w))
                    }
                } else {
                    measured
                }
            }
            Axis::Vertical => {
                if matches!(len_req, LenReq::MinContent) && (self.clamp_axes.1 || cap_h.is_some()) {
                    return Length::ZERO;
                }
                let eff_cross = cap_w.map(Length::px).or(cross_length);
                let child_auto = if self.preferred_max.1.is_some()
                    && let Some(h) = cap_h
                {
                    LenDef::Fixed(Length::px(h))
                } else if self.clamp_axes.1 {
                    LenDef::MaxContent
                } else {
                    len_req.into()
                };
                let child_ctx = opt_layout_size(eff_cross, cap_h.map(Length::px));
                let measured = ctx.compute_length(
                    &mut self.child,
                    child_auto,
                    child_ctx,
                    Axis::Vertical,
                    eff_cross,
                );
                if let Some(h) = cap_h {
                    if self.preferred_max.1.is_some() {
                        Length::px(h)
                    } else {
                        Length::px(measured.get().min(h))
                    }
                } else {
                    measured
                }
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.child, size);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.derive_baselines(&self.child);
    }

    fn compose(&mut self, ctx: &mut ComposeCtx<'_>) {
        let delta = self.current_translation();
        ctx.set_animated_child_scroll_translation(&mut self.child, delta);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let alpha = self.current_alpha();
        if alpha < 0.999 {
            painter.push_group(
                GroupRef::new().with_composite(Composite::new(peniko::BlendMode::default(), alpha)),
            );
        }
    }

    fn post_paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        if self.current_alpha() < 0.999 {
            painter.pop_group();
        }
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
        trace_span!("SlideTransition", id = id.trace())
    }
}

/// Wraps `child` in a keyed slide-and-fade entrance animation whenever `key` changes.
pub fn ui_transition<State, Action, V>(key: u64, child: V) -> UiTransition<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    UiTransition {
        key,
        child,
        offset: Vec2::new(0.0, 12.0),
        duration: MotionDuration::Short4,
        easing: MotionEasing::EmphasizedDecelerate,
        phantom: PhantomData,
    }
}

/// View wrapper configuring [`SlideTransition`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains generic child View")]
pub struct UiTransition<V, State, Action> {
    key: u64,
    child: V,
    offset: Vec2,
    duration: MotionDuration,
    easing: MotionEasing,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> UiTransition<V, State, Action> {
    /// Sets the starting spatial offset for entrance animation.
    pub fn offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the transition duration preset.
    pub fn motion(mut self, duration: MotionDuration, easing: MotionEasing) -> Self {
        self.duration = duration;
        self.easing = easing;
        self
    }

    /// Sets duration in milliseconds via the closest or custom duration.
    pub fn duration(mut self, duration: MotionDuration) -> Self {
        self.duration = duration;
        self
    }
}

impl<V, State, Action> ViewMarker for UiTransition<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiTransition<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<SlideTransition>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let reduced = crate::style_engine::current_spec_arc(ctx).reduced_motion;
        let (offset, duration_ms) = if reduced {
            (Vec2::ZERO, 0)
        } else {
            (self.offset, self.duration.as_millis())
        };
        let (child_pod, child_state) = self.child.build(ctx, app_state);
        let pod = ctx.create_pod(SlideTransition::new(
            child_pod.new_widget,
            offset,
            duration_ms,
            self.easing,
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
        if self.key != prev.key {
            let reduced = crate::style_engine::current_spec_arc(ctx).reduced_motion;
            let (offset, duration_ms) = if reduced {
                (Vec2::ZERO, 0)
            } else {
                (self.offset, self.duration.as_millis())
            };
            SlideTransition::restart(&mut element, offset, duration_ms, self.easing);
        }
        let mut child = SlideTransition::child_mut(&mut element);
        self.child
            .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child = SlideTransition::child_mut(&mut element);
        self.child.teardown(view_state, ctx, child.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut child = SlideTransition::child_mut(&mut element);
        self.child
            .message(view_state, message, child.downcast(), app_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_axis_cap_respects_extreme_narrow_and_wide_bounds() {
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(160.0)), true, Some(380.0)),
            Some(148.0)
        );
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(1200.0)), true, Some(380.0)),
            Some(380.0)
        );
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(160.0)), false, Some(240.0)),
            Some(160.0)
        );
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(1200.0)), false, Some(240.0)),
            Some(240.0)
        );
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(160.0)), false, None),
            None
        );
        assert_eq!(
            SlideTransition::effective_axis_cap(Some(Length::px(180.0)), true, None),
            Some(168.0)
        );
    }
}
