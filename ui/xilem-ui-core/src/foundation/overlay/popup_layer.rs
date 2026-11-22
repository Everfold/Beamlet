//! Anchored floating popup layer (`PopupLayer`) and trigger host (`AnchoredPopupHost`) for menus,
//! exposed dropdown selects, and interactive rich tooltips.

use std::marker::PhantomData;
use std::sync::Arc;

use tracing::{Span, trace_span};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Size, Vec2};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::keyboard::{Key, NamedKey};
use xilem::masonry::core::{
    AccessCtx, AccessEvent, ActionCtx, ArcStr, ChildrenIds, ErasedAction, EventCtx, Layer,
    LayerType, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PointerButtonEvent,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, StyleProperty, TextEvent, Update,
    UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{AsUnit, LayoutSize, LenReq, Length, SizeDef};
use xilem::masonry::parley::style::FontWeight;
use xilem::masonry::properties::types::CrossAxisAlignment;
use xilem::masonry::properties::{
    Background, BorderColor, BorderWidth, ContentColor, CornerRadius, Dimensions, Gap, Padding,
};
use xilem::masonry::widgets::{self, ButtonPress, Divider, Flex, Label, SizedBox, Svg};
use xilem::{Color, Pod, ViewCtx, WidgetView};

use crate::foundation::icon::cached_icon_tree;
use crate::style_engine::scope::current_spec;
use crate::tokens::{DesignSpec, ElevationLevel};

const POPUP_TRIGGER_ID: ViewId = ViewId::new(0x5549_3420);

/// Descriptor for a single entry inside an anchored [`PopupLayer`].
#[derive(Clone, Debug, PartialEq)]
pub enum PopupEntrySpec {
    /// Interactive menu / footer action button identified by `action_idx`.
    Item {
        /// Index passed to [`PopupAction::Selected`].
        action_idx: usize,
        /// Primary caption text.
        label: ArcStr,
        /// Optional leading SVG icon path.
        leading_icon: Option<&'static str>,
        /// Optional trailing shortcut or annotation text.
        trailing_text: Option<ArcStr>,
        /// Whether this entry renders as currently selected.
        selected: bool,
        /// Whether this entry ignores clicks.
        disabled: bool,
        /// Foreground text and icon color.
        fg: Color,
        /// Masonry class applied to the entry button.
        class: &'static str,
    },
    /// Hairline horizontal divider separating groups of entries.
    Divider,
}

/// Style and content descriptor for [`AnchoredPopupHost`] / [`PopupLayer`].
#[derive(Clone, Debug, PartialEq)]
pub struct PopupLayerSpec {
    /// Minimum popup width in logical pixels.
    pub min_width: f64,
    /// Maximum popup width in logical pixels.
    pub max_width: f64,
    /// Popup card background color.
    pub surface_bg: Color,
    /// Drop shadow color.
    pub shadow_color: Color,
    /// Corner radius in logical pixels.
    pub corner_radius: f64,
    /// Divider tint.
    pub divider_color: Color,
    /// Vertical container padding in logical pixels.
    pub padding_v: f64,
    /// Horizontal container padding in logical pixels.
    pub padding_h: f64,
    /// Optional header `(subhead, subhead_color, body, body_color)`.
    pub header: Option<(Option<ArcStr>, Color, ArcStr, Color)>,
    /// Interactive menu entries and dividers.
    pub entries: Vec<PopupEntrySpec>,
}

/// Action submitted by [`AnchoredPopupHost`] when a popup entry is activated or the popup closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupAction {
    /// Menu/action button at `usize` was clicked.
    Selected(usize),
    /// Popup closed without selecting an item (outside click, Escape, or window focus loss).
    Dismissed,
}

/// Masonry `Layer` widget rendering an elevated popup card (menus, selects, or interactive tooltips).
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct PopupLayer {
    creator: WidgetId,
    child: WidgetPod<SizedBox>,
    button_map: Vec<(WidgetId, usize)>,
    min_width: f64,
    max_width: f64,
    dismissed: bool,
}

impl PopupLayer {
    fn from_spec(creator: WidgetId, spec: &PopupLayerSpec) -> NewWidget<Self> {
        let mut col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Stretch);
        let mut button_map = Vec::new();

