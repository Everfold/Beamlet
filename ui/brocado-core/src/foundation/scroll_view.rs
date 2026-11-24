//! Themeable scrollable viewport (`ScrollView`, `ScrollViewWidget`) and custom [`UiScrollBar`].
//!
//! Upstream provenance: Viewport range clamping (`compute_pan_range`), `measure` semantics,
//! and normalized scrollbar thumb/track math follow upstream Linebender Masonry
//! (`masonry/src/widgets/{portal,scroll_bar}.rs`), adapted into a non-generic widget driven
//! by [`ScrollBarStyle`] properties and Brocado [`DesignSpec`] tokens.

use std::any::TypeId;
use std::marker::PhantomData;
use std::ops::Range;

use masonry::accesskit::{self, Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, AllowRawMut, ChildrenIds, ComposeCtx, EventCtx, LayoutCtx, MeasureCtx,
    NewWidget, NoAction, PaintCtx, PointerButtonEvent, PointerEvent, PointerScrollEvent,
    PointerUpdate, PropertiesMut, PropertiesRef, Property, RegisterCtx, TextEvent, Update,
    UpdateCtx, UsesProperty, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::dpi::{LogicalPosition, PhysicalPosition};
use masonry::imaging::{Composite, GroupRef, Painter};
use masonry::kurbo::{Axis, Point, Rect, Size, Vec2};
use masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, SizeDef};
use masonry::peniko::Color;
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::{Pod, ViewCtx, WidgetView};

use crate::foundation::state_layer::with_opacity;
use crate::tokens::{ControlVisualGrammar, DesignSpec};

const SCROLL_LINE_STEP: f64 = 120.0;
const SCROLLBAR_FADE_MS: f64 = 160.0;

/// Controls whether and how scrollbars appear inside a [`ScrollView`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollBarVisibility {
    /// Floats above content on overflow; wakes on scroll/hover and fades out after idle delay.
    #[default]
    OverlayAutoHide,
    /// Floats above content and remains visible whenever content overflows.
    OverlayAlways,
    /// Reserves gutter space on overflowing axes so constrained children never sit under the track.
    Reserved,
    /// Hides scrollbars visually while preserving wheel, keyboard, and focus-pan scrolling.
    Never,
}

/// Visual and behavioral style properties for [`ScrollViewWidget`] and [`UiScrollBar`].
///
/// Register via Masonry `DefaultProperties` (automatically configured per [`DesignSpec`]) or
/// override on a specific [`ScrollView`] with [`ScrollView::scrollbar_style`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollBarStyle {
    /// Scrollbar presentation mode (`OverlayAutoHide`, `OverlayAlways`, `Reserved`, `Never`).
    pub visibility: ScrollBarVisibility,
    /// Resting thumb fill color.
    pub thumb_color: Color,
    /// Hovered thumb fill color.
    pub thumb_hover_color: Color,
    /// Actively dragged thumb fill color.
    pub thumb_active_color: Color,
    /// Optional track fill painted behind the thumb when `Reserved`, hovered, or dragging.
    pub track_color: Option<Color>,
    /// Optional 1px separator/border painted alongside the active track.
    pub track_border_color: Option<Color>,
    /// Resting thumb thickness in logical pixels.
    pub thickness: f64,
    /// Expanded thumb thickness when hovered, dragging, or `Reserved`.
    pub hover_thickness: f64,
    /// Edge inset between the thumb/track and the viewport edge.
    pub padding: f64,
    /// Thumb corner radius (`>= half_thickness` renders a full pill/capsule).
    pub corner_radius: f64,
    /// Minimum draggable thumb length along the scrolling axis.
    pub min_thumb_length: f64,
    /// Idle delay in milliseconds before `OverlayAutoHide` bars fade out (`0` hides immediately).
    pub auto_hide_delay_ms: u64,
    /// Disables opacity fade transitions when `true`.
    pub reduced_motion: bool,
}

impl Property for ScrollBarStyle {
    fn static_default() -> &'static Self {
        static DEFAULT: ScrollBarStyle = ScrollBarStyle {
            visibility: ScrollBarVisibility::OverlayAutoHide,
            thumb_color: Color::from_rgba8(0x66, 0x66, 0x66, 0x66),
            thumb_hover_color: Color::from_rgba8(0x66, 0x66, 0x66, 0x99),
            thumb_active_color: Color::from_rgba8(0x66, 0x66, 0x66, 0xCC),
            track_color: None,
            track_border_color: None,
            thickness: 6.0,
            hover_thickness: 10.0,
            padding: 2.0,
            corner_radius: 999.0,
            min_thumb_length: 32.0,
            auto_hide_delay_ms: 800,
            reduced_motion: false,
        };
        &DEFAULT
    }
}

impl Default for ScrollBarStyle {
    fn default() -> Self {
        *Self::static_default()
    }
}

impl ScrollBarStyle {
    /// Derives the platform-idiomatic [`ScrollBarStyle`] for `spec`.
    #[must_use]
    pub fn from_spec(spec: &DesignSpec) -> Self {
        let p = &spec.palette;
        match spec.control_grammar() {
            ControlVisualGrammar::MacosHig => Self {
                visibility: ScrollBarVisibility::OverlayAutoHide,
                thumb_color: with_opacity(p.text_primary, 0.34),
                thumb_hover_color: with_opacity(p.text_primary, 0.52),
                thumb_active_color: with_opacity(p.text_primary, 0.70),
                track_color: None,
                track_border_color: None,
                thickness: 6.0,
                hover_thickness: 8.0,
                padding: 2.0,
                corner_radius: 999.0,
                min_thumb_length: 32.0,
                auto_hide_delay_ms: 800,
                reduced_motion: spec.reduced_motion,
            },
            ControlVisualGrammar::Fluent2 => Self {
                visibility: ScrollBarVisibility::OverlayAutoHide,
                thumb_color: with_opacity(p.text_secondary, 0.44),
                thumb_hover_color: with_opacity(p.text_secondary, 0.64),
                thumb_active_color: with_opacity(p.text_secondary, 0.82),
                track_color: Some(with_opacity(p.surface_highest, 0.74)),
                track_border_color: Some(with_opacity(p.border, 0.45)),
                thickness: 4.0,
                hover_thickness: 10.0,
                padding: 2.0,
                corner_radius: 999.0,
                min_thumb_length: 32.0,
                auto_hide_delay_ms: 800,
                reduced_motion: spec.reduced_motion,
            },
            ControlVisualGrammar::Adwaita => Self {
                visibility: ScrollBarVisibility::OverlayAutoHide,
                thumb_color: with_opacity(p.text_primary, 0.28),
                thumb_hover_color: with_opacity(p.text_primary, 0.48),
                thumb_active_color: with_opacity(p.text_primary, 0.68),
                track_color: Some(with_opacity(p.surface_high, 0.55)),
                track_border_color: None,
                thickness: 6.0,
                hover_thickness: 9.0,
                padding: 2.0,
                corner_radius: 999.0,
                min_thumb_length: 32.0,
                auto_hide_delay_ms: 800,
                reduced_motion: spec.reduced_motion,
            },
            ControlVisualGrammar::Material3 => Self {
                visibility: ScrollBarVisibility::OverlayAutoHide,
                thumb_color: with_opacity(p.text_secondary, 0.38),
                thumb_hover_color: with_opacity(p.text_secondary, 0.58),
                thumb_active_color: with_opacity(p.text_secondary, 0.78),
                track_color: Some(with_opacity(p.surface_highest, 0.45)),
                track_border_color: None,
                thickness: 6.0,
                hover_thickness: 10.0,
                padding: 2.0,
                corner_radius: 999.0,
                min_thumb_length: 32.0,
                auto_hide_delay_ms: 800,
                reduced_motion: spec.reduced_motion,
            },
        }
    }

