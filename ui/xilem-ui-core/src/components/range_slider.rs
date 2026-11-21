//! Dual-thumb `RangeSlider` custom Masonry widget and Xilem view.

use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

use xilem::Color;
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::kurbo::{Axis, Circle, Point, Rect, Size, Stroke};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, PaintCtx, PointerButton,
    PointerButtonEvent, PointerEvent, PointerUpdate, PropertiesMut, PropertiesRef, Property,
    RegisterCtx, Update, UpdateCtx, UsesProperty, Widget, WidgetMut,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{LenReq, Length};
use xilem::masonry::properties::{ThumbColor, ThumbRadius, TrackColor, TrackThickness};
use xilem::{Pod, ViewCtx};

/// Emitted by [`RangeSliderWidget`] whenever either thumb moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeChanged {
    /// Updated lower bound.
    pub start: f64,
    /// Updated upper bound.
    pub end: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveThumb {
    Start,
    End,
}

/// Custom dual-thumb range slider widget honoring Masonry's `TrackColor`, `TrackThickness`,
/// `ThumbColor`, and `ThumbRadius` properties.
#[derive(Debug)]
pub struct RangeSliderWidget {
    min: f64,
    max: f64,
    start: f64,
    end: f64,
    step: Option<f64>,
    active_thumb: Option<ActiveThumb>,
}

impl UsesProperty<TrackThickness> for RangeSliderWidget {}
impl UsesProperty<TrackColor> for RangeSliderWidget {}
impl UsesProperty<ThumbColor> for RangeSliderWidget {}
impl UsesProperty<ThumbRadius> for RangeSliderWidget {}

impl RangeSliderWidget {
    /// Creates a new [`RangeSliderWidget`] clamping `start..=end` inside `min..=max`.
    #[must_use]
    pub fn new(min: f64, max: f64, start: f64, end: f64, step: Option<f64>) -> Self {
        let lo_bound = min.min(max);
        let hi_bound = max.max(lo_bound);
        let (s, e) = normalize_pair(lo_bound, hi_bound, start, end, step);
        Self {
            min: lo_bound,
            max: hi_bound,
            start: s,
            end: e,
            step: step.filter(|v| *v > 0.0),
            active_thumb: None,
        }
    }

    /// Updates bounds/values cleanly from the view layer.
    pub fn set_values(
        this: &mut WidgetMut<'_, Self>,
        min: f64,
        max: f64,
        start: f64,
        end: f64,
        step: Option<f64>,
    ) {
        let lo_bound = min.min(max);
        let hi_bound = max.max(lo_bound);
        let step = step.filter(|v| *v > 0.0);
        let (s, e) = normalize_pair(lo_bound, hi_bound, start, end, step);
        if this.widget.min != lo_bound
            || this.widget.max != hi_bound
            || (this.widget.start - s).abs() > 1e-9
            || (this.widget.end - e).abs() > 1e-9
            || this.widget.step != step
        {
            this.widget.min = lo_bound;
            this.widget.max = hi_bound;
            this.widget.start = s;
            this.widget.end = e;
            this.widget.step = step;
            this.ctx.request_render();
        }
    }

    fn value_from_x(&self, x: f64, width: f64, thumb_radius: f64) -> f64 {
        let usable = (width - thumb_radius * 2.0).max(1.0);
        let frac = ((x - thumb_radius) / usable).clamp(0.0, 1.0);
        let raw = self.min + frac * (self.max - self.min);
        snap_step(self.min, self.max, raw, self.step)
    }

    fn update_active_thumb(&mut self, value: f64) -> bool {
        let Some(thumb) = self.active_thumb else {
            return false;
        };
        let (new_s, new_e) = match thumb {
            ActiveThumb::Start => (value.min(self.end), self.end),
            ActiveThumb::End => (self.start, value.max(self.start)),
        };
        if (new_s - self.start).abs() > 1e-9 || (new_e - self.end).abs() > 1e-9 {
            self.start = new_s;
            self.end = new_e;
            true
        } else {
            false
        }
    }
}

