//! Responsive two-slot row-to-column container keeping leading text and trailing controls overflow-free on narrow windows.

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size};
use masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, SizeDef};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

const PAIR_LEADING_ID: ViewId = ViewId::new(0x5549_3470);
const PAIR_TRAILING_ID: ViewId = ViewId::new(0x5549_3471);

fn should_stay_row(avail_w: f64, trail_w: f64, gap: f64, min_leading: f64) -> bool {
    !avail_w.is_finite() || trail_w <= 0.5 || (avail_w - trail_w - gap) >= min_leading
}

/// Masonry container placing `leading` and `trailing` side-by-side when space allows,
/// and stacking `trailing` cleanly onto a second row once `leading` would be crushed below `min_leading`.
#[expect(
    missing_debug_implementations,
    reason = "Holds erased Masonry WidgetPods"
)]
pub struct AdaptivePairWidget {
    leading: WidgetPod<dyn Widget>,
    trailing: WidgetPod<dyn Widget>,
    gap: f64,
    run_gap: f64,
    min_leading: f64,
    fill_available: bool,
}

impl AdaptivePairWidget {
    fn new(
        leading: NewWidget<impl Widget + ?Sized>,
        trailing: NewWidget<impl Widget + ?Sized>,
        gap: f64,
        run_gap: f64,
        min_leading: f64,
        fill_available: bool,
    ) -> Self {
        Self {
            leading: leading.erased().to_pod(),
            trailing: trailing.erased().to_pod(),
            gap: gap.max(0.0),
            run_gap: run_gap.max(0.0),
            min_leading: min_leading.max(0.0),
            fill_available,
        }
    }

    fn set_params(
        this: &mut WidgetMut<'_, Self>,
        gap: f64,
        run_gap: f64,
        min_leading: f64,
        fill_available: bool,
    ) {
        let gap = gap.max(0.0);
        let run_gap = run_gap.max(0.0);
        let min_leading = min_leading.max(0.0);
        if (this.widget.gap - gap).abs() > f64::EPSILON
            || (this.widget.run_gap - run_gap).abs() > f64::EPSILON
            || (this.widget.min_leading - min_leading).abs() > f64::EPSILON
            || this.widget.fill_available != fill_available
        {
            this.widget.gap = gap;
            this.widget.run_gap = run_gap;
            this.widget.min_leading = min_leading;
            this.widget.fill_available = fill_available;
            this.ctx.request_layout();
        }
    }

    /// Returns mutable access to the leading widget.
    pub fn leading_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.leading)
    }

    /// Returns mutable access to the trailing widget.
    pub fn trailing_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.trailing)
    }
}

