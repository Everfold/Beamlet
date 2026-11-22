//! Determinate & indeterminate linear and circular progress indicators driven by [`DesignSpec`](crate::tokens::DesignSpec).

use std::any::TypeId;
use std::f64::consts::TAU;
use std::marker::PhantomData;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::kurbo::{Arc as KurboArc, Axis, Point, Rect, Size, Stroke, Vec2};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NoAction, PaintCtx, PropertiesMut,
    PropertiesRef, Property, RegisterCtx, Update, UpdateCtx, UsesProperty, Widget, WidgetMut,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{AsUnit, LenReq, Length};
use xilem::masonry::properties::{Dimensions, TrackColor};
use xilem::style::Style as _;
use xilem::view::{progress_bar, sized_box, spinner};
use xilem::{Pod, ViewCtx, WidgetView};

/// Custom linear progress bar rendering indeterminate sweeping track + determinate stop dot.
#[derive(Debug)]
pub struct LinearProgressWidget {
    progress: Option<f64>,
    phase: f64,
}

impl UsesProperty<TrackColor> for LinearProgressWidget {}

impl LinearProgressWidget {
    /// Creates a new [`LinearProgressWidget`] (`Some(0..=1)` determinate or `None` indeterminate).
    #[must_use]
    pub fn new(progress: Option<f64>) -> Self {
        Self {
            progress: progress.map(|p| p.clamp(0.0, 1.0)),
            phase: 0.0,
        }
    }

    /// Updates progress value and requests animation frames when indeterminate.
    pub fn set_progress(this: &mut WidgetMut<'_, Self>, progress: Option<f64>) {
        let next = progress.map(|p| p.clamp(0.0, 1.0));
        if this.widget.progress != next {
            let became_indet = this.widget.progress.is_some() && next.is_none();
            this.widget.progress = next;
            if became_indet {
                this.ctx.request_anim_frame();
            }
            this.ctx.request_paint_only();
        }
    }
}