fn snap_step(min: f64, max: f64, value: f64, step: Option<f64>) -> f64 {
    let clamped = value.clamp(min, max);
    match step {
        Some(s) if s > 0.0 => (min + ((clamped - min) / s).round() * s).clamp(min, max),
        _ => clamped,
    }
}

fn normalize_pair(min: f64, max: f64, start: f64, end: f64, step: Option<f64>) -> (f64, f64) {
    let s = snap_step(min, max, start.min(end), step);
    let e = snap_step(min, max, start.max(end), step);
    (s, e)
}

fn choose_thumb(value: f64, start: f64, end: f64) -> ActiveThumb {
    let d_start = (value - start).abs();
    let d_end = (value - end).abs();
    if d_start < d_end {
        ActiveThumb::Start
    } else if d_end < d_start || value >= end {
        ActiveThumb::End
    } else {
        ActiveThumb::Start
    }
}

impl Widget for RangeSliderWidget {
    type Action = RangeChanged;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        TrackThickness::prop_changed(ctx, property_type);
        ThumbColor::prop_changed(ctx, property_type);
        ThumbRadius::prop_changed(ctx, property_type);
        if TrackColor::matches(property_type) {
            ctx.request_paint_only();
        }
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        let thumb_r = props
            .get::<ThumbRadius>(ctx.property_cache())
            .0
            .get()
            .max(8.0);
        let width = ctx.content_box().size().width;
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                state,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                let pos = ctx.local_position(state.position);
                let val = self.value_from_x(pos.x, width, thumb_r);
                self.active_thumb = Some(choose_thumb(val, self.start, self.end));
                if self.update_active_thumb(val) {
                    ctx.submit_action::<Self::Action>(RangeChanged {
                        start: self.start,
                        end: self.end,
                    });
                }
                ctx.request_paint_only();
            }
            PointerEvent::Move(PointerUpdate { current, .. }) if ctx.is_active() => {
                let pos = ctx.local_position(current.position);
                let val = self.value_from_x(pos.x, width, thumb_r);
                if self.update_active_thumb(val) {
                    ctx.submit_action::<Self::Action>(RangeChanged {
                        start: self.start,
                        end: self.end,
                    });
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Up(_) | PointerEvent::Cancel(_) => {
                self.active_thumb = None;
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match axis {
            Axis::Horizontal => match len_req {
                LenReq::MinContent | LenReq::MaxContent => Length::const_px(160.0),
                LenReq::FitContent(space) => space,
            },
            Axis::Vertical => {
                let cache = ctx.property_cache();
                let thumb_r = props.get::<ThumbRadius>(cache).0.get().max(8.0);
                let track_h = props.get::<TrackThickness>(cache).0.get().max(4.0);
                Length::px((thumb_r * 2.0 + 8.0).max(track_h + 8.0))
            }
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let cache = ctx.property_cache();
        let track_color = props.get::<TrackColor>(cache);
        let thumb_color = props.get::<ThumbColor>(cache).0;
        let track_h = props.get::<TrackThickness>(cache).0.get().max(4.0);
        let thumb_r = props.get::<ThumbRadius>(cache).0.get().max(8.0);

        let size = ctx.content_box().size();
        let span = (self.max - self.min).max(1e-9);
        let p_start = ((self.start - self.min) / span).clamp(0.0, 1.0);
        let p_end = ((self.end - self.min) / span).clamp(0.0, 1.0);
        let usable = (size.width - thumb_r * 2.0).max(1.0);

        let x_start = thumb_r + p_start * usable;
        let x_end = thumb_r + p_end * usable;
        let cy = size.height / 2.0;
        let track_y = cy - track_h / 2.0;

        let full_track = Rect::new(thumb_r, track_y, size.width - thumb_r, track_y + track_h)
            .to_rounded_rect(track_h / 2.0);
        painter.fill(full_track, track_color.inactive).draw();

        if x_end > x_start {
            let active_track = Rect::new(x_start, track_y, x_end, track_y + track_h)
                .to_rounded_rect(track_h / 2.0);
            painter.fill(active_track, track_color.active).draw();
        }

        for (tx, active_kind) in [(x_start, ActiveThumb::Start), (x_end, ActiveThumb::End)] {
            let r = if self.active_thumb == Some(active_kind) {
                thumb_r + 1.5
            } else {
                thumb_r
            };
            let circle = Circle::new(Point::new(tx, cy), r);
            painter.fill(circle, thumb_color).draw();
            painter
                .stroke(circle, &Stroke::new(1.5), Color::WHITE.with_alpha(0.85))
                .draw();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Slider
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

/// Callback type for [`range_slider`].
pub type RangeCallback<State, Action> = Arc<dyn Fn(&mut State, (f64, f64)) -> Action + Send + Sync>;

/// Dual-thumb range slider view.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct RangeSlider<State, Action> {
    min: f64,
    max: f64,
    start: f64,
    end: f64,
    step: Option<f64>,
    on_change: RangeCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> RangeSlider<State, Action> {
    /// Sets discrete quantization step spacing.
    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }
}

impl<State: 'static, Action: 'static> ViewMarker for RangeSlider<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for RangeSlider<State, Action> {
    type Element = Pod<RangeSliderWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec(ctx);
        let p = &spec.palette;
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(RangeSliderWidget::new(
                self.min, self.max, self.start, self.end, self.step,
            ));
            pod.new_widget.properties.insert(TrackColor {
                active: p.accent,
                inactive: p.surface_highest,
            });
            pod.new_widget.properties.insert(ThumbColor(p.accent));
            pod.new_widget
                .properties
                .insert(TrackThickness(Length::px(4.0)));
            pod.new_widget
                .properties
                .insert(ThumbRadius(Length::px(9.0)));
            pod
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
        RangeSliderWidget::set_values(
            &mut element,
            self.min,
            self.max,
            self.start,
            self.end,
            self.step,
        );
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_message::<RangeChanged>() {
            Some(msg) => MessageResult::Action((self.on_change)(app_state, (msg.start, msg.end))),
            None => MessageResult::Stale,
        }
    }
}

/// Creates a dual-thumb range slider within `min..=max` emitting updated `(start, end)` pairs.
pub fn range_slider<State: 'static, Action: 'static, F>(
    min: f64,
    max: f64,
    values: (f64, f64),
    on_change: F,
) -> RangeSlider<State, Action>
where
    F: Fn(&mut State, (f64, f64)) -> Action + Send + Sync + 'static,
{
    RangeSlider {
        min,
        max,
        start: values.0,
        end: values.1,
        step: None,
        on_change: Arc::new(on_change),
        phantom: PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pair_clamps_and_orders() {
        let (s, e) = normalize_pair(0.0, 100.0, 80.0, 20.0, None);
        assert_eq!((s, e), (20.0, 80.0));

        let (s, e) = normalize_pair(0.0, 100.0, -5.0, 140.0, Some(10.0));
        assert_eq!((s, e), (0.0, 100.0));

        let (s, e) = normalize_pair(0.0, 100.0, 14.0, 46.0, Some(5.0));
        assert_eq!((s, e), (15.0, 45.0));
    }

    #[test]
    fn choose_thumb_selects_closest_thumb() {
        assert_eq!(choose_thumb(12.0, 20.0, 80.0), ActiveThumb::Start);
        assert_eq!(choose_thumb(78.0, 20.0, 80.0), ActiveThumb::End);
        assert_eq!(choose_thumb(50.0, 50.0, 50.0), ActiveThumb::End);
        assert_eq!(choose_thumb(30.0, 50.0, 50.0), ActiveThumb::Start);
    }

    #[test]
    fn update_active_thumb_prevents_crossover() {
        let mut w = RangeSliderWidget::new(0.0, 100.0, 25.0, 75.0, None);
        w.active_thumb = Some(ActiveThumb::Start);
        assert!(w.update_active_thumb(90.0));
        assert_eq!(w.start, 75.0);
        assert_eq!(w.end, 75.0);

        w.active_thumb = Some(ActiveThumb::Start);
        assert!(w.update_active_thumb(40.0));
        assert_eq!(w.start, 40.0);

        w.active_thumb = Some(ActiveThumb::End);
        assert!(w.update_active_thumb(10.0));
        assert_eq!(w.end, 40.0);
        assert!(!w.update_active_thumb(10.0));
    }
}
