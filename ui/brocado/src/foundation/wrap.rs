//! Wrapping flow container laying children left-to-right across multiple lines cleanly.
//!
//! Upstream provenance: `WrapView`'s `ViewSequence` / `ElementSplice` reconciliation pattern follows
//! upstream `xilem_masonry` container views (`linebender/xilem`, `xilem_masonry/src/view/{flex,zstack}.rs`).

use std::marker::PhantomData;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size};
use masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, SizeDef};
use masonry::properties::types::{CrossAxisAlignment, MainAxisAlignment};
use tracing::{Span, trace_span};
use xilem_core::{
    AppendVec, ElementSplice, MessageCtx, MessageResult, Mut, View, ViewMarker, ViewSequence,
};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;

struct PlannedItem {
    index: usize,
    size: Size,
    x: f64,
}

struct PlannedRun {
    items: Vec<PlannedItem>,
    y: f64,
    height: f64,
}

fn plan_runs(
    sizes: &[(usize, Size)],
    max_width: f64,
    gap: f64,
    run_gap: f64,
) -> (Vec<PlannedRun>, Size) {
    let mut runs: Vec<PlannedRun> = Vec::new();
    let mut cur_items: Vec<PlannedItem> = Vec::new();
    let mut x = 0.0f64;
    let mut y = 0.0f64;
    let mut run_h = 0.0f64;
    let mut max_w = 0.0f64;

    for &(index, sz) in sizes {
        let next_x = if cur_items.is_empty() { 0.0 } else { x + gap };
        if !cur_items.is_empty() && max_width.is_finite() && next_x + sz.width > max_width + 0.5 {
            max_w = max_w.max(x);
            runs.push(PlannedRun {
                items: std::mem::take(&mut cur_items),
                y,
                height: run_h,
            });
            y += run_h + run_gap;
            x = 0.0;
            run_h = 0.0;
        }
        let item_x = if cur_items.is_empty() { 0.0 } else { x + gap };
        cur_items.push(PlannedItem {
            index,
            size: sz,
            x: item_x,
        });
        x = item_x + sz.width;
        run_h = run_h.max(sz.height);
    }

    if !cur_items.is_empty() {
        max_w = max_w.max(x);
        runs.push(PlannedRun {
            items: cur_items,
            y,
            height: run_h,
        });
        y += run_h;
    }

    (runs, Size::new(max_w, y))
}

/// Masonry container widget flowing children left-to-right and wrapping onto new rows on overflow.
#[expect(
    missing_debug_implementations,
    reason = "Holds erased child Masonry WidgetPods"
)]
pub struct WrapWidget {
    children: Vec<WidgetPod<dyn Widget>>,
    gap: f64,
    run_gap: f64,
    cross_align: CrossAxisAlignment,
    main_align: MainAxisAlignment,
}

impl WrapWidget {
    fn new(
        gap: f64,
        run_gap: f64,
        cross_align: CrossAxisAlignment,
        main_align: MainAxisAlignment,
    ) -> Self {
        Self {
            children: Vec::new(),
            gap,
            run_gap,
            cross_align,
            main_align,
        }
    }

    fn set_params(
        this: &mut WidgetMut<'_, Self>,
        gap: f64,
        run_gap: f64,
        cross_align: CrossAxisAlignment,
        main_align: MainAxisAlignment,
    ) {
        if (this.widget.gap - gap).abs() > f64::EPSILON
            || (this.widget.run_gap - run_gap).abs() > f64::EPSILON
            || this.widget.cross_align != cross_align
            || this.widget.main_align != main_align
        {
            this.widget.gap = gap;
            this.widget.run_gap = run_gap;
            this.widget.cross_align = cross_align;
            this.widget.main_align = main_align;
            this.ctx.request_layout();
        }
    }

    fn insert_child(this: &mut WidgetMut<'_, Self>, idx: usize, child: NewWidget<dyn Widget>) {
        this.widget.children.insert(idx, child.to_pod());
        this.ctx.children_changed();
    }

    fn remove_child(this: &mut WidgetMut<'_, Self>, idx: usize) {
        let pod = this.widget.children.remove(idx);
        this.ctx.remove_child(pod);
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>, idx: usize) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.children[idx])
    }
}

