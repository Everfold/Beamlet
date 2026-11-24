//! General-purpose box constraint wrapper enforcing optional `min`/`max` width and height bounds.

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesMut,
    PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size};
use masonry::layout::{LayoutSize, LenDef, LenReq, Length};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

/// Masonry single-child layout constraint box supporting `min_width`, `max_width`,
/// `min_height`, `max_height`, and optional `fill_max_width`.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct ConstrainedBox {
    child: WidgetPod<dyn Widget>,
    min_width: f64,
    max_width: f64,
    min_height: f64,
    max_height: f64,
    fill_max_width: bool,
}

impl ConstrainedBox {
    /// Creates a new [`ConstrainedBox`] wrapping `child`.
    #[must_use]
    pub fn new(
        child: NewWidget<impl Widget + ?Sized>,
        min_width: f64,
        max_width: f64,
        min_height: f64,
        max_height: f64,
        fill_max_width: bool,
    ) -> Self {
        let min_width = min_width.max(0.0);
        let min_height = min_height.max(0.0);
        Self {
            child: child.erased().to_pod(),
            min_width,
            max_width: max_width.max(min_width),
            min_height,
            max_height: max_height.max(min_height),
            fill_max_width,
        }
    }

    /// Mutable access to the single wrapped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    fn set_constraints(
        this: &mut WidgetMut<'_, Self>,
        min_width: f64,
        max_width: f64,
        min_height: f64,
        max_height: f64,
        fill_max_width: bool,
    ) {
        let min_width = min_width.max(0.0);
        let max_width = max_width.max(min_width);
        let min_height = min_height.max(0.0);
        let max_height = max_height.max(min_height);
        let changed = (this.widget.min_width - min_width).abs() > 0.01
            || (this.widget.max_width - max_width).abs() > 0.01
            || (this.widget.min_height - min_height).abs() > 0.01
            || (this.widget.max_height - max_height).abs() > 0.01
            || this.widget.fill_max_width != fill_max_width;
        if changed {
            this.widget.min_width = min_width;
            this.widget.max_width = max_width;
            this.widget.min_height = min_height;
            this.widget.max_height = max_height;
            this.widget.fill_max_width = fill_max_width;
            this.ctx.request_layout();
        }
    }

    fn clamp_width(&self, w: f64) -> f64 {
        w.clamp(self.min_width, self.max_width)
    }

    fn clamp_height(&self, h: f64) -> f64 {
        h.clamp(self.min_height, self.max_height)
    }
}

