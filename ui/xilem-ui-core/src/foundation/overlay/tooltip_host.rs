//! Hover tooltip host widget and Xilem view wrapper backed by Masonry's `LayerStack`.

use std::marker::PhantomData;
use std::sync::Arc;

use tracing::{Span, trace_span};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Size, Vec2};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, EventCtx, Layer, LayerType, LayoutCtx, MeasureCtx, NewWidget,
    NoAction, PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, StyleProperty,
    Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{AsUnit, LayoutSize, LenReq, Length, SizeDef};
use xilem::masonry::parley::style::FontWeight;
use xilem::masonry::properties::types::CrossAxisAlignment;
use xilem::masonry::properties::{
    Background, ContentColor, CornerRadius, Dimensions, Gap, Padding,
};
use xilem::masonry::widgets::{Flex, Label, SizedBox};
use xilem::{Color, Pod, ViewCtx, WidgetView};

use crate::style_engine::scope::current_spec_arc;
use crate::tokens::{DesignSpec, ElevationLevel};

const TOOLTIP_HOVER_DELAY_NS: u64 = 450_000_000;
const TOOLTIP_CHILD_ID: ViewId = ViewId::new(0x5549_3410);

/// Visual content description for a passive hover tooltip spawned on Masonry `LayerStack`.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipContentSpec {
    /// Optional bold header line (used by rich tooltips).
    pub subhead: Option<ArcStr>,
    /// Tooltip body text.
    pub body: ArcStr,
    /// Tooltip container fill color.
    pub bg: Color,
    /// Subhead text color.
    pub subhead_color: Color,
    /// Body text color.
    pub body_color: Color,
    /// Shadow tint for rich tooltips.
    pub shadow_color: Color,
    /// Corner radius in logical pixels.
    pub corner_radius: f64,
    /// Whether this renders a compact plain banner (`false`) or a rich card (`true`).
    pub is_rich: bool,
}

impl TooltipContentSpec {
    fn build_tooltip_widget(&self, creator: WidgetId) -> NewWidget<TooltipLayer> {
        let content: NewWidget<dyn Widget> = if self.is_rich {
            let mut col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
            if let Some(sub) = &self.subhead {
                let lbl = Label::new(sub.clone())
                    .with_style(StyleProperty::FontSize(14.0))
                    .with_style(StyleProperty::FontWeight(FontWeight::MEDIUM))
                    .prepare()
                    .with_props(ContentColor::new(self.subhead_color));
                col = col.with_fixed(lbl);
            }
            let body_lbl = Label::new(self.body.clone())
                .with_style(StyleProperty::FontSize(14.0))
                .prepare()
                .with_props(ContentColor::new(self.body_color));
            col = col.with_fixed(body_lbl);
            SizedBox::new(col.prepare().with_props(Gap::new(4.px())))
                .prepare()
                .with_props((
                    Background::Color(self.bg),
                    CornerRadius {
                        radius: Length::px(self.corner_radius),
                    },
                    Padding::from_vh(12.px(), 16.px()),
                    ElevationLevel::Level2.box_shadow(self.shadow_color),
                    Dimensions::new(
                        xilem::masonry::layout::Dim::MaxContent,
                        xilem::masonry::layout::Dim::MaxContent,
                    ),
                ))
                .erased()
        } else {
            let body_lbl = Label::new(self.body.clone())
                .with_style(StyleProperty::FontSize(12.0))
                .prepare()
                .with_props(ContentColor::new(self.body_color));
            SizedBox::new(body_lbl)
                .prepare()
                .with_props((
                    Background::Color(self.bg),
                    CornerRadius {
                        radius: Length::px(self.corner_radius),
                    },
                    Padding::from_vh(4.px(), 8.px()),
                ))
                .erased()
        };
        NewWidget::new(TooltipLayer {
            creator,
            child: content.to_pod(),
            dismissed: false,
        })
    }
}

struct TooltipLayer {
    creator: WidgetId,
    child: WidgetPod<dyn Widget>,
    dismissed: bool,
}

impl Widget for TooltipLayer {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let auto_length = len_req.into();
        let context_size = LayoutSize::maybe(axis.cross(), cross_length);
        ctx.compute_length(
            &mut self.child,
            auto_length,
            context_size,
            axis,
            cross_length,
        )
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let child_size = ctx.compute_size(&mut self.child, SizeDef::fit(size), size.into());
        ctx.run_layout(&mut self.child, child_size);
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
        Role::Tooltip
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

    fn as_layer(&mut self) -> Option<&mut dyn Layer> {
        Some(self)
    }

    fn accepts_pointer_interaction(&self) -> bool {
        false
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("TooltipLayer", id = id.trace())
    }
}

impl Layer for TooltipLayer {
    fn capture_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        let remove_tooltip = matches!(
            event,
            PointerEvent::Down(_)
                | PointerEvent::Up(_)
                | PointerEvent::Move(_)
                | PointerEvent::Leave(_)
                | PointerEvent::Cancel(_)
        );
        if remove_tooltip && !self.dismissed {
            self.dismissed = true;
            let self_id = ctx.widget_id();
            ctx.remove_layer(self_id);
            ctx.mutate_later(self.creator, move |mut host| {
                let host = host.downcast::<HoverTooltipHost>();
                if host.widget.layer_id == Some(self_id) {
                    host.widget.layer_id = None;
                }
            });
        }
    }
}

