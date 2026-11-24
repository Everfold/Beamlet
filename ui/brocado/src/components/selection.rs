//! Theme-authentic custom-painted selection controls: switch, checkbox, tri-state checkbox, radio, and slider.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{self, Node, Orientation, Role, Toggled};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, ArcStr, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget,
    PaintCtx, PointerButton, PointerButtonEvent, PointerEvent, PointerUpdate, PropertiesMut,
    PropertiesRef, PropertySet, RegisterCtx, StyleProperty, TextEvent, Update, UpdateCtx, Widget,
    WidgetId, WidgetMut, WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, BezPath, Circle, Line, Point, Rect, RoundedRect, Size, Stroke};
use masonry::layout::{AsUnit, LayoutSize, LenReq, Length, SizeDef};
use masonry::parley::LineHeight;
use masonry::parley::style::{FontFamily, OverflowWrap};
use masonry::peniko::Color;
use masonry::properties::{ContentColor, Dimensions, LineBreaking};
use masonry::widgets::{self, RadioButtonSelected};
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt as _, button, flex_col, flex_row, radio_group as raw_radio_group,
    sized_box,
};

use crate::components::typography::ui_label;
use crate::foundation::state_layer::{blend_over, with_opacity};
use crate::foundation::{UiClassExt, clip_box, icons, ui_icon};
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::{
    Brightness, ControlVisualGrammar, DesignSpec, MotionDuration, MotionEasing, TextStyleSpec,
    TypeRole,
};

const SWITCH_ANIM_NS: u64 = MotionDuration::Short2.as_millis() * 1_000_000;

/// Compact copyable visual snapshot extracted from [`DesignSpec`] for custom-painted controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ControlVisualTheme {
    pub grammar: ControlVisualGrammar,
    pub accent: Color,
    pub on_accent: Color,
    pub surface: Color,
    pub surface_highest: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub disabled_content: Color,
    pub disabled_container: Color,
    pub shadow: Color,
    pub brightness: Brightness,
    pub reduced_motion: bool,
    pub checkbox_radius: f64,
    pub switch_width: f64,
    pub switch_height: f64,
}

impl ControlVisualTheme {
    pub(crate) fn from_spec(spec: &DesignSpec) -> Self {
        let p = &spec.palette;
        let m = &spec.metrics;
        Self {
            grammar: spec.control_grammar(),
            accent: p.accent,
            on_accent: p.on_accent,
            surface: p.surface,
            surface_highest: p.surface_highest,
            border: p.border,
            border_strong: p.border_strong,
            text_primary: p.text_primary,
            text_secondary: p.text_secondary,
            disabled_content: spec.disabled_content(),
            disabled_container: spec.disabled_container(),
            shadow: p.shadow,
            brightness: spec.brightness,
            reduced_motion: spec.reduced_motion,
            checkbox_radius: m.checkbox_radius,
            switch_width: m.switch_width,
            switch_height: m.switch_height,
        }
    }

    pub(crate) fn effective_accent(self, window_focused: bool, disabled: bool) -> Color {
        if disabled {
            self.disabled_content
        } else if self.grammar == ControlVisualGrammar::MacosHig && !window_focused {
            match self.brightness {
                Brightness::Light => Color::from_rgb8(0x9A, 0x9A, 0xA0),
                Brightness::Dark => Color::from_rgb8(0x6E, 0x6E, 0x74),
            }
        } else {
            self.accent
        }
    }

    pub(crate) fn label_color(self, disabled: bool) -> Color {
        if disabled {
            self.disabled_content
        } else {
            self.text_primary
        }
    }

    pub(crate) fn halo_color(self, accent: Color, active: bool) -> Color {
        let alpha = match (self.grammar, active) {
            (ControlVisualGrammar::Material3, true) => 0.14,
            (ControlVisualGrammar::Material3, false) => 0.09,
            (ControlVisualGrammar::Adwaita, true) => 0.14,
            (ControlVisualGrammar::Adwaita, false) => 0.08,
            (_, true) => 0.12,
            (_, false) => 0.07,
        };
        with_opacity(accent, alpha)
    }
}

