//! Anchored floating popup layer (`PopupLayer`) and trigger host (`AnchoredPopupHost`) for menus,
//! exposed dropdown selects, and interactive rich tooltips.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, ActionCtx, ArcStr, ChildrenIds, ErasedAction, EventCtx, Layer,
    LayerType, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx, PointerButton,
    PointerButtonEvent, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, StyleProperty,
    TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Rect, Size, Vec2};
use masonry::layout::{AsUnit, LayoutSize, LenReq, Length, SizeDef};
use masonry::parley::style::FontWeight;
use masonry::peniko::Color;
use masonry::properties::types::CrossAxisAlignment;
use masonry::properties::{
    Background, BorderColor, BorderWidth, ContentColor, CornerRadius, Dimensions, Gap, Padding,
};
use masonry::widgets::{self, ButtonPress, Divider, Flex, Label, SizedBox, Svg};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::foundation::icon::cached_icon_tree;
use crate::style_engine::scope::current_spec_arc;
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
    button_map: Vec<(WidgetId, usize, bool)>,
    highlighted: Option<usize>,
    min_width: f64,
    max_width: f64,
    anchor_rect: Rect,
    offset: Vec2,
    placed_pos: Point,
    window_size: Size,
    layer_size: Size,
    needs_reposition: bool,
    dismissed: bool,
}