impl Widget for AdaptivePairWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.leading);
        ctx.register_child(&mut self.trailing);
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
                LenReq::MinContent => {
                    let l_min = ctx
                        .compute_length(
                            &mut self.leading,
                            LenDef::MinContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    let t_min = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::MinContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    Length::px(l_min.max(t_min))
                }
                LenReq::MaxContent => {
                    let l_max = ctx
                        .compute_length(
                            &mut self.leading,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    let t_max = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    let g = if l_max > 0.5 && t_max > 0.5 {
                        self.gap
                    } else {
                        0.0
                    };
                    Length::px(l_max + t_max + g)
                }
                LenReq::FitContent(space) => {
                    let avail = space.get().max(0.0);
                    if self.fill_available {
                        return Length::px(avail);
                    }
                    let l_max = ctx
                        .compute_length(
                            &mut self.leading,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    let t_max = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    if should_stay_row(avail, t_max, self.gap, self.min_leading) {
                        let g = if l_max > 0.5 && t_max > 0.5 {
                            self.gap
                        } else {
                            0.0
                        };
                        Length::px((l_max + t_max + g).min(avail))
                    } else {
                        let t_fit = ctx
                            .compute_length(
                                &mut self.trailing,
                                LenDef::FitContent(Length::px(avail)),
                                LayoutSize::one(Axis::Horizontal, Length::px(avail)),
                                Axis::Horizontal,
                                None,
                            )
                            .get()
                            .min(avail);
                        Length::px(l_max.min(avail).max(t_fit))
                    }
                }
            },
            Axis::Vertical => {
                let avail_w = cross_length
                    .or_else(|| ctx.context_size().length(Axis::Horizontal))
                    .map_or(f64::INFINITY, |l| l.get().max(0.0));
                let t_max = ctx
                    .compute_length(
                        &mut self.trailing,
                        LenDef::MaxContent,
                        LayoutSize::NONE,
                        Axis::Horizontal,
                        None,
                    )
                    .get();

                if should_stay_row(avail_w, t_max, self.gap, self.min_leading) {
                    let t_cross = Length::px(t_max);
                    let lead_cross = avail_w
                        .is_finite()
                        .then(|| Length::px((avail_w - t_max - self.gap).max(0.0)));
                    let l_h = ctx
                        .compute_length(
                            &mut self.leading,
                            LenDef::MaxContent,
                            LayoutSize::maybe(Axis::Horizontal, lead_cross),
                            Axis::Vertical,
                            lead_cross,
                        )
                        .get();
                    let t_h = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::MaxContent,
                            LayoutSize::one(Axis::Horizontal, t_cross),
                            Axis::Vertical,
                            Some(t_cross),
                        )
                        .get();
                    Length::px(l_h.max(t_h))
                } else {
                    let avail_len = Length::px(avail_w);
                    let l_h = ctx
                        .compute_length(
                            &mut self.leading,
                            LenDef::MaxContent,
                            LayoutSize::one(Axis::Horizontal, avail_len),
                            Axis::Vertical,
                            Some(avail_len),
                        )
                        .get();
                    let t_w = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::FitContent(avail_len),
                            LayoutSize::one(Axis::Horizontal, avail_len),
                            Axis::Horizontal,
                            None,
                        )
                        .get()
                        .min(avail_w);
                    let t_w_len = Length::px(t_w);
                    let t_h = ctx
                        .compute_length(
                            &mut self.trailing,
                            LenDef::MaxContent,
                            LayoutSize::one(Axis::Horizontal, t_w_len),
                            Axis::Vertical,
                            Some(t_w_len),
                        )
                        .get();
                    let gap_y = if l_h > 0.5 && t_h > 0.5 {
                        self.run_gap
                    } else {
                        0.0
                    };
                    Length::px(l_h + gap_y + t_h)
                }
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let avail_w = size.width.max(0.0);
        let avail_h = size.height.max(0.0);
        let t_natural = ctx.compute_size(
            &mut self.trailing,
            SizeDef::new(LenDef::MaxContent, LenDef::MaxContent),
            LayoutSize::NONE,
        );

        if should_stay_row(avail_w, t_natural.width, self.gap, self.min_leading) {
            let t_w = t_natural.width.min(avail_w);
            let t_h = t_natural.height.min(avail_h.max(t_natural.height));
            let t_size = Size::new(t_w, t_h);

            let lead_gap = if t_w > 0.5 { self.gap } else { 0.0 };
            let lead_w = (avail_w - t_w - lead_gap).max(0.0);
            let lead_ctx_sz = Size::new(lead_w, avail_h);
            let l_measured = ctx.compute_size(
                &mut self.leading,
                SizeDef::new(LenDef::Fixed(Length::px(lead_w)), LenDef::MaxContent),
                lead_ctx_sz.into(),
            );
            let lead_size = Size::new(lead_w, l_measured.height);
            let row_h = avail_h.max(lead_size.height.max(t_size.height));

            ctx.run_layout(&mut self.leading, lead_size);
            ctx.place_child(
                &mut self.leading,
                Point::new(0.0, ((row_h - lead_size.height) * 0.5).max(0.0)),
            );
            ctx.run_layout(&mut self.trailing, t_size);
            ctx.place_child(
                &mut self.trailing,
                Point::new(
                    (avail_w - t_size.width).max(0.0),
                    ((row_h - t_size.height) * 0.5).max(0.0),
                ),
            );
            ctx.derive_baselines(&self.leading);
        } else {
            let lead_ctx_sz = Size::new(avail_w, avail_h);
            let l_measured = ctx.compute_size(
                &mut self.leading,
                SizeDef::new(LenDef::Fixed(Length::px(avail_w)), LenDef::MaxContent),
                lead_ctx_sz.into(),
            );
            let lead_size = Size::new(avail_w, l_measured.height);

            let t_fit = ctx.compute_size(
                &mut self.trailing,
                SizeDef::new(LenDef::FitContent(Length::px(avail_w)), LenDef::MaxContent),
                Size::new(avail_w, avail_h).into(),
            );
            let t_size = Size::new(t_fit.width.min(avail_w), t_fit.height);

            let gap_y = if lead_size.height > 0.5 && t_size.height > 0.5 {
                self.run_gap
            } else {
                0.0
            };

            ctx.run_layout(&mut self.leading, lead_size);
            ctx.place_child(&mut self.leading, Point::ORIGIN);
            ctx.run_layout(&mut self.trailing, t_size);
            ctx.place_child(
                &mut self.trailing,
                Point::new(0.0, lead_size.height + gap_y),
            );
            ctx.derive_baselines(&self.leading);
        }
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
        ChildrenIds::from_slice(&[self.leading.id(), self.trailing.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("AdaptivePairWidget", id = id.trace())
    }
}

/// View wrapper constructed by [`adaptive_pair`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct AdaptivePairView<L, T, State, Action> {
    leading: L,
    trailing: T,
    gap: Length,
    run_gap: Length,
    min_leading: f64,
    fill_available: bool,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Pairs `leading` and `trailing` horizontally when width permits, stacking `trailing` cleanly below `leading` when squeezed.
pub fn adaptive_pair<State: 'static, Action: 'static, L, T>(
    leading: L,
    trailing: T,
) -> AdaptivePairView<L, T, State, Action>
where
    L: WidgetView<State, Action>,
    T: WidgetView<State, Action>,
{
    AdaptivePairView {
        leading,
        trailing,
        gap: 12.px(),
        run_gap: 8.px(),
        min_leading: 56.0,
        fill_available: true,
        phantom: PhantomData,
    }
}

impl<L, T, State, Action> AdaptivePairView<L, T, State, Action> {
    /// Sets horizontal spacing when laid out side-by-side.
    pub fn gap(mut self, gap: Length) -> Self {
        self.gap = gap;
        self
    }

    /// Sets vertical spacing when `trailing` wraps onto a second row.
    pub fn run_gap(mut self, run_gap: Length) -> Self {
        self.run_gap = run_gap;
        self
    }

    /// Sets the minimum width reserved for `leading` before wrapping `trailing` below (`56px` by default).
    pub fn min_leading(mut self, min_leading: f64) -> Self {
        self.min_leading = min_leading;
        self
    }

    /// Controls whether `FitContent` stretches to full available width (`true`, default for rows/cards)
    /// or hugs natural width (`false`, ideal for compact toasts).
    pub fn fill_available(mut self, fill: bool) -> Self {
        self.fill_available = fill;
        self
    }
}

impl<L, T, State, Action> ViewMarker for AdaptivePairView<L, T, State, Action> {}

impl<L, T, State, Action> View<State, Action, ViewCtx> for AdaptivePairView<L, T, State, Action>
where
    L: WidgetView<State, Action>,
    T: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<AdaptivePairWidget>;
    type ViewState = (L::ViewState, T::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (lead_pod, lead_state) =
            ctx.with_id(PAIR_LEADING_ID, |ctx| self.leading.build(ctx, app_state));
        let (trail_pod, trail_state) =
            ctx.with_id(PAIR_TRAILING_ID, |ctx| self.trailing.build(ctx, app_state));
        let pod = ctx.create_pod(AdaptivePairWidget::new(
            lead_pod.new_widget,
            trail_pod.new_widget,
            self.gap.get(),
            self.run_gap.get(),
            self.min_leading,
            self.fill_available,
        ));
        (pod, (lead_state, trail_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (lead_state, trail_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        AdaptivePairWidget::set_params(
            &mut element,
            self.gap.get(),
            self.run_gap.get(),
            self.min_leading,
            self.fill_available,
        );
        {
            let mut lead = AdaptivePairWidget::leading_mut(&mut element);
            ctx.with_id(PAIR_LEADING_ID, |ctx| {
                self.leading
                    .rebuild(&prev.leading, lead_state, ctx, lead.downcast(), app_state);
            });
        }
        let mut trail = AdaptivePairWidget::trailing_mut(&mut element);
        ctx.with_id(PAIR_TRAILING_ID, |ctx| {
            self.trailing.rebuild(
                &prev.trailing,
                trail_state,
                ctx,
                trail.downcast(),
                app_state,
            );
        });
    }

    fn teardown(
        &self,
        (lead_state, trail_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut lead = AdaptivePairWidget::leading_mut(&mut element);
            ctx.with_id(PAIR_LEADING_ID, |ctx| {
                self.leading.teardown(lead_state, ctx, lead.downcast());
            });
        }
        let mut trail = AdaptivePairWidget::trailing_mut(&mut element);
        ctx.with_id(PAIR_TRAILING_ID, |ctx| {
            self.trailing.teardown(trail_state, ctx, trail.downcast());
        });
    }

    fn message(
        &self,
        (lead_state, trail_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(PAIR_LEADING_ID) => {
                let mut lead = AdaptivePairWidget::leading_mut(&mut element);
                self.leading
                    .message(lead_state, message, lead.downcast(), app_state)
            }
            Some(PAIR_TRAILING_ID) => {
                let mut trail = AdaptivePairWidget::trailing_mut(&mut element);
                self.trailing
                    .message(trail_state, message, trail.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_stay_row;

    #[test]
    fn row_collapses_to_column_only_when_leading_would_be_crushed() {
        assert!(should_stay_row(f64::INFINITY, 100.0, 12.0, 56.0));
        assert!(should_stay_row(120.0, 0.0, 12.0, 56.0));
        // 160 - 44 (switch) - 12 = 104 >= 56 -> stays row
        assert!(should_stay_row(160.0, 44.0, 12.0, 56.0));
        // 112 (160px card interior) - 88 (button) - 12 = 12 < 56 -> stacks cleanly
        assert!(!should_stay_row(112.0, 88.0, 12.0, 56.0));
    }
}