fn label_typography_differs(
    a: &(TextStyleSpec, FontFamily<'static>),
    b: &(TextStyleSpec, FontFamily<'static>),
) -> bool {
    a.0 != b.0 || a.1 != b.1
}

fn prepare_label_pod(
    text: ArcStr,
    style: TextStyleSpec,
    font: &FontFamily<'static>,
    color: Color,
) -> NewWidget<widgets::Label> {
    let lbl = widgets::Label::new(text)
        .with_style(StyleProperty::FontSize(style.font_size))
        .with_style(StyleProperty::FontWeight(style.weight))
        .with_style(StyleProperty::LineHeight(LineHeight::Absolute(
            style.line_height,
        )))
        .with_style(StyleProperty::LetterSpacing(style.letter_spacing))
        .with_style(StyleProperty::FontFamily(font.clone()))
        .with_style(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
    NewWidget::new(lbl).with_props(PropertySet::new().with(ContentColor::new(color)))
}

fn sync_label_pod(
    mut label: WidgetMut<'_, widgets::Label>,
    text_changed: bool,
    new_text: ArcStr,
    prev_typo: &mut (TextStyleSpec, FontFamily<'static>),
    next_typo: &(TextStyleSpec, FontFamily<'static>),
    color: Color,
    prev_color: Color,
) {
    if text_changed {
        widgets::Label::set_text(&mut label, new_text);
    }
    if prev_color != color {
        label.insert_prop(ContentColor::new(color));
    }
    if label_typography_differs(prev_typo, next_typo) {
        if prev_typo.0.font_size != next_typo.0.font_size {
            widgets::Label::insert_style(
                &mut label,
                StyleProperty::FontSize(next_typo.0.font_size),
            );
        }
        if prev_typo.0.weight != next_typo.0.weight {
            widgets::Label::insert_style(&mut label, StyleProperty::FontWeight(next_typo.0.weight));
        }
        if prev_typo.0.line_height != next_typo.0.line_height {
            widgets::Label::insert_style(
                &mut label,
                StyleProperty::LineHeight(LineHeight::Absolute(next_typo.0.line_height)),
            );
        }
        if prev_typo.0.letter_spacing != next_typo.0.letter_spacing {
            widgets::Label::insert_style(
                &mut label,
                StyleProperty::LetterSpacing(next_typo.0.letter_spacing),
            );
        }
        if prev_typo.1 != next_typo.1 {
            widgets::Label::insert_style(
                &mut label,
                StyleProperty::FontFamily(next_typo.1.clone()),
            );
        }
        *prev_typo = next_typo.clone();
    }
}

fn draw_checkmark(painter: &mut Painter<'_>, rect: Rect, color: Color, stroke_w: f64) {
    let mut path = BezPath::new();
    path.move_to(Point::new(
        rect.x0 + rect.width() * 0.22,
        rect.y0 + rect.height() * 0.52,
    ));
    path.line_to(Point::new(
        rect.x0 + rect.width() * 0.43,
        rect.y0 + rect.height() * 0.73,
    ));
    path.line_to(Point::new(
        rect.x0 + rect.width() * 0.79,
        rect.y0 + rect.height() * 0.30,
    ));
    painter.stroke(path, &Stroke::new(stroke_w), color).draw();
}

fn draw_focus_ring(
    painter: &mut Painter<'_>,
    rrect: RoundedRect,
    visual: ControlVisualTheme,
    focused: bool,
    win_focused: bool,
) {
    if !focused || !win_focused {
        return;
    }
    let halo = visual.accent;
    if visual.grammar == ControlVisualGrammar::MacosHig {
        let outer = rrect
            .rect()
            .inflate(2.0, 2.0)
            .to_rounded_rect(rrect.radii().top_left + 2.0);
        painter
            .stroke(outer, &Stroke::new(3.0), with_opacity(halo, 0.35))
            .draw();
    } else {
        let outer = rrect
            .rect()
            .inflate(1.5, 1.5)
            .to_rounded_rect(rrect.radii().top_left + 1.5);
        painter
            .stroke(outer, &Stroke::new(1.5), with_opacity(halo, 0.85))
            .draw();
    }
}

/// Logical state of a tri-state checkbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToggleableState {
    /// Unchecked (`Off`).
    #[default]
    Off,
    /// Checked (`On`).
    On,
    /// Partially selected (`Indeterminate`).
    Indeterminate,
}

/// Callback type for [`checkbox`] / [`switch`].
pub type BoolCallback<State, Action> = Arc<dyn Fn(&mut State, bool) -> Action + Send + Sync>;

/// Action emitted when a [`UiCheckbox`] is toggled by user interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckboxToggled(pub bool);

/// Custom theme-painted checkbox honoring MD3, Fluent 2, macOS HIG, and GNOME Adwaita geometry.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct UiCheckbox {
    checked: bool,
    label: WidgetPod<widgets::Label>,
    visual: ControlVisualTheme,
}

impl UiCheckbox {
    fn new(checked: bool, label: NewWidget<widgets::Label>, visual: ControlVisualTheme) -> Self {
        Self {
            checked,
            label: label.to_pod(),
            visual,
        }
    }

    fn set_checked(this: &mut WidgetMut<'_, Self>, checked: bool) {
        if this.widget.checked != checked {
            this.widget.checked = checked;
            this.ctx.request_accessibility_update();
            this.ctx.request_paint_only();
        }
    }

    fn set_visual(this: &mut WidgetMut<'_, Self>, visual: ControlVisualTheme) {
        if this.widget.visual != visual {
            let layout_diff = this.widget.visual.grammar != visual.grammar;
            this.widget.visual = visual;
            if layout_diff {
                this.ctx.request_layout();
            } else {
                this.ctx.request_paint_only();
            }
        }
    }

    fn label_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, widgets::Label> {
        this.ctx.get_mut(&mut this.widget.label)
    }

    fn box_side(&self) -> f64 {
        match self.visual.grammar {
            ControlVisualGrammar::MacosHig => 16.0,
            _ => 18.0,
        }
    }
}

impl Widget for UiCheckbox {
    type Action = CheckboxToggled;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.label);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                if ctx.is_active() && ctx.is_hovered() {
                    ctx.submit_action::<Self::Action>(CheckboxToggled(!self.checked));
                }
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
        if ctx.is_disabled() {
            return;
        }
        match event {
            TextEvent::Keyboard(ke)
                if ctx.is_focus_target()
                    && ke.state.is_up()
                    && (matches!(&ke.key, Key::Character(c) if c == " ")
                        || ke.key == Key::Named(NamedKey::Enter)) =>
            {
                ctx.submit_action::<Self::Action>(CheckboxToggled(!self.checked));
            }
            TextEvent::WindowFocusChange(_) => ctx.request_paint_only(),
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        if event.action == accesskit::Action::Click {
            ctx.submit_action::<Self::Action>(CheckboxToggled(!self.checked));
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
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
        let side = self.box_side();
        let gap = 8.0;
        let other = match axis {
            Axis::Horizontal => Length::px(side + gap),
            Axis::Vertical => Length::ZERO,
        };
        let cross = axis.cross();
        let cross_other = match cross {
            Axis::Horizontal => Length::px(side + gap),
            Axis::Vertical => Length::ZERO,
        };
        let cross_space = cross_length.map(|l| l.saturating_sub(cross_other));
        let auto_len = len_req.reduce(other).into();
        let context_size = LayoutSize::maybe(cross, cross_space);
        let label_len =
            ctx.compute_length(&mut self.label, auto_len, context_size, axis, cross_space);
        match axis {
            Axis::Horizontal => label_len.saturating_add(other),
            Axis::Vertical => label_len.max(Length::px(side + 4.0)),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let side = self.box_side();
        let gap = 8.0;
        let avail_w = (size.width - (side + gap)).max(0.0);
        let space = Size::new(avail_w, size.height);
        let label_size = ctx.compute_size(&mut self.label, SizeDef::fit(space), space.into());
        ctx.run_layout(&mut self.label, label_size);
        let origin_y = ((size.height - label_size.height) * 0.5).max(0.0);
        ctx.place_child(&mut self.label, Point::new(side + gap, origin_y));
        ctx.derive_baselines(&self.label);
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let disabled = ctx.is_disabled();
        let hovered = ctx.is_hovered();
        let active = ctx.is_active();
        let win_focused = ctx.is_window_focused();
        let side = self.box_side();
        let size = ctx.content_box().size();
        let y0 = (size.height - side) * 0.5;
        let box_rect = Rect::new(0.0, y0, side, y0 + side);
        let r = self.visual.checkbox_radius;
        let rrect = box_rect.to_rounded_rect(r);
        let accent = self.visual.effective_accent(win_focused, disabled);

        if !disabled && (hovered || active) {
            let halo = box_rect.inflate(4.0, 4.0).to_rounded_rect(r + 4.0);
            painter
                .fill(halo, self.visual.halo_color(accent, active))
                .draw();
        }

        if self.checked {
            let fill = if !disabled && active {
                blend_over(accent, self.visual.on_accent, 0.14)
            } else {
                accent
            };
            painter.fill(rrect, fill).draw();
            if self.visual.grammar == ControlVisualGrammar::MacosHig && !disabled {
                let border_c = blend_over(fill, Color::BLACK, 0.14);
                painter.stroke(rrect, &Stroke::new(1.0), border_c).draw();
            }
            let check_ink = if disabled {
                self.visual.surface
            } else {
                self.visual.on_accent
            };
            draw_checkmark(painter, box_rect, check_ink, 2.0);
        } else {
            let bg = if disabled {
                self.visual.disabled_container
            } else if hovered {
                blend_over(self.visual.surface, self.visual.text_primary, 0.04)
            } else {
                self.visual.surface
            };
            let stroke_w = if self.visual.grammar == ControlVisualGrammar::Material3 {
                2.0
            } else {
                1.5
            };
            let border_c = if disabled {
                self.visual.disabled_content
            } else if hovered || active {
                self.visual.text_primary
            } else {
                self.visual.border_strong
            };
            painter.fill(rrect, bg).draw();
            painter
                .stroke(
                    box_rect
                        .inset(stroke_w * 0.5)
                        .to_rounded_rect((r - 0.5).max(0.0)),
                    &Stroke::new(stroke_w),
                    border_c,
                )
                .draw();
        }

        draw_focus_ring(
            painter,
            rrect,
            self.visual,
            ctx.is_focus_target(),
            win_focused,
        );
    }

    fn accessibility_role(&self) -> Role {
        Role::CheckBox
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.add_action(accesskit::Action::Click);
        node.set_toggled(if self.checked {
            Toggled::True
        } else {
            Toggled::False
        });
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.label.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("UiCheckbox", id = id.trace())
    }
}

/// View creating a [`UiCheckbox`] styled dynamically from ambient [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiCheckboxView<State, Action> {
    text: ArcStr,
    checked: bool,
    disabled: bool,
    on_toggle: BoolCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiCheckboxView<State, Action> {
    /// Disables pointer and keyboard interaction when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, Action> ViewMarker for UiCheckboxView<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for UiCheckboxView<State, Action>
{
    type Element = Pod<UiCheckbox>;
    type ViewState = (TextStyleSpec, FontFamily<'static>, Color);

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let ts = spec.typography.get(TypeRole::BodyMedium);
        let font = spec.typography.resolved_font_family();
        let color = visual.label_color(self.disabled);
        let lbl = prepare_label_pod(self.text.clone(), ts, &font, color);
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(UiCheckbox::new(self.checked, lbl, visual));
            pod.new_widget.options.disabled = self.disabled;
            pod
        });
        (pod, (ts, font, color))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (prev_ts, prev_font, prev_color): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let next_ts = spec.typography.get(TypeRole::BodyMedium);
        let next_font = spec.typography.resolved_font_family();
        let next_color = visual.label_color(self.disabled);

        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        UiCheckbox::set_checked(&mut element, self.checked);
        UiCheckbox::set_visual(&mut element, visual);
        let mut typo = (*prev_ts, prev_font.clone());
        sync_label_pod(
            UiCheckbox::label_mut(&mut element),
            prev.text != self.text,
            self.text.clone(),
            &mut typo,
            &(next_ts, next_font),
            next_color,
            *prev_color,
        );
        *prev_ts = typo.0;
        *prev_font = typo.1;
        *prev_color = next_color;
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
        match message.take_message::<CheckboxToggled>() {
            Some(msg) => MessageResult::Action((self.on_toggle)(app_state, msg.0)),
            None => MessageResult::Stale,
        }
    }
}

/// Spec-styled theme-authentic checkbox with label.
pub fn checkbox<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    checked: bool,
    on_toggle: F,
) -> UiCheckboxView<State, Action>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    UiCheckboxView {
        text: text.into(),
        checked,
        disabled: false,
        on_toggle: Arc::new(on_toggle),
        phantom: PhantomData,
    }
}

/// Tri-state checkbox cycling `Off -> On -> Off` (or `Indeterminate -> On`).
pub fn tri_state_checkbox<State: 'static, Action: 'static, F>(
    label_text: impl Into<ArcStr>,
    state: ToggleableState,
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State, ToggleableState) -> Action + Send + Sync + 'static,
{
    let label_text = label_text.into();
    let cb = Arc::new(on_toggle);
    crate::style_engine::with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let next = match state {
            ToggleableState::Off | ToggleableState::Indeterminate => ToggleableState::On,
            ToggleableState::On => ToggleableState::Off,
        };
        let (box_bg, border_c, icon_opt) = match state {
            ToggleableState::Off => (Color::TRANSPARENT, p.border_strong, None),
            ToggleableState::On => (p.accent, p.accent, Some(icons::CHECK)),
            ToggleableState::Indeterminate => {
                (p.accent, p.accent, Some(icons::INDETERMINATE_MINUS))
            }
        };
        let box_view = sized_box(
            icon_opt
                .map(|path| ui_icon(path, 14.0, p.on_accent))
                .unwrap_or_else(|| ui_icon(icons::CHECK, 0.0, Color::TRANSPARENT)),
        )
        .dims(Dimensions::fixed(18.px(), 18.px()))
        .background_color(box_bg)
        .border(border_c, 2.px())
        .corner_radius(spec.metrics.checkbox_radius.px());

        let text_view = clip_box(
            ui_label(TypeRole::BodyMedium, label_text.clone())
                .color(p.text_primary)
                .line_breaking(LineBreaking::Clip),
        );
        let cb = cb.clone();
        button(
            clip_box(
                flex_row((box_view, text_view.flex(1.0)))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
            ),
            move |s: &mut State| cb(s, next),
        )
        .ui_class(classes::MENU_ITEM)
    })
}

