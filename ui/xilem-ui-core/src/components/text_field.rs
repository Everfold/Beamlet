//! Outlined & Filled single-line and multi-line text fields with reactive [`FocusFrame`](crate::foundation::FocusFrame) indicators.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::kurbo::{Axis, Point, Size};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, LayoutCtx, MeasureCtx, PaintCtx, PropertiesMut, PropertiesRef,
    RegisterCtx, Update, UpdateCtx, Widget, WidgetMut, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, SizeDef, UnitPoint};
use xilem::masonry::properties::{ContentColor, Gap, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, Flex, FlexExt, Label, Prop, ZStackExt, button, flex_col, flex_row, label,
    portal, sized_box, text_input, zstack,
};
use xilem::{AnyWidgetView, InsertNewline, Pod, ViewCtx, WidgetView};

use crate::foundation::{UiClassExt, focus_frame, icons, ui_icon};
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::{ControlVisualGrammar, DesignSpec, TypeRole};

/// Text field surface variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextFieldVariant {
    /// Outlined input frame.
    #[default]
    Outlined,
    /// Filled tonal input container.
    Filled,
}

/// Callback type invoked when text content changes.
pub type TextChangeCallback<State, Action> =
    Arc<dyn Fn(&mut State, String) -> Action + Send + Sync>;

/// Optional trailing action icon attached inside [`UiTextField`].
pub type TrailingAction<State, Action> = (
    &'static str,
    Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
);

type TextFieldFlex<State, Action> = Prop<
    Gap,
    Flex<
        (
            Option<Prop<ContentColor, Label, State, Action>>,
            Box<AnyWidgetView<State, Action>>,
            Option<Box<AnyWidgetView<State, Action>>>,
        ),
        State,
        Action,
    >,
    State,
    Action,
>;

/// Cross-theme text field view exposing `Pod<widgets::Flex>` directly and reacting to keyboard focus.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiTextField<State, Action> {
    variant: TextFieldVariant,
    value: String,
    label_text: Option<ArcStr>,
    placeholder: Option<ArcStr>,
    prefix_text: Option<ArcStr>,
    suffix_text: Option<ArcStr>,
    supporting_text: Option<ArcStr>,
    error_text: Option<ArcStr>,
    max_chars: Option<usize>,
    disabled: bool,
    on_change: TextChangeCallback<State, Action>,
    trailing_action: Option<TrailingAction<State, Action>>,
}

impl<State, Action> UiTextField<State, Action> {
    /// Sets a top caption label above the input box.
    pub fn label(mut self, text: impl Into<ArcStr>) -> Self {
        self.label_text = Some(text.into());
        self
    }

    /// Sets placeholder hint text shown when empty.
    pub fn placeholder(mut self, text: impl Into<ArcStr>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Sets static prefix text inside the leading edge of the input row.
    pub fn prefix_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.prefix_text = Some(text.into());
        self
    }

