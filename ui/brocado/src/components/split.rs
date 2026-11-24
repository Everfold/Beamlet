//! Platform-themed draggable split-pane container (`NSSplitView` / `GtkPaned` / `SplitView`)
//! with per-pane clipping, spec-driven colors, and extreme-size proportional clamping.
//!
//! Upstream provenance: axis splitting geometry and `SplitPoint` clamping model are adapted from
//! upstream `masonry::widgets::Split` (`linebender/xilem`, `masonry/src/widgets/split.rs`),
//! rewritten with theme-driven `SplitVisualTheme`, per-pane `SplitPaneClip`, and proportional min-length clamping.

use std::marker::PhantomData;

use masonry::accesskit::{ActionData, Node, Orientation, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, ChildrenIds, CursorIcon, EventCtx, FromDynWidget, LayoutCtx,
    MeasureCtx, NewWidget, NoAction, PaintCtx, PointerButton, PointerButtonEvent, PointerEvent,
    PointerUpdate, PropertiesMut, PropertiesRef, QueryCtx, RegisterCtx, TextEvent, Update,
    UpdateCtx, Widget, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Line, Point, Rect, RoundedRect, Size, Stroke};
use masonry::layout::{AsUnit, LayoutSize, LenReq, Length};
use masonry::peniko::Color;
use masonry::widgets::SplitPoint;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::foundation::with_opacity;
use crate::style_engine::current_theme_arc;
use crate::theme::Theme;

const SPLIT_CHILD1_ID: ViewId = ViewId::new(0x5549_3450);
const SPLIT_CHILD2_ID: ViewId = ViewId::new(0x5549_3451);

/// Material Design 3 splitter styling for [`UiSplitWidget`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct SplitVisualTheme {
    bar_thickness: f64,
    min_bar_area: f64,
    solid: bool,
    idle_color: Color,
    hover_color: Color,
    active_color: Color,
    disabled_color: Color,
    track_color: Color,
}

impl SplitVisualTheme {
    fn from_theme(theme: &Theme) -> Self {
        let s = &theme.scheme;
        Self {
            bar_thickness: 4.0,
            min_bar_area: 10.0,
            solid: false,
            idle_color: s.outline,
            hover_color: s.on_surface_variant,
            active_color: s.primary,
            disabled_color: with_opacity(s.outline_variant, 0.5),
            track_color: s.outline_variant,
        }
    }
}

/// Internal per-pane clip wrapper ensuring children never paint or receive hits across the splitter bar.
#[expect(
    missing_debug_implementations,
    reason = "Wraps a generic Masonry WidgetPod"
)]
pub struct SplitPaneClip<W: Widget + ?Sized> {
    child: WidgetPod<W>,
}

impl<W: Widget + FromDynWidget + ?Sized> SplitPaneClip<W> {
    fn new(child: NewWidget<W>) -> Self {
        Self {
            child: child.to_pod(),
        }
    }
}

impl<W: Widget + ?Sized> Widget for SplitPaneClip<W> {
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
        ctx.compute_length(
            &mut self.child,
            len_req.into(),
            LayoutSize::maybe(axis.cross(), cross_length),
            axis,
            cross_length,
        )
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let clamped = Size::new(size.width.max(0.0), size.height.max(0.0));
        ctx.run_layout(&mut self.child, clamped);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.set_clip_path(Rect::from_origin_size(Point::ORIGIN, clamped));
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
}

/// Masonry widget dividing space between two clipped panes with a draggable, spec-themed divider bar.
#[expect(
    missing_debug_implementations,
    reason = "Holds generic Masonry WidgetPods"
)]
pub struct UiSplitWidget<ChildA: Widget + ?Sized, ChildB: Widget + ?Sized> {
    split_axis: Axis,
    split_point_chosen: SplitPoint,
    split_point_effective: f64,
    min_lengths: (Length, Length),
    draggable: bool,
    dragging: bool,
    click_offset: f64,
    visual: SplitVisualTheme,
    child1: WidgetPod<SplitPaneClip<ChildA>>,
    child2: WidgetPod<SplitPaneClip<ChildB>>,
}