/// Action emitted when [`UiSwitch`] toggles state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwitchToggled(pub bool);

/// Custom theme-painted animated toggle switch honoring MD3, Fluent 2, macOS HIG, and Adwaita.
#[derive(Debug)]
pub struct UiSwitch {
    on: bool,
    anim_t: f64,
    elapsed_ns: u64,
    visual: ControlVisualTheme,
}

impl UiSwitch {
    fn new(on: bool, visual: ControlVisualTheme) -> Self {
        Self {
            on,
            anim_t: if on { 1.0 } else { 0.0 },
            elapsed_ns: SWITCH_ANIM_NS,
            visual,
        }
    }

    fn set_on(this: &mut WidgetMut<'_, Self>, on: bool) {
        if this.widget.on != on {
            this.widget.on = on;
            if this.widget.visual.reduced_motion {
                this.widget.anim_t = if on { 1.0 } else { 0.0 };
                this.widget.elapsed_ns = SWITCH_ANIM_NS;
                this.ctx.request_paint_only();
            } else {
                this.widget.elapsed_ns = 0;
                this.ctx.request_anim_frame();
                this.ctx.request_paint_only();
            }
            this.ctx.request_accessibility_update();
        }
    }

    fn set_visual(this: &mut WidgetMut<'_, Self>, visual: ControlVisualTheme) {
        if this.widget.visual != visual {
            let size_changed = (this.widget.visual.switch_width - visual.switch_width).abs() > 1e-6
                || (this.widget.visual.switch_height - visual.switch_height).abs() > 1e-6;
            this.widget.visual = visual;
            if size_changed {
                this.ctx.request_layout();
            } else {
                this.ctx.request_paint_only();
            }
        }
    }
}

