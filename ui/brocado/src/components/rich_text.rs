//! Inline rich-text paragraph widget supporting mixed weights, monospace spans, background highlights, clickable links, and pointer selection + clipboard copy.
//!
//! Upstream provenance: `RichTextWidget`'s Parley layout and pointer selection hit-testing follow
//! upstream `masonry::widgets::{Prose, TextArea}` patterns (`linebender/xilem`, `masonry/src/widgets/`).

use std::marker::PhantomData;
use std::ops::Range;
use std::sync::Arc;

use masonry::accesskit::{self, Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, ArcStr, BrushIndex, ChildrenIds, CursorIcon, EventCtx, LayoutCtx,
    MeasureCtx, PaintCtx, PointerButton, PointerEvent, PointerUpdate, PropertiesMut, PropertiesRef,
    QueryCtx, RegisterCtx, StyleProperty, TextEvent, Update, UpdateCtx, Widget, WidgetMut,
    render_text,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Affine, Axis, Point, Rect, RoundedRect, Size};
use masonry::layout::{LenReq, Length};
use masonry::parley::Alignment as TextAlign;
use masonry::parley::layout::AlignmentOptions;
use masonry::parley::style::{FontStyle, FontWeight, GenericFamily, LineHeight};
use masonry::parley::{Affinity, Cursor, FontFamily, Layout, Selection};
use masonry::peniko::{Brush, Color};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;

use crate::foundation::with_opacity;
use crate::style_engine::current_theme_arc;
use crate::theme::Theme;
use crate::tokens::TypeRole;

/// Inline styling attributes for a [`TextSpan`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpanStyle {
    /// Bold font weight (`700`).
    pub bold: bool,
    /// Italic slant.
    pub italic: bool,
    /// Single underline stroke.
    pub underline: bool,
    /// Strikethrough line.
    pub strikethrough: bool,
    /// Inline monospace code appearance (switches family and adds subtle surface pill if `bg` is `None`).
    pub code: bool,
    /// Optional explicit foreground tint.
    pub color: Option<Color>,
    /// Optional highlight / code background tint.
    pub bg: Option<Color>,
    /// Relative font-size multiplier (`1.0` by default).
    pub scale: f32,
    /// Optional interactive link identifier clicked when tapped.
    pub link_id: Option<ArcStr>,
}

impl SpanStyle {
    /// Default unstyled span.
    #[must_use]
    pub fn plain() -> Self {
        Self {
            scale: 1.0,
            ..Self::default()
        }
    }

    /// Marks the span as bold (`FontWeight::BOLD`).
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Marks the span as italic.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Adds underline decoration.
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Adds strikethrough decoration.
    #[must_use]
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Formats the span as inline monospace code with a subtle tinted container.
    #[must_use]
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Sets an explicit foreground color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets an explicit background pill highlight color.
    #[must_use]
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Sets a relative font scale multiplier (`1.0` = 100%).
    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.5);
        self
    }

    /// Turns the span into an interactive clickable link emitting `link_id`.
    #[must_use]
    pub fn link(mut self, link_id: impl Into<ArcStr>) -> Self {
        self.link_id = Some(link_id.into());
        self
    }
}

/// A styled segment of inline text within [`UiRichProse`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    /// Text slice rendered for this span.
    pub text: ArcStr,
    /// Visual and link styling applied to this slice.
    pub style: SpanStyle,
}

impl TextSpan {
    /// Plain span with default paragraph typography.
    #[must_use]
    pub fn new(text: impl Into<ArcStr>) -> Self {
        Self {
            text: text.into(),
            style: SpanStyle::plain(),
        }
    }

    /// Bold inline span.
    #[must_use]
    pub fn bold(text: impl Into<ArcStr>) -> Self {
        Self::new(text).styled(SpanStyle::plain().bold())
    }

    /// Italic inline span.
    #[must_use]
    pub fn italic(text: impl Into<ArcStr>) -> Self {
        Self::new(text).styled(SpanStyle::plain().italic())
    }

    /// Inline monospace code span.
    #[must_use]
    pub fn code(text: impl Into<ArcStr>) -> Self {
        Self::new(text).styled(SpanStyle::plain().code())
    }

    /// Clickable hyperlink span emitting `link_id` on press-release.
    #[must_use]
    pub fn link(link_id: impl Into<ArcStr>, text: impl Into<ArcStr>) -> Self {
        Self::new(text).styled(SpanStyle::plain().link(link_id))
    }

