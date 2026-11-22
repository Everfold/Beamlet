//! WinUI 3 / Fluent 2 beveled elevation surface wrapper ([`BeveledBorder`] / [`beveled_surface`]).

use std::marker::PhantomData;

use tracing::{Span, trace_span};
use xilem::Color;
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Line, Point, Size, Stroke};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesRef,
    RegisterCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{LenReq, Length};
use xilem::{Pod, ViewCtx, WidgetView};

use crate::style_engine::current_spec_arc;
use crate::tokens::{Brightness, ControlBorderStyle};

const BEVEL_CHILD_ID: ViewId = ViewId::new(0x5549_3620);

#[derive(Clone, Copy, Debug, PartialEq)]
struct BevelConfig {
    enabled: bool,
    brightness: Brightness,
    radius: f64,
}

/// Masonry single-child wrapper painting WinUI 3 directional 1px beveled card highlights
/// when [`ControlBorderStyle::BeveledElevation`] is active.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct BeveledBorder {
    child: WidgetPod<dyn Widget>,
    config: BevelConfig,
}

impl BeveledBorder {
    fn new(child: NewWidget<impl Widget + ?Sized>, config: BevelConfig) -> Self {
        Self {
            child: child.erased().to_pod(),
            config,
        }
    }

    fn set_config(this: &mut WidgetMut<'_, Self>, config: BevelConfig) {
        if this.widget.config != config {
            this.widget.config = config;
            this.ctx.request_post_paint();
        }
    }

    /// Returns a mutable reference to the wrapped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for BeveledBorder {
    type Action = NoAction;

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
        if !self.config.enabled {
            return;
        }
        let rect = ctx.border_box();
        if rect.width() <= 6.0 || rect.height() <= 4.0 {
            return;
        }
        let inset = (self.config.radius * 0.45)
            .max(1.5)
            .min(rect.width() * 0.25);
        match self.config.brightness {
            Brightness::Light => {
                let y = rect.y1 - 0.5;
                let line = Line::new(
                    Point::new(rect.x0 + inset, y),
                    Point::new(rect.x1 - inset, y),
                );
                painter
                    .stroke(line, &Stroke::new(1.0), Color::from_rgba8(0, 0, 0, 24))
                    .draw();
            }
            Brightness::Dark => {
                let y = rect.y0 + 0.5;
                let line = Line::new(
                    Point::new(rect.x0 + inset, y),
                    Point::new(rect.x1 - inset, y),
                );
                painter
                    .stroke(
                        line,
                        &Stroke::new(1.0),
                        Color::from_rgba8(255, 255, 255, 22),
                    )
                    .draw();
            }
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
        trace_span!("BeveledBorder", id = id.trace())
    }
}

/// View wrapper constructed by [`beveled_surface`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct BeveledBorderView<V, State, Action> {
    child: V,
    radius: Option<f64>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> BeveledBorderView<V, State, Action> {
    /// Overrides the corner inset radius in logical pixels.
    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = Some(radius);
        self
    }

    fn resolve_config(&self, ctx: &ViewCtx) -> BevelConfig {
        let spec = current_spec_arc(ctx);
        BevelConfig {
            enabled: spec.control_border == ControlBorderStyle::BeveledElevation,
            brightness: spec.brightness,
            radius: self.radius.unwrap_or(spec.metrics.card_radius),
        }
    }
}

/// Wraps `child` with a WinUI 3 directional [`BeveledBorder`] edge when [`ControlBorderStyle::BeveledElevation`] is active.
pub fn beveled_surface<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> BeveledBorderView<V, State, Action> {
    BeveledBorderView {
        child,
        radius: None,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for BeveledBorderView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for BeveledBorderView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<BeveledBorder>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let config = self.resolve_config(ctx);
        let (child_pod, child_state) =
            ctx.with_id(BEVEL_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(BeveledBorder::new(child_pod.new_widget, config));
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
        BeveledBorder::set_config(&mut element, config);
        let mut child_dyn = BeveledBorder::child_mut(&mut element);
        ctx.with_id(BEVEL_CHILD_ID, |ctx| {
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
        let mut child_dyn = BeveledBorder::child_mut(&mut element);
        ctx.with_id(BEVEL_CHILD_ID, |ctx| {
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
            Some(BEVEL_CHILD_ID) => {
                let mut child_dyn = BeveledBorder::child_mut(&mut element);
                self.child
                    .message(view_state, message, child_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