impl Widget for UiSwitch {
    type Action = SwitchToggled;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                if ctx.is_active() && ctx.is_hovered() {
                    ctx.submit_action::<Self::Action>(SwitchToggled(!self.on));
                }
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
        if ctx.is_disabled() {
            return;
        }
        match event {
            TextEvent::Keyboard(ke)
                if ctx.is_focus_target()
                    && ke.state.is_up()
                    && (matches!(&ke.key, Key::Character(c) if c == " ")
                        || ke.key == Key::Named(NamedKey::Enter)) =>
            {
                ctx.submit_action::<Self::Action>(SwitchToggled(!self.on));
            }
            TextEvent::WindowFocusChange(_) => ctx.request_paint_only(),
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        if event.action == accesskit::Action::Click {
            ctx.submit_action::<Self::Action>(SwitchToggled(!self.on));
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.visual.reduced_motion || self.elapsed_ns >= SWITCH_ANIM_NS {
            self.anim_t = if self.on { 1.0 } else { 0.0 };
            ctx.request_paint_only();
            return;
        }
        self.elapsed_ns = (self.elapsed_ns + interval).min(SWITCH_ANIM_NS);
        let raw = (self.elapsed_ns as f64 / SWITCH_ANIM_NS as f64).clamp(0.0, 1.0);
        let eased = MotionEasing::Standard.sample(raw);
        self.anim_t = if self.on { eased } else { 1.0 - eased };
        ctx.request_paint_only();
        if self.elapsed_ns < SWITCH_ANIM_NS {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => ctx.request_paint_only(),
            _ => {}
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        _len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match axis {
            Axis::Horizontal => Length::px(self.visual.switch_width),
            Axis::Vertical => Length::px(self.visual.switch_height),
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let disabled = ctx.is_disabled();
        let hovered = ctx.is_hovered();
        let active = ctx.is_active();
        let win_focused = ctx.is_window_focused();
        let size = ctx.content_box().size();
        let w = size.width;
        let h = size.height;
        let r = h * 0.5;
        let track_rect = Rect::new(0.0, 0.0, w, h);
        let track_rr = track_rect.to_rounded_rect(r);
        let t = self.anim_t.clamp(0.0, 1.0);
        let accent = self.visual.effective_accent(win_focused, disabled);

        match self.visual.grammar {
            ControlVisualGrammar::Material3 => {
                // MD3: off=surface_highest + 2px border_strong + 16px thumb; on=accent + 24px thumb + check
                let track_bg = if disabled {
                    self.visual.disabled_container
                } else if t > 0.5 {
                    if active {
                        blend_over(accent, self.visual.on_accent, 0.12)
                    } else {
                        accent
                    }
                } else {
                    self.visual.surface_highest
                };
                painter.fill(track_rr, track_bg).draw();
                if t <= 0.5 || disabled {
                    let b_col = if disabled {
                        self.visual.disabled_content
                    } else {
                        self.visual.border_strong
                    };
                    painter
                        .stroke(
                            track_rect.inset(1.0).to_rounded_rect((r - 1.0).max(0.0)),
                            &Stroke::new(2.0),
                            b_col,
                        )
                        .draw();
                }

                let base_thumb_r = (8.0 + 4.0 * t).min(h * 0.42);
                let thumb_r = if !disabled && active {
                    (base_thumb_r + 2.0).min(h * 0.46)
                } else {
                    base_thumb_r
                };
                let margin = h * 0.5;
                let cx = margin + (w - margin * 2.0) * t;
                let cy = h * 0.5;

                if !disabled && (hovered || active) {
                    let halo = Circle::new(Point::new(cx, cy), (thumb_r + 6.0).min(h * 0.62));
                    painter
                        .fill(halo, self.visual.halo_color(accent, active))
                        .draw();
                }

                let thumb_col = if disabled {
                    if self.on {
                        self.visual.surface
                    } else {
                        self.visual.disabled_content
                    }
                } else if t > 0.5 {
                    self.visual.on_accent
                } else {
                    self.visual.border_strong
                };
                painter
                    .fill(Circle::new(Point::new(cx, cy), thumb_r), thumb_col)
                    .draw();
                if !disabled && self.on && t > 0.65 {
                    let check_box = Rect::new(
                        cx - thumb_r * 0.55,
                        cy - thumb_r * 0.55,
                        cx + thumb_r * 0.55,
                        cy + thumb_r * 0.55,
                    );
                    draw_checkmark(painter, check_box, accent, 1.6);
                }
            }
            ControlVisualGrammar::Fluent2 => {
                // Fluent 2: 40x20, off=1px outlined track, on=solid accent, thumb stretches slightly on press
                let track_bg = if disabled {
                    self.visual.disabled_container
                } else if t > 0.5 {
                    if hovered {
                        blend_over(accent, self.visual.on_accent, 0.08)
                    } else {
                        accent
                    }
                } else if hovered {
                    blend_over(self.visual.surface, self.visual.text_primary, 0.04)
                } else {
                    Color::TRANSPARENT
                };
                painter.fill(track_rr, track_bg).draw();
                if t <= 0.5 && !disabled {
                    painter
                        .stroke(
                            track_rect.inset(0.5).to_rounded_rect((r - 0.5).max(0.0)),
                            &Stroke::new(1.0),
                            self.visual.border_strong,
                        )
                        .draw();
                }
                let thumb_h = if !disabled && hovered { 14.0 } else { 12.0 };
                let thumb_w = if !disabled && active {
                    thumb_h + 4.0
                } else {
                    thumb_h
                };
                let inset = (h - thumb_h) * 0.5;
                let left = inset + (w - inset * 2.0 - thumb_w) * t;
                let thumb_rect = Rect::new(left, inset, left + thumb_w, inset + thumb_h);
                let thumb_col = if disabled {
                    self.visual.disabled_content
                } else if t > 0.5 {
                    self.visual.on_accent
                } else {
                    self.visual.text_secondary
                };
                painter
                    .fill(thumb_rect.to_rounded_rect(thumb_h * 0.5), thumb_col)
                    .draw();
            }
            ControlVisualGrammar::MacosHig | ControlVisualGrammar::Adwaita => {
                // HIG: crisp Aqua track + shadowed white thumb; Adwaita: flat tonal pill + clean white thumb
                let off_bg = if disabled {
                    self.visual.disabled_container
                } else if self.visual.grammar == ControlVisualGrammar::Adwaita {
                    with_opacity(self.visual.text_primary, if hovered { 0.24 } else { 0.18 })
                } else {
                    self.visual.surface_highest
                };
                let track_bg = if disabled {
                    self.visual.disabled_container
                } else if t > 0.5 {
                    if hovered {
                        blend_over(accent, self.visual.on_accent, 0.08)
                    } else {
                        accent
                    }
                } else {
                    off_bg
                };
                painter.fill(track_rr, track_bg).draw();
                if self.visual.grammar == ControlVisualGrammar::MacosHig && t <= 0.5 && !disabled {
                    painter
                        .stroke(
                            track_rect.inset(0.5).to_rounded_rect((r - 0.5).max(0.0)),
                            &Stroke::new(1.0),
                            self.visual.border,
                        )
                        .draw();
                }

                let thumb_r = (h * 0.5 - 2.5).max(5.0);
                let margin = h * 0.5;
                let cx = margin + (w - margin * 2.0) * t;
                let cy = h * 0.5;
                if self.visual.grammar == ControlVisualGrammar::MacosHig && !disabled {
                    painter
                        .fill(
                            Circle::new(Point::new(cx, cy + 0.8), thumb_r),
                            with_opacity(self.visual.shadow, 0.18),
                        )
                        .draw();
                } else if !disabled && hovered {
                    painter
                        .fill(
                            Circle::new(Point::new(cx, cy), thumb_r + 3.0),
                            self.visual.halo_color(accent, active),
                        )
                        .draw();
                }
                let thumb_col = if disabled {
                    self.visual.surface
                } else {
                    Color::WHITE
                };
                painter
                    .fill(Circle::new(Point::new(cx, cy), thumb_r), thumb_col)
                    .draw();
            }
        }

        draw_focus_ring(
            painter,
            track_rr,
            self.visual,
            ctx.is_focus_target(),
            win_focused,
        );
    }

    fn accessibility_role(&self) -> Role {
        Role::Switch
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.add_action(accesskit::Action::Click);
        node.set_toggled(if self.on {
            Toggled::True
        } else {
            Toggled::False
        });
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("UiSwitch", id = id.trace())
    }
}

/// View creating a [`UiSwitch`] styled dynamically from ambient [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiSwitchView<State, Action> {
    on: bool,
    disabled: bool,
    on_toggle: BoolCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiSwitchView<State, Action> {
    /// Disables switch interaction when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, Action> ViewMarker for UiSwitchView<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiSwitchView<State, Action> {
    type Element = Pod<UiSwitch>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(UiSwitch::new(self.on, visual));
            pod.new_widget.options.disabled = self.disabled;
            pod
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        UiSwitch::set_visual(&mut element, visual);
        UiSwitch::set_on(&mut element, self.on);
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_message::<SwitchToggled>() {
            Some(msg) => MessageResult::Action((self.on_toggle)(app_state, msg.0)),
            None => MessageResult::Stale,
        }
    }
}

/// Spec-styled theme-authentic toggle switch.
pub fn switch<State: 'static, Action: 'static, F>(
    on: bool,
    on_toggle: F,
) -> UiSwitchView<State, Action>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    UiSwitchView {
        on,
        disabled: false,
        on_toggle: Arc::new(on_toggle),
        phantom: PhantomData,
    }
}

/// Custom theme-painted radio button honoring MD3, Fluent 2 inverted ring, macOS HIG, and Adwaita.
#[expect(
    missing_debug_implementations,
    reason = "Holds an erased Masonry WidgetPod"
)]
pub struct UiRadioButton {
    selected: bool,
    label: WidgetPod<widgets::Label>,
    visual: ControlVisualTheme,
}