        if let Some((subhead, sub_color, body, body_color)) = &spec.header {
            let mut header_col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
            if let Some(sub) = subhead {
                let lbl = Label::new(sub.clone())
                    .with_style(StyleProperty::FontSize(14.0))
                    .with_style(StyleProperty::FontWeight(FontWeight::MEDIUM))
                    .prepare()
                    .with_props(ContentColor::new(*sub_color));
                header_col = header_col.with_fixed(lbl);
            }
            let body_lbl = Label::new(body.clone())
                .with_style(StyleProperty::FontSize(14.0))
                .prepare()
                .with_props(ContentColor::new(*body_color));
            header_col = header_col.with_fixed(body_lbl);
            let header_box = SizedBox::new(header_col.prepare().with_props(Gap::new(4.px())))
                .prepare()
                .with_props(Padding::from_vh(8.px(), 12.px()));
            col = col.with_fixed(header_box);
        }

        for entry in &spec.entries {
            match entry {
                PopupEntrySpec::Divider => {
                    let div = Divider::horizontal()
                        .prepare()
                        .with_props(ContentColor::new(spec.divider_color));
                    col = col.with_fixed(div);
                }
                PopupEntrySpec::Item {
                    action_idx,
                    label,
                    leading_icon,
                    trailing_text,
                    disabled,
                    fg,
                    class,
                    ..
                } => {
                    let mut row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Center);
                    if let Some(svg_path) = leading_icon {
                        let icon = Svg::new(cached_icon_tree(svg_path, *fg))
                            .prepare()
                            .with_props(Dimensions::fixed(18.px(), 18.px()));
                        row = row.with_fixed(icon);
                    }
                    let title = Label::new(label.clone())
                        .with_style(StyleProperty::FontSize(14.0))
                        .prepare()
                        .with_props(ContentColor::new(*fg));
                    row = row.with(title, 1.0);
                    if let Some(trailing) = trailing_text {
                        let tr = Label::new(trailing.clone())
                            .with_style(StyleProperty::FontSize(12.0))
                            .prepare()
                            .with_props(ContentColor::new(*fg));
                        row = row.with_fixed(tr);
                    }

                    let mut btn =
                        widgets::Button::new(row.prepare().with_props(Gap::new(12.px()))).prepare();
                    btn.classes.insert((*class).to_string());
                    btn.options.disabled = *disabled;
                    let btn_id = btn.id();
                    button_map.push((btn_id, *action_idx));
                    col = col.with_fixed(btn);
                }
            }
        }

        let card = SizedBox::new(col.prepare().with_props(Gap::new(2.px())))
            .prepare()
            .with_props((
                Background::Color(spec.surface_bg),
                CornerRadius {
                    radius: Length::px(spec.corner_radius),
                },
                Padding::from_vh(Length::px(spec.padding_v), Length::px(spec.padding_h)),
                ElevationLevel::Level2.box_shadow(spec.shadow_color),
                BorderWidth { width: 0.px() },
                BorderColor::new(Color::TRANSPARENT),
                Dimensions::new(
                    xilem::masonry::layout::Dim::MaxContent,
                    xilem::masonry::layout::Dim::MaxContent,
                ),
            ));

        NewWidget::new(Self {
            creator,
            child: card.to_pod(),
            button_map,
            min_width: spec.min_width,
            max_width: spec.max_width.max(spec.min_width),
            dismissed: false,
        })
    }

    fn dismiss_layer(&mut self, ctx: &mut EventCtx<'_>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        let self_id = ctx.widget_id();
        ctx.remove_layer(self_id);
        let creator = self.creator;
        ctx.mutate_later(creator, move |mut host| {
            let mut host = host.downcast::<AnchoredPopupHost>();
            if host.widget.layer_id == Some(self_id) {
                host.widget.layer_id = None;
            }
            if host.widget.open {
                host.ctx
                    .submit_action::<PopupAction>(PopupAction::Dismissed);
            }
        });
    }
}