impl Widget for LinearProgressWidget {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) && self.progress.is_none() {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.progress.is_none() {
            let dt = interval as f64 / 1_000_000_000.0;
            self.phase = (self.phase + dt * 0.75) % 1.0;
            ctx.request_paint_only();
            ctx.request_anim_frame();
        }
    }

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        if TrackColor::matches(property_type) {
            ctx.request_paint_only();
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match axis {
            Axis::Horizontal => match len_req {
                LenReq::MinContent | LenReq::MaxContent => Length::const_px(160.0),
                LenReq::FitContent(space) => space,
            },
            Axis::Vertical => Length::const_px(4.0),
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let track = props.get::<TrackColor>(ctx.property_cache());
        let size = ctx.content_box().size();
        let r = (size.height / 2.0).max(2.0);
        let full = Rect::new(0.0, 0.0, size.width, size.height).to_rounded_rect(r);
        painter.fill(full, track.inactive).draw();

        if let Some(p) = self.progress {
            let w = (size.width * p).clamp(0.0, size.width);
            if w > 0.5 {
                let active = Rect::new(0.0, 0.0, w, size.height).to_rounded_rect(r);
                painter.fill(active, track.active).draw();
            }
        } else {
            let bar_w = size.width * 0.34;
            let start = -bar_w + (size.width + bar_w * 2.0) * self.phase;
            let left = start.clamp(0.0, size.width);
            let right = (start + bar_w).clamp(0.0, size.width);
            if right > left {
                let sweep = Rect::new(left, 0.0, right, size.height).to_rounded_rect(r);
                painter.fill(sweep, track.active).draw();
            }
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ProgressIndicator
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// View wrapper around [`LinearProgressWidget`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiLinearProgress<State, Action> {
    progress: Option<f64>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State: 'static, Action: 'static> ViewMarker for UiLinearProgress<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for UiLinearProgress<State, Action>
{
    type Element = Pod<LinearProgressWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec_arc(ctx);
        let mut pod = ctx.create_pod(LinearProgressWidget::new(self.progress));
        pod.new_widget.properties.insert(TrackColor {
            active: spec.palette.accent,
            inactive: spec.palette.secondary_container,
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        LinearProgressWidget::set_progress(&mut element, self.progress);
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        _element: Mut<'_, Self::Element>,
    ) {
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        _message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) -> MessageResult<Action> {
        MessageResult::Stale
    }
}

/// Spec-themed linear progress bar (`Some(0.0..=1.0)` determinate, `None` indeterminate).
pub fn linear_progress<State: 'static, Action: 'static>(
    progress: Option<f64>,
) -> impl WidgetView<State, Action, Widget = LinearProgressWidget> {
    if progress.is_some() {
        let _ = progress_bar(progress);
    }
    UiLinearProgress {
        progress,
        phantom: PhantomData,
    }
}

/// Indeterminate circular progress spinner (`40px`).
pub fn circular_progress<State: 'static, Action: 'static>() -> impl WidgetView<State, Action> {
    sized_box(spinner()).dims(Dimensions::fixed(40.px(), 40.px()))
}

/// Compact `20px` circular progress spinner suitable for inline buttons/toolbars.
pub fn small_circular_progress<State: 'static, Action: 'static>() -> impl WidgetView<State, Action>
{
    sized_box(spinner()).dims(Dimensions::fixed(20.px(), 20.px()))
}

/// Determinate circular arc progress widget.
#[derive(Debug)]
pub struct DeterminateCircularWidget {
    progress: f64,
}

impl UsesProperty<TrackColor> for DeterminateCircularWidget {}

impl Widget for DeterminateCircularWidget {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        if TrackColor::matches(property_type) {
            ctx.request_paint_only();
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        _axis: Axis,
        _len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        Length::const_px(40.0)
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let track = props.get::<TrackColor>(ctx.property_cache());
        let size = ctx.content_box().size();
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let stroke_w = 3.5;
        let radius = (size.min_side() / 2.0 - stroke_w / 2.0).max(4.0);
        let stroke = Stroke::new(stroke_w);

        let full_arc = KurboArc::new(center, Vec2::new(radius, radius), 0.0, TAU, 0.0);
        painter.stroke(full_arc, &stroke, track.inactive).draw();

        let sweep = self.progress.clamp(0.0, 1.0) * TAU;
        if sweep > 0.01 {
            let active_arc = KurboArc::new(
                center,
                Vec2::new(radius, radius),
                -std::f64::consts::FRAC_PI_2,
                sweep,
                0.0,
            );
            painter.stroke(active_arc, &stroke, track.active).draw();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ProgressIndicator
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// View wrapper for [`DeterminateCircularWidget`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiDeterminateCircular<State, Action> {
    progress: f64,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State: 'static, Action: 'static> ViewMarker for UiDeterminateCircular<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for UiDeterminateCircular<State, Action>
{
    type Element = Pod<DeterminateCircularWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec_arc(ctx);
        let mut pod = ctx.create_pod(DeterminateCircularWidget {
            progress: self.progress.clamp(0.0, 1.0),
        });
        pod.new_widget.properties.insert(TrackColor {
            active: spec.palette.accent,
            inactive: spec.palette.secondary_container,
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let next = self.progress.clamp(0.0, 1.0);
        if (element.widget.progress - next).abs() > 1e-6 {
            element.widget.progress = next;
            element.ctx.request_paint_only();
        }
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        _element: Mut<'_, Self::Element>,
    ) {
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        _message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) -> MessageResult<Action> {
        MessageResult::Stale
    }
}

/// Determinate circular progress indicator (`0.0..=1.0`).
pub fn determinate_circular_progress<State: 'static, Action: 'static>(
    progress: f64,
) -> UiDeterminateCircular<State, Action> {
    UiDeterminateCircular {
        progress,
        phantom: PhantomData,
    }
}