impl Widget for ConstrainedBox {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
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
        match axis {
            Axis::Horizontal => {
                let eff_cross = cross_length
                    .or_else(|| cs.length(Axis::Vertical))
                    .map(|h| Length::px(self.clamp_height(h.get())));
                let ctx_w = cs.length(Axis::Horizontal).map(Length::get);
                match len_req {
                    LenReq::MinContent => {
                        if self.fill_max_width || self.max_width.is_finite() {
                            return Length::px(self.min_width.min(self.max_width));
                        }
                        let measured = ctx
                            .compute_length(
                                &mut self.child,
                                LenDef::MinContent,
                                LayoutSize::maybe(Axis::Vertical, eff_cross),
                                Axis::Horizontal,
                                eff_cross,
                            )
                            .get();
                        Length::px(self.clamp_width(measured))
                    }
                    LenReq::FitContent(space) => {
                        let eff_max = self.max_width.min(space.get().max(0.0));
                        let eff_min = self.min_width.min(eff_max);
                        if self.fill_max_width {
                            return Length::px(eff_max);
                        }
                        let child_def = LenDef::FitContent(Length::px(eff_max));
                        let child_ctx = LayoutSize::maybe(Axis::Vertical, eff_cross);
                        let measured = ctx
                            .compute_length(
                                &mut self.child,
                                child_def,
                                child_ctx,
                                Axis::Horizontal,
                                eff_cross,
                            )
                            .get();
                        Length::px(measured.clamp(eff_min, eff_max))
                    }
                    LenReq::MaxContent => {
                        if self.fill_max_width && self.max_width.is_finite() {
                            let target =
                                ctx_w.map_or(self.max_width, |av| self.max_width.min(av.max(0.0)));
                            return Length::px(target.max(self.min_width.min(target)));
                        }
                        let eff_cap =
                            ctx_w.map_or(self.max_width, |av| self.max_width.min(av.max(0.0)));
                        let child_def = if eff_cap.is_finite() {
                            LenDef::FitContent(Length::px(eff_cap))
                        } else {
                            LenDef::MaxContent
                        };
                        let measured = ctx
                            .compute_length(
                                &mut self.child,
                                child_def,
                                LayoutSize::maybe(Axis::Vertical, eff_cross),
                                Axis::Horizontal,
                                eff_cross,
                            )
                            .get();
                        Length::px(measured.clamp(self.min_width.min(eff_cap), eff_cap))
                    }
                }
            }
            Axis::Vertical => {
                let eff_w = cross_length
                    .or_else(|| cs.length(Axis::Horizontal))
                    .map(|w| Length::px(self.clamp_width(w.get())))
                    .or_else(|| {
                        (self.fill_max_width && self.max_width.is_finite())
                            .then(|| Length::px(self.max_width))
                    });
                let child_ctx = LayoutSize::maybe(Axis::Horizontal, eff_w);
                match len_req {
                    LenReq::FitContent(space) => {
                        let eff_max = self.max_height.min(space.get().max(0.0));
                        let eff_min = self.min_height.min(eff_max);
                        let child_def = if self.max_height.is_finite() {
                            LenDef::FitContent(Length::px(eff_max))
                        } else {
                            LenDef::FitContent(space)
                        };
                        let measured = ctx
                            .compute_length(
                                &mut self.child,
                                child_def,
                                child_ctx,
                                Axis::Vertical,
                                eff_w,
                            )
                            .get();
                        Length::px(measured.clamp(eff_min, eff_max))
                    }
                    LenReq::MinContent | LenReq::MaxContent => {
                        let measured = ctx
                            .compute_length(
                                &mut self.child,
                                len_req.into(),
                                child_ctx,
                                Axis::Vertical,
                                eff_w,
                            )
                            .get();
                        Length::px(self.clamp_height(measured))
                    }
                }
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let clamped = Size::new(
            size.width
                .clamp(self.min_width.min(size.width), self.max_width),
            size.height
                .clamp(self.min_height.min(size.height), self.max_height),
        );
        ctx.run_layout(&mut self.child, clamped);
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
        trace_span!("ConstrainedBox", id = id.trace())
    }
}

/// View wrapper applying optional min/max width and height constraints onto a child view.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains generic child View")]
pub struct ConstrainedBoxView<V, State, Action> {
    child: V,
    min_width: f64,
    max_width: f64,
    min_height: f64,
    max_height: f64,
    fill_max_width: bool,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Wraps `child` in a [`ConstrainedBox`] enforcing minimum and maximum extents.
pub fn constrained_box<State, Action, V>(child: V) -> ConstrainedBoxView<V, State, Action> {
    ConstrainedBoxView {
        child,
        min_width: 0.0,
        max_width: f64::INFINITY,
        min_height: 0.0,
        max_height: f64::INFINITY,
        fill_max_width: false,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ConstrainedBoxView<V, State, Action> {
    /// Sets the minimum logical width in pixels.
    pub fn min_width(mut self, width: f64) -> Self {
        self.min_width = width.max(0.0);
        self
    }

    /// Sets the maximum logical width in pixels.
    pub fn max_width(mut self, width: f64) -> Self {
        self.max_width = width.max(0.0);
        self
    }

    /// Sets the minimum logical height in pixels.
    pub fn min_height(mut self, height: f64) -> Self {
        self.min_height = height.max(0.0);
        self
    }

    /// Sets the maximum logical height in pixels.
    pub fn max_height(mut self, height: f64) -> Self {
        self.max_height = height.max(0.0);
        self
    }

    /// Expands horizontally up to `min(max_width, available)` rather than shrinking to content.
    pub fn fill_max_width(mut self, fill: bool) -> Self {
        self.fill_max_width = fill;
        self
    }
}

impl<V, State, Action> ViewMarker for ConstrainedBoxView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for ConstrainedBoxView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = Pod<ConstrainedBox>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, state) = self.child.build(ctx, app_state);
        let pod = ctx.create_pod(ConstrainedBox::new(
            child_pod.new_widget,
            self.min_width,
            self.max_width,
            self.min_height,
            self.max_height,
            self.fill_max_width,
        ));
        (pod, state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        ConstrainedBox::set_constraints(
            &mut element,
            self.min_width,
            self.max_width,
            self.min_height,
            self.max_height,
            self.fill_max_width,
        );
        let mut child = ConstrainedBox::child_mut(&mut element);
        self.child
            .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child = ConstrainedBox::child_mut(&mut element);
        self.child.teardown(view_state, ctx, child.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut child = ConstrainedBox::child_mut(&mut element);
        self.child
            .message(view_state, message, child.downcast(), app_state)
    }
}

#[cfg(test)]
mod tests {
    use masonry::core::NewWidget;
    use masonry::widgets::Label;

    use super::*;

    #[test]
    fn constrained_box_clamps_invariants_and_fills_max_width() {
        let b = ConstrainedBox::new(
            NewWidget::new(Label::new("test")),
            80.0,
            240.0,
            40.0,
            120.0,
            true,
        );
        assert_eq!(b.clamp_width(30.0), 80.0);
        assert_eq!(b.clamp_width(300.0), 240.0);
        assert_eq!(b.clamp_height(20.0), 40.0);
        assert_eq!(b.clamp_height(200.0), 120.0);

        let inverted = ConstrainedBox::new(
            NewWidget::new(Label::new("inv")),
            150.0,
            90.0,
            60.0,
            20.0,
            false,
        );
        assert_eq!(inverted.max_width, 150.0);
        assert_eq!(inverted.max_height, 60.0);
    }
}