impl Widget for PopupLayer {
    type Action = NoAction;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(key_event)
                if !key_event.state.is_up() && key_event.key == Key::Named(NamedKey::Escape) =>
            {
                ctx.set_handled();
                self.dismiss_layer(ctx);
            }
            TextEvent::WindowFocusChange(false) => {
                self.dismiss_layer(ctx);
            }
            _ => {}
        }
    }

    fn on_action(
        &mut self,
        ctx: &mut ActionCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        action: &ErasedAction,
        source: WidgetId,
    ) {
        if action.downcast_ref::<ButtonPress>().is_some()
            && !self.dismissed
            && let Some(&(_, idx)) = self.button_map.iter().find(|(id, _)| *id == source)
        {
            self.dismissed = true;
            ctx.set_handled();
            let self_id = ctx.widget_id();
            ctx.remove_layer(self_id);
            ctx.mutate_later(self.creator, move |mut host| {
                let mut host = host.downcast::<AnchoredPopupHost>();
                if host.widget.layer_id == Some(self_id) {
                    host.widget.layer_id = None;
                }
                host.ctx
                    .submit_action::<PopupAction>(PopupAction::Selected(idx));
            });
        }
    }

    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
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
        let len = ctx.compute_length(
            &mut self.child,
            len_req.into(),
            LayoutSize::maybe(axis.cross(), cross_length),
            axis,
            cross_length,
        );
        if axis == Axis::Horizontal {
            Length::px(len.get().clamp(self.min_width, self.max_width))
        } else {
            len
        }
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
        Role::Menu
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

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("PopupLayer", id = id.trace())
    }
}

impl Layer for PopupLayer {
    fn capture_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        let should_dismiss = match event {
            PointerEvent::Down(PointerButtonEvent { state, .. }) => !ctx
                .border_box()
                .contains(ctx.local_position(state.position)),
            PointerEvent::Cancel(..) => true,
            _ => false,
        };
        if should_dismiss {
            self.dismiss_layer(ctx);
        }
    }
}

/// Masonry host widget managing an anchored [`PopupLayer`] tied to a trigger widget.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod and PopupLayerSpec"
)]
pub struct AnchoredPopupHost {
    trigger: WidgetPod<dyn Widget>,
    open: bool,
    pending_spawn: bool,
    offset: Vec2,
    spec: PopupLayerSpec,
    layer_id: Option<WidgetId>,
}

impl AnchoredPopupHost {
    fn new(
        trigger: NewWidget<impl Widget + ?Sized>,
        open: bool,
        offset: Vec2,
        spec: PopupLayerSpec,
    ) -> Self {
        Self {
            trigger: trigger.erased().to_pod(),
            open,
            pending_spawn: open,
            offset,
            spec,
            layer_id: None,
        }
    }

    fn trigger_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.trigger)
    }

    /// Returns the active Masonry `Layer` root ID when the popup is currently mounted.
    #[must_use]
    pub fn layer_id(&self) -> Option<WidgetId> {
        self.layer_id
    }

    fn remove_active_layer_with(layer_id: Option<WidgetId>, mut remove_fn: impl FnMut(WidgetId)) {
        if let Some(id) = layer_id {
            remove_fn(id);
        }
    }

    fn sync(this: &mut WidgetMut<'_, Self>, open: bool, offset: Vec2, spec: PopupLayerSpec) {
        let state_changed =
            this.widget.open != open || this.widget.offset != offset || this.widget.spec != spec;
        if !state_changed {
            return;
        }
        this.widget.open = open;
        this.widget.offset = offset;
        this.widget.spec = spec;

        if !open {
            this.widget.pending_spawn = false;
            Self::remove_active_layer_with(this.widget.layer_id.take(), |id| {
                this.ctx.mutate_later(id, |mut layer| {
                    layer.downcast::<PopupLayer>().widget.dismissed = true;
                });
                this.ctx.remove_layer(id);
            });
            return;
        }

        Self::remove_active_layer_with(this.widget.layer_id.take(), |id| {
            this.ctx.mutate_later(id, |mut layer| {
                layer.downcast::<PopupLayer>().widget.dismissed = true;
            });
            this.ctx.remove_layer(id);
        });
        let border_box = this.ctx.border_box();
        if border_box.size() != Size::ZERO {
            let layer = PopupLayer::from_spec(this.ctx.widget_id(), &this.widget.spec);
            this.widget.layer_id = Some(layer.id());
            let pos = this.ctx.to_window(border_box.origin())
                + Vec2::new(0.0, border_box.height())
                + offset;
            this.ctx.create_layer(LayerType::Other, layer, pos);
            this.widget.pending_spawn = false;
        } else {
            this.widget.pending_spawn = true;
            this.ctx.request_anim_frame();
        }
    }

    fn close_layer(this: &mut WidgetMut<'_, Self>) {
        this.widget.pending_spawn = false;
        if let Some(id) = this.widget.layer_id.take() {
            this.ctx.mutate_later(id, |mut layer| {
                layer.downcast::<PopupLayer>().widget.dismissed = true;
            });
            this.ctx.remove_layer(id);
        }
    }
}