    /// Replaces the span style fluently.
    #[must_use]
    pub fn styled(mut self, style: SpanStyle) -> Self {
        self.style = style;
        self
    }
}

/// Widget action emitted by [`RichTextWidget`] when a link span is clicked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichTextAction {
    /// User clicked the link span with the given identifier.
    LinkClicked(ArcStr),
}

#[derive(Clone, Debug, PartialEq)]
struct ResolvedSpan {
    range: Range<usize>,
    style: SpanStyle,
    brush_idx: usize,
    resolved_bg: Option<Color>,
}

/// Masonry widget rendering styled inline spans with pointer selection, clipboard copy, and link clicks.
#[expect(
    missing_debug_implementations,
    reason = "Holds Parley Layout which does not implement Debug"
)]
pub struct RichTextWidget {
    full_text: String,
    spans: Vec<ResolvedSpan>,
    brushes: Vec<Brush>,
    base_size: f32,
    base_weight: FontWeight,
    base_line_height: f32,
    base_font: FontFamily<'static>,
    selection_color: Color,
    selectable: bool,
    layout: Option<Layout<BrushIndex>>,
    last_max_advance: Option<f32>,
    selection: Option<(usize, usize)>,
    drag_anchor: usize,
    dragging: bool,
    pressed_link: Option<ArcStr>,
    hovered_link: Option<ArcStr>,
}

#[derive(Clone, PartialEq)]
struct RichLayoutConfig {
    full_text: String,
    spans: Vec<ResolvedSpan>,
    brushes: Vec<Brush>,
    base_size: f32,
    base_weight: FontWeight,
    base_line_height: f32,
    base_font: FontFamily<'static>,
    selection_color: Color,
    selectable: bool,
}

fn compile_rich_config(
    raw_spans: &[TextSpan],
    theme: &Theme,
    role: TypeRole,
    color_override: Option<Color>,
    selectable: bool,
) -> RichLayoutConfig {
    let s = &theme.scheme;
    let ts = theme.typography.get(role);
    let base_fg = color_override.unwrap_or(s.on_surface);
    let mut brushes = vec![Brush::Solid(base_fg)];
    let mut full_text = String::new();
    let mut resolved = Vec::with_capacity(raw_spans.len());

    for span in raw_spans {
        if span.text.is_empty() {
            continue;
        }
        let start = full_text.len();
        full_text.push_str(&span.text);
        let end = full_text.len();

        let fg = if let Some(c) = span.style.color {
            c
        } else if span.style.link_id.is_some() {
            s.primary
        } else if span.style.code {
            s.on_surface
        } else {
            base_fg
        };

        let brush_idx = if fg == base_fg {
            0
        } else {
            let idx = brushes.len();
            brushes.push(Brush::Solid(fg));
            idx
        };

        let resolved_bg = span.style.bg.or_else(|| {
            span.style
                .code
                .then(|| with_opacity(s.surface_container, 0.85))
        });

        resolved.push(ResolvedSpan {
            range: start..end,
            style: span.style.clone(),
            brush_idx,
            resolved_bg,
        });
    }

    RichLayoutConfig {
        full_text,
        spans: resolved,
        brushes,
        base_size: ts.font_size,
        base_weight: ts.weight,
        base_line_height: ts.line_height,
        base_font: theme.typography.resolved_font_family(),
        selection_color: with_opacity(s.primary, 0.25),
        selectable,
    }
}

impl RichTextWidget {
    fn from_config(cfg: RichLayoutConfig) -> Self {
        Self {
            full_text: cfg.full_text,
            spans: cfg.spans,
            brushes: cfg.brushes,
            base_size: cfg.base_size,
            base_weight: cfg.base_weight,
            base_line_height: cfg.base_line_height,
            base_font: cfg.base_font,
            selection_color: cfg.selection_color,
            selectable: cfg.selectable,
            layout: None,
            last_max_advance: None,
            selection: None,
            drag_anchor: 0,
            dragging: false,
            pressed_link: None,
            hovered_link: None,
        }
    }