impl UiRadioButton {
    fn new(selected: bool, label: NewWidget<widgets::Label>, visual: ControlVisualTheme) -> Self {
        Self {
            selected,
            label: label.to_pod(),
            visual,
        }
    }

    fn set_selected(this: &mut WidgetMut<'_, Self>, selected: bool) {
        if this.widget.selected != selected {
            this.widget.selected = selected;
            this.ctx.request_accessibility_update();
            this.ctx.request_paint_only();
        }
    }

    fn set_visual(this: &mut WidgetMut<'_, Self>, visual: ControlVisualTheme) {
        if this.widget.visual != visual {
            let layout_diff = this.widget.visual.grammar != visual.grammar;
            this.widget.visual = visual;
            if layout_diff {
                this.ctx.request_layout();
            } else {
                this.ctx.request_paint_only();
            }
        }
    }

    fn label_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, widgets::Label> {
        this.ctx.get_mut(&mut this.widget.label)
    }

    fn ring_dia(&self) -> f64 {
        match self.visual.grammar {
            ControlVisualGrammar::MacosHig => 16.0,
            _ => 20.0,
        }
    }
}

impl Widget for UiRadioButton {
    type Action = RadioButtonSelected;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.label);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                ..
            }) => {
                if ctx.is_active() && ctx.is_hovered() {
                    ctx.submit_action::<Self::Action>(RadioButtonSelected);
                }
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
        if ctx.is_disabled() {
            return;
        }
        match event {
            TextEvent::Keyboard(ke)
                if ctx.is_focus_target()
                    && ke.state.is_up()
                    && (matches!(&ke.key, Key::Character(c) if c == " ")
                        || ke.key == Key::Named(NamedKey::Enter)) =>
            {
                ctx.submit_action::<Self::Action>(RadioButtonSelected);
            }
            TextEvent::WindowFocusChange(_) => ctx.request_paint_only(),
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        if event.action == accesskit::Action::Click {
            ctx.submit_action::<Self::Action>(RadioButtonSelected);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
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
        let dia = self.ring_dia();
        let gap = 8.0;
        let other = match axis {
            Axis::Horizontal => Length::px(dia + gap),
            Axis::Vertical => Length::ZERO,
        };
        let cross = axis.cross();
        let cross_other = match cross {
            Axis::Horizontal => Length::px(dia + gap),
            Axis::Vertical => Length::ZERO,
        };
        let cross_space = cross_length.map(|l| l.saturating_sub(cross_other));
        let auto_len = len_req.reduce(other).into();
        let context_size = LayoutSize::maybe(cross, cross_space);
        let label_len =
            ctx.compute_length(&mut self.label, auto_len, context_size, axis, cross_space);
        match axis {
            Axis::Horizontal => label_len.saturating_add(other),
            Axis::Vertical => label_len.max(Length::px(dia + 4.0)),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let dia = self.ring_dia();
        let gap = 8.0;
        let avail_w = (size.width - (dia + gap)).max(0.0);
        let space = Size::new(avail_w, size.height);
        let label_size = ctx.compute_size(&mut self.label, SizeDef::fit(space), space.into());
        ctx.run_layout(&mut self.label, label_size);
        let origin_y = ((size.height - label_size.height) * 0.5).max(0.0);
        ctx.place_child(&mut self.label, Point::new(dia + gap, origin_y));
        ctx.derive_baselines(&self.label);
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let disabled = ctx.is_disabled();
        let hovered = ctx.is_hovered();
        let active = ctx.is_active();
        let win_focused = ctx.is_window_focused();
        let dia = self.ring_dia();
        let outer_r = dia * 0.5;
        let center = Point::new(outer_r, ctx.content_box().size().height * 0.5);
        let accent = self.visual.effective_accent(win_focused, disabled);

        if !disabled && (hovered || active) {
            painter
                .fill(
                    Circle::new(center, outer_r + 4.0),
                    self.visual.halo_color(accent, active),
                )
                .draw();
        }

        match self.visual.grammar {
            ControlVisualGrammar::Fluent2 | ControlVisualGrammar::MacosHig => {
                if self.selected {
                    painter.fill(Circle::new(center, outer_r), accent).draw();
                    let inner_r = if self.visual.grammar == ControlVisualGrammar::MacosHig {
                        3.0
                    } else if active && !disabled {
                        5.0
                    } else if hovered && !disabled {
                        4.5
                    } else {
                        4.0
                    };
                    let dot_col = if disabled {
                        self.visual.surface
                    } else {
                        self.visual.on_accent
                    };
                    painter.fill(Circle::new(center, inner_r), dot_col).draw();
                } else {
                    let bg = if disabled {
                        self.visual.disabled_container
                    } else {
                        self.visual.surface
                    };
                    let b_col = if disabled {
                        self.visual.disabled_content
                    } else if hovered {
                        self.visual.text_primary
                    } else {
                        self.visual.border_strong
                    };
                    painter.fill(Circle::new(center, outer_r), bg).draw();
                    painter
                        .stroke(
                            Circle::new(center, outer_r - 0.75),
                            &Stroke::new(1.5),
                            b_col,
                        )
                        .draw();
                }
            }
            ControlVisualGrammar::Material3 | ControlVisualGrammar::Adwaita => {
                let ring_col = if disabled {
                    self.visual.disabled_content
                } else if self.selected {
                    accent
                } else if hovered {
                    self.visual.text_primary
                } else {
                    self.visual.border_strong
                };
                painter
                    .stroke(
                        Circle::new(center, outer_r - 1.0),
                        &Stroke::new(2.0),
                        ring_col,
                    )
                    .draw();
                if self.selected {
                    painter
                        .fill(Circle::new(center, outer_r * 0.5), ring_col)
                        .draw();
                }
            }
        }

        if ctx.is_focus_target() && win_focused {
            painter
                .stroke(
                    Circle::new(center, outer_r + 2.5),
                    &Stroke::new(1.5),
                    with_opacity(self.visual.accent, 0.85),
                )
                .draw();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::RadioButton
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.add_action(accesskit::Action::Click);
        node.set_toggled(if self.selected {
            Toggled::True
        } else {
            Toggled::False
        });
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.label.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("UiRadioButton", id = id.trace())
    }
}

/// Callback type for [`UiRadio`].
pub type RadioCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Concrete radio option view exposing `Pod<UiRadioButton>` uniformly for [`radio_group`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiRadio<State, Action> {
    text: ArcStr,
    selected: bool,
    disabled: bool,
    on_select: RadioCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiRadio<State, Action> {
    /// Disables radio selection when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, Action> ViewMarker for UiRadio<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiRadio<State, Action> {
    type Element = Pod<UiRadioButton>;
    type ViewState = (TextStyleSpec, FontFamily<'static>, Color);

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let ts = spec.typography.get(TypeRole::BodyMedium);
        let font = spec.typography.resolved_font_family();
        let color = visual.label_color(self.disabled);
        let lbl = prepare_label_pod(self.text.clone(), ts, &font, color);
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(UiRadioButton::new(self.selected, lbl, visual));
            pod.new_widget.options.disabled = self.disabled;
            pod
        });
        (pod, (ts, font, color))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (prev_ts, prev_font, prev_color): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let next_ts = spec.typography.get(TypeRole::BodyMedium);
        let next_font = spec.typography.resolved_font_family();
        let next_color = visual.label_color(self.disabled);

        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        UiRadioButton::set_selected(&mut element, self.selected);
        UiRadioButton::set_visual(&mut element, visual);
        let mut typo = (*prev_ts, prev_font.clone());
        sync_label_pod(
            UiRadioButton::label_mut(&mut element),
            prev.text != self.text,
            self.text.clone(),
            &mut typo,
            &(next_ts, next_font),
            next_color,
            *prev_color,
        );
        *prev_ts = typo.0;
        *prev_font = typo.1;
        *prev_color = next_color;
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
        match message.take_message::<RadioButtonSelected>() {
            Some(_) => MessageResult::Action((self.on_select)(app_state)),
            None => MessageResult::Stale,
        }
    }
}

/// Spec-styled theme-authentic radio button option returning a uniform [`UiRadio`] concrete view.
pub fn radio<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    selected: bool,
    on_select: F,
) -> UiRadio<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiRadio {
        text: text.into(),
        selected,
        disabled: false,
        on_select: Arc::new(on_select),
        phantom: PhantomData,
    }
}

/// Vertical or horizontal radio group container with accessible semantics.
pub fn radio_group<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    items: Vec<V>,
) -> impl WidgetView<State, Action> {
    raw_radio_group(
        flex_col(items)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(6.px()),
    )
}