impl<ChildA: Widget + FromDynWidget + ?Sized, ChildB: Widget + FromDynWidget + ?Sized>
    UiSplitWidget<ChildA, ChildB>
{
    fn new(
        child1: NewWidget<ChildA>,
        child2: NewWidget<ChildB>,
        split_axis: Axis,
        split_point: SplitPoint,
        min_lengths: (Length, Length),
        draggable: bool,
        visual: SplitVisualTheme,
    ) -> Self {
        Self {
            split_axis,
            split_point_chosen: clamp_split_point(split_point),
            split_point_effective: 0.5,
            min_lengths,
            draggable,
            dragging: false,
            click_offset: 0.0,
            visual,
            child1: NewWidget::new(SplitPaneClip::new(child1)).to_pod(),
            child2: NewWidget::new(SplitPaneClip::new(child2)).to_pod(),
        }
    }

    fn with_child1_mut<R>(
        this: &mut WidgetMut<'_, Self>,
        f: impl FnOnce(WidgetMut<'_, ChildA>) -> R,
    ) -> R {
        let mut clip = this.ctx.get_mut(&mut this.widget.child1);
        f(clip.ctx.get_mut(&mut clip.widget.child))
    }

    fn with_child2_mut<R>(
        this: &mut WidgetMut<'_, Self>,
        f: impl FnOnce(WidgetMut<'_, ChildB>) -> R,
    ) -> R {
        let mut clip = this.ctx.get_mut(&mut this.widget.child2);
        f(clip.ctx.get_mut(&mut clip.widget.child))
    }

    fn set_visual(this: &mut WidgetMut<'_, Self>, visual: SplitVisualTheme) {
        if this.widget.visual != visual {
            let layout_changed = (this.widget.visual.bar_thickness - visual.bar_thickness).abs()
                > f64::EPSILON
                || (this.widget.visual.min_bar_area - visual.min_bar_area).abs() > f64::EPSILON;
            this.widget.visual = visual;
            if layout_changed {
                this.ctx.request_layout();
            } else {
                this.ctx.request_paint_only();
            }
        }
    }
}

fn clamp_split_point(sp: SplitPoint) -> SplitPoint {
    match sp {
        SplitPoint::Fraction(frac) => SplitPoint::Fraction(frac.clamp(0.0, 1.0)),
        other => other,
    }
}

impl<ChildA: Widget + ?Sized, ChildB: Widget + ?Sized> UiSplitWidget<ChildA, ChildB> {
    #[inline]
    fn bar_area(&self) -> f64 {
        self.visual.bar_thickness.max(self.visual.min_bar_area)
    }

    fn effective_bar_thickness(&self, length: f64) -> f64 {
        self.visual.bar_thickness.min(length.max(0.0))
    }

    fn bar_edges(&self, length: f64) -> (f64, f64) {
        let bar_t = self.effective_bar_thickness(length);
        let split_space = (length - bar_t).max(0.0);
        let edge1 = (split_space * self.split_point_effective).clamp(0.0, split_space);
        (edge1, edge1 + bar_t)
    }

    fn bar_center(&self, length: f64) -> f64 {
        let (e1, e2) = self.bar_edges(length);
        (e1 + e2) * 0.5
    }

    fn bar_area_edges(&self, length: f64) -> (f64, f64) {
        let (edge1, edge2) = self.bar_edges(length);
        let bar_t = edge2 - edge1;
        let hit_w = self.bar_area().min(length.max(0.0)).max(bar_t);
        let pad = (hit_w - bar_t).max(0.0);
        let space1 = edge1.max(0.0);
        let space2 = (length - edge2).max(0.0);
        let pad1 = (0.5 * pad).min(space1);
        let pad2 = (pad - pad1).min(space2);
        let pad1 = (pad - pad2).min(space1);
        (edge1 - pad1, edge2 + pad2)
    }

    fn bar_area_hit_test(&self, length: f64, pos: f64) -> bool {
        let (e1, e2) = self.bar_area_edges(length);
        pos >= e1 && pos <= e2
    }

    fn split_side_limits(&self, split_space: f64) -> (f64, f64) {
        let split_space = split_space.max(0.0);
        let m1 = self.min_lengths.0.get().max(0.0);
        let m2 = self.min_lengths.1.get().max(0.0);
        let total_min = m1 + m2;
        if total_min > split_space {
            if split_space <= f64::EPSILON || total_min <= f64::EPSILON {
                let half = split_space * 0.5;
                return (half, half);
            }
            // Scale constraints proportionally when total min lengths exceed tight viewport width.
            let share = (split_space * (m1 / total_min)).clamp(0.0, split_space);
            return (share, share);
        }
        (m1, (split_space - m2).max(m1))
    }

    fn calc_effective_split_point(&self, split_space: f64) -> f64 {
        if split_space <= f64::EPSILON {
            return 0.5;
        }
        let (min_limit, max_limit) = self.split_side_limits(split_space);
        let child1_len = match self.split_point_chosen {
            SplitPoint::Fraction(frac) => split_space * frac.clamp(0.0, 1.0),
            SplitPoint::FromStart(len) => len.get().max(0.0),
            SplitPoint::FromEnd(len) => split_space - len.get().max(0.0),
        };
        (child1_len.clamp(min_limit, max_limit) / split_space).clamp(0.0, 1.0)
    }

    fn set_chosen_from_child1_len(&mut self, split_space: f64, child1_len: f64) {
        let (min_limit, max_limit) = self.split_side_limits(split_space);
        let clamped = child1_len.clamp(min_limit, max_limit);
        match self.split_point_chosen {
            SplitPoint::Fraction(_) => {
                self.split_point_chosen = SplitPoint::Fraction(if split_space <= f64::EPSILON {
                    0.5
                } else {
                    (clamped / split_space).clamp(0.0, 1.0)
                });
            }
            SplitPoint::FromStart(_) => {
                self.split_point_chosen = SplitPoint::FromStart(Length::px(clamped));
            }
            SplitPoint::FromEnd(_) => {
                let child2_len = (split_space - clamped).max(0.0);
                self.split_point_chosen = SplitPoint::FromEnd(Length::px(child2_len));
            }
        }
    }

    fn update_from_bar_center(&mut self, total_length: f64, bar_center: f64) {
        let bar_t = self.effective_bar_thickness(total_length);
        let split_space = (total_length - bar_t).max(0.0);
        let child1_len = bar_center - bar_t * 0.5;
        self.set_chosen_from_child1_len(split_space, child1_len);
    }

    fn current_bar_color(&self, ctx: &PaintCtx<'_>) -> Color {
        if !self.draggable || ctx.is_disabled() {
            return self.visual.disabled_color;
        }
        if self.dragging || ctx.is_active() {
            self.visual.active_color
        } else if ctx.is_hovered() || ctx.is_focus_target() {
            self.visual.hover_color
        } else {
            self.visual.idle_color
        }
    }
}

impl<ChildA: Widget + ?Sized, ChildB: Widget + ?Sized> Widget for UiSplitWidget<ChildA, ChildB> {
    type Action = NoAction;

    fn accepts_focus(&self) -> bool {
        self.draggable
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child1);
        ctx.register_child(&mut self.child2);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if !self.draggable || ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                state,
                ..
            }) => {
                let pos = ctx
                    .local_position(state.position)
                    .get_coord(self.split_axis);
                let length = ctx.content_box().size().get_coord(self.split_axis);
                if self.bar_area_hit_test(length, pos) {
                    self.dragging = true;
                    self.click_offset = pos - self.bar_center(length);
                    ctx.set_handled();
                    ctx.capture_pointer();
                    ctx.request_focus();
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Move(PointerUpdate { current, .. })
                if self.dragging && ctx.is_active() =>
            {
                let pos = ctx
                    .local_position(current.position)
                    .get_coord(self.split_axis);
                let length = ctx.content_box().size().get_coord(self.split_axis);
                self.update_from_bar_center(length, pos - self.click_offset);
                ctx.request_layout();
                ctx.set_handled();
            }
            PointerEvent::Up(..) | PointerEvent::Cancel(..)
                if self.dragging || self.click_offset.abs() > 0.0 =>
            {
                self.dragging = false;
                self.click_offset = 0.0;
                ctx.request_paint_only();
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
        if ctx.is_disabled() || !ctx.is_focus_target() || !self.draggable {
            return;
        }
        let TextEvent::Keyboard(kev) = event else {
            return;
        };
        if !kev.state.is_down() {
            return;
        }
        let length = ctx.content_box().size().get_coord(self.split_axis);
        let split_space = (length - self.effective_bar_thickness(length)).max(0.0);
        if split_space <= f64::EPSILON {
            return;
        }
        let step = (split_space / 50.0).max(2.0);
        let delta = if kev.modifiers.shift() {
            step * 5.0
        } else {
            step
        };
        let mut child1_len = split_space * self.split_point_effective;
        match kev.key {
            Key::Named(NamedKey::ArrowLeft) if self.split_axis == Axis::Horizontal => {
                child1_len -= delta;
            }
            Key::Named(NamedKey::ArrowRight) if self.split_axis == Axis::Horizontal => {
                child1_len += delta;
            }
            Key::Named(NamedKey::ArrowUp) if self.split_axis == Axis::Vertical => {
                child1_len -= delta;
            }
            Key::Named(NamedKey::ArrowDown) if self.split_axis == Axis::Vertical => {
                child1_len += delta;
            }
            Key::Named(NamedKey::Home) => child1_len = self.split_side_limits(split_space).0,
            Key::Named(NamedKey::End) => child1_len = self.split_side_limits(split_space).1,
            _ => return,
        }
        self.set_chosen_from_child1_len(split_space, child1_len);
        ctx.request_layout();
        ctx.set_handled();
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() || !self.draggable {
            return;
        }
        let length = ctx.content_box().size().get_coord(self.split_axis);
        let split_space = (length - self.effective_bar_thickness(length)).max(0.0);
        if split_space <= f64::EPSILON {
            return;
        }
        let step = (split_space / 50.0).max(2.0);
        let mut child1_len = split_space * self.split_point_effective;
        match event.action {
            masonry::accesskit::Action::Increment => child1_len += step,
            masonry::accesskit::Action::Decrement => child1_len -= step,
            masonry::accesskit::Action::SetValue => match &event.data {
                Some(ActionData::NumericValue(v)) => child1_len = *v,
                Some(ActionData::Value(v)) => {
                    if let Ok(parsed) = v.parse() {
                        child1_len = parsed;
                    }
                }
                _ => return,
            },
            _ => return,
        }
        self.set_chosen_from_child1_len(split_space, child1_len);
        ctx.request_layout();
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::FocusChanged(false) => {
                self.dragging = false;
                self.click_offset = 0.0;
                ctx.request_paint_only();
            }
            Update::FocusChanged(true)
            | Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::DisabledChanged(_) => ctx.request_paint_only(),
            _ => {}
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
        if let LenReq::FitContent(space) = len_req {
            return if axis == self.split_axis {
                space.max(Length::px(self.visual.bar_thickness))
            } else {
                space
            };
        }
        let auto_len = len_req.into();
        let cross = axis.cross();
        let (c1_cross, c2_cross) = cross_length
            .map(|cl| {
                if cross == self.split_axis {
                    let cs = cl.saturating_sub(Length::px(self.visual.bar_thickness));
                    let sp = self.calc_effective_split_point(cs.get());
                    let c1 = (cs.get() * sp).px();
                    (c1, cs.saturating_sub(c1))
                } else {
                    (cl, cl)
                }
            })
            .unzip();
        let l1 = ctx.compute_length(
            &mut self.child1,
            auto_len,
            LayoutSize::maybe(cross, c1_cross),
            axis,
            c1_cross,
        );
        let l2 = ctx.compute_length(
            &mut self.child2,
            auto_len,
            LayoutSize::maybe(cross, c2_cross),
            axis,
            c2_cross,
        );
        if axis == self.split_axis {
            l1.saturating_add(l2)
                .saturating_add(Length::px(self.visual.bar_thickness))
        } else {
            l1.max(l2)
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let total = size.get_coord(self.split_axis).max(0.0);
        let bar_t = self.effective_bar_thickness(total);
        let split_space = (total - bar_t).max(0.0);
        let cross_space = size.get_coord(self.split_axis.cross()).max(0.0);

        self.split_point_effective = self.calc_effective_split_point(split_space);

        let c1_len = (split_space * self.split_point_effective).max(0.0);
        let c2_len = (split_space - c1_len).max(0.0);

        ctx.run_layout(
            &mut self.child1,
            self.split_axis.pack_size(c1_len, cross_space),
        );
        ctx.run_layout(
            &mut self.child2,
            self.split_axis.pack_size(c2_len, cross_space),
        );

        ctx.place_child(&mut self.child1, Point::ORIGIN);
        ctx.place_child(
            &mut self.child2,
            self.split_axis.pack_point(c1_len + bar_t, 0.0),
        );
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let size = ctx.content_box().size();
        let total = size.get_coord(self.split_axis);
        let cross = size.get_coord(self.split_axis.cross());
        if total <= 0.5 || cross <= 0.5 {
            return;
        }
        let (edge1, edge2) = self.bar_edges(total);
        let color = self.current_bar_color(ctx);
        let border_box = ctx.border_box();

        if self.visual.solid {
            let mut rect = border_box;
            rect.set_coords(self.split_axis, edge1, edge2);
            painter.fill(rect, color).draw();
        } else {
            // Draw hairline track along split seam + centered rounded drag handle pill.
            let mid = (edge1 + edge2) * 0.5;
            let track = Line::new(
                self.split_axis.pack_point(mid, 0.0),
                self.split_axis.pack_point(mid, cross),
            );
            painter
                .stroke(track, &Stroke::new(1.0), self.visual.track_color)
                .draw();

            let handle_len = (cross * 0.35).clamp(16.0_f64.min(cross), 36.0_f64.min(cross));
            let c0 = (cross - handle_len) * 0.5;
            let c1 = c0 + handle_len;
            let mut pill_rect = border_box;
            pill_rect.set_coords(self.split_axis, edge1, edge2);
            pill_rect.set_coords(self.split_axis.cross(), c0, c1);
            let r = ((edge2 - edge1) * 0.5).max(1.0);
            painter
                .fill(RoundedRect::from_rect(pill_rect, r), color)
                .draw();
        }

        if ctx.is_focus_target() && self.draggable && !ctx.is_disabled() {
            let center = (edge1 + edge2) * 0.5;
            let half = ((edge2 - edge1) * 0.5 + 1.5).min(total * 0.5);
            let mut focus_rect = border_box.inset(-0.5).intersect(border_box);
            focus_rect.set_coords(
                self.split_axis,
                (center - half).max(0.0),
                (center + half).min(total),
            );
            focus_rect.set_coords(self.split_axis.cross(), 1.0, (cross - 1.0).max(1.0));
            painter
                .stroke(focus_rect, &Stroke::new(1.5), self.visual.active_color)
                .draw();
        }
    }

    fn get_cursor(&self, ctx: &QueryCtx<'_>, pos: Point) -> CursorIcon {
        let length = ctx.content_box().size().get_coord(self.split_axis);
        let local_pos = ctx.to_local(pos).get_coord(self.split_axis);
        if self.draggable
            && !ctx.is_disabled()
            && (self.dragging || ctx.is_active() || self.bar_area_hit_test(length, local_pos))
        {
            match self.split_axis {
                Axis::Horizontal => CursorIcon::EwResize,
                Axis::Vertical => CursorIcon::NsResize,
            }
        } else {
            CursorIcon::Default
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Splitter
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        let length = ctx.content_box().size().get_coord(self.split_axis);
        let split_space = (length - self.effective_bar_thickness(length)).max(0.0);
        let (min_l, max_l) = self.split_side_limits(split_space);
        let cur = split_space * self.split_point_effective;
        node.set_orientation(match self.split_axis {
            Axis::Horizontal => Orientation::Horizontal,
            Axis::Vertical => Orientation::Vertical,
        });
        node.set_value(cur.to_string());
        node.set_numeric_value(cur);
        node.set_min_numeric_value(min_l);
        node.set_max_numeric_value(max_l);
        node.set_numeric_value_step((split_space / 50.0).max(1.0));
        if self.draggable && !ctx.is_disabled() {
            node.add_action(masonry::accesskit::Action::SetValue);
            node.add_action(masonry::accesskit::Action::Increment);
            node.add_action(masonry::accesskit::Action::Decrement);
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child1.id(), self.child2.id()])
    }
}

/// Draggable split view styling bar thickness, hit area, colors, and pane clipping from [`Theme`](crate::theme::Theme).
#[must_use = "View values do nothing unless provided to Brocado."]
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

impl<A, B, State, Action> ViewMarker for UiSplit<A, B, State, Action> {}

impl<A, B, State, Action> View<State, Action, ViewCtx> for UiSplit<A, B, State, Action>
where
    State: 'static,
    Action: 'static,
    A: WidgetView<State, Action>,
    B: WidgetView<State, Action>,
{
    type Element = Pod<UiSplitWidget<A::Widget, B::Widget>>;
    type ViewState = (A::ViewState, B::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let visual = SplitVisualTheme::from_theme(&theme);
        let (child1, st1) = ctx.with_id(SPLIT_CHILD1_ID, |ctx| self.first.build(ctx, app_state));
        let (child2, st2) = ctx.with_id(SPLIT_CHILD2_ID, |ctx| self.second.build(ctx, app_state));
        let pod = ctx.create_pod(UiSplitWidget::new(
            child1.new_widget,
            child2.new_widget,
            self.axis,
            self.split_point,
            self.min_lengths,
            self.draggable,
            visual,
        ));
        (pod, (st1, st2))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (st1, st2): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let theme = current_theme_arc(ctx);
        UiSplitWidget::set_visual(&mut element, SplitVisualTheme::from_theme(&theme));

        if prev.axis != self.axis {
            element.widget.split_axis = self.axis;
            element.ctx.request_layout();
        }
        if prev.split_point != self.split_point {
            element.widget.split_point_chosen = clamp_split_point(self.split_point);
            element.ctx.request_layout();
        }
        if prev.min_lengths != self.min_lengths {
            element.widget.min_lengths = self.min_lengths;
            element.ctx.request_layout();
        }
        if prev.draggable != self.draggable {
            element.widget.draggable = self.draggable;
            if !self.draggable {
                element.widget.dragging = false;
                element.widget.click_offset = 0.0;
            }
            element.ctx.request_accessibility_update();
            element.ctx.request_paint_only();
        }

        ctx.with_id(SPLIT_CHILD1_ID, |ctx| {
            UiSplitWidget::with_child1_mut(&mut element, |child1_el| {
                self.first
                    .rebuild(&prev.first, st1, ctx, child1_el, app_state);
            });
        });
        ctx.with_id(SPLIT_CHILD2_ID, |ctx| {
            UiSplitWidget::with_child2_mut(&mut element, |child2_el| {
                self.second
                    .rebuild(&prev.second, st2, ctx, child2_el, app_state);
            });
        });
    }

    fn teardown(
        &self,
        (st1, st2): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        UiSplitWidget::with_child1_mut(&mut element, |child1_el| {
            self.first.teardown(st1, ctx, child1_el);
        });
        UiSplitWidget::with_child2_mut(&mut element, |child2_el| {
            self.second.teardown(st2, ctx, child2_el);
        });
    }

    fn message(
        &self,
        (st1, st2): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(SPLIT_CHILD1_ID) => UiSplitWidget::with_child1_mut(&mut element, |child1_el| {
                self.first.message(st1, message, child1_el, app_state)
            }),
            Some(SPLIT_CHILD2_ID) => UiSplitWidget::with_child2_mut(&mut element, |child2_el| {
                self.second.message(st2, message, child2_el, app_state)
            }),
            _ => MessageResult::Stale,
        }
    }
}