    /// Sets static suffix text inside the trailing edge of the input row.
    pub fn suffix_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.suffix_text = Some(text.into());
        self
    }

    /// Attaches a clickable icon button inside the trailing edge of the input.
    pub fn trailing_action<F>(mut self, svg_path: &'static str, on_click: F) -> Self
    where
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        self.trailing_action = Some((svg_path, Arc::new(on_click)));
        self
    }

    /// Displays a `len / max` character counter in the supporting row.
    pub fn max_chars(mut self, max: usize) -> Self {
        self.max_chars = Some(max);
        self
    }

    /// Sets helper text displayed beneath the field.
    pub fn supporting_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.supporting_text = Some(text.into());
        self
    }

    /// Sets validation error text (switches frame tint to `palette.error`).
    pub fn error_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.error_text = Some(text.into());
        self
    }

    /// Disables editing and dims foreground chrome.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn build_flex(&self, spec: &DesignSpec) -> TextFieldFlex<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let m = &spec.metrics;
        let is_error = self.error_text.is_some();
        let is_md3_floating =
            spec.control_grammar() == ControlVisualGrammar::Material3 && self.label_text.is_some();
        let base_class = match self.variant {
            TextFieldVariant::Outlined => classes::TEXT_FIELD_OUTLINED,
            TextFieldVariant::Filled => classes::TEXT_FIELD_FILLED,
        };

        let label_color = if is_error {
            p.error
        } else {
            spec.content_color(p.text_secondary, self.disabled)
        };
        let top_label = (!is_md3_floating)
            .then(|| build_top_label(spec, self.label_text.as_ref(), label_color))
            .flatten();

        let style = spec.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();
        let mut ti = text_input(self.value.clone(), move |s: &mut State, txt: String| {
            on_change(s, txt)
        })
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(spec.content_color(p.text_primary, self.disabled))
        .disabled(self.disabled);

        if let Some(ph) = &self.placeholder {
            ti = ti.placeholder(ph.clone());
        }
        if let Some(font) = spec.typography.font_family() {
            ti = ti.font(font.clone());
        }

        let has_addons = self.prefix_text.is_some()
            || self.suffix_text.is_some()
            || self.trailing_action.is_some();

        let make_row_inner = |ti_view: Box<AnyWidgetView<State, Action>>| {
            let sub_ink = spec.content_color(p.text_secondary, self.disabled);
            let prefix = self.prefix_text.as_ref().map(|t| {
                spec.typography
                    .style_label(TypeRole::BodyMedium, label(t.clone()))
                    .color(sub_ink)
            });
            let suffix = self.suffix_text.as_ref().map(|t| {
                spec.typography
                    .style_label(TypeRole::BodyMedium, label(t.clone()))
                    .color(sub_ink)
            });
            let trail = self.trailing_action.as_ref().map(|(icon, cb)| {
                let cb = cb.clone();
                button(
                    ui_icon(icon, 16.0, if is_error { p.error } else { sub_ink }),
                    move |s: &mut State| cb(s),
                )
                .disabled(self.disabled)
                .ui_class(classes::ICON_BTN_STD)
            });
            let trail_err = if is_error && trail.is_none() {
                Some(ui_icon(icons::ERROR_CIRCLE, 16.0, p.error))
            } else {
                None
            };
            flex_row((prefix, ti_view.flex(1.0), suffix, trail, trail_err))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
        };

        let frame_slot: Box<AnyWidgetView<State, Action>> = if is_md3_floating {
            let float_lbl = build_floating_label(
                spec,
                self.label_text.clone(),
                self.value.is_empty(),
                is_error,
                self.disabled,
                label_color,
            );

            match self.variant {
                TextFieldVariant::Filled => {
                    let flush = ti.ui_class(classes::TEXT_FIELD_FLUSH).boxed();
                    let row = if has_addons {
                        make_row_inner(flush).boxed()
                    } else {
                        flush
                    };
                    let container = flex_col((float_lbl, row))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(2.px())
                        .ui_class(classes::TEXT_FIELD_FILLED)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .padding(Padding::from_vh(7.px(), m.input_pad_h.px()))
                        .boxed();
                    focus_frame(container).error(is_error).boxed()
                }
                TextFieldVariant::Outlined => {
                    let input_widget: Box<AnyWidgetView<State, Action>> = if has_addons {
                        make_row_inner(ti.ui_class(classes::TEXT_FIELD_FLUSH).boxed())
                            .ui_class(base_class)
                            .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                            .boxed()
                    } else {
                        ti.ui_class(base_class)
                            .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                            .boxed()
                    };
                    wrap_outlined_floating_notch(p.surface, input_widget, float_lbl, is_error)
                }
            }
        } else {
            let input_widget: Box<AnyWidgetView<State, Action>> = if has_addons {
                make_row_inner(ti.ui_class(classes::TEXT_FIELD_FLUSH).boxed())
                    .ui_class(base_class)
                    .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                    .boxed()
            } else {
                ti.ui_class(base_class)
                    .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                    .boxed()
            };
            focus_frame(input_widget).error(is_error).boxed()
        };

        let bottom_row = build_bottom_row(
            spec,
            self.error_text.as_ref(),
            self.supporting_text.as_ref(),
            self.value.chars().count(),
            self.max_chars,
        );

        flex_col((top_label, frame_slot, bottom_row))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(4.px())
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiTextField<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiTextField<State, Action> {
    type Element = Pod<widgets::Flex>;
    type ViewState = (
        TextFieldFlex<State, Action>,
        <TextFieldFlex<State, Action> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let flex = self.build_flex(&spec);
        let (pod, st) = flex.build(ctx, app_state);
        (pod, (flex, st))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_flex = self.build_flex(&spec);
        next_flex.rebuild(prev_flex, st, ctx, element, app_state);
        *prev_flex = next_flex;
    }

    fn teardown(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_flex.teardown(st, ctx, element);
    }

    fn message(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_flex.message(st, message, element, app_state)
    }
}