/// Action emitted when [`UiSlider`] value changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderMoved {
    /// Updated slider value clamped to `[min, max]`.
    pub value: f64,
}

fn snap_slider(min: f64, max: f64, val: f64, step: Option<f64>) -> f64 {
    let clamped = val.clamp(min, max);
    match step {
        Some(s) if s > 0.0 => (min + ((clamped - min) / s).round() * s).clamp(min, max),
        _ => clamped,
    }
}

/// Custom theme-painted horizontal slider honoring MD3 Expressive, Fluent 2 concentric, HIG, and Adwaita.
#[derive(Debug)]
pub struct UiSlider {
    min: f64,
    max: f64,
    value: f64,
    step: Option<f64>,
    dragging: bool,
    visual: ControlVisualTheme,
}

impl UiSlider {
    fn new(min: f64, max: f64, value: f64, step: Option<f64>, visual: ControlVisualTheme) -> Self {
        let lo = min.min(max);
        let hi = max.max(lo);
        let step = step.filter(|s| *s > 0.0);
        Self {
            min: lo,
            max: hi,
            value: snap_slider(lo, hi, value, step),
            step,
            dragging: false,
            visual,
        }
    }

    fn set_params(
        this: &mut WidgetMut<'_, Self>,
        min: f64,
        max: f64,
        value: f64,
        step: Option<f64>,
        visual: ControlVisualTheme,
    ) {
        let lo = min.min(max);
        let hi = max.max(lo);
        let step = step.filter(|s| *s > 0.0);
        let snapped = snap_slider(lo, hi, value, step);
        let mut changed = false;
        if (this.widget.min - lo).abs() > 1e-9
            || (this.widget.max - hi).abs() > 1e-9
            || (this.widget.value - snapped).abs() > 1e-9
            || this.widget.step != step
        {
            this.widget.min = lo;
            this.widget.max = hi;
            this.widget.value = snapped;
            this.widget.step = step;
            this.ctx.request_accessibility_update();
            changed = true;
        }
        if this.widget.visual != visual {
            this.widget.visual = visual;
            changed = true;
        }
        if changed {
            this.ctx.request_paint_only();
        }
    }

