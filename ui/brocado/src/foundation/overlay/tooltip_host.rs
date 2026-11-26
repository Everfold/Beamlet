//! Hover tooltip host widget and Brocado view wrapper backed by Masonry's `LayerStack`.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx,
    PointerEvent, PointerUpdate, PropertiesMut, PropertiesRef, RegisterCtx, StyleProperty,
    Subwindow, SubwindowType, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Rect, Size, Vec2};
use masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, SizeDef};
use masonry::parley::style::{FontFamily, FontWeight, OverflowWrap};
use masonry::peniko::Color;
use masonry::properties::types::CrossAxisAlignment;
use masonry::properties::{Background, ContentColor, CornerRadius, Gap, LineBreaking, Padding};
use masonry::widgets::{Flex, Label, SizedBox};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::style_engine::current_theme_arc;
use crate::theme::{ElevationLevel, Theme};

const TOOLTIP_CHILD_ID: ViewId = ViewId::new(0x5549_3410);

/// Visual content description for a passive hover tooltip spawned on Masonry `LayerStack`.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipContentSpec {
    /// Optional bold header line (used by rich tooltips).
    pub subhead: Option<ArcStr>,
    /// Tooltip body text.
    pub body: ArcStr,
    /// Font family applied to tooltip labels (resolved from active `TypeScale`).
    pub font_family: FontFamily<'static>,
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
    /// Maximum tooltip card width before wrapping (`240.0` plain / `320.0` rich by default).
    pub max_width: f64,
    /// Hover dwell delay in milliseconds before displaying the tooltip (defaults to `450ms`).
    pub hover_delay_ms: u64,
}

impl TooltipContentSpec {
    fn build_tooltip_widget(
        &self,
        creator: WidgetId,
        anchor_rect: Rect,
        placed_pos: Point,
    ) -> NewWidget<TooltipLayer> {
        let content: NewWidget<dyn Widget> = if self.is_rich {
            let mut col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
            if let Some(sub) = &self.subhead {
                let lbl = Label::new(sub.clone())
                    .with_style(StyleProperty::FontSize(14.0))
                    .with_style(StyleProperty::FontWeight(FontWeight::MEDIUM))
                    .with_style(StyleProperty::FontFamily(self.font_family.clone()))
                    .with_style(StyleProperty::OverflowWrap(OverflowWrap::Anywhere))
                    .prepare()
                    .with_props((
                        ContentColor::new(self.subhead_color),
                        LineBreaking::WordWrap,
                    ));
                col = col.with_fixed(lbl);
            }
            let body_lbl = Label::new(self.body.clone())
                .with_style(StyleProperty::FontSize(14.0))
                .with_style(StyleProperty::FontFamily(self.font_family.clone()))
                .with_style(StyleProperty::OverflowWrap(OverflowWrap::Anywhere))
                .prepare()
                .with_props((ContentColor::new(self.body_color), LineBreaking::WordWrap));
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
                ))
                .erased()
        } else {
            let body_lbl = Label::new(self.body.clone())
                .with_style(StyleProperty::FontSize(12.0))
                .with_style(StyleProperty::FontFamily(self.font_family.clone()))
                .with_style(StyleProperty::OverflowWrap(OverflowWrap::Anywhere))
                .prepare()
                .with_props((ContentColor::new(self.body_color), LineBreaking::WordWrap));
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
            max_width: self.max_width.max(32.0),
            anchor_rect,
            placed_pos,
            window_size: Size::ZERO,
            layer_size: Size::ZERO,
            needs_reposition: true,
            dismissed: false,
        })
    }
}

struct TooltipLayer {
    creator: WidgetId,
    child: WidgetPod<dyn Widget>,
    max_width: f64,
    anchor_rect: Rect,
    placed_pos: Point,
    window_size: Size,
    layer_size: Size,
    needs_reposition: bool,
    dismissed: bool,
}