    /// Returns cross-axis gutter width/height reserved by a full scrollbar lane.
    #[must_use]
    pub fn reserved_size(&self) -> f64 {
        (self.hover_thickness.max(self.thickness).max(1.0) + self.padding.max(0.0) * 2.0).max(0.0)
    }

    const fn auto_hides(&self) -> bool {
        matches!(self.visibility, ScrollBarVisibility::OverlayAutoHide) && !self.reduced_motion
    }

    /// Sets presentation mode.
    #[must_use]
    pub fn with_visibility(mut self, visibility: ScrollBarVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Sets resting, hovered, and active thumb fill colors.
    #[must_use]
    pub fn with_thumb_colors(mut self, rest: Color, hover: Color, active: Color) -> Self {
        self.thumb_color = rest;
        self.thumb_hover_color = hover;
        self.thumb_active_color = active;
        self
    }

    /// Sets optional track fill and border colors.
    #[must_use]
    pub fn with_track(mut self, fill: Option<Color>, border: Option<Color>) -> Self {
        self.track_color = fill;
        self.track_border_color = border;
        self
    }

    /// Sets resting and hovered/reserved thumb thicknesses.
    #[must_use]
    pub fn with_thickness(mut self, resting: f64, hovered: f64) -> Self {
        self.thickness = resting.max(1.0);
        self.hover_thickness = hovered.max(self.thickness);
        self
    }

    /// Sets thumb edge padding.
    #[must_use]
    pub fn with_padding(mut self, padding: f64) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    /// Sets thumb corner radius (`>= half_thickness` renders a pill capsule).
    #[must_use]
    pub fn with_corner_radius(mut self, radius: f64) -> Self {
        self.corner_radius = radius.max(0.0);
        self
    }

    /// Sets minimum draggable thumb length.
    #[must_use]
    pub fn with_min_thumb_length(mut self, min_length: f64) -> Self {
        self.min_thumb_length = min_length.max(8.0);
        self
    }

    /// Sets auto-hide idle delay in milliseconds.
    #[must_use]
    pub fn with_auto_hide_delay_ms(mut self, delay_ms: u64) -> Self {
        self.auto_hide_delay_ms = delay_ms;
        self
    }
}

fn compute_pan_range(mut viewport: Range<f64>, target: Range<f64>) -> Range<f64> {
    if target.start <= viewport.start && viewport.end <= target.end {
        return viewport;
    }
    if viewport.start <= target.start && target.end <= viewport.end {
        return viewport;
    }

    let viewport_width = viewport.end - viewport.start;
    let target_width = f64::min(viewport_width, target.end - target.start);
    if viewport.start >= target.start {
        viewport.start = target.end - target_width;
        viewport.end = viewport.start + viewport_width;
    } else {
        viewport.end = target.start + target_width;
        viewport.start = viewport.end - viewport_width;
    }
    viewport
}

/// Themeable scrollbar widget managed by [`ScrollViewWidget`].
#[derive(Debug)]
pub struct UiScrollBar {
    axis: Axis,
    cursor_progress: f64,
    moved: bool,
    portal_size: f64,
    content_size: f64,
    opacity: f32,
    target_opacity: f32,
    hovered: bool,
    style: ScrollBarStyle,
    grab_anchor: Option<f64>,
}

impl UiScrollBar {
    /// Creates a new scrollbar tracking `portal_size` and `content_size` along `axis`.
    #[must_use]
    pub fn new(axis: Axis, portal_size: f64, content_size: f64) -> Self {
        Self {
            axis,
            cursor_progress: 0.0,
            moved: false,
            portal_size,
            content_size,
            opacity: 1.0,
            target_opacity: 1.0,
            hovered: false,
            style: ScrollBarStyle::default(),
            grab_anchor: None,
        }
    }

    /// Returns normalized scroll progress in `0.0..=1.0`.
    #[must_use]
    pub fn cursor_progress(&self) -> f64 {
        self.cursor_progress
    }

    fn is_interacting(&self) -> bool {
        self.hovered || self.grab_anchor.is_some()
    }

    fn scroll_range(&self) -> f64 {
        (self.content_size - self.portal_size).max(0.0)
    }