    fn thumb_pad(&self) -> f64 {
        11.0
    }

    fn value_from_x(&self, x: f64, width: f64) -> f64 {
        let pad = self.thumb_pad();
        let usable = (width - pad * 2.0).max(1.0);
        let frac = ((x - pad) / usable).clamp(0.0, 1.0);
        let raw = self.min + frac * (self.max - self.min);
        snap_slider(self.min, self.max, raw, self.step)
    }

    fn commit_value(&mut self, ctx: &mut EventCtx<'_>, next: f64) {
        let clamped = snap_slider(self.min, self.max, next, self.step);
        if (clamped - self.value).abs() > 1e-9 {
            self.value = clamped;
            ctx.request_accessibility_update();
            ctx.submit_action::<SliderMoved>(SliderMoved { value: clamped });
            ctx.request_paint_only();
        }
    }
}

impl Widget for UiSlider {
    type Action = SliderMoved;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        let width = ctx.content_box().size().width;
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                state,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                self.dragging = true;
                let pos = ctx.local_position(state.position);
                let next = self.value_from_x(pos.x, width);
                self.commit_value(ctx, next);
                ctx.request_paint_only();
            }
            PointerEvent::Move(PointerUpdate { current, .. }) if ctx.is_active() => {
                let pos = ctx.local_position(current.position);
                let next = self.value_from_x(pos.x, width);
                self.commit_value(ctx, next);
            }
            PointerEvent::Up(_) | PointerEvent::Cancel(_) => {
                self.dragging = false;
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
        if ctx.is_disabled() {
            return;
        }
        match event {
            TextEvent::Keyboard(ke) if ctx.is_focus_target() && !ke.state.is_up() => {
                let span = (self.max - self.min).abs().max(1e-9);
                let step = self.step.unwrap_or(span / 100.0);
                let big = step * 10.0;
                let next = match &ke.key {
                    Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => {
                        Some(self.value - if ke.modifiers.shift() { big } else { step })
                    }
                    Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => {
                        Some(self.value + if ke.modifiers.shift() { big } else { step })
                    }
                    Key::Named(NamedKey::Home) => Some(self.min),
                    Key::Named(NamedKey::End) => Some(self.max),
                    _ => None,
                };
                if let Some(v) = next {
                    self.commit_value(ctx, v);
                }
            }
            TextEvent::WindowFocusChange(_) => ctx.request_paint_only(),
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        let step = self.step.unwrap_or((self.max - self.min) / 100.0);
        match event.action {
            accesskit::Action::Increment => self.commit_value(ctx, self.value + step),
            accesskit::Action::Decrement => self.commit_value(ctx, self.value - step),
            accesskit::Action::SetValue => {
                if let Some(accesskit::ActionData::NumericValue(v)) = event.data {
                    self.commit_value(ctx, v);
                }
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => ctx.request_paint_only(),
            _ => {}
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
        match axis {
            Axis::Horizontal => match len_req {
                LenReq::MinContent => Length::const_px(48.0),
                LenReq::MaxContent => Length::const_px(160.0),
                LenReq::FitContent(space) => space,
            },
            Axis::Vertical => Length::const_px(32.0),
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let disabled = ctx.is_disabled();
        let hovered = ctx.is_hovered();
        let active = ctx.is_active() || self.dragging;
        let win_focused = ctx.is_window_focused();
        let size = ctx.content_box().size();
        let pad = self.thumb_pad();
        let span = (self.max - self.min).max(1e-9);
        let frac = ((self.value - self.min) / span).clamp(0.0, 1.0);
        let usable = (size.width - pad * 2.0).max(1.0);
        let tx = pad + frac * usable;
        let cy = size.height * 0.5;
        let accent = self.visual.effective_accent(win_focused, disabled);
        let inactive_col = if disabled {
            self.visual.disabled_container
        } else {
            self.visual.surface_highest
        };

        match self.visual.grammar {
            ControlVisualGrammar::Material3 => {
                // MD3 Expressive slider: 8px active/inactive track with gap + 4px stop dot + vertical pill thumb
                let track_h = 8.0;
                let y0 = cy - track_h * 0.5;
                let gap = 5.0;
                let left_end = (tx - gap).max(pad);
                if left_end > pad + 1.0 {
                    let act =
                        Rect::new(pad, y0, left_end, y0 + track_h).to_rounded_rect(track_h * 0.5);
                    painter.fill(act, accent).draw();
                }
                let right_start = (tx + gap).min(size.width - pad);
                let right_end = size.width - pad;
                if right_end > right_start + 1.0 {
                    let inact = Rect::new(right_start, y0, right_end, y0 + track_h)
                        .to_rounded_rect(track_h * 0.5);
                    painter.fill(inact, inactive_col).draw();
                    if right_end - tx > 8.0 {
                        painter
                            .fill(Circle::new(Point::new(right_end - 4.0, cy), 2.0), accent)
                            .draw();
                    }
                }
                let thumb_w = if active { 4.0 } else { 5.0 };
                let thumb_h = 24.0;
                if !disabled && (hovered || active) {
                    let halo = Rect::new(
                        tx - thumb_w * 0.5 - 4.0,
                        cy - thumb_h * 0.5 - 2.0,
                        tx + thumb_w * 0.5 + 4.0,
                        cy + thumb_h * 0.5 + 2.0,
                    )
                    .to_rounded_rect(6.0);
                    painter
                        .fill(halo, self.visual.halo_color(accent, active))
                        .draw();
                }
                let handle = Rect::new(
                    tx - thumb_w * 0.5,
                    cy - thumb_h * 0.5,
                    tx + thumb_w * 0.5,
                    cy + thumb_h * 0.5,
                )
                .to_rounded_rect(thumb_w * 0.5);
                painter.fill(handle, accent).draw();
            }
            ControlVisualGrammar::Fluent2 => {
                // Fluent 2: 4px track + dual-layer concentric white outer & dynamic accent inner thumb
                let track_h = 4.0;
                let y0 = cy - track_h * 0.5;
                painter
                    .fill(
                        Rect::new(pad, y0, size.width - pad, y0 + track_h).to_rounded_rect(2.0),
                        inactive_col,
                    )
                    .draw();
                if tx > pad {
                    painter
                        .fill(
                            Rect::new(pad, y0, tx, y0 + track_h).to_rounded_rect(2.0),
                            accent,
                        )
                        .draw();
                }
                let thumb_c = Point::new(tx, cy);
                painter
                    .fill(Circle::new(thumb_c, 9.0), self.visual.surface)
                    .draw();
                painter
                    .stroke(
                        Circle::new(thumb_c, 9.0),
                        &Stroke::new(1.0),
                        self.visual.border_strong,
                    )
                    .draw();
                let inner_r = if disabled {
                    4.0
                } else if active {
                    3.8
                } else if hovered {
                    5.8
                } else {
                    4.8
                };
                painter.fill(Circle::new(thumb_c, inner_r), accent).draw();
            }
            ControlVisualGrammar::MacosHig | ControlVisualGrammar::Adwaita => {
                let track_h = 4.0;
                let y0 = cy - track_h * 0.5;
                painter
                    .fill(
                        Rect::new(pad, y0, size.width - pad, y0 + track_h).to_rounded_rect(2.0),
                        inactive_col,
                    )
                    .draw();
                if tx > pad {
                    painter
                        .fill(
                            Rect::new(pad, y0, tx, y0 + track_h).to_rounded_rect(2.0),
                            accent,
                        )
                        .draw();
                }
                let thumb_r = if self.visual.grammar == ControlVisualGrammar::Adwaita {
                    9.5
                } else {
                    8.5
                };
                let thumb_c = Point::new(tx, cy);
                if !disabled
                    && self.visual.grammar == ControlVisualGrammar::Adwaita
                    && (hovered || active)
                {
                    painter
                        .fill(
                            Circle::new(thumb_c, thumb_r + 3.5),
                            self.visual.halo_color(accent, active),
                        )
                        .draw();
                } else if self.visual.grammar == ControlVisualGrammar::MacosHig && !disabled {
                    painter
                        .fill(
                            Circle::new(Point::new(tx, cy + 1.0), thumb_r),
                            with_opacity(self.visual.shadow, 0.20),
                        )
                        .draw();
                }
                let thumb_fill = if disabled {
                    self.visual.surface_highest
                } else {
                    Color::WHITE
                };
                painter
                    .fill(Circle::new(thumb_c, thumb_r), thumb_fill)
                    .draw();
                painter
                    .stroke(
                        Circle::new(thumb_c, thumb_r),
                        &Stroke::new(1.0),
                        self.visual.border_strong,
                    )
                    .draw();
            }
        }

        if ctx.is_focus_target() && win_focused {
            painter
                .stroke(
                    Line::new(
                        Point::new(pad, cy + 12.0),
                        Point::new(size.width - pad, cy + 12.0),
                    ),
                    &Stroke::new(1.5),
                    with_opacity(self.visual.accent, 0.75),
                )
                .draw();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Slider
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_orientation(Orientation::Horizontal);
        node.set_value(format!("{:.3}", self.value));
        node.set_numeric_value(self.value);
        node.set_min_numeric_value(self.min);
        node.set_max_numeric_value(self.max);
        if let Some(step) = self.step {
            node.set_numeric_value_step(step);
        }
        node.add_action(accesskit::Action::SetValue);
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("UiSlider", id = id.trace())
    }
}

/// Callback type for [`slider`].
pub type SliderCallback<State, Action> = Arc<dyn Fn(&mut State, f64) -> Action + Send + Sync>;

/// Theme-authentic slider view emitting `on_change` on drag, click, keyboard, and AccessKit updates.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiSliderView<State, Action> {
    min: f64,
    max: f64,
    value: f64,
    step: Option<f64>,
    disabled: bool,
    on_change: SliderCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiSliderView<State, Action> {
    /// Disables slider interaction when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, Action> ViewMarker for UiSliderView<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiSliderView<State, Action> {
    type Element = Pod<UiSlider>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        let pod = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(UiSlider::new(
                self.min, self.max, self.value, self.step, visual,
            ));
            pod.new_widget.options.disabled = self.disabled;
            pod
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let visual = ControlVisualTheme::from_spec(&spec);
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        UiSlider::set_params(
            &mut element,
            self.min,
            self.max,
            self.value,
            self.step,
            visual,
        );
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_message::<SliderMoved>() {
            Some(msg) => MessageResult::Action((self.on_change)(app_state, msg.value)),
            None => MessageResult::Stale,
        }
    }
}

/// Spec-styled continuous or stepped slider (`min..=max`).
pub fn slider<State: 'static, Action: 'static, F>(
    min: f64,
    max: f64,
    value: f64,
    step: Option<f64>,
    on_change: F,
) -> UiSliderView<State, Action>
where
    F: Fn(&mut State, f64) -> Action + Send + Sync + 'static,
{
    UiSliderView {
        min,
        max,
        value,
        step,
        disabled: false,
        on_change: Arc::new(on_change),
        phantom: PhantomData,
    }
}
