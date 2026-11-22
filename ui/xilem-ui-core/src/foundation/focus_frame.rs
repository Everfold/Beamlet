//! Reactive focus indicator frame rendering platform-authentic focus rings, halos, and bottom accent bars.

use std::marker::PhantomData;

use tracing::{Span, trace_span};
use xilem::Color;
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Rect, Size, Stroke};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PropertiesMut,
    PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{LenReq, Length};
use xilem::{Pod, ViewCtx, WidgetView};

use crate::foundation::state_layer::with_opacity;
use crate::style_engine::current_spec_arc;
use crate::tokens::InputFocusStyle;

const FOCUS_FRAME_CHILD_ID: ViewId = ViewId::new(0x5549_3601);

#[derive(Clone, Copy, Debug, PartialEq)]
struct FocusFrameConfig {
    style: InputFocusStyle,
    radius: f64,
    accent_color: Color,
    rest_bottom_color: Color,
    error_color: Option<Color>,
}

/// Masonry single-child wrapper painting platform-authentic focus rings and bottom accent bars
/// dynamically on [`Update::FocusChanged`] and [`Update::ChildFocusChanged`].
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct FocusFrame {
    child: WidgetPod<dyn Widget>,
    config: FocusFrameConfig,
    focused: bool,
}

impl FocusFrame {
    fn new(child: NewWidget<impl Widget + ?Sized>, config: FocusFrameConfig) -> Self {
        Self {
            child: child.erased().to_pod(),
            config,
            focused: false,
        }
    }

    fn set_config(this: &mut WidgetMut<'_, Self>, config: FocusFrameConfig) {
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

impl Widget for FocusFrame {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::FocusChanged(focused) | Update::ChildFocusChanged(focused) = event
            && self.focused != *focused
        {
            self.focused = *focused;
            ctx.request_post_paint();
        }
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
        let rect = ctx.border_box();
        if rect.width() <= 2.0 || rect.height() <= 2.0 {
            return;
        }
        let active_color = self.config.error_color.unwrap_or(self.config.accent_color);
        let show_active = self.focused || self.config.error_color.is_some();
        let radius = self.config.radius.max(0.0);

        match self.config.style {
            InputFocusStyle::Outline2px => {
                if show_active {
                    let rrect = rect.inset(1.0).to_rounded_rect((radius - 1.0).max(0.0));
                    painter
                        .stroke(rrect, &Stroke::new(2.0), active_color)
                        .draw();
                }
            }
            InputFocusStyle::BottomAccentLine2px => {
                let (bar_h, col) = if show_active {
                    (2.0, active_color)
                } else {
                    (1.0, self.config.rest_bottom_color)
                };
                if rect.height() > bar_h {
                    let bar = Rect::new(rect.x0 + 1.0, rect.y1 - bar_h, rect.x1 - 1.0, rect.y1)
                        .to_rounded_rect(1.0);
                    painter.fill(bar, col).draw();
                }
            }
            InputFocusStyle::OuterHalo3px => {
                if show_active {
                    let outer = rect.inflate(1.5, 1.5).to_rounded_rect(radius + 1.5);
                    painter
                        .stroke(outer, &Stroke::new(3.0), with_opacity(active_color, 0.32))
                        .draw();
                    let inner = rect.inset(0.5).to_rounded_rect((radius - 0.5).max(0.0));
                    painter
                        .stroke(inner, &Stroke::new(1.0), active_color)
                        .draw();
                }
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
        trace_span!("FocusFrame", id = id.trace())
    }
}

/// View wrapper constructed by [`focus_frame`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct FocusFrameView<V, State, Action> {
    child: V,
    style: Option<InputFocusStyle>,
    radius: Option<f64>,
    accent_color: Option<Color>,
    rest_bottom_color: Option<Color>,
    is_error: bool,
    error_color: Option<Color>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> FocusFrameView<V, State, Action> {
    /// Overrides the focus ring / accent bar strategy from the active [`DesignSpec`](crate::tokens::DesignSpec).
    pub fn style(mut self, style: InputFocusStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Overrides the focus stroke corner radius in logical pixels.
    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = Some(radius);
        self
    }

    /// Overrides the active focus highlight color.
    pub fn accent(mut self, color: Color) -> Self {
        self.accent_color = Some(color);
        self
    }

    /// Overrides the resting bottom border tint when [`InputFocusStyle::BottomAccentLine2px`] is active.
    pub fn rest_bottom_color(mut self, color: Color) -> Self {
        self.rest_bottom_color = Some(color);
        self
    }

    /// Marks the frame as being in a validation error state.
    pub fn error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }

    /// Overrides the validation error color (also enables `error(true)`).
    pub fn error_color(mut self, color: Color) -> Self {
        self.is_error = true;
        self.error_color = Some(color);
        self
    }

    fn resolve_config(&self, ctx: &ViewCtx) -> FocusFrameConfig {
        let spec = current_spec_arc(ctx);
        FocusFrameConfig {
            style: self.style.unwrap_or(spec.input_focus),
            radius: self.radius.unwrap_or(spec.metrics.input_radius),
            accent_color: self.accent_color.unwrap_or(spec.palette.accent),
            rest_bottom_color: self.rest_bottom_color.unwrap_or(spec.palette.border_strong),
            error_color: self
                .is_error
                .then(|| self.error_color.unwrap_or(spec.palette.error)),
        }
    }
}

/// Wraps `child` with a reactive [`FocusFrame`] honoring [`InputFocusStyle`] on actual keyboard focus.
pub fn focus_frame<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> FocusFrameView<V, State, Action> {
    FocusFrameView {
        child,
        style: None,
        radius: None,
        accent_color: None,
        rest_bottom_color: None,
        is_error: false,
        error_color: None,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for FocusFrameView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for FocusFrameView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<FocusFrame>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let config = self.resolve_config(ctx);
        let (child_pod, child_state) =
            ctx.with_id(FOCUS_FRAME_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(FocusFrame::new(child_pod.new_widget, config));
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
        FocusFrame::set_config(&mut element, config);
        let mut child_dyn = FocusFrame::child_mut(&mut element);
        ctx.with_id(FOCUS_FRAME_CHILD_ID, |ctx| {
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
        let mut child_dyn = FocusFrame::child_mut(&mut element);
        ctx.with_id(FOCUS_FRAME_CHILD_ID, |ctx| {
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
            Some(FOCUS_FRAME_CHILD_ID) => {
                let mut child_dyn = FocusFrame::child_mut(&mut element);
                self.child
                    .message(view_state, message, child_dyn.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