    fn apply_config(this: &mut WidgetMut<'_, Self>, cfg: RichLayoutConfig) {
        let text_changed = this.widget.full_text != cfg.full_text;
        let layout_changed = text_changed
            || this.widget.spans != cfg.spans
            || (this.widget.base_size - cfg.base_size).abs() > 0.01
            || this.widget.base_weight != cfg.base_weight
            || (this.widget.base_line_height - cfg.base_line_height).abs() > 0.01
            || this.widget.base_font != cfg.base_font;

        if layout_changed {
            this.widget.full_text = cfg.full_text;
            this.widget.spans = cfg.spans;
            this.widget.base_size = cfg.base_size;
            this.widget.base_weight = cfg.base_weight;
            this.widget.base_line_height = cfg.base_line_height;
            this.widget.base_font = cfg.base_font;
            this.widget.layout = None;
            this.widget.last_max_advance = None;
            if text_changed {
                this.widget.selection = None;
                this.widget.dragging = false;
                this.widget.pressed_link = None;
                this.widget.hovered_link = None;
            }
            this.ctx.request_layout();
            this.ctx.request_accessibility_update();
        }
        if this.widget.brushes != cfg.brushes || this.widget.selection_color != cfg.selection_color
        {
            this.widget.brushes = cfg.brushes;
            this.widget.selection_color = cfg.selection_color;
            this.ctx.request_paint_only();
        }
        this.widget.selectable = cfg.selectable;
    }

    fn build_parley_layout(
        &self,
        font_ctx: &mut masonry::parley::FontContext,
        layout_ctx: &mut masonry::parley::LayoutContext<BrushIndex>,
        max_advance: Option<f32>,
    ) -> Layout<BrushIndex> {
        let mut layout = Layout::new();
        let mut builder = layout_ctx.ranged_builder(font_ctx, &self.full_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(self.base_size));
        builder.push_default(StyleProperty::FontWeight(self.base_weight));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
            self.base_line_height,
        )));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        builder.push_default(StyleProperty::FontFamily(self.base_font.clone()));

        for span in &self.spans {
            let r = span.range.clone();
            if span.style.bold {
                builder.push(StyleProperty::FontWeight(FontWeight::BOLD), r.clone());
            }
            if span.style.italic {
                builder.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
            }
            if span.style.underline || span.style.link_id.is_some() {
                builder.push(StyleProperty::Underline(true), r.clone());
            }
            if span.style.strikethrough {
                builder.push(StyleProperty::Strikethrough(true), r.clone());
            }
            if span.style.code {
                builder.push(
                    StyleProperty::FontFamily(FontFamily::from(GenericFamily::Monospace)),
                    r.clone(),
                );
            }
            if (span.style.scale - 1.0).abs() > 0.01 {
                builder.push(
                    StyleProperty::FontSize(self.base_size * span.style.scale),
                    r.clone(),
                );
            }
            if span.brush_idx != 0 {
                builder.push(StyleProperty::Brush(BrushIndex(span.brush_idx)), r);
            }
        }

        builder.build_into(&mut layout, &self.full_text);
        layout.break_all_lines(max_advance);
        layout.align(max_advance, TextAlign::Start, AlignmentOptions::default());
        layout
    }

    fn hit_byte_index(&self, local: Point) -> usize {
        let Some(layout) = &self.layout else {
            return 0;
        };
        let cursor = Cursor::from_point(layout, local.x as f32, local.y as f32);
        cursor.index().min(self.full_text.len())
    }

    fn link_at_point(&self, local: Point) -> Option<ArcStr> {
        let layout = self.layout.as_ref()?;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        self.spans.iter().find_map(|s| {
            let link = s.style.link_id.as_ref()?;
            if s.range.start >= s.range.end {
                return None;
            }
            let mut inside = false;
            let a = Cursor::from_byte_index(layout, s.range.start, Affinity::Downstream);
            let b = Cursor::from_byte_index(layout, s.range.end, Affinity::Upstream);
            Selection::new(a, b).geometry_with(layout, |bb, _| {
                if local.x >= bb.x0 - 2.0
                    && local.x <= bb.x1 + 2.0
                    && local.y >= bb.y0 - 2.0
                    && local.y <= bb.y1 + 2.0
                {
                    inside = true;
                }
            });
            inside.then(|| link.clone())
        })
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        let (a, b) = self.selection?;
        let lo = a.min(b).min(self.full_text.len());
        let hi = a.max(b).min(self.full_text.len());
        (lo < hi).then_some(lo..hi)
    }

    fn paint_range_bg(
        painter: &mut Painter<'_>,
        layout: &Layout<BrushIndex>,
        range: Range<usize>,
        color: Color,
        radius: f64,
        pad_x: f64,
        max_width: f64,
    ) {
        let start = Cursor::from_byte_index(layout, range.start, Affinity::Downstream);
        let end = Cursor::from_byte_index(layout, range.end, Affinity::Upstream);
        Selection::new(start, end).geometry_with(layout, |bb, _| {
            let x0 = (bb.x0 - pad_x).max(0.0);
            let x1 = (bb.x1 + pad_x).min(max_width.max(x0));
            let r = Rect::new(x0, bb.y0, x1, bb.y1);
            if radius > 0.0 {
                painter
                    .fill(RoundedRect::from_rect(r, radius), color)
                    .draw();
            } else {
                painter.fill(r, color).draw();
            }
        });
    }
}