    fn thumb_lengths(&self, layout_size: Size, min_length: f64) -> (f64, f64) {
        let track_len = layout_size.get_coord(self.axis).max(0.0);
        let ratio = if self.content_size > 0.0 {
            (self.portal_size / self.content_size).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_len = (ratio * track_len).max(min_length).min(track_len);
        let empty_len = (track_len - thumb_len).max(0.0);
        (thumb_len, empty_len)
    }

    fn hit_thumb_rect(&self, layout_size: Size) -> Rect {
        let (thumb_len, empty_len) = self.thumb_lengths(layout_size, self.style.min_thumb_length);
        let pos_major = self.cursor_progress.clamp(0.0, 1.0) * empty_len;
        let origin = self.axis.pack_point(pos_major, 0.0);
        let size_minor = layout_size.get_coord(self.axis.cross());
        Rect::from_origin_size(origin, self.axis.pack_size(thumb_len, size_minor))
    }

    fn visual_thumb_rect(&self, layout_size: Size, thickness: f64) -> Rect {
        let pad = self.style.padding.max(0.0);
        let major_len = (layout_size.get_coord(self.axis) - pad * 2.0).max(0.0);
        let minor_len = layout_size.get_coord(self.axis.cross()).max(0.0);
        let eff_thickness = thickness.min((minor_len - pad * 2.0).max(1.0));
        let min_len = self.style.min_thumb_length.min(major_len);
        let (thumb_len, empty_len) = {
            let ratio = if self.content_size > 0.0 {
                (self.portal_size / self.content_size).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let len = (ratio * major_len).max(min_len).min(major_len);
            (len, (major_len - len).max(0.0))
        };
        let major_start = pad + self.cursor_progress.clamp(0.0, 1.0) * empty_len;
        let minor_start = (minor_len - pad - eff_thickness).max(0.0);
        Rect::from_origin_size(
            self.axis.pack_point(major_start, minor_start),
            self.axis.pack_size(thumb_len, eff_thickness),
        )
    }

    fn progress_from_mouse_pos(&self, layout_size: Size, anchor: f64, mouse_pos: Point) -> f64 {
        let (thumb_len, empty_len) = self.thumb_lengths(layout_size, self.style.min_thumb_length);
        if empty_len <= 1e-12 {
            return 0.0;
        }
        let new_start = mouse_pos.get_coord(self.axis) - anchor * thumb_len;
        (new_start / empty_len).clamp(0.0, 1.0)
    }

    fn set_cursor_progress(&mut self, next: f64) -> bool {
        let next = next.clamp(0.0, 1.0);
        if (next - self.cursor_progress).abs() > 1e-12 {
            self.cursor_progress = next;
            self.moved = true;
            true
        } else {
            false
        }
    }

    fn adjust_by_pixels(&mut self, delta_pixels: f64) -> bool {
        let range = self.scroll_range();
        if range <= 1e-12 {
            return false;
        }
        self.set_cursor_progress(self.cursor_progress + delta_pixels / range)
    }

    /// Returns `true` when an opacity fade animation frame should be scheduled.
    fn set_target_opacity(&mut self, target: f32, immediate: bool) -> bool {
        let target = target.clamp(0.0, 1.0);
        if immediate || !self.style.auto_hides() {
            self.opacity = target;
            self.target_opacity = target;
            false
        } else {
            self.target_opacity = target;
            (self.target_opacity - self.opacity).abs() > 1e-4
        }
    }
}

impl UsesProperty<ScrollBarStyle> for UiScrollBar {}
impl AllowRawMut for UiScrollBar {}

impl Widget for UiScrollBar {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::HoveredChanged(hovered) = event {
            self.hovered = *hovered;
            if self.style.auto_hides() {
                if *hovered {
                    if self.set_target_opacity(1.0, false) {
                        ctx.request_anim_frame();
                    }
                } else if !self.is_interacting()
                    && (self.target_opacity - self.opacity).abs() > 1e-4
                {
                    ctx.request_anim_frame();
                }
            }
            ctx.request_paint_only();
        }
    }

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        if ScrollBarStyle::matches(property_type) {
            ctx.request_paint_only();
        }
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(PointerButtonEvent { state, .. }) => {
                ctx.capture_pointer();
                let size = ctx.content_box().size();
                let thumb_rect = self.hit_thumb_rect(size);
                let mouse_pos = ctx.local_position(state.position);
                let mut changed = false;
                if thumb_rect.contains(mouse_pos) {
                    let (t0, t1) = thumb_rect.get_coords(self.axis);
                    let denom = (t1 - t0).max(1.0);
                    self.grab_anchor = Some((mouse_pos.get_coord(self.axis) - t0) / denom);
                } else {
                    let progress = self.progress_from_mouse_pos(size, 0.5, mouse_pos);
                    changed |= self.set_cursor_progress(progress);
                    self.grab_anchor = Some(0.5);
                }
                if changed || self.grab_anchor.is_some() {
                    ctx.request_render();
                }
            }
            PointerEvent::Move(PointerUpdate { current, .. }) => {
                if ctx.is_active()
                    && let Some(anchor) = self.grab_anchor
                {
                    let size = ctx.content_box().size();
                    let progress = self.progress_from_mouse_pos(
                        size,
                        anchor,
                        ctx.local_position(current.position),
                    );
                    if self.set_cursor_progress(progress) {
                        ctx.request_render();
                    }
                }
            }
            PointerEvent::Up(..) | PointerEvent::Cancel(..) => {
                if self.grab_anchor.take().is_some() {
                    if self.style.auto_hides()
                        && !self.is_interacting()
                        && (self.target_opacity - self.opacity).abs() > 1e-4
                    {
                        ctx.request_anim_frame();
                    }
                    ctx.request_paint_only();
                }
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
        let TextEvent::Keyboard(kev) = event else {
            return;
        };
        if !kev.state.is_down() {
            return;
        }
        let line = SCROLL_LINE_STEP;
        let page = self.portal_size;
        let changed = match (&kev.key, self.axis) {
            (Key::Named(NamedKey::ArrowUp), Axis::Vertical) => self.adjust_by_pixels(-line),
            (Key::Named(NamedKey::ArrowDown), Axis::Vertical) => self.adjust_by_pixels(line),
            (Key::Named(NamedKey::PageUp), Axis::Vertical) => self.adjust_by_pixels(-page),
            (Key::Named(NamedKey::PageDown), Axis::Vertical) => self.adjust_by_pixels(page),
            (Key::Named(NamedKey::Home), Axis::Vertical | Axis::Horizontal) => {
                self.set_cursor_progress(0.0)
            }
            (Key::Named(NamedKey::End), Axis::Vertical | Axis::Horizontal) => {
                self.set_cursor_progress(1.0)
            }
            (Key::Named(NamedKey::ArrowLeft), Axis::Horizontal) => self.adjust_by_pixels(-line),
            (Key::Named(NamedKey::ArrowRight), Axis::Horizontal) => self.adjust_by_pixels(line),
            _ => false,
        };
        if changed {
            ctx.request_render();
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        let matches_axis = matches!(
            (event.action, self.axis),
            (
                accesskit::Action::ScrollUp | accesskit::Action::ScrollDown,
                Axis::Vertical
            ) | (
                accesskit::Action::ScrollLeft | accesskit::Action::ScrollRight,
                Axis::Horizontal
            )
        );
        if !matches_axis {
            return;
        }
        let unit = match &event.data {
            Some(accesskit::ActionData::ScrollUnit(u)) => *u,
            _ => accesskit::ScrollUnit::Item,
        };
        let mag = match unit {
            accesskit::ScrollUnit::Item => SCROLL_LINE_STEP,
            accesskit::ScrollUnit::Page => self.portal_size,
        };
        let signed = match event.action {
            accesskit::Action::ScrollUp | accesskit::Action::ScrollLeft => -mag,
            accesskit::Action::ScrollDown | accesskit::Action::ScrollRight => mag,
            _ => 0.0,
        };
        if self.adjust_by_pixels(signed) {
            ctx.request_render();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if !self.style.auto_hides() {
            let target = if self.style.visibility == ScrollBarVisibility::Never {
                0.0
            } else {
                1.0
            };
            if (self.opacity - target).abs() > 1e-4 {
                self.opacity = target;
                self.target_opacity = target;
                ctx.request_paint_only();
            }
            return;
        }

        if (self.target_opacity - self.opacity).abs() <= 1e-4 {
            self.opacity = self.target_opacity;
            ctx.request_paint_only();
            return;
        }

        let delta = ((interval as f64 * 1e-6) / SCROLLBAR_FADE_MS) as f32;
        if self.target_opacity > self.opacity {
            self.opacity = (self.opacity + delta).min(self.target_opacity);
        } else {
            self.opacity = (self.opacity - delta).max(self.target_opacity);
        }
        ctx.request_paint_only();
        if (self.target_opacity - self.opacity).abs() > 1e-4 {
            ctx.request_anim_frame();
        } else {
            self.opacity = self.target_opacity;
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        if axis == self.axis {
            match len_req {
                LenReq::MinContent | LenReq::MaxContent => self.portal_size.px(),
                LenReq::FitContent(space) => space,
            }
        } else {
            self.style.reserved_size().px()
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let effective_opacity = if self.is_interacting() {
            1.0
        } else {
            self.opacity
        };
        if effective_opacity <= 0.001
            || self.scroll_range() <= 0.5
            || self.style.visibility == ScrollBarVisibility::Never
        {
            return;
        }

        let size = ctx.content_box().size();
        if size.width <= 0.5 || size.height <= 0.5 {
            return;
        }

        let needs_group = effective_opacity < 0.999;
        if needs_group {
            painter.push_fill_clip(ctx.border_box());
            painter.push_group(GroupRef::new().with_composite(Composite::new(
                masonry::peniko::BlendMode::default(),
                effective_opacity,
            )));
        }

        let expanded =
            self.is_interacting() || self.style.visibility == ScrollBarVisibility::Reserved;
        if expanded && let Some(track_col) = self.style.track_color {
            let track_rect = size.to_rect();
            painter.fill(track_rect, track_col).draw();
            if let Some(border_col) = self.style.track_border_color {
                let edge = match self.axis {
                    Axis::Vertical => Rect::new(0.0, 0.0, 1.0, size.height),
                    Axis::Horizontal => Rect::new(0.0, 0.0, size.width, 1.0),
                };
                painter.fill(edge, border_col).draw();
            }
        }

        let thickness = if expanded {
            self.style.hover_thickness.max(self.style.thickness)
        } else {
            self.style.thickness
        };
        let thumb_rect = self.visual_thumb_rect(size, thickness);
        let max_r = (thumb_rect.width().min(thumb_rect.height()) * 0.5).max(0.0);
        let rr = thumb_rect.to_rounded_rect(self.style.corner_radius.min(max_r));

        let thumb_color = if self.grab_anchor.is_some() {
            self.style.thumb_active_color
        } else if self.hovered {
            self.style.thumb_hover_color
        } else {
            self.style.thumb_color
        };
        painter.fill(rr, thumb_color).draw();

        if needs_group {
            painter.pop_group();
            painter.pop_clip();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ScrollBar
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_orientation(match self.axis {
            Axis::Horizontal => accesskit::Orientation::Horizontal,
            Axis::Vertical => accesskit::Orientation::Vertical,
        });
        let range = self.scroll_range();
        let val = self.cursor_progress.clamp(0.0, 1.0) * range;
        match self.axis {
            Axis::Horizontal => {
                node.set_scroll_x_min(0.0);
                node.set_scroll_x_max(range);
                node.set_scroll_x(val);
                if self.cursor_progress > 1e-12 {
                    node.add_action(accesskit::Action::ScrollLeft);
                }
                if self.cursor_progress + 1e-12 < 1.0 {
                    node.add_action(accesskit::Action::ScrollRight);
                }
            }
            Axis::Vertical => {
                node.set_scroll_y_min(0.0);
                node.set_scroll_y_max(range);
                node.set_scroll_y(val);
                if self.cursor_progress > 1e-12 {
                    node.add_action(accesskit::Action::ScrollUp);
                }
                if self.cursor_progress + 1e-12 < 1.0 {
                    node.add_action(accesskit::Action::ScrollDown);
                }
            }
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("UiScrollBar", id = id.trace())
    }

    fn accepts_focus(&self) -> bool {
        true
    }
}

/// Non-generic scrollable container widget paired with [`UiScrollBar`] and [`ScrollBarStyle`].
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct ScrollViewWidget {
    child: WidgetPod<dyn Widget>,
    content_size: Size,
    viewport_size: Size,
    viewport_pos: Point,
    constrain_horizontal: bool,
    constrain_vertical: bool,
    must_fill: bool,
    style_override: Option<ScrollBarStyle>,
    visibility_override: Option<ScrollBarVisibility>,
    scrollbar_horizontal: WidgetPod<UiScrollBar>,
    scrollbar_horizontal_visible: bool,
    scrollbar_vertical: WidgetPod<UiScrollBar>,
    scrollbar_vertical_visible: bool,
    idle_ns: Option<u64>,
}

impl ScrollViewWidget {
    /// Creates a new scrollable viewport wrapping `child`.
    #[must_use]
    pub fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: child.erased().to_pod(),
            content_size: Size::ZERO,
            viewport_size: Size::ZERO,
            viewport_pos: Point::ORIGIN,
            constrain_horizontal: false,
            constrain_vertical: false,
            must_fill: false,
            style_override: None,
            visibility_override: None,
            scrollbar_horizontal: WidgetPod::new(UiScrollBar::new(Axis::Horizontal, 0.0, 0.0)),
            scrollbar_horizontal_visible: false,
            scrollbar_vertical: WidgetPod::new(UiScrollBar::new(Axis::Vertical, 0.0, 0.0)),
            scrollbar_vertical_visible: false,
            idle_ns: None,
        }
    }

    /// Constrains child width to viewport width (`true` disables horizontal scrolling).
    #[must_use]
    pub fn constrain_horizontal(mut self, constrain: bool) -> Self {
        self.constrain_horizontal = constrain;
        self
    }

    /// Constrains child height to viewport height (`true` disables vertical scrolling).
    #[must_use]
    pub fn constrain_vertical(mut self, constrain: bool) -> Self {
        self.constrain_vertical = constrain;
        self
    }

    /// Expands child size to at least fill the viewport (`true` guarantees `>= viewport_size`).
    #[must_use]
    pub fn content_must_fill(mut self, must_fill: bool) -> Self {
        self.must_fill = must_fill;
        self
    }

    /// Applies an explicit [`ScrollBarStyle`] override instead of inheriting from [`DesignSpec`].
    #[must_use]
    pub fn with_style(mut self, style: ScrollBarStyle) -> Self {
        self.style_override = Some(style);
        self
    }

    /// Overrides the [`ScrollBarVisibility`] mode on top of the resolved [`ScrollBarStyle`].
    #[must_use]
    pub fn with_visibility(mut self, visibility: ScrollBarVisibility) -> Self {
        self.visibility_override = Some(visibility);
        self
    }

    /// Returns the current viewport scroll offset.
    #[must_use]
    pub fn viewport_pos(&self) -> Point {
        self.viewport_pos
    }

    /// Returns mutable access to the wrapped child widget.
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    /// Updates horizontal constraint flag.
    pub fn set_constrain_horizontal(this: &mut WidgetMut<'_, Self>, constrain: bool) {
        if this.widget.constrain_horizontal != constrain {
            this.widget.constrain_horizontal = constrain;
            this.ctx.request_layout();
        }
    }

    /// Updates vertical constraint flag.
    pub fn set_constrain_vertical(this: &mut WidgetMut<'_, Self>, constrain: bool) {
        if this.widget.constrain_vertical != constrain {
            this.widget.constrain_vertical = constrain;
            this.ctx.request_layout();
        }
    }

    /// Updates whether child fills the viewport when smaller.
    pub fn set_content_must_fill(this: &mut WidgetMut<'_, Self>, must_fill: bool) {
        if this.widget.must_fill != must_fill {
            this.widget.must_fill = must_fill;
            this.ctx.request_layout();
        }
    }

    /// Updates the per-widget [`ScrollBarStyle`] override.
    pub fn set_style_override(this: &mut WidgetMut<'_, Self>, style: Option<ScrollBarStyle>) {
        if this.widget.style_override != style {
            this.widget.style_override = style;
            this.ctx.request_layout();
        }
    }

    /// Updates the per-widget [`ScrollBarVisibility`] override.
    pub fn set_visibility_override(
        this: &mut WidgetMut<'_, Self>,
        visibility: Option<ScrollBarVisibility>,
    ) {
        if this.widget.visibility_override != visibility {
            this.widget.visibility_override = visibility;
            this.ctx.request_layout();
        }
    }

    fn resolve_style(
        &self,
        props: &PropertiesRef<'_>,
        cache: &mut masonry::core::PropertyCache,
    ) -> ScrollBarStyle {
        let mut s = self
            .style_override
            .unwrap_or_else(|| *props.get::<ScrollBarStyle>(cache));
        if let Some(vis) = self.visibility_override {
            s.visibility = vis;
        }
        s
    }

    fn compute_child_size(&mut self, ctx: &mut LayoutCtx<'_>, avail: Size) -> Size {
        let auto_size = SizeDef::new(
            if self.constrain_horizontal {
                LenDef::FitContent(avail.width.px())
            } else {
                LenDef::MaxContent
            },
            if self.constrain_vertical {
                LenDef::FitContent(avail.height.px())
            } else {
                LenDef::MaxContent
            },
        );
        let child_size = ctx.compute_size(&mut self.child, auto_size, avail.into());
        if self.must_fill {
            child_size.max(avail)
        } else {
            child_size
        }
    }

    fn set_viewport_pos_raw(
        &mut self,
        viewport_size: Size,
        content_size: Size,
        pos: Point,
    ) -> bool {
        let max_pos = (content_size - viewport_size).max(Size::ZERO);
        let clamped = Point::new(
            pos.x.clamp(0.0, max_pos.width),
            pos.y.clamp(0.0, max_pos.height),
        );
        if (clamped - self.viewport_pos).hypot2() > 1e-12 {
            self.viewport_pos = clamped;
            true
        } else {
            false
        }
    }

    fn pan_viewport_to_raw(
        &mut self,
        viewport_size: Size,
        content_size: Size,
        target: Rect,
    ) -> bool {
        let vp = Rect::from_origin_size(self.viewport_pos, viewport_size);
        let next_x =
            compute_pan_range(vp.min_x()..vp.max_x(), target.min_x()..target.max_x()).start;
        let next_y =
            compute_pan_range(vp.min_y()..vp.max_y(), target.min_y()..target.max_y()).start;
        self.set_viewport_pos_raw(viewport_size, content_size, Point::new(next_x, next_y))
    }

    fn sync_scrollbars_from_viewport(
        &mut self,
        ctx: &mut EventCtx<'_>,
        viewport_size: Size,
        content_size: Size,
        wake_auto_hide: bool,
    ) {
        let range = (content_size - viewport_size).max(Size::ZERO);
        let px = if range.width > 1e-12 {
            (self.viewport_pos.x / range.width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let py = if range.height > 1e-12 {
            (self.viewport_pos.y / range.height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut woke_any = false;
        for (bar_pod, visible, progress) in [
            (
                &mut self.scrollbar_horizontal,
                self.scrollbar_horizontal_visible,
                px,
            ),
            (
                &mut self.scrollbar_vertical,
                self.scrollbar_vertical_visible,
                py,
            ),
        ] {
            let (bar, mut bar_ctx) = ctx.get_raw_mut(bar_pod);
            bar.cursor_progress = progress;
            if wake_auto_hide && visible && bar.style.auto_hides() {
                woke_any = true;
                if bar.set_target_opacity(1.0, false) {
                    bar_ctx.request_anim_frame();
                }
            }
            bar_ctx.request_render();
        }
        if woke_any {
            self.idle_ns = Some(0);
            ctx.request_anim_frame();
        }
    }

    fn sync_viewport_from_scrollbars(
        &mut self,
        ctx: &mut EventCtx<'_>,
        viewport_size: Size,
        content_size: Size,
    ) -> bool {
        let range = (content_size - viewport_size).max(Size::ZERO);
        let mut changed = false;

        {
            let (bar, _) = ctx.get_raw_mut(&mut self.scrollbar_horizontal);
            if bar.moved {
                bar.moved = false;
                let x = bar.cursor_progress * range.width;
                changed |= self.set_viewport_pos_raw(
                    viewport_size,
                    content_size,
                    Point::new(x, self.viewport_pos.y),
                );
            }
        }
        {
            let (bar, _) = ctx.get_raw_mut(&mut self.scrollbar_vertical);
            if bar.moved {
                bar.moved = false;
                let y = bar.cursor_progress * range.height;
                changed |= self.set_viewport_pos_raw(
                    viewport_size,
                    content_size,
                    Point::new(self.viewport_pos.x, y),
                );
            }
        }

        if changed {
            ctx.request_compose();
            self.sync_scrollbars_from_viewport(ctx, viewport_size, content_size, true);
        }
        changed
    }

    fn pan_by_event_ctx(
        &mut self,
        ctx: &mut EventCtx<'_>,
        viewport_size: Size,
        content_size: Size,
        mut delta: Vec2,
    ) -> bool {
        if self.constrain_horizontal {
            delta.x = 0.0;
        }
        if self.constrain_vertical {
            delta.y = 0.0;
        }
        if delta.x == 0.0 && delta.y == 0.0 {
            return false;
        }
        let changed =
            self.set_viewport_pos_raw(viewport_size, content_size, self.viewport_pos + delta);
        if changed {
            ctx.request_compose();
            self.sync_scrollbars_from_viewport(ctx, viewport_size, content_size, true);
        }
        changed
    }
}

impl UsesProperty<ScrollBarStyle> for ScrollViewWidget {}

impl Widget for ScrollViewWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
        ctx.register_child(&mut self.scrollbar_horizontal);
        ctx.register_child(&mut self.scrollbar_vertical);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, props: &mut PropertiesMut<'_>, event: &Update) {
        let style = {
            let mut s = self
                .style_override
                .unwrap_or_else(|| *props.get::<ScrollBarStyle>(ctx.property_cache()));
            if let Some(vis) = self.visibility_override {
                s.visibility = vis;
            }
            s
        };

        match event {
            Update::RequestPanToChild(target) => {
                let vp_size = self.viewport_size;
                let content_size = self.content_size;
                self.pan_viewport_to_raw(vp_size, content_size, *target);
                ctx.request_compose();
                let range = (content_size - vp_size).max(Size::ZERO);
                let px = if range.width > 1e-12 {
                    (self.viewport_pos.x / range.width).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let py = if range.height > 1e-12 {
                    (self.viewport_pos.y / range.height).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let mut woke_any = false;
                for (bar_pod, visible, progress) in [
                    (
                        &mut self.scrollbar_horizontal,
                        self.scrollbar_horizontal_visible,
                        px,
                    ),
                    (
                        &mut self.scrollbar_vertical,
                        self.scrollbar_vertical_visible,
                        py,
                    ),
                ] {
                    let (bar, mut bar_ctx) = ctx.get_raw_mut(bar_pod);
                    bar.cursor_progress = progress;
                    if visible && style.auto_hides() {
                        woke_any = true;
                        if bar.set_target_opacity(1.0, false) {
                            bar_ctx.request_anim_frame();
                        }
                    }
                    bar_ctx.request_render();
                }
                if woke_any {
                    self.idle_ns = Some(0);
                    ctx.request_anim_frame();
                }
            }
            Update::HoveredChanged(true) | Update::ChildHoveredChanged(true) => {
                if style.auto_hides()
                    && (self.scrollbar_horizontal_visible || self.scrollbar_vertical_visible)
                {
                    self.idle_ns = Some(0);
                    for bar_pod in [&mut self.scrollbar_horizontal, &mut self.scrollbar_vertical] {
                        let (bar, mut bar_ctx) = ctx.get_raw_mut(bar_pod);
                        if bar.set_target_opacity(1.0, false) {
                            bar_ctx.request_anim_frame();
                        }
                    }
                    ctx.request_anim_frame();
                }
            }
            _ => {}
        }
    }

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        if ScrollBarStyle::matches(property_type) {
            ctx.request_layout();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        let mut style = self
            .style_override
            .unwrap_or_else(|| *props.get::<ScrollBarStyle>(ctx.property_cache()));
        if let Some(vis) = self.visibility_override {
            style.visibility = vis;
        }

        if !style.auto_hides() {
            self.idle_ns = None;
            return;
        }

        let Some(elapsed) = self.idle_ns else {
            return;
        };
        let total = elapsed.saturating_add(interval);
        let delay_ns = style.auto_hide_delay_ms.saturating_mul(1_000_000);
        if total >= delay_ns {
            self.idle_ns = None;
            for bar_pod in [&mut self.scrollbar_horizontal, &mut self.scrollbar_vertical] {
                ctx.mutate_child_later(bar_pod, |mut bar: WidgetMut<'_, UiScrollBar>| {
                    if !bar.widget.is_interacting() && bar.widget.set_target_opacity(0.0, false) {
                        bar.ctx.request_anim_frame();
                    } else {
                        bar.ctx.request_paint_only();
                    }
                });
            }
        } else {
            self.idle_ns = Some(total);
            ctx.request_anim_frame();
        }
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        let mut style = self
            .style_override
            .unwrap_or_else(|| *props.get::<ScrollBarStyle>(ctx.property_cache()));
        if let Some(vis) = self.visibility_override {
            style.visibility = vis;
        }

        let viewport_size = self.viewport_size;
        let content_size = self.content_size;

        match event {
            PointerEvent::Scroll(PointerScrollEvent { delta, state, .. }) => {
                let scale = ctx.scale_factor();
                let line_px = PhysicalPosition {
                    x: SCROLL_LINE_STEP * scale,
                    y: SCROLL_LINE_STEP * scale,
                };
                let page_px = PhysicalPosition {
                    x: viewport_size.width * scale,
                    y: viewport_size.height * scale,
                };
                let delta_px = delta.to_pixel_delta(line_px, page_px);
                let LogicalPosition { x, y } = delta_px.to_logical::<f64>(scale);
                let mut vec = -Vec2 { x, y };
                if state.modifiers.shift()
                    && vec.x.abs() < 1e-6
                    && vec.y.abs() > 1e-6
                    && !self.constrain_horizontal
                {
                    vec.x = vec.y;
                    vec.y = 0.0;
                }
                if self.pan_by_event_ctx(ctx, viewport_size, content_size, vec) {
                    ctx.set_handled();
                }
            }
            PointerEvent::Move(_)
                if style.auto_hides()
                    && (self.scrollbar_horizontal_visible || self.scrollbar_vertical_visible) =>
            {
                self.idle_ns = Some(0);
                for bar_pod in [&mut self.scrollbar_horizontal, &mut self.scrollbar_vertical] {
                    let (bar, mut bar_ctx) = ctx.get_raw_mut(bar_pod);
                    if bar.set_target_opacity(1.0, false) {
                        bar_ctx.request_anim_frame();
                    }
                }
                ctx.request_anim_frame();
            }
            _ => {}
        }

        if self.sync_viewport_from_scrollbars(ctx, viewport_size, content_size) {
            ctx.set_handled();
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        let viewport_size = self.viewport_size;
        let content_size = self.content_size;
        let target = ctx.target();
        let on_bar =
            target == self.scrollbar_vertical.id() || target == self.scrollbar_horizontal.id();

        if let TextEvent::Keyboard(kev) = event
            && kev.state.is_down()
            && !on_bar
        {
            let delta = match &kev.key {
                Key::Named(NamedKey::PageDown) => Vec2::new(0.0, viewport_size.height),
                Key::Named(NamedKey::PageUp) => Vec2::new(0.0, -viewport_size.height),
                Key::Named(NamedKey::ArrowDown) => Vec2::new(0.0, SCROLL_LINE_STEP),
                Key::Named(NamedKey::ArrowUp) => Vec2::new(0.0, -SCROLL_LINE_STEP),
                Key::Named(NamedKey::ArrowRight) => Vec2::new(SCROLL_LINE_STEP, 0.0),
                Key::Named(NamedKey::ArrowLeft) => Vec2::new(-SCROLL_LINE_STEP, 0.0),
                Key::Named(NamedKey::Home) => -self.viewport_pos.to_vec2(),
                Key::Named(NamedKey::End) => {
                    let max_pos = (content_size - viewport_size).max(Size::ZERO);
                    max_pos.to_vec2() - self.viewport_pos.to_vec2()
                }
                _ => Vec2::ZERO,
            };
            if self.pan_by_event_ctx(ctx, viewport_size, content_size, delta) {
                ctx.set_handled();
            }
        }

        if self.sync_viewport_from_scrollbars(ctx, viewport_size, content_size) {
            ctx.set_handled();
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        let viewport_size = self.viewport_size;
        let content_size = self.content_size;
        let target = ctx.target();
        let on_bar =
            target == self.scrollbar_vertical.id() || target == self.scrollbar_horizontal.id();

        if !on_bar
            && matches!(
                event.action,
                accesskit::Action::ScrollUp
                    | accesskit::Action::ScrollDown
                    | accesskit::Action::ScrollLeft
                    | accesskit::Action::ScrollRight
            )
        {
            let unit = match &event.data {
                Some(accesskit::ActionData::ScrollUnit(u)) => *u,
                _ => accesskit::ScrollUnit::Item,
            };
            let mag = match unit {
                accesskit::ScrollUnit::Item => SCROLL_LINE_STEP,
                accesskit::ScrollUnit::Page => match event.action {
                    accesskit::Action::ScrollLeft | accesskit::Action::ScrollRight => {
                        viewport_size.width
                    }
                    _ => viewport_size.height,
                },
            };
            let delta = match event.action {
                accesskit::Action::ScrollUp => Vec2::new(0.0, -mag),
                accesskit::Action::ScrollDown => Vec2::new(0.0, mag),
                accesskit::Action::ScrollLeft => Vec2::new(-mag, 0.0),
                accesskit::Action::ScrollRight => Vec2::new(mag, 0.0),
                _ => Vec2::ZERO,
            };
            if self.pan_by_event_ctx(ctx, viewport_size, content_size, delta) {
                ctx.set_handled();
            }
        }

        if self.sync_viewport_from_scrollbars(ctx, viewport_size, content_size) {
            ctx.set_handled();
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
        match len_req {
            LenReq::MinContent => Length::ZERO,
            LenReq::MaxContent => {
                let context_size = LayoutSize::maybe(axis.cross(), cross_length);
                let cross = axis.cross();
                let cross_space = cross_length.filter(|_| match cross {
                    Axis::Horizontal => self.constrain_horizontal,
                    Axis::Vertical => self.constrain_vertical,
                });
                ctx.compute_length(
                    &mut self.child,
                    LenDef::MaxContent,
                    context_size,
                    axis,
                    cross_space,
                )
            }
            LenReq::FitContent(space) => space,
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, props: &PropertiesRef<'_>, size: Size) {
        let style = self.resolve_style(props, ctx.property_cache());
        let mut viewport_size = size;
        let mut content_size = self.compute_child_size(ctx, viewport_size);

        let mut has_h =
            !self.constrain_horizontal && content_size.width > viewport_size.width + 0.5;
        let mut has_v =
            !self.constrain_vertical && content_size.height > viewport_size.height + 0.5;

        if style.visibility == ScrollBarVisibility::Reserved && (has_h || has_v) {
            let gutter = style.reserved_size();
            let rw = if has_v { gutter } else { 0.0 };
            let rh = if has_h { gutter } else { 0.0 };
            viewport_size = Size::new((size.width - rw).max(0.0), (size.height - rh).max(0.0));
            content_size = self.compute_child_size(ctx, viewport_size);
            has_h = !self.constrain_horizontal && content_size.width > viewport_size.width + 0.5;
            has_v = !self.constrain_vertical && content_size.height > viewport_size.height + 0.5;
        }

        ctx.run_layout(&mut self.child, content_size);
        self.content_size = content_size;
        self.viewport_size = viewport_size;
        self.set_viewport_pos_raw(viewport_size, content_size, self.viewport_pos);
        ctx.set_clip_path(viewport_size.to_rect());
        ctx.place_child(&mut self.child, Point::ORIGIN);

        let show_bars = style.visibility != ScrollBarVisibility::Never;
        self.scrollbar_horizontal_visible = has_h && show_bars;
        self.scrollbar_vertical_visible = has_v && show_bars;

        let range = (content_size - viewport_size).max(Size::ZERO);
        let px = if range.width > 1e-12 {
            (self.viewport_pos.x / range.width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let py = if range.height > 1e-12 {
            (self.viewport_pos.y / range.height).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let init_opacity = if style.visibility == ScrollBarVisibility::Never {
            0.0
        } else if style.auto_hides() && self.idle_ns.is_none() {
            0.0
        } else {
            1.0
        };

        let v_thickness = if self.scrollbar_vertical_visible {
            style.reserved_size()
        } else {
            0.0
        };
        let h_thickness = if self.scrollbar_horizontal_visible {
            style.reserved_size()
        } else {
            0.0
        };

        ctx.set_stashed(
            &mut self.scrollbar_horizontal,
            !self.scrollbar_horizontal_visible,
        );
        if self.scrollbar_horizontal_visible {
            let h_portal_w = (size.width - v_thickness).max(0.0);
            {
                let (bar, mut bar_ctx) = ctx.get_raw_mut(&mut self.scrollbar_horizontal);
                bar.style = style;
                bar.portal_size = viewport_size.width;
                bar.content_size = content_size.width;
                bar.cursor_progress = px;
                bar.target_opacity = init_opacity;
                if style.visibility != ScrollBarVisibility::OverlayAutoHide || style.reduced_motion
                {
                    bar.opacity = init_opacity;
                }
                bar_ctx.request_render();
            }
            let h_avail = Size::new(h_portal_w, size.height);
            let sb_size = ctx.compute_size(
                &mut self.scrollbar_horizontal,
                SizeDef::fit(h_avail),
                h_avail.into(),
            );
            let final_size = Size::new(h_portal_w, sb_size.height.min(size.height));
            ctx.run_layout(&mut self.scrollbar_horizontal, final_size);
            ctx.place_child(
                &mut self.scrollbar_horizontal,
                Point::new(0.0, (size.height - final_size.height).max(0.0)),
            );
        }

        ctx.set_stashed(
            &mut self.scrollbar_vertical,
            !self.scrollbar_vertical_visible,
        );
        if self.scrollbar_vertical_visible {
            let v_portal_h = (size.height - h_thickness).max(0.0);
            {
                let (bar, mut bar_ctx) = ctx.get_raw_mut(&mut self.scrollbar_vertical);
                bar.style = style;
                bar.portal_size = viewport_size.height;
                bar.content_size = content_size.height;
                bar.cursor_progress = py;
                bar.target_opacity = init_opacity;
                if style.visibility != ScrollBarVisibility::OverlayAutoHide || style.reduced_motion
                {
                    bar.opacity = init_opacity;
                }
                bar_ctx.request_render();
            }
            let v_avail = Size::new(size.width, v_portal_h);
            let sb_size = ctx.compute_size(
                &mut self.scrollbar_vertical,
                SizeDef::fit(v_avail),
                v_avail.into(),
            );
            let final_size = Size::new(sb_size.width.min(size.width), v_portal_h);
            ctx.run_layout(&mut self.scrollbar_vertical, final_size);
            ctx.place_child(
                &mut self.scrollbar_vertical,
                Point::new((size.width - final_size.width).max(0.0), 0.0),
            );
        }
    }

    fn compose(&mut self, ctx: &mut ComposeCtx<'_>) {
        ctx.set_child_scroll_translation(
            &mut self.child,
            Vec2::new(-self.viewport_pos.x, -self.viewport_pos.y),
        );
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::ScrollView
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_clips_children();
        let range = (self.content_size - self.viewport_size).max(Size::ZERO);
        let can_x = !self.constrain_horizontal && range.width > 1e-12;
        let can_y = !self.constrain_vertical && range.height > 1e-12;
        if can_x {
            node.set_scroll_x_min(0.0);
            node.set_scroll_x_max(range.width);
            node.set_scroll_x(self.viewport_pos.x);
            if self.viewport_pos.x > 1e-12 {
                node.add_action(accesskit::Action::ScrollLeft);
            }
            if self.viewport_pos.x + 1e-12 < range.width {
                node.add_action(accesskit::Action::ScrollRight);
            }
        }
        if can_y {
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(range.height);
            node.set_scroll_y(self.viewport_pos.y);
            if self.viewport_pos.y > 1e-12 {
                node.add_action(accesskit::Action::ScrollUp);
            }
            if self.viewport_pos.y + 1e-12 < range.height {
                node.add_action(accesskit::Action::ScrollDown);
            }
        }
        if can_y && !can_x {
            node.set_orientation(accesskit::Orientation::Vertical);
        } else if can_x && !can_y {
            node.set_orientation(accesskit::Orientation::Horizontal);
        }
        node.add_child_action(accesskit::Action::ScrollIntoView);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[
            self.child.id(),
            self.scrollbar_vertical.id(),
            self.scrollbar_horizontal.id(),
        ])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("ScrollViewWidget", id = id.trace())
    }

    fn accepts_focus(&self) -> bool {
        !(self.constrain_horizontal && self.constrain_vertical)
    }
}

/// Reactive view wrapping a child in [`ScrollViewWidget`] with themeable scrollbars.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Wraps arbitrary child WidgetView"
)]
pub struct ScrollView<V, State, Action> {
    child: V,
    constrain_horizontal: bool,
    constrain_vertical: bool,
    must_fill: bool,
    style: Option<ScrollBarStyle>,
    visibility: Option<ScrollBarVisibility>,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Wraps `child` in a themeable scrollable viewport ([`ScrollView`]).
pub fn scroll_view<V, State, Action>(child: V) -> ScrollView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    ScrollView {
        child,
        constrain_horizontal: false,
        constrain_vertical: false,
        must_fill: false,
        style: None,
        visibility: None,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ScrollView<V, State, Action> {
    /// Constrains child width to viewport width (disables horizontal scrolling).
    pub fn constrain_horizontal(mut self, constrain: bool) -> Self {
        self.constrain_horizontal = constrain;
        self
    }

    /// Constrains child height to viewport height (disables vertical scrolling).
    pub fn constrain_vertical(mut self, constrain: bool) -> Self {
        self.constrain_vertical = constrain;
        self
    }

    /// Ensures child fills at least the full viewport dimensions when smaller.
    pub fn must_fill(mut self, must_fill: bool) -> Self {
        self.must_fill = must_fill;
        self
    }

    /// Overrides the theme-derived [`ScrollBarStyle`] on this scroll view.
    pub fn scrollbar_style(mut self, style: ScrollBarStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Overrides the [`ScrollBarVisibility`] policy on this scroll view.
    pub fn scrollbar_visibility(mut self, visibility: ScrollBarVisibility) -> Self {
        self.visibility = Some(visibility);
        self
    }
}

impl<V, State, Action> ViewMarker for ScrollView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for ScrollView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<ScrollViewWidget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child, child_state) = self.child.build(ctx, app_state);
        let mut widget = ScrollViewWidget::new(child.new_widget)
            .constrain_horizontal(self.constrain_horizontal)
            .constrain_vertical(self.constrain_vertical)
            .content_must_fill(self.must_fill);
        if let Some(style) = self.style {
            widget = widget.with_style(style);
        }
        if let Some(vis) = self.visibility {
            widget = widget.with_visibility(vis);
        }
        (ctx.create_pod(widget), child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        if self.constrain_horizontal != prev.constrain_horizontal {
            ScrollViewWidget::set_constrain_horizontal(&mut element, self.constrain_horizontal);
        }
        if self.constrain_vertical != prev.constrain_vertical {
            ScrollViewWidget::set_constrain_vertical(&mut element, self.constrain_vertical);
        }
        if self.must_fill != prev.must_fill {
            ScrollViewWidget::set_content_must_fill(&mut element, self.must_fill);
        }
        if self.style != prev.style {
            ScrollViewWidget::set_style_override(&mut element, self.style);
        }
        if self.visibility != prev.visibility {
            ScrollViewWidget::set_visibility_override(&mut element, self.visibility);
        }

        let mut child_el = ScrollViewWidget::child_mut(&mut element);
        self.child
            .rebuild(&prev.child, view_state, ctx, child_el.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child_el = ScrollViewWidget::child_mut(&mut element);
        self.child.teardown(view_state, ctx, child_el.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut child_el = ScrollViewWidget::child_mut(&mut element);
        self.child
            .message(view_state, message, child_el.downcast(), app_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{Brightness, ControlBorderStyle};

    #[test]
    fn pan_range_handles_containment_and_edges() {
        assert_eq!(compute_pan_range(100.0..200.0, 120.0..150.0), 100.0..200.0);
        assert_eq!(compute_pan_range(100.0..200.0, 50.0..250.0), 100.0..200.0);
        assert_eq!(compute_pan_range(100.0..200.0, 40.0..80.0), 40.0..140.0);
        assert_eq!(compute_pan_range(100.0..200.0, 220.0..260.0), 160.0..260.0);
    }

    #[test]
    fn scrollbar_style_from_spec_covers_all_grammars_and_polarities() {
        let borders = [
            ControlBorderStyle::UniformHairline,
            ControlBorderStyle::BeveledElevation,
            ControlBorderStyle::CrispElevation,
            ControlBorderStyle::BorderlessFlat,
        ];
        for brightness in [Brightness::Light, Brightness::Dark] {
            for border in borders {
                let mut spec = DesignSpec::neutral_fallback(brightness);
                spec.control_border = border;
                let style = ScrollBarStyle::from_spec(&spec);
                assert!(style.thumb_color.components[3] > 0.1);
                assert!(style.thumb_hover_color.components[3] > style.thumb_color.components[3]);
                assert!(
                    style.thumb_active_color.components[3] > style.thumb_hover_color.components[3]
                );
                assert!(style.reserved_size() >= style.hover_thickness);
            }
        }
    }

    #[test]
    fn ui_scrollbar_clamps_progress_and_min_thumb_length() {
        let mut bar = UiScrollBar::new(Axis::Vertical, 100.0, 2000.0);
        let (len, empty) = bar.thumb_lengths(Size::new(14.0, 100.0), 32.0);
        assert!((len - 32.0).abs() < 1e-6);
        assert!((empty - 68.0).abs() < 1e-6);

        assert!(bar.set_cursor_progress(2.5));
        assert_eq!(bar.cursor_progress(), 1.0);
        assert!(bar.moved);

        assert!(bar.adjust_by_pixels(-950.0));
        assert!((bar.cursor_progress() - 0.5).abs() < 1e-6);
    }
}