impl PopupLayer {
    fn from_spec(
        creator: WidgetId,
        spec: &PopupLayerSpec,
        anchor_rect: Rect,
        offset: Vec2,
        placed_pos: Point,
    ) -> (NewWidget<Self>, Vec<(WidgetId, usize, bool)>) {
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
                    button_map.push((btn_id, *action_idx, *disabled));
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
                    masonry::layout::Dim::MaxContent,
                    masonry::layout::Dim::MaxContent,
                ),
            ));

        let widget = NewWidget::new(Self {
            creator,
            child: card.to_pod(),
            button_map: button_map.clone(),
            highlighted: None,
            min_width: spec.min_width,
            max_width: spec.max_width.max(spec.min_width),
            anchor_rect,
            offset,
            placed_pos,
            window_size: Size::ZERO,
            layer_size: Size::ZERO,
            needs_reposition: true,
            dismissed: false,
        });
        (widget, button_map)
    }

    fn enabled_slot_indices(&self) -> Vec<usize> {
        self.button_map
            .iter()
            .enumerate()
            .filter_map(|(slot, &(_, _, disabled))| (!disabled).then_some(slot))
            .collect()
    }

    fn select_slot(&mut self, next_slot: Option<usize>, ctx: &mut EventCtx<'_>) {
        if let Some(slot) = next_slot
            && let Some(&(btn_id, _, false)) = self.button_map.get(slot)
        {
            self.highlighted = Some(slot);
            ctx.set_focus(btn_id);
        }
    }

    fn activate_index(&mut self, ctx: &mut EventCtx<'_>, idx: usize) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        let self_id = ctx.widget_id();
        ctx.remove_layer(self_id);
        ctx.mutate_later(self.creator, move |host| {
            clear_creator_layer(host, self_id, Some(PopupAction::Selected(idx)));
        });
    }

    fn dismiss_layer(&mut self, ctx: &mut EventCtx<'_>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        let self_id = ctx.widget_id();
        ctx.remove_layer(self_id);
        ctx.mutate_later(self.creator, move |host| {
            clear_creator_layer(host, self_id, Some(PopupAction::Dismissed));
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
        let current_slot = ctx
            .focus_target_id()
            .and_then(|fid| self.button_map.iter().position(|(id, _, _)| *id == fid))
            .or(self.highlighted);

        match event {
            TextEvent::Keyboard(key_event) if !key_event.state.is_up() => match &key_event.key {
                Key::Named(NamedKey::Escape) => {
                    ctx.set_handled();
                    self.dismiss_layer(ctx);
                }
                Key::Named(NamedKey::ArrowDown) => {
                    let enabled = self.enabled_slot_indices();
                    let next = if enabled.is_empty() {
                        None
                    } else if let Some(cur) = current_slot
                        && let Some(pos) = enabled.iter().position(|&s| s == cur)
                    {
                        Some(enabled[(pos + 1) % enabled.len()])
                    } else {
                        enabled.first().copied()
                    };
                    self.select_slot(next, ctx);
                    ctx.set_handled();
                }
                Key::Named(NamedKey::ArrowUp) => {
                    let enabled = self.enabled_slot_indices();
                    let prev = if enabled.is_empty() {
                        None
                    } else if let Some(cur) = current_slot
                        && let Some(pos) = enabled.iter().position(|&s| s == cur)
                    {
                        let p = if pos == 0 { enabled.len() - 1 } else { pos - 1 };
                        Some(enabled[p])
                    } else {
                        enabled.last().copied()
                    };
                    self.select_slot(prev, ctx);
                    ctx.set_handled();
                }
                Key::Named(NamedKey::Home) => {
                    let first = self.enabled_slot_indices().first().copied();
                    self.select_slot(first, ctx);
                    ctx.set_handled();
                }
                Key::Named(NamedKey::End) => {
                    let last = self.enabled_slot_indices().last().copied();
                    self.select_slot(last, ctx);
                    ctx.set_handled();
                }
                Key::Named(NamedKey::Enter) => {
                    if ctx.is_focus_target()
                        && let Some(slot) =
                            current_slot.or_else(|| self.enabled_slot_indices().first().copied())
                        && let Some(&(_, action_idx, false)) = self.button_map.get(slot)
                    {
                        ctx.set_handled();
                        self.activate_index(ctx, action_idx);
                    }
                }
                Key::Character(c) if c == " " => {
                    if ctx.is_focus_target()
                        && let Some(slot) =
                            current_slot.or_else(|| self.enabled_slot_indices().first().copied())
                        && let Some(&(_, action_idx, false)) = self.button_map.get(slot)
                    {
                        ctx.set_handled();
                        self.activate_index(ctx, action_idx);
                    }
                }
                _ => {}
            },
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
            && let Some(&(_, idx, _)) = self.button_map.iter().find(|(id, _, _)| *id == source)
        {
            self.dismissed = true;
            ctx.set_handled();
            let self_id = ctx.widget_id();
            ctx.remove_layer(self_id);
            ctx.mutate_later(self.creator, move |host| {
                clear_creator_layer(host, self_id, Some(PopupAction::Selected(idx)));
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
                self.offset,
            );
            if (adjusted.x - self.placed_pos.x).abs() > 0.5
                || (adjusted.y - self.placed_pos.y).abs() > 0.5
            {
                self.placed_pos = adjusted;
                ctx.reposition_layer(ctx.widget_id(), adjusted);
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
        let len = ctx.compute_length(
            &mut self.child,
            len_req.into(),
            LayoutSize::maybe(axis.cross(), cross_length),
            axis,
            cross_length,
        );
        if axis == Axis::Horizontal {
            let vp_cap = if self.window_size.width > 0.0 {
                (self.window_size.width - 16.0).max(0.0)
            } else {
                self.max_width
            };
            let max_w = self.max_width.min(vp_cap);
            let min_w = self.min_width.min(max_w);
            Length::px(len.get().clamp(min_w, max_w))
        } else if self.window_size.height > 0.0 {
            let max_h = (self.window_size.height - 16.0).max(0.0);
            Length::px(len.get().min(max_h))
        } else {
            len
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let vp_w = if self.window_size.width > 0.0 {
            size.width.min((self.window_size.width - 16.0).max(0.0))
        } else {
            size.width
        };
        let vp_h = if self.window_size.height > 0.0 {
            size.height.min((self.window_size.height - 16.0).max(0.0))
        } else {
            size.height
        };
        let bounded = Size::new(vp_w, vp_h);
        let fit = ctx.compute_size(&mut self.child, SizeDef::fit(bounded), bounded.into());
        let child_size = Size::new(
            fit.width.max(self.min_width.min(vp_w)).min(vp_w),
            fit.height.min(vp_h),
        );
        ctx.run_layout(&mut self.child, child_size);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.derive_baselines(&self.child);
        self.layer_size = child_size;
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
    layer_buttons: Vec<(WidgetId, usize, bool)>,
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
            layer_buttons: Vec::new(),
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
        if this.widget.open != open {
            this.ctx.request_accessibility_update();
        }
        this.widget.open = open;
        this.widget.offset = offset;
        this.widget.spec = spec;

        if !open || this.ctx.is_stashed() {
            Self::close_layer(this);
            return;
        }

        this.widget.layer_buttons.clear();
        Self::remove_active_layer_with(this.widget.layer_id.take(), |id| {
            this.ctx.mutate_later(id, |mut layer| {
                layer.downcast::<PopupLayer>().widget.dismissed = true;
            });
            this.ctx.remove_layer(id);
        });
        let border_box = this.ctx.border_box();
        if border_box.size() != Size::ZERO {
            let anchor_origin = this.ctx.to_window(border_box.origin());
            let anchor_rect = Rect::from_origin_size(anchor_origin, border_box.size());
            let pos = anchor_origin + Vec2::new(0.0, border_box.height()) + offset;
            let (layer, buttons) = PopupLayer::from_spec(
                this.ctx.widget_id(),
                &this.widget.spec,
                anchor_rect,
                offset,
                pos,
            );
            this.widget.layer_id = Some(layer.id());
            this.widget.layer_buttons = buttons;
            this.ctx.create_layer(LayerType::Other, layer, pos);
            this.widget.pending_spawn = false;
        } else {
            this.widget.pending_spawn = true;
            this.ctx.request_anim_frame();
        }
    }

    fn close_layer(this: &mut WidgetMut<'_, Self>) {
        this.widget.pending_spawn = false;
        this.widget.layer_buttons.clear();
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
        match event {
            Update::WidgetAdded if self.open && self.pending_spawn && !ctx.is_stashed() => {
                ctx.request_anim_frame();
            }
            Update::StashedChanged(true) => {
                self.pending_spawn = false;
                self.layer_buttons.clear();
                if let Some(old_id) = self.layer_id.take() {
                    ctx.mutate_later(old_id, |mut layer| {
                        layer.downcast::<PopupLayer>().widget.dismissed = true;
                    });
                    ctx.remove_layer(old_id);
                }
            }
            Update::StashedChanged(false) if self.open && self.layer_id.is_none() => {
                self.pending_spawn = true;
                ctx.request_anim_frame();
            }
            _ => {}
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if !self.open {
            return;
        }
        let Some(layer_id) = self.layer_id else {
            return;
        };
        if let TextEvent::Keyboard(key_event) = event
            && !key_event.state.is_up()
        {
            match &key_event.key {
                Key::Named(NamedKey::Escape) => {
                    ctx.mutate_later(layer_id, |mut layer| {
                        layer.downcast::<PopupLayer>().widget.dismissed = true;
                    });
                    ctx.remove_layer(layer_id);
                    self.layer_id = None;
                    self.layer_buttons.clear();
                    self.pending_spawn = false;
                    ctx.submit_action::<PopupAction>(PopupAction::Dismissed);
                    ctx.set_handled();
                }
                Key::Named(NamedKey::ArrowDown | NamedKey::Home) => {
                    if let Some(&(btn_id, _, false)) =
                        self.layer_buttons.iter().find(|(_, _, d)| !*d)
                    {
                        ctx.set_focus(btn_id);
                        ctx.set_handled();
                    }
                }
                Key::Named(NamedKey::ArrowUp | NamedKey::End) => {
                    if let Some(&(btn_id, _, false)) =
                        self.layer_buttons.iter().rev().find(|(_, _, d)| !*d)
                    {
                        ctx.set_focus(btn_id);
                        ctx.set_handled();
                    }
                }
                _ => {}
            }
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _interval: u64,
    ) {
        if ctx.is_stashed() {
            self.pending_spawn = false;
            self.layer_buttons.clear();
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<PopupLayer>().widget.dismissed = true;
                });
                ctx.remove_layer(old_id);
            }
            return;
        }
        if self.pending_spawn && self.open {
            let border_box = ctx.border_box();
            if border_box.size() == Size::ZERO {
                return;
            }
            self.pending_spawn = false;
            self.layer_buttons.clear();
            if let Some(old_id) = self.layer_id.take() {
                ctx.mutate_later(old_id, |mut layer| {
                    layer.downcast::<PopupLayer>().widget.dismissed = true;
                });
                ctx.remove_layer(old_id);
            }
            let anchor_origin = ctx.to_window(border_box.origin());
            let anchor_rect = Rect::from_origin_size(anchor_origin, border_box.size());
            let pos = anchor_origin + Vec2::new(0.0, border_box.height()) + self.offset;
            let (layer, buttons) =
                PopupLayer::from_spec(ctx.widget_id(), &self.spec, anchor_rect, self.offset, pos);
            self.layer_id = Some(layer.id());
            self.layer_buttons = buttons;
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
        node: &mut Node,
    ) {
        node.set_expanded(self.open);
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

/// Brocado [`View`] wrapper pairing a trigger view with an [`AnchoredPopupHost`].
#[must_use = "View values do nothing unless provided to Brocado."]
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
        let spec_theme = current_spec_arc(ctx);
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
        let spec_theme = current_spec_arc(ctx);
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

fn clear_creator_layer(
    mut host: WidgetMut<'_, dyn Widget>,
    self_id: WidgetId,
    action: Option<PopupAction>,
) {
    if let Some(mut anchored) = host.try_downcast::<AnchoredPopupHost>() {
        if anchored.widget.layer_id == Some(self_id) {
            anchored.widget.layer_id = None;
            anchored.widget.layer_buttons.clear();
        }
        match action {
            Some(PopupAction::Selected(idx)) => {
                anchored
                    .ctx
                    .submit_action::<PopupAction>(PopupAction::Selected(idx));
            }
            Some(PopupAction::Dismissed) if anchored.widget.open => {
                anchored
                    .ctx
                    .submit_action::<PopupAction>(PopupAction::Dismissed);
            }
            Some(PopupAction::Dismissed) | None => {}
        }
    } else if let Some(mut ctx_menu) = host.try_downcast::<ContextMenuHost>() {
        if ctx_menu.widget.layer_id == Some(self_id) {
            ctx_menu.widget.layer_id = None;
            ctx_menu.widget.layer_buttons.clear();
        }
        if let Some(act) = action {
            ctx_menu.ctx.submit_action::<PopupAction>(act);
        }
    }
}

/// Masonry host widget spawning a floating context menu [`PopupLayer`] on secondary click or keyboard shortcut.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod and PopupLayerSpec"
)]
pub struct ContextMenuHost {
    target: WidgetPod<dyn Widget>,
    spec: PopupLayerSpec,
    layer_id: Option<WidgetId>,
    layer_buttons: Vec<(WidgetId, usize, bool)>,
}

impl ContextMenuHost {
    fn new(target: NewWidget<impl Widget + ?Sized>, spec: PopupLayerSpec) -> Self {
        Self {
            target: target.erased().to_pod(),
            spec,
            layer_id: None,
            layer_buttons: Vec::new(),
        }
    }

    fn target_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.target)
    }

    /// Returns the active Masonry `Layer` root ID when the context menu is currently open.
    #[must_use]
    pub fn layer_id(&self) -> Option<WidgetId> {
        self.layer_id
    }

    fn spawn_at_event(&mut self, ctx: &mut EventCtx<'_>, window_pos: Point) {
        if ctx.is_stashed() {
            return;
        }
        if let Some(old_id) = self.layer_id.take() {
            ctx.mutate_later(old_id, |mut layer| {
                layer.downcast::<PopupLayer>().widget.dismissed = true;
            });
            ctx.remove_layer(old_id);
        }
        let anchor_rect = Rect::from_origin_size(window_pos, Size::ZERO);
        let (layer, buttons) = PopupLayer::from_spec(
            ctx.widget_id(),
            &self.spec,
            anchor_rect,
            Vec2::ZERO,
            window_pos,
        );
        self.layer_id = Some(layer.id());
        self.layer_buttons = buttons;
        ctx.create_layer(LayerType::Other, layer, window_pos);
    }

    fn close_layer_with_update(this: &mut WidgetMut<'_, Self>) {
        this.widget.layer_buttons.clear();
        if let Some(id) = this.widget.layer_id.take() {
            this.ctx.mutate_later(id, |mut layer| {
                layer.downcast::<PopupLayer>().widget.dismissed = true;
            });
            this.ctx.remove_layer(id);
        }
    }
}

impl Widget for ContextMenuHost {
    type Action = PopupAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.target);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if let PointerEvent::Down(btn_ev) = event
            && btn_ev.button == Some(PointerButton::Secondary)
        {
            let window_pos = ctx.to_window(ctx.local_position(btn_ev.state.position));
            self.spawn_at_event(ctx, window_pos);
            ctx.set_handled();
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(kev) = event
            && kev.state.is_down()
        {
            if kev.key == Key::Named(NamedKey::ContextMenu)
                || (kev.modifiers.shift() && kev.key == Key::Named(NamedKey::F10))
            {
                let center = ctx.to_window(ctx.border_box().center());
                self.spawn_at_event(ctx, center);
                ctx.set_handled();
                return;
            }
            if let Some(layer_id) = self.layer_id {
                match &kev.key {
                    Key::Named(NamedKey::Escape) => {
                        ctx.mutate_later(layer_id, |mut layer| {
                            layer.downcast::<PopupLayer>().widget.dismissed = true;
                        });
                        ctx.remove_layer(layer_id);
                        self.layer_id = None;
                        self.layer_buttons.clear();
                        ctx.submit_action::<PopupAction>(PopupAction::Dismissed);
                        ctx.set_handled();
                    }
                    Key::Named(NamedKey::ArrowDown | NamedKey::Home) => {
                        if let Some(&(btn_id, _, false)) =
                            self.layer_buttons.iter().find(|(_, _, d)| !*d)
                        {
                            ctx.set_focus(btn_id);
                            ctx.set_handled();
                        }
                    }
                    Key::Named(NamedKey::ArrowUp | NamedKey::End) => {
                        if let Some(&(btn_id, _, false)) =
                            self.layer_buttons.iter().rev().find(|(_, _, d)| !*d)
                        {
                            ctx.set_focus(btn_id);
                            ctx.set_handled();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::StashedChanged(true)) {
            self.layer_buttons.clear();
            if let Some(id) = self.layer_id.take() {
                ctx.mutate_later(id, |mut layer| {
                    layer.downcast::<PopupLayer>().widget.dismissed = true;
                });
                ctx.remove_layer(id);
            }
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
        ctx.redirect_measurement(&mut self.target, axis, cross_length)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.target, size);
        ctx.place_child(&mut self.target, Point::ORIGIN);
        ctx.derive_baselines(&self.target);
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
        ChildrenIds::from_slice(&[self.target.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("ContextMenuHost", id = id.trace())
    }
}

const CONTEXT_TARGET_ID: ViewId = ViewId::new(0x5549_3430);

/// Callback type invoked when a popup or context menu dismisses without selection.
pub type PopupDismissCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Brocado [`View`] wrapper opening an anchored floating [`PopupLayer`] on secondary mouse click (`Right-Click`) or `ContextMenu`/`Shift+F10`.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Contains generic target View and closure trait objects"
)]
pub struct ContextMenuView<TargetV, State, Action> {
    /// Wrapped target subtree that opens the context menu on secondary click.
    pub target: TargetV,
    /// Theme-aware popup spec constructor.
    pub spec_builder: Arc<dyn Fn(&DesignSpec) -> PopupLayerSpec + Send + Sync>,
    /// Selection callback invoked with clicked item's `action_idx`.
    pub on_select: PopupSelectCallback<State, Action>,
    /// Optional dismiss callback invoked when dismissed without selecting an item.
    pub on_dismiss: Option<PopupDismissCallback<State, Action>>,
    /// State/Action marker.
    pub phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<TargetV, State, Action> ContextMenuView<TargetV, State, Action> {
    /// Attaches an optional callback invoked when the context menu closes without item selection.
    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        self.on_dismiss = Some(Arc::new(on_dismiss));
        self
    }
}

impl<TargetV, State, Action> ViewMarker for ContextMenuView<TargetV, State, Action> {}

impl<TargetV, State, Action> View<State, Action, ViewCtx>
    for ContextMenuView<TargetV, State, Action>
where
    TargetV: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<ContextMenuHost>;
    type ViewState = TargetV::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec_theme = current_spec_arc(ctx);
        let popup_spec = (self.spec_builder)(&spec_theme);
        let (target_pod, target_state) =
            ctx.with_id(CONTEXT_TARGET_ID, |ctx| self.target.build(ctx, app_state));
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(ContextMenuHost::new(target_pod.new_widget, popup_spec))
        });
        (pod, target_state)
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
            let mut target = ContextMenuHost::target_mut(&mut element);
            ctx.with_id(CONTEXT_TARGET_ID, |ctx| {
                self.target
                    .rebuild(&prev.target, view_state, ctx, target.downcast(), app_state);
            });
        }
        let spec_theme = current_spec_arc(ctx);
        let popup_spec = (self.spec_builder)(&spec_theme);
        element.widget.spec = popup_spec;
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        ContextMenuHost::close_layer_with_update(&mut element);
        {
            let mut target = ContextMenuHost::target_mut(&mut element);
            ctx.with_id(CONTEXT_TARGET_ID, |ctx| {
                self.target.teardown(view_state, ctx, target.downcast());
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
            Some(CONTEXT_TARGET_ID) => {
                let mut target = ContextMenuHost::target_mut(&mut element);
                self.target
                    .message(view_state, message, target.downcast(), app_state)
            }
            None => match message.take_message::<PopupAction>() {
                Some(action) => match *action {
                    PopupAction::Selected(idx) => {
                        MessageResult::Action((self.on_select)(app_state, idx))
                    }
                    PopupAction::Dismissed => {
                        if let Some(cb) = &self.on_dismiss {
                            MessageResult::Action(cb(app_state))
                        } else {
                            MessageResult::Nop
                        }
                    }
                },
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}