impl Widget for TooltipLayer {
    type Action = NoAction;

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) {
            self.needs_reposition = true;
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _interval: u64,
    ) {
        if self.needs_reposition
            && !self.dismissed
            && self.window_size.width > 0.0
            && self.window_size.height > 0.0
            && self.layer_size.width > 0.0
            && self.layer_size.height > 0.0
        {
            self.needs_reposition = false;
            let adjusted = super::clamp_anchored_layer_origin(
                self.anchor_rect,
                self.layer_size,
                self.window_size,
                Vec2::new(0.0, 6.0),
            );
            if (adjusted.x - self.placed_pos.x).abs() > 0.5
                || (adjusted.y - self.placed_pos.y).abs() > 0.5
            {
                self.placed_pos = adjusted;
                ctx.reposition_subwindow(ctx.widget_id(), adjusted);
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
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let cs = ctx.context_size();
        if let (Some(w), Some(h)) = (cs.length(Axis::Horizontal), cs.length(Axis::Vertical)) {
            self.window_size = Size::new(w.get(), h.get());
        }
        let max_w = if self.window_size.width > 0.0 {
            (self.window_size.width - 12.0).max(0.0).min(self.max_width)
        } else {
            self.max_width
        };
        let max_h = if self.window_size.height > 0.0 {
            (self.window_size.height - 12.0).max(0.0)
        } else {
            f64::INFINITY
        };

        match axis {
            Axis::Horizontal => {
                let measured = ctx
                    .compute_length(
                        &mut self.child,
                        LenDef::MaxContent,
                        LayoutSize::maybe(Axis::Vertical, cross_length),
                        Axis::Horizontal,
                        cross_length,
                    )
                    .get();
                let limit = match len_req {
                    LenReq::FitContent(space) => space.get().min(max_w),
                    _ => max_w,
                };
                Length::px(measured.min(limit))
            }
            Axis::Vertical => {
                let eff_w = cross_length.map_or_else(
                    || {
                        ctx.compute_length(
                            &mut self.child,
                            LenDef::MaxContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get()
                        .min(max_w)
                    },
                    |l| l.get().min(max_w),
                );
                let eff_w_len = Length::px(eff_w);
                let measured = ctx
                    .compute_length(
                        &mut self.child,
                        LenDef::MaxContent,
                        LayoutSize::one(Axis::Horizontal, eff_w_len),
                        Axis::Vertical,
                        Some(eff_w_len),
                    )
                    .get();
                let limit = match len_req {
                    LenReq::FitContent(space) => space.get().min(max_h),
                    _ => max_h,
                };
                Length::px(measured.min(limit))
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let vp_w = if self.window_size.width > 0.0 {
            size.width.min((self.window_size.width - 12.0).max(0.0))
        } else {
            (size.width - 12.0).max(0.0)
        };
        let vp_h = if self.window_size.height > 0.0 {
            size.height.min((self.window_size.height - 12.0).max(0.0))
        } else {
            (size.height - 12.0).max(0.0)
        };
        let natural_w = ctx
            .compute_size(
                &mut self.child,
                SizeDef::new(LenDef::MaxContent, LenDef::MaxContent),
                LayoutSize::NONE,
            )
            .width;
        let target_w = natural_w.min(vp_w).min(self.max_width);
        let wrapped = ctx.compute_size(
            &mut self.child,
            SizeDef::new(LenDef::Fixed(Length::px(target_w)), LenDef::MaxContent),
            LayoutSize::one(Axis::Horizontal, Length::px(target_w)),
        );
        let clamped = Size::new(target_w.min(vp_w), wrapped.height.min(vp_h));
        ctx.run_layout(&mut self.child, clamped);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.derive_baselines(&self.child);
        self.layer_size = clamped;
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

    fn as_subwindow(&mut self) -> Option<&mut dyn Subwindow> {
        Some(self)
    }

    fn accepts_pointer_interaction(&self) -> bool {
        false
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("TooltipLayer", id = id.trace())
    }
}

impl Subwindow for TooltipLayer {
    fn capture_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        let remove_tooltip = match event {
            PointerEvent::Move(PointerUpdate { current, .. }) => {
                let logical = current.position.to_logical::<f64>(ctx.scale_factor());
                !self.anchor_rect.contains(Point::new(logical.x, logical.y))
            }
            PointerEvent::Down(_)
            | PointerEvent::Up(_)
            | PointerEvent::Leave(_)
            | PointerEvent::Cancel(_) => true,
            _ => false,
        };
        if remove_tooltip && !self.dismissed {
            self.dismissed = true;
            let self_id = ctx.widget_id();
            ctx.remove_subwindow(self_id);
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
            this.ctx.remove_subwindow(old_id);
        }
    }

    /// Shows the attached tooltip layer immediately (useful for deterministic testing).
    pub fn show_now(this: &mut WidgetMut<'_, Self>) {
        Self::close_layer(this);
        if this.ctx.is_stashed() {
            return;
        }
        let border_box = this.ctx.border_box();
        if border_box.size() == Size::ZERO {
            return;
        }
        let anchor_origin = this.ctx.to_window(border_box.origin());
        let anchor_rect = Rect::from_origin_size(anchor_origin, border_box.size());
        let pos = anchor_origin + Vec2::new(0.0, border_box.height() + 6.0);
        let layer = this
            .widget
            .spec
            .build_tooltip_widget(this.ctx.widget_id(), anchor_rect, pos);
        let layer_id = layer.id();
        this.widget.layer_id = Some(layer_id);
        this.ctx.create_subwindow(
            SubwindowType::Tooltip(this.widget.spec.body.to_string()),
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
        if matches!(event, PointerEvent::Move(_)) && self.layer_id.is_none() {
            self.hover_pending = true;
            self.hover_elapsed_ns = 0;
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(
            event,
            Update::HoveredChanged(false)
                | Update::ChildHoveredChanged(false)
                | Update::StashedChanged(true)
        ) {
            self.hover_pending = false;
            self.hover_elapsed_ns = 0;
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<TooltipLayer>().widget.dismissed = true;
                });
                ctx.remove_subwindow(old_id);
            }
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if ctx.is_stashed() {
            self.hover_pending = false;
            self.hover_elapsed_ns = 0;
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<TooltipLayer>().widget.dismissed = true;
                });
                ctx.remove_subwindow(old_id);
            }
            return;
        }
        if self.hover_pending {
            self.hover_elapsed_ns = self.hover_elapsed_ns.saturating_add(interval);
            let delay_ns = self.spec.hover_delay_ms.saturating_mul(1_000_000);
            if self.hover_elapsed_ns >= delay_ns {
                self.hover_pending = false;
                self.hover_elapsed_ns = 0;
                if let Some(old_id) = self.layer_id.take() {
                    ctx.mutate_later(old_id, |mut layer| {
                        layer.downcast::<TooltipLayer>().widget.dismissed = true;
                    });
                    ctx.remove_subwindow(old_id);
                }
                let border_box = ctx.border_box();
                if border_box.size() == Size::ZERO {
                    return;
                }
                let anchor_origin = ctx.to_window(border_box.origin());
                let anchor_rect = Rect::from_origin_size(anchor_origin, border_box.size());
                let pos = anchor_origin + Vec2::new(0.0, border_box.height() + 6.0);
                let layer = self
                    .spec
                    .build_tooltip_widget(ctx.widget_id(), anchor_rect, pos);
                self.layer_id = Some(layer.id());
                ctx.create_subwindow(
                    SubwindowType::Tooltip(self.spec.body.to_string()),
                    layer,
                    pos,
                );
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

/// Brocado [`View`] wrapper pairing a child view with a [`HoverTooltipHost`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic child View and closure trait object"
)]
pub struct HoverTooltipView<V, State, Action> {
    /// Target widget view that displays a tooltip when hovered.
    pub child: V,
    /// Theme-aware tooltip spec constructor.
    pub spec_builder: Arc<dyn Fn(&Theme) -> TooltipContentSpec + Send + Sync>,
    /// Optional maximum tooltip width override in logical pixels.
    pub max_width: Option<f64>,
    /// Optional hover dwell delay override in milliseconds.
    pub hover_delay_ms: Option<u64>,
    /// State/Action marker.
    pub phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> HoverTooltipView<V, State, Action> {
    /// Overrides the maximum width of the tooltip overlay before wrapping.
    pub fn max_width(mut self, width: f64) -> Self {
        self.max_width = Some(width.max(32.0));
        self
    }

    /// Overrides the hover dwell delay in milliseconds before displaying the tooltip.
    pub fn hover_delay_ms(mut self, delay_ms: u64) -> Self {
        self.hover_delay_ms = Some(delay_ms);
        self
    }

    fn resolve_spec(&self, theme: &Theme) -> TooltipContentSpec {
        let mut s = (self.spec_builder)(theme);
        if let Some(mw) = self.max_width {
            s.max_width = mw;
        }
        if let Some(d) = self.hover_delay_ms {
            s.hover_delay_ms = d;
        }
        s
    }
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
        let theme = current_theme_arc(ctx);
        let tip_spec = self.resolve_spec(&theme);
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
        let theme = current_theme_arc(ctx);
        HoverTooltipHost::set_spec(&mut element, self.resolve_spec(&theme));
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