impl Widget for AnchoredPopupHost {
    type Action = PopupAction;

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) && self.open && self.pending_spawn {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _interval: u64,
    ) {
        if self.pending_spawn && self.open {
            self.pending_spawn = false;
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<PopupLayer>().widget.dismissed = true;
                });
                ctx.remove_layer(old_id);
            }
            let layer = PopupLayer::from_spec(ctx.widget_id(), &self.spec);
            self.layer_id = Some(layer.id());
            let border_box = ctx.border_box();
            let pos = ctx.to_window(border_box.origin())
                + Vec2::new(0.0, border_box.height())
                + self.offset;
            ctx.create_layer(LayerType::Other, layer, pos);
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.trigger);
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        _len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        ctx.redirect_measurement(&mut self.trigger, axis, cross_length)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.trigger, size);
        ctx.place_child(&mut self.trigger, Point::ORIGIN);
        ctx.derive_baselines(&self.trigger);
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
        ChildrenIds::from_slice(&[self.trigger.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("AnchoredPopupHost", id = id.trace())
    }
}

/// Callback invoked when a popup menu item is activated by index.
pub type PopupSelectCallback<State, Action> =
    Arc<dyn Fn(&mut State, usize) -> Action + Send + Sync>;

/// Xilem [`View`] wrapper pairing a trigger view with an [`AnchoredPopupHost`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic trigger View and closure trait objects"
)]
pub struct AnchoredPopupView<TriggerV, State, Action> {
    /// Anchor trigger view.
    pub trigger: TriggerV,
    /// Whether the anchored popup layer is currently displayed.
    pub open: bool,
    /// Spatial offset relative to the trigger's bottom-left anchor point.
    pub offset: Vec2,
    /// Theme-aware popup spec constructor.
    pub spec_builder: Arc<dyn Fn(&DesignSpec) -> PopupLayerSpec + Send + Sync>,
    /// Selection callback invoked with clicked item's `action_idx`.
    pub on_select: PopupSelectCallback<State, Action>,
    /// Dismiss callback invoked on outside click, Escape, or window focus loss.
    pub on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
    /// State/Action marker.
    pub phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<TriggerV, State, Action> ViewMarker for AnchoredPopupView<TriggerV, State, Action> {}

impl<TriggerV, State, Action> View<State, Action, ViewCtx>
    for AnchoredPopupView<TriggerV, State, Action>
where
    TriggerV: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<AnchoredPopupHost>;
    type ViewState = TriggerV::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec_theme = current_spec(ctx);
        let popup_spec = (self.spec_builder)(&spec_theme);
        let (trigger_pod, trigger_state) =
            ctx.with_id(POPUP_TRIGGER_ID, |ctx| self.trigger.build(ctx, app_state));
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(AnchoredPopupHost::new(
                trigger_pod.new_widget,
                self.open,
                self.offset,
                popup_spec,
            ))
        });
        (pod, trigger_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        {
            let mut trigger = AnchoredPopupHost::trigger_mut(&mut element);
            ctx.with_id(POPUP_TRIGGER_ID, |ctx| {
                self.trigger.rebuild(
                    &prev.trigger,
                    view_state,
                    ctx,
                    trigger.downcast(),
                    app_state,
                );
            });
        }
        let spec_theme = current_spec(ctx);
        let popup_spec = (self.spec_builder)(&spec_theme);
        AnchoredPopupHost::sync(&mut element, self.open, self.offset, popup_spec);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        AnchoredPopupHost::close_layer(&mut element);
        {
            let mut trigger = AnchoredPopupHost::trigger_mut(&mut element);
            ctx.with_id(POPUP_TRIGGER_ID, |ctx| {
                self.trigger.teardown(view_state, ctx, trigger.downcast());
            });
        }
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(POPUP_TRIGGER_ID) => {
                let mut trigger = AnchoredPopupHost::trigger_mut(&mut element);
                self.trigger
                    .message(view_state, message, trigger.downcast(), app_state)
            }
            None => match message.take_message::<PopupAction>() {
                Some(action) => match *action {
                    PopupAction::Selected(idx) => {
                        MessageResult::Action((self.on_select)(app_state, idx))
                    }
                    PopupAction::Dismissed => MessageResult::Action((self.on_dismiss)(app_state)),
                },
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}