impl Widget for RichTextWidget {
    type Action = RichTextAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn accepts_focus(&self) -> bool {
        self.selectable
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Move(PointerUpdate { current, .. }) => {
                let local = ctx.local_position(current.position);
                let new_hover = self.link_at_point(local);
                if new_hover != self.hovered_link {
                    self.hovered_link = new_hover;
                    ctx.request_cursor_icon_change();
                }
                if self.dragging && self.selectable {
                    let idx = self.hit_byte_index(local);
                    self.selection = Some((self.drag_anchor, idx));
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Down(btn_ev) if btn_ev.button == Some(PointerButton::Primary) => {
                let local = ctx.local_position(btn_ev.state.position);
                self.pressed_link = self.link_at_point(local);
                if self.selectable {
                    let idx = self.hit_byte_index(local);
                    self.drag_anchor = idx;
                    self.dragging = true;
                    self.selection = Some((idx, idx));
                    ctx.request_focus();
                    ctx.capture_pointer();
                    ctx.request_paint_only();
                    ctx.set_handled();
                } else if self.pressed_link.is_some() {
                    ctx.capture_pointer();
                    ctx.set_handled();
                }
            }
            PointerEvent::Up(btn_ev) if btn_ev.button == Some(PointerButton::Primary) => {
                let local = ctx.local_position(btn_ev.state.position);
                let had_drag_selection = self.selected_range().is_some();
                self.dragging = false;
                if let Some(pressed) = self.pressed_link.take()
                    && !had_drag_selection
                    && self.link_at_point(local).as_ref() == Some(&pressed)
                {
                    ctx.submit_action::<RichTextAction>(RichTextAction::LinkClicked(pressed));
                    ctx.set_handled();
                }
            }
            PointerEvent::Leave(_) => {
                self.hovered_link = None;
            }
            PointerEvent::Cancel(_) => {
                self.hovered_link = None;
                self.dragging = false;
                self.pressed_link = None;
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
        if let TextEvent::Keyboard(key_ev) = event
            && key_ev.state.is_down()
            && self.selectable
        {
            let primary_mod = key_ev.modifiers.ctrl() || key_ev.modifiers.meta();
            if primary_mod
                && matches!(&key_ev.key, Key::Character(c) if c.eq_ignore_ascii_case("c"))
                && let Some(range) = self.selected_range()
            {
                ctx.set_clipboard(self.full_text[range].to_string());
                ctx.set_handled();
            } else if matches!(key_ev.key, Key::Named(NamedKey::Escape))
                && self.selection.take().is_some()
            {
                ctx.request_paint_only();
                ctx.set_handled();
            }
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == accesskit::Action::Click
            && let Some(link) = self
                .spans
                .iter()
                .find_map(|s| s.style.link_id.as_ref().cloned())
        {
            ctx.submit_action::<RichTextAction>(RichTextAction::LinkClicked(link));
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::FocusChanged(false) => {
                self.selection = None;
                self.dragging = false;
                ctx.request_paint_only();
            }
            Update::FontsChanged => {
                self.layout = None;
                self.last_max_advance = None;
                ctx.request_layout();
            }
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
        let max_advance = match axis {
            Axis::Horizontal => match len_req {
                LenReq::MinContent => Some(0.0),
                LenReq::MaxContent => None,
                LenReq::FitContent(space) => Some(space.get() as f32),
            },
            Axis::Vertical => cross_length
                .or_else(|| ctx.context_size().length(Axis::Horizontal))
                .map(|l| l.get().max(1.0) as f32),
        };
        let (fctx, lctx) = ctx.text_contexts();
        let layout = self.build_parley_layout(fctx, lctx, max_advance);
        match axis {
            Axis::Horizontal => Length::px(match max_advance {
                Some(limit) => (layout.width() as f64).min(limit as f64),
                None => layout.full_width() as f64,
            }),
            Axis::Vertical => Length::px(layout.height() as f64),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let max_advance = Some(size.width.max(1.0) as f32);
        if self.layout.is_none() || self.last_max_advance != max_advance {
            let (fctx, lctx) = ctx.text_contexts();
            self.layout = Some(self.build_parley_layout(fctx, lctx, max_advance));
            self.last_max_advance = max_advance;
        }
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let Some(layout) = &self.layout else {
            return;
        };
        let content_w = ctx.content_box().size().width;
        for span in &self.spans {
            if let Some(bg) = span.resolved_bg {
                Self::paint_range_bg(painter, layout, span.range.clone(), bg, 4.0, 3.0, content_w);
            }
        }
        if let Some(sel) = self.selected_range() {
            Self::paint_range_bg(
                painter,
                layout,
                sel,
                self.selection_color,
                0.0,
                0.0,
                content_w,
            );
        }
        render_text(painter, Affine::IDENTITY, layout, &self.brushes, true);
    }

    fn get_cursor(&self, _ctx: &QueryCtx<'_>, pos: Point) -> CursorIcon {
        if self.link_at_point(pos).is_some() {
            CursorIcon::Pointer
        } else if self.selectable {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Paragraph
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_value(&*self.full_text);
        if let Some(first_link) = self
            .spans
            .iter()
            .find_map(|s| s.style.link_id.as_ref().cloned())
        {
            node.set_description(first_link.as_ref());
            node.add_action(accesskit::Action::Click);
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Callback triggered when an inline link span inside [`UiRichProse`] is clicked.
pub type LinkClickCallback<State, Action> = Arc<dyn Fn(&mut State, ArcStr) -> Action + Send + Sync>;

/// Spec-driven selectable multi-span rich text paragraph view.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Holds optional closure callback"
)]
pub struct UiRichProse<State, Action> {
    spans: Vec<TextSpan>,
    role: TypeRole,
    color_override: Option<Color>,
    selectable: bool,
    on_link_click: Option<LinkClickCallback<State, Action>>,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Creates a rich inline paragraph from styled [`TextSpan`]s consuming [`Theme`](crate::theme::Theme) typography.
pub fn rich_prose<State, Action>(spans: Vec<TextSpan>) -> UiRichProse<State, Action> {
    UiRichProse {
        spans,
        role: TypeRole::BodyMedium,
        color_override: None,
        selectable: true,
        on_link_click: None,
        phantom: PhantomData,
    }
}

impl<State, Action> UiRichProse<State, Action> {
    /// Overrides the base [`TypeRole`] (defaults to `TypeRole::BodyMedium`).
    pub fn text_role(mut self, role: TypeRole) -> Self {
        self.role = role;
        self
    }

    /// Overrides the default paragraph text color (`palette.text_primary`).
    pub fn text_color(mut self, color: Color) -> Self {
        self.color_override = Some(color);
        self
    }

    /// Controls whether pointer text selection & `Ctrl/Cmd+C` clipboard copy are enabled (`true` by default).
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// Attaches a handler called when the user clicks any span marked with [`SpanStyle::link`].
    pub fn on_link_click<F>(mut self, on_link_click: F) -> Self
    where
        F: Fn(&mut State, ArcStr) -> Action + Send + Sync + 'static,
    {
        self.on_link_click = Some(Arc::new(on_link_click));
        self
    }
}

impl<State, Action> ViewMarker for UiRichProse<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiRichProse<State, Action> {
    type Element = Pod<RichTextWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let cfg = compile_rich_config(
            &self.spans,
            &theme,
            self.role,
            self.color_override,
            self.selectable,
        );
        (
            ctx.with_action_widget(|ctx| ctx.create_pod(RichTextWidget::from_config(cfg))),
            (),
        )
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let theme = current_theme_arc(ctx);
        let cfg = compile_rich_config(
            &self.spans,
            &theme,
            self.role,
            self.color_override,
            self.selectable,
        );
        RichTextWidget::apply_config(&mut element, cfg);
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_message::<RichTextAction>() {
            Some(action) => match *action {
                RichTextAction::LinkClicked(link_id) => {
                    if let Some(cb) = &self.on_link_click {
                        MessageResult::Action(cb(app_state, link_id))
                    } else {
                        MessageResult::Nop
                    }
                }
            },
            None => MessageResult::Stale,
        }
    }
}
