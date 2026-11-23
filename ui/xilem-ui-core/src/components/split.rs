//! Platform-themed draggable split-pane container (`NSSplitView` / `GtkPaned` / `SplitView`).

use std::marker::PhantomData;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::Axis;
use xilem::masonry::layout::{AsUnit, Length};
use xilem::masonry::widgets::{self, SplitPoint};
use xilem::{Pod, ViewCtx, WidgetView};

use crate::style_engine::current_spec_arc;
use crate::tokens::{ControlVisualGrammar, DesignSpec};

const SPLIT_CHILD1_ID: ViewId = ViewId::new(0x5549_3450);
const SPLIT_CHILD2_ID: ViewId = ViewId::new(0x5549_3451);

/// Cross-theme draggable split view styling `bar_thickness`, `min_bar_area`, and `solid_bar` from [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiSplit<A, B, State, Action> {
    first: A,
    second: B,
    axis: Axis,
    split_point: SplitPoint,
    min_lengths: (Length, Length),
    draggable: bool,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Creates a draggable horizontal split view dividing `first` and `second` at 50%.
pub fn split_view<A, B, State, Action>(first: A, second: B) -> UiSplit<A, B, State, Action>
where
    State: 'static,
    A: WidgetView<State, Action>,
    B: WidgetView<State, Action>,
{
    UiSplit {
        first,
        second,
        axis: Axis::Horizontal,
        split_point: SplitPoint::Fraction(0.5),
        min_lengths: (60.px(), 60.px()),
        draggable: true,
        phantom: PhantomData,
    }
}

impl<A, B, State, Action> UiSplit<A, B, State, Action> {
    /// Sets the split axis (`Horizontal` = left/right, `Vertical` = top/bottom).
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    /// Sets the split point as a fraction in `(0.0..=1.0)`.
    pub fn split_point(mut self, fraction: f64) -> Self {
        self.split_point = SplitPoint::Fraction(fraction.clamp(0.0, 1.0));
        self
    }

    /// Sets the first pane length explicitly from the leading edge.
    pub fn split_point_from_start(mut self, len: Length) -> Self {
        self.split_point = SplitPoint::FromStart(len);
        self
    }

    /// Sets the second pane length explicitly from the trailing edge.
    pub fn split_point_from_end(mut self, len: Length) -> Self {
        self.split_point = SplitPoint::FromEnd(len);
        self
    }

    /// Sets minimum lengths `(first_min, second_min)` enforced while dragging or resizing.
    pub fn min_lengths(mut self, first: Length, second: Length) -> Self {
        self.min_lengths = (first, second);
        self
    }

    /// Controls whether the user can drag the splitter bar (`true` by default).
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }
}

fn bar_metrics_for_spec(spec: &DesignSpec) -> (Length, Length, bool) {
    match spec.control_grammar() {
        ControlVisualGrammar::MacosHig | ControlVisualGrammar::Adwaita => (2.px(), 8.px(), true),
        ControlVisualGrammar::Material3 | ControlVisualGrammar::Fluent2 => (4.px(), 10.px(), false),
    }
}

impl<A, B, State, Action> ViewMarker for UiSplit<A, B, State, Action> {}

impl<A, B, State, Action> View<State, Action, ViewCtx> for UiSplit<A, B, State, Action>
where
    State: 'static,
    Action: 'static,
    A: WidgetView<State, Action>,
    B: WidgetView<State, Action>,
{
    type Element = Pod<widgets::Split<A::Widget, B::Widget>>;
    type ViewState = (A::ViewState, B::ViewState, (Length, Length, bool));

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let (thickness, hit_area, solid) = bar_metrics_for_spec(&spec);
        let (child1, st1) = ctx.with_id(SPLIT_CHILD1_ID, |ctx| self.first.build(ctx, app_state));
        let (child2, st2) = ctx.with_id(SPLIT_CHILD2_ID, |ctx| self.second.build(ctx, app_state));
        let widget_pod = ctx.create_pod(
            widgets::Split::new(child1.new_widget, child2.new_widget)
                .split_axis(self.axis)
                .split_point(self.split_point)
                .min_lengths(self.min_lengths.0, self.min_lengths.1)
                .bar_thickness(thickness)
                .min_bar_area(hit_area)
                .draggable(self.draggable)
                .solid_bar(solid),
        );
        (widget_pod, (st1, st2, (thickness, hit_area, solid)))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (st1, st2, prev_bar): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_bar @ (thickness, hit_area, solid) = bar_metrics_for_spec(&spec);

        if prev.axis != self.axis {
            widgets::Split::set_split_axis(&mut element, self.axis);
        }
        if prev.split_point != self.split_point {
            widgets::Split::set_split_point(&mut element, self.split_point);
        }
        if prev.min_lengths != self.min_lengths {
            widgets::Split::set_min_lengths(&mut element, self.min_lengths.0, self.min_lengths.1);
        }
        if prev_bar.0 != thickness {
            widgets::Split::set_bar_thickness(&mut element, thickness);
        }
        if prev_bar.1 != hit_area {
            widgets::Split::set_min_bar_area(&mut element, hit_area);
        }
        if prev.draggable != self.draggable {
            widgets::Split::set_draggable(&mut element, self.draggable);
        }
        if prev_bar.2 != solid {
            widgets::Split::set_bar_solid(&mut element, solid);
        }
        *prev_bar = next_bar;

        ctx.with_id(SPLIT_CHILD1_ID, |ctx| {
            let child1_el = widgets::Split::child1_mut(&mut element);
            self.first
                .rebuild(&prev.first, st1, ctx, child1_el, app_state);
        });
        ctx.with_id(SPLIT_CHILD2_ID, |ctx| {
            let child2_el = widgets::Split::child2_mut(&mut element);
            self.second
                .rebuild(&prev.second, st2, ctx, child2_el, app_state);
        });
    }

    fn teardown(
        &self,
        (st1, st2, _): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let child1_el = widgets::Split::child1_mut(&mut element);
        self.first.teardown(st1, ctx, child1_el);

        let child2_el = widgets::Split::child2_mut(&mut element);
        self.second.teardown(st2, ctx, child2_el);
    }

    fn message(
        &self,
        (st1, st2, _): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(SPLIT_CHILD1_ID) => {
                let child1_el = widgets::Split::child1_mut(&mut element);
                self.first.message(st1, message, child1_el, app_state)
            }
            Some(SPLIT_CHILD2_ID) => {
                let child2_el = widgets::Split::child2_mut(&mut element);
                self.second.message(st2, message, child2_el, app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