impl Widget for WrapWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for child in &mut self.children {
            ctx.register_child(child);
        }
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let mut sizes: Vec<(usize, Size)> = Vec::new();
        match (axis, len_req) {
            (Axis::Horizontal, LenReq::MinContent) => {
                let mut max_w = 0.0f64;
                for c in &mut self.children {
                    let w = ctx
                        .compute_length(
                            c,
                            LenReq::MinContent.into(),
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    max_w = max_w.max(w);
                }
                Length::px(max_w)
            }
            (Axis::Horizontal, LenReq::MaxContent) => {
                let mut total_w = 0.0f64;
                let mut count = 0usize;
                for c in &mut self.children {
                    let w = ctx
                        .compute_length(
                            c,
                            LenReq::MaxContent.into(),
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    total_w += w;
                    count += 1;
                }
                if count > 1 {
                    total_w += self.gap * ((count - 1) as f64);
                }
                Length::px(total_w)
            }
            (Axis::Horizontal, LenReq::FitContent(space)) => {
                if self.main_align != MainAxisAlignment::Start {
                    return space;
                }
                let avail_w = space.get().max(0.0);
                for (idx, c) in self.children.iter_mut().enumerate() {
                    let w = ctx
                        .compute_length(
                            c,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get()
                        .min(avail_w);
                    sizes.push((idx, Size::new(w, 0.0)));
                }
                let (_, total) = plan_runs(&sizes, avail_w, self.gap, self.run_gap);
                Length::px(total.width.min(avail_w))
            }
            (Axis::Vertical, _) => {
                let avail_w = cross_length
                    .or_else(|| ctx.context_size().length(Axis::Horizontal))
                    .map_or(f64::INFINITY, |l| l.get().max(0.0));
                for (idx, c) in self.children.iter_mut().enumerate() {
                    let w = ctx
                        .compute_length(
                            c,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get()
                        .min(avail_w);
                    let h = ctx
                        .compute_length(
                            c,
                            LenDef::MaxContent,
                            LayoutSize::one(Axis::Horizontal, Length::px(w)),
                            Axis::Vertical,
                            Some(Length::px(w)),
                        )
                        .get();
                    sizes.push((idx, Size::new(w, h)));
                }
                let (_, total) = plan_runs(&sizes, avail_w, self.gap, self.run_gap);
                Length::px(total.height)
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let avail_w = size.width.max(0.0);
        let mut sizes: Vec<(usize, Size)> = Vec::new();
        for (idx, c) in self.children.iter_mut().enumerate() {
            let child_def = SizeDef::new(LenDef::MaxContent, LenDef::MaxContent);
            let mut c_sz = ctx.compute_size(c, child_def, LayoutSize::NONE);
            if c_sz.width > avail_w {
                let constrained_def =
                    SizeDef::new(LenDef::FitContent(Length::px(avail_w)), LenDef::MaxContent);
                c_sz = ctx.compute_size(
                    c,
                    constrained_def,
                    LayoutSize::one(Axis::Horizontal, Length::px(avail_w)),
                );
                c_sz.width = c_sz.width.min(avail_w);
            }
            sizes.push((idx, c_sz));
        }
        let (runs, _) = plan_runs(&sizes, avail_w, self.gap, self.run_gap);

        let mut first_baseline = None;
        for run in runs {
            let run_w = run
                .items
                .last()
                .map_or(0.0, |item| item.x + item.size.width);
            let run_dx = match self.main_align {
                MainAxisAlignment::End => (avail_w - run_w).max(0.0),
                MainAxisAlignment::Center => ((avail_w - run_w) * 0.5).max(0.0),
                _ => 0.0,
            };
            for item in run.items {
                let target_size = match self.cross_align {
                    CrossAxisAlignment::Stretch => Size::new(item.size.width, run.height),
                    _ => item.size,
                };
                let dy = match self.cross_align {
                    CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => 0.0,
                    CrossAxisAlignment::Center
                    | CrossAxisAlignment::FirstBaseline
                    | CrossAxisAlignment::LastBaseline => (run.height - target_size.height) * 0.5,
                    CrossAxisAlignment::End => run.height - target_size.height,
                };
                let child = &mut self.children[item.index];
                ctx.run_layout(child, target_size);
                ctx.place_child(child, Point::new(run_dx + item.x, run.y + dy));
                if first_baseline.is_none() {
                    ctx.derive_baselines(child);
                    first_baseline = Some(item.index);
                }
            }
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
        self.children.iter().map(WidgetPod::id).collect()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("WrapWidget", id = id.trace())
    }
}

struct WrapSplice<'w, 's> {
    idx: usize,
    element: WidgetMut<'w, WrapWidget>,
    scratch: &'s mut AppendVec<Pod<dyn Widget>>,
}

impl<'w, 's> WrapSplice<'w, 's> {
    fn new(
        element: WidgetMut<'w, WrapWidget>,
        scratch: &'s mut AppendVec<Pod<dyn Widget>>,
    ) -> Self {
        debug_assert!(scratch.is_empty());
        Self {
            idx: 0,
            element,
            scratch,
        }
    }
}

impl ElementSplice<Pod<dyn Widget>> for WrapSplice<'_, '_> {
    fn insert(&mut self, element: Pod<dyn Widget>) {
        WrapWidget::insert_child(&mut self.element, self.idx, element.new_widget);
        self.idx += 1;
    }

    fn with_scratch<R>(&mut self, f: impl FnOnce(&mut AppendVec<Pod<dyn Widget>>) -> R) -> R {
        let ret = f(self.scratch);
        for element in self.scratch.drain() {
            WrapWidget::insert_child(&mut self.element, self.idx, element.new_widget);
            self.idx += 1;
        }
        ret
    }

    fn mutate<R>(&mut self, f: impl FnOnce(Mut<'_, Pod<dyn Widget>>) -> R) -> R {
        let ret = {
            let child = WrapWidget::child_mut(&mut self.element, self.idx);
            f(child)
        };
        self.idx += 1;
        ret
    }

    fn skip(&mut self, n: usize) {
        self.idx += n;
    }

    fn delete<R>(&mut self, f: impl FnOnce(Mut<'_, Pod<dyn Widget>>) -> R) -> R {
        let ret = {
            let child = WrapWidget::child_mut(&mut self.element, self.idx);
            f(child)
        };
        WrapWidget::remove_child(&mut self.element, self.idx);
        ret
    }

    fn index(&self) -> usize {
        self.idx
    }
}

/// Internal view-state for [`WrapView`].
#[expect(
    missing_debug_implementations,
    reason = "Holds opaque sequence state and Pod scratch buffer"
)]
pub struct WrapState<SeqState> {
    seq_state: SeqState,
    scratch: AppendVec<Pod<dyn Widget>>,
}

/// Brocado [`View`] wrapper laying out a sequence of child views left-to-right and wrapping onto new rows cleanly.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct WrapView<Seq, State, Action> {
    sequence: Seq,
    gap: Length,
    run_gap: Length,
    cross_align: CrossAxisAlignment,
    main_align: MainAxisAlignment,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Creates a [`WrapView`] flowing `sequence` left-to-right across multiple rows when constrained horizontally.
pub fn wrap<State: 'static, Action, Seq>(sequence: Seq) -> WrapView<Seq, State, Action>
where
    Seq: ViewSequence<State, Action, ViewCtx, Pod<dyn Widget>>,
{
    WrapView {
        sequence,
        gap: 8.px(),
        run_gap: 8.px(),
        cross_align: CrossAxisAlignment::Center,
        main_align: MainAxisAlignment::Start,
        phantom: PhantomData,
    }
}

impl<Seq, State, Action> WrapView<Seq, State, Action> {
    /// Sets horizontal spacing between adjacent items in the same line.
    pub fn gap(mut self, gap: Length) -> Self {
        self.gap = gap;
        self
    }

    /// Sets vertical spacing between wrapped rows.
    pub fn run_gap(mut self, run_gap: Length) -> Self {
        self.run_gap = run_gap;
        self
    }

    /// Sets vertical alignment of items within each wrapped row.
    pub fn cross_axis_alignment(mut self, align: CrossAxisAlignment) -> Self {
        self.cross_align = align;
        self
    }

    /// Sets horizontal alignment of each wrapped row (`Start`, `Center`, or `End`).
    pub fn main_axis_alignment(mut self, align: MainAxisAlignment) -> Self {
        self.main_align = align;
        self
    }
}

impl<Seq, State, Action> ViewMarker for WrapView<Seq, State, Action> {}

impl<Seq, State, Action> View<State, Action, ViewCtx> for WrapView<Seq, State, Action>
where
    State: 'static,
    Action: 'static,
    Seq: ViewSequence<State, Action, ViewCtx, Pod<dyn Widget>>,
{
    type Element = Pod<WrapWidget>;
    type ViewState = WrapState<Seq::SeqState>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let mut elements = AppendVec::default();
        let seq_state = self.sequence.seq_build(ctx, &mut elements, app_state);
        let mut widget = WrapWidget::new(
            self.gap.get(),
            self.run_gap.get(),
            self.cross_align,
            self.main_align,
        );
        for child in elements.drain() {
            widget.children.push(child.new_widget.to_pod());
        }
        let pod = ctx.create_pod(widget);
        (
            pod,
            WrapState {
                seq_state,
                scratch: elements,
            },
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        WrapState { seq_state, scratch }: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        WrapWidget::set_params(
            &mut element,
            self.gap.get(),
            self.run_gap.get(),
            self.cross_align,
            self.main_align,
        );
        let mut splice = WrapSplice::new(element, scratch);
        self.sequence
            .seq_rebuild(&prev.sequence, seq_state, ctx, &mut splice, app_state);
        debug_assert!(scratch.is_empty());
    }

    fn teardown(
        &self,
        WrapState { seq_state, scratch }: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        let mut splice = WrapSplice::new(element, scratch);
        self.sequence.seq_teardown(seq_state, ctx, &mut splice);
        debug_assert!(scratch.is_empty());
    }

    fn message(
        &self,
        WrapState { seq_state, scratch }: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut splice = WrapSplice::new(element, scratch);
        let res = self
            .sequence
            .seq_message(seq_state, message, &mut splice, app_state);
        debug_assert!(scratch.is_empty());
        res
    }
}