fn make_tf<State, Action, F>(
    variant: TextFieldVariant,
    value: impl Into<String>,
    on_change: F,
) -> UiTextField<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    UiTextField {
        variant,
        value: value.into(),
        label_text: None,
        placeholder: None,
        prefix_text: None,
        suffix_text: None,
        supporting_text: None,
        error_text: None,
        max_chars: None,
        disabled: false,
        on_change: Arc::new(on_change),
        trailing_action: None,
    }
}

/// Outlined text field driven by [`DesignSpec`] and wrapped in a reactive [`FocusFrame`](crate::foundation::FocusFrame).
pub fn text_field<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextField<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_tf(TextFieldVariant::Outlined, value, on_change)
}

/// Filled tonal container text field driven by [`DesignSpec`] and wrapped in a reactive [`FocusFrame`](crate::foundation::FocusFrame).
pub fn filled_text_field<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextField<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_tf(TextFieldVariant::Filled, value, on_change)
}

fn build_top_label<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    label_text: Option<&ArcStr>,
    label_color: xilem::masonry::peniko::Color,
) -> Option<Prop<ContentColor, Label, State, Action>> {
    label_text.map(|t| {
        spec.typography
            .style_label(TypeRole::LabelMedium, label(t.clone()))
            .color(label_color)
    })
}

fn build_floating_label<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    label_text: Option<ArcStr>,
    is_empty: bool,
    is_error: bool,
    disabled: bool,
    label_color: xilem::masonry::peniko::Color,
) -> Prop<ContentColor, Label, State, Action> {
    let p = &spec.palette;
    let float_text = label_text.unwrap_or_default();
    spec.typography
        .style_label(TypeRole::LabelSmall, label(float_text))
        .color(if is_error {
            p.error
        } else if !is_empty {
            spec.content_color(p.accent, disabled)
        } else {
            label_color
        })
}

fn wrap_outlined_floating_notch<State: 'static, Action: 'static>(
    surface: xilem::masonry::peniko::Color,
    input_widget: Box<AnyWidgetView<State, Action>>,
    float_lbl: Prop<ContentColor, Label, State, Action>,
    is_error: bool,
) -> Box<AnyWidgetView<State, Action>> {
    let framed = sized_box(focus_frame(input_widget).error(is_error))
        .padding(Padding::top(7.px()))
        .alignment(UnitPoint::TOP_LEFT);
    let notch = sized_box(
        sized_box(float_lbl)
            .padding(Padding::from_vh(0.px(), 4.px()))
            .background_color(surface),
    )
    .padding(Padding::left(12.px()))
    .alignment(UnitPoint::TOP_LEFT);
    zstack((framed, notch)).boxed()
}