/// Masonry single-child wrapper showing a tooltip overlay layer after hovering.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct HoverTooltipHost {
    child: WidgetPod<dyn Widget>,
    spec: TooltipContentSpec,
    hover_pending: bool,
    hover_elapsed_ns: u64,
    layer_id: Option<WidgetId>,
}

impl HoverTooltipHost {
    fn new(child: NewWidget<impl Widget + ?Sized>, spec: TooltipContentSpec) -> Self {
        Self {
            child: child.erased().to_pod(),
            spec,
            hover_pending: false,
            hover_elapsed_ns: 0,
            layer_id: None,
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    fn set_spec(this: &mut WidgetMut<'_, Self>, spec: TooltipContentSpec) {
        if this.widget.spec != spec {
            this.widget.spec = spec;
        }
    }

    /// Closes any active tooltip layer owned by this host without panicking.
    pub fn close_layer(this: &mut WidgetMut<'_, Self>) {
        this.widget.hover_pending = false;
        this.widget.hover_elapsed_ns = 0;
        if let Some(old_id) = this.widget.layer_id.take() {
            this.ctx.mutate_later(old_id, |mut layer| {
                layer.downcast::<TooltipLayer>().widget.dismissed = true;
            });
            this.ctx.remove_layer(old_id);
        }
    }

    /// Shows the attached tooltip layer immediately (useful for deterministic testing).
    pub fn show_now(this: &mut WidgetMut<'_, Self>) {
        Self::close_layer(this);
        let layer = this.widget.spec.build_tooltip_widget(this.ctx.widget_id());
        let layer_id = layer.id();
        let border_box = this.ctx.border_box();
        let pos =
            this.ctx.to_window(border_box.origin()) + Vec2::new(0.0, border_box.height() + 6.0);
        this.widget.layer_id = Some(layer_id);
        this.ctx.create_layer(
            LayerType::Tooltip(this.widget.spec.body.to_string()),
            layer,
            pos,
        );
    }
}

impl Widget for HoverTooltipHost {
    type Action = NoAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if matches!(event, PointerEvent::Move(_)) {
            self.hover_pending = true;
            self.hover_elapsed_ns = 0;
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::HoveredChanged(false) = event {
            self.hover_pending = false;
            self.hover_elapsed_ns = 0;
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<TooltipLayer>().widget.dismissed = true;
                });
                ctx.remove_layer(old_id);
            }
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.hover_pending {
            self.hover_elapsed_ns = self.hover_elapsed_ns.saturating_add(interval);
            if self.hover_elapsed_ns >= TOOLTIP_HOVER_DELAY_NS {
                self.hover_pending = false;
                self.hover_elapsed_ns = 0;
                if let Some(old_id) = self.layer_id.take() {
                    ctx.mutate_later(old_id, |mut layer| {
                        layer.downcast::<TooltipLayer>().widget.dismissed = true;
                    });
                    ctx.remove_layer(old_id);
                }
                let layer = self.spec.build_tooltip_widget(ctx.widget_id());
                self.layer_id = Some(layer.id());
                let border_box = ctx.border_box();
                let pos =
                    ctx.to_window(border_box.origin()) + Vec2::new(0.0, border_box.height() + 6.0);
                ctx.create_layer(LayerType::Tooltip(self.spec.body.to_string()), layer, pos);
            } else {
                ctx.request_anim_frame();
            }
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
        trace_span!("HoverTooltipHost", id = id.trace())
    }
}

/// Xilem [`View`] wrapper pairing a child view with a [`HoverTooltipHost`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic child View and closure trait object"
)]
pub struct HoverTooltipView<V, State, Action> {
    /// Target widget view that displays a tooltip when hovered.
    pub child: V,
    /// Theme-aware tooltip spec constructor.
    pub spec_builder: Arc<dyn Fn(&DesignSpec) -> TooltipContentSpec + Send + Sync>,
    /// State/Action marker.
    pub phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> ViewMarker for HoverTooltipView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for HoverTooltipView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<HoverTooltipHost>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec_theme = current_spec_arc(ctx);
        let tip_spec = (self.spec_builder)(&spec_theme);
        let (child_pod, child_state) =
            ctx.with_id(TOOLTIP_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx.create_pod(HoverTooltipHost::new(child_pod.new_widget, tip_spec));
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
        let spec_theme = current_spec_arc(ctx);
        HoverTooltipHost::set_spec(&mut element, (self.spec_builder)(&spec_theme));
        let mut child = HoverTooltipHost::child_mut(&mut element);
        ctx.with_id(TOOLTIP_CHILD_ID, |ctx| {
            self.child
                .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        HoverTooltipHost::close_layer(&mut element);
        let mut child = HoverTooltipHost::child_mut(&mut element);
        ctx.with_id(TOOLTIP_CHILD_ID, |ctx| {
            self.child.teardown(view_state, ctx, child.downcast());
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
            Some(TOOLTIP_CHILD_ID) => {
                let mut child = HoverTooltipHost::child_mut(&mut element);
                self.child
                    .message(view_state, message, child.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}