fn build_bottom_row<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    error_text: Option<&ArcStr>,
    supporting_text: Option<&ArcStr>,
    char_count: usize,
    max_chars: Option<usize>,
) -> Option<Box<AnyWidgetView<State, Action>>> {
    let p = &spec.palette;
    let is_error = error_text.is_some();
    let msg_text = error_text.or(supporting_text);
    let counter_text = max_chars.map(|max| format!("{char_count}/{max}"));

    if msg_text.is_none() && counter_text.is_none() {
        return None;
    }

    let msg_view = msg_text.map(|t| {
        let c = if is_error { p.error } else { p.text_secondary };
        spec.typography
            .style_label(TypeRole::BodySmall, label(t.clone()))
            .color(c)
    });
    let cnt_view = counter_text.map(|txt| {
        spec.typography
            .style_label(TypeRole::LabelSmall, label(txt))
            .color(p.text_secondary)
    });

    Some(
        flex_row((
            msg_view
                .map_or_else(|| label("").boxed(), WidgetView::boxed)
                .flex(1.0),
            cnt_view,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px())
        .boxed(),
    )
}

/// Internal vertical bounds container so [`UiTextArea`] starts at `min_height`, expands up to `max_height`, and scrolls beyond.
struct HeightRangeWidget {
    child: WidgetPod<dyn Widget>,
    min_height: f64,
    max_height: f64,
}

impl HeightRangeWidget {
    fn set_bounds(this: &mut WidgetMut<'_, Self>, min_height: f64, max_height: f64) {
        let max_height = max_height.max(min_height);
        if (this.widget.min_height - min_height).abs() > 0.01
            || (this.widget.max_height - max_height).abs() > 0.01
        {
            this.widget.min_height = min_height;
            this.widget.max_height = max_height;
            this.ctx.request_layout();
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for HeightRangeWidget {
    type Action = xilem::masonry::core::NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let len_def = match len_req {
            LenReq::MinContent => LenDef::MinContent,
            LenReq::MaxContent => LenDef::MaxContent,
            LenReq::FitContent(l) => LenDef::FitContent(l),
        };
        let context_size = LayoutSize::maybe(axis.cross(), cross_length);
        let len = ctx.compute_length(&mut self.child, len_def, context_size, axis, cross_length);
        match axis {
            Axis::Vertical => {
                let (lo, hi) = match len_req {
                    LenReq::FitContent(space) => (
                        self.min_height.min(space.get()),
                        self.max_height.min(space.get()),
                    ),
                    _ => (self.min_height, self.max_height),
                };
                Length::px(len.get().clamp(lo, hi.max(lo)))
            }
            Axis::Horizontal => len,
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let child_def = SizeDef::fixed(size);
        let measured = ctx.compute_size(&mut self.child, child_def, size.into());
        ctx.run_layout(&mut self.child, measured);
        ctx.place_child(&mut self.child, Point::ORIGIN);
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

struct HeightRangeView<V, State, Action> {
    child: V,
    min_height: f64,
    max_height: f64,
    phantom: PhantomData<fn(State) -> Action>,
}

fn height_range<V, State, Action>(
    child: V,
    min_height: f64,
    max_height: f64,
) -> HeightRangeView<V, State, Action> {
    HeightRangeView {
        child,
        min_height,
        max_height: max_height.max(min_height),
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for HeightRangeView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for HeightRangeView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = Pod<HeightRangeWidget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, st) = self.child.build(ctx, app_state);
        let pod = ctx.create_pod(HeightRangeWidget {
            child: child_pod.erased().new_widget.to_pod(),
            min_height: self.min_height,
            max_height: self.max_height,
        });
        (pod, st)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        HeightRangeWidget::set_bounds(&mut element, self.min_height, self.max_height);
        let mut child = HeightRangeWidget::child_mut(&mut element);
        self.child
            .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child = HeightRangeWidget::child_mut(&mut element);
        self.child.teardown(view_state, ctx, child.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut child = HeightRangeWidget::child_mut(&mut element);
        self.child
            .message(view_state, message, child.downcast(), app_state)
    }
}

/// Optional text submission callback invoked on Enter (when [`InsertNewline`] is `OnShiftEnter` or `Never`).
pub type TextSubmitCallback<State, Action> =
    Arc<dyn Fn(&mut State, String) -> Action + Send + Sync>;

/// Cross-theme multi-line text area with configurable newline behavior, vertical auto-growth bounds, scrolling, and focus chrome.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiTextArea<State, Action> {
    variant: TextFieldVariant,
    value: String,
    label_text: Option<ArcStr>,
    placeholder: Option<ArcStr>,
    supporting_text: Option<ArcStr>,
    error_text: Option<ArcStr>,
    max_chars: Option<usize>,
    disabled: bool,
    insert_newline: InsertNewline,
    min_height: f64,
    max_height: f64,
    on_change: TextChangeCallback<State, Action>,
    on_submit: Option<TextSubmitCallback<State, Action>>,
    on_cancel: Option<TextSubmitCallback<State, Action>>,
}

impl<State, Action> UiTextArea<State, Action> {
    /// Sets a top caption label above the text area.
    pub fn label(mut self, text: impl Into<ArcStr>) -> Self {
        self.label_text = Some(text.into());
        self
    }

    /// Sets placeholder hint text shown when empty.
    pub fn placeholder(mut self, text: impl Into<ArcStr>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Sets helper text displayed beneath the text area.
    pub fn supporting_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.supporting_text = Some(text.into());
        self
    }

    /// Sets validation error text (switches frame tint to `palette.error`).
    pub fn error_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.error_text = Some(text.into());
        self
    }

    /// Displays a `len / max` character counter in the supporting row.
    pub fn max_chars(mut self, max: usize) -> Self {
        self.max_chars = Some(max);
        self
    }

    /// Disables editing and dims foreground chrome.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Configures Enter vs Shift+Enter newline insertion behavior (`InsertNewline::OnEnter` by default).
    pub fn insert_newline(mut self, insert_newline: InsertNewline) -> Self {
        self.insert_newline = insert_newline;
        self
    }

    /// Sets minimum viewport height (`88px` by default).
    pub fn min_height(mut self, min_height: Length) -> Self {
        self.min_height = min_height.get().max(32.0);
        self
    }

    /// Sets maximum auto-grow height before vertical scrolling kicks in (`220px` by default).
    pub fn max_height(mut self, max_height: Length) -> Self {
        self.max_height = max_height.get().max(32.0);
        self
    }

    /// Attaches a submit callback invoked when Enter is pressed under `InsertNewline::OnShiftEnter` or `InsertNewline::Never`.
    pub fn on_submit<F>(mut self, on_submit: F) -> Self
    where
        F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
    {
        self.on_submit = Some(Arc::new(on_submit));
        self
    }

    /// Attaches a cancel callback invoked when Escape is pressed.
    pub fn on_cancel<F>(mut self, on_cancel: F) -> Self
    where
        F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
    {
        self.on_cancel = Some(Arc::new(on_cancel));
        self
    }

    fn build_flex(&self, spec: &DesignSpec) -> TextFieldFlex<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let m = &spec.metrics;
        let is_error = self.error_text.is_some();
        let is_md3_floating =
            spec.control_grammar() == ControlVisualGrammar::Material3 && self.label_text.is_some();
        let base_class = match self.variant {
            TextFieldVariant::Outlined => classes::TEXT_FIELD_OUTLINED,
            TextFieldVariant::Filled => classes::TEXT_FIELD_FILLED,
        };

        let label_color = if is_error {
            p.error
        } else {
            spec.content_color(p.text_secondary, self.disabled)
        };
        let top_label = (!is_md3_floating)
            .then(|| build_top_label(spec, self.label_text.as_ref(), label_color))
            .flatten();

        let style = spec.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();
        let mut ti = text_input(self.value.clone(), move |s: &mut State, txt: String| {
            on_change(s, txt)
        })
        .insert_newline(self.insert_newline)
        .clip(false)
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(spec.content_color(p.text_primary, self.disabled))
        .disabled(self.disabled);

        if let Some(ph) = &self.placeholder {
            ti = ti.placeholder(ph.clone());
        }
        if let Some(font) = spec.typography.font_family() {
            ti = ti.font(font.clone());
        }
        if let Some(on_submit) = &self.on_submit {
            let on_submit = on_submit.clone();
            ti = ti.on_enter(move |s: &mut State, txt: String| on_submit(s, txt));
        }
        if let Some(on_cancel) = &self.on_cancel {
            let on_cancel = on_cancel.clone();
            ti = ti.on_cancel(move |s: &mut State, txt: String| on_cancel(s, txt));
        }

        let bounded_portal = height_range(
            portal(ti.ui_class(classes::TEXT_FIELD_FLUSH))
                .constrain_horizontal(true)
                .must_fill(true),
            self.min_height,
            self.max_height,
        );

        let frame_slot: Box<AnyWidgetView<State, Action>> = if is_md3_floating {
            let float_lbl = build_floating_label(
                spec,
                self.label_text.clone(),
                self.value.is_empty(),
                is_error,
                self.disabled,
                label_color,
            );
            match self.variant {
                TextFieldVariant::Filled => {
                    let container = flex_col((float_lbl, bounded_portal))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px())
                        .ui_class(classes::TEXT_FIELD_FILLED)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .padding(Padding::from_vh(8.px(), m.input_pad_h.px()))
                        .boxed();
                    focus_frame(container).error(is_error).boxed()
                }
                TextFieldVariant::Outlined => {
                    let input_widget = flex_col((bounded_portal,))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .ui_class(base_class)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .boxed();
                    wrap_outlined_floating_notch(p.surface, input_widget, float_lbl, is_error)
                }
            }
        } else {
            let input_widget = flex_col((bounded_portal,))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .ui_class(base_class)
                .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                .boxed();
            focus_frame(input_widget).error(is_error).boxed()
        };

        let bottom_row = build_bottom_row(
            spec,
            self.error_text.as_ref(),
            self.supporting_text.as_ref(),
            self.value.chars().count(),
            self.max_chars,
        );

        flex_col((top_label, frame_slot, bottom_row))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(4.px())
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiTextArea<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiTextArea<State, Action> {
    type Element = Pod<widgets::Flex>;
    type ViewState = (
        TextFieldFlex<State, Action>,
        <TextFieldFlex<State, Action> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let flex = self.build_flex(&spec);
        let (pod, st) = flex.build(ctx, app_state);
        (pod, (flex, st))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_flex = self.build_flex(&spec);
        next_flex.rebuild(prev_flex, st, ctx, element, app_state);
        *prev_flex = next_flex;
    }

    fn teardown(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_flex.teardown(st, ctx, element);
    }

    fn message(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_flex.message(st, message, element, app_state)
    }
}

fn make_ta<State, Action, F>(
    variant: TextFieldVariant,
    value: impl Into<String>,
    on_change: F,
) -> UiTextArea<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    UiTextArea {
        variant,
        value: value.into(),
        label_text: None,
        placeholder: None,
        supporting_text: None,
        error_text: None,
        max_chars: None,
        disabled: false,
        insert_newline: InsertNewline::OnEnter,
        min_height: 88.0,
        max_height: 220.0,
        on_change: Arc::new(on_change),
        on_submit: None,
        on_cancel: None,
    }
}

/// Outlined multi-line text area driven by [`DesignSpec`] with scrollable vertical auto-growth and [`FocusFrame`](crate::foundation::FocusFrame).
pub fn text_area<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextArea<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_ta(TextFieldVariant::Outlined, value, on_change)
}

/// Filled tonal multi-line text area driven by [`DesignSpec`] with scrollable vertical auto-growth and [`FocusFrame`](crate::foundation::FocusFrame).
pub fn filled_text_area<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextArea<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_ta(TextFieldVariant::Filled, value, on_change)
}
