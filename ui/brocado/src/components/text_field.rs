//! Outlined & Filled single-line and multi-line text fields with reactive [`FocusFrame`](crate::foundation::FocusFrame) indicators.

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::{AsUnit, Dim, Length, UnitPoint};
use masonry::parley::LineHeight;
use masonry::properties::{Dimensions, Gap, LineBreaking, Padding};
use masonry::widgets;
use masonry::widgets::InsertNewline;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, Flex, FlexExt, Prop, ZStackExt, button, flex_col, flex_row, label,
    sized_box, text_input, zstack,
};

use crate::components::typography::{UiLabel, ui_label};
use crate::foundation::{
    ScrollBarVisibility, UiClassExt, clip_box, constrained_box, focus_frame, icons, scroll_view,
    ui_icon,
};
use crate::style_engine::{classes, current_theme_arc};
use crate::theme::Theme;
use crate::theme::metrics::INPUT_PAD_H;
use crate::tokens::TypeRole;

/// Text field surface variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextFieldVariant {
    /// Outlined input frame.
    #[default]
    Outlined,
    /// Filled tonal input container.
    Filled,
}

/// Matches Masonry's `default_text_styles` (`LineHeight::FontSizeRelative(1.2)`) used by
/// `TextInput` / `TextArea` so prefix/suffix labels share identical vertical metrics and baselines.
const TEXT_INPUT_LINE_HEIGHT: LineHeight = LineHeight::FontSizeRelative(1.2);

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
#[must_use = "View values do nothing unless provided to Brocado."]
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

    fn build_flex(&self, theme: &Theme) -> TextFieldFlex<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let s = &theme.scheme;
        let is_error = self.error_text.is_some();
        let is_floating = self.label_text.is_some();
        let base_class = match self.variant {
            TextFieldVariant::Outlined => classes::TEXT_FIELD_OUTLINED,
            TextFieldVariant::Filled => classes::TEXT_FIELD_FILLED,
        };

        let label_color = if is_error {
            s.error
        } else {
            theme.content_color(s.on_surface_variant, self.disabled)
        };

        let style = theme.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();
        let mut ti = text_input(self.value.clone(), move |st: &mut State, txt: String| {
            on_change(st, txt)
        })
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(theme.content_color(s.on_surface, self.disabled))
        .font(theme.typography.resolved_font_family())
        .disabled(self.disabled);

        if let Some(ph) = &self.placeholder {
            ti = ti.placeholder(ph.clone());
        }

        let has_addons = self.prefix_text.is_some()
            || self.suffix_text.is_some()
            || self.trailing_action.is_some();

        let make_row_inner = |ti_view: Box<AnyWidgetView<State, Action>>| {
            let sub_ink = theme.content_color(s.on_surface_variant, self.disabled);
            let prefix = self.prefix_text.as_ref().map(|t| {
                ui_label(TypeRole::BodyMedium, t.clone())
                    .line_height(TEXT_INPUT_LINE_HEIGHT)
                    .color(sub_ink)
                    .line_breaking(LineBreaking::Clip)
            });
            let suffix = self.suffix_text.as_ref().map(|t| {
                ui_label(TypeRole::BodyMedium, t.clone())
                    .line_height(TEXT_INPUT_LINE_HEIGHT)
                    .color(sub_ink)
                    .line_breaking(LineBreaking::Clip)
            });
            let trail = self.trailing_action.as_ref().map(|(icon, cb)| {
                let cb = cb.clone();
                button(
                    ui_icon(icon, 16.0, if is_error { s.error } else { sub_ink }),
                    move |st: &mut State| cb(st),
                )
                .disabled(self.disabled)
                .ui_class(classes::ICON_BTN_STD)
            });
            let trail_err = if is_error && trail.is_none() {
                Some(ui_icon(icons::ERROR_CIRCLE, 16.0, s.error))
            } else {
                None
            };
            let text_row = flex_row((prefix, ti_view.flex(1.0), suffix))
                .cross_axis_alignment(CrossAxisAlignment::FirstBaseline)
                .gap(6.px())
                .dims(Dimensions::height(Dim::MaxContent));
            flex_row((text_row.flex(1.0), trail, trail_err))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
        };

        let frame_slot: Box<AnyWidgetView<State, Action>> = if is_floating {
            let float_lbl = build_floating_label(
                theme,
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
                    let container = flex_col((clip_box(float_lbl), row))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(2.px())
                        .ui_class(classes::TEXT_FIELD_FILLED)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .padding(Padding::from_vh(7.px(), INPUT_PAD_H.px()))
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
                    wrap_outlined_floating_notch(s.surface, input_widget, float_lbl, is_error)
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
            theme,
            self.error_text.as_ref(),
            self.supporting_text.as_ref(),
            self.value.chars().count(),
            self.max_chars,
        );

        flex_col((frame_slot, bottom_row))
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
        let theme = current_theme_arc(ctx);
        let flex = self.build_flex(&theme);
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
        let theme = current_theme_arc(ctx);
        let next_flex = self.build_flex(&theme);
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

/// Outlined text field driven by [`Theme`] and wrapped in a reactive [`FocusFrame`](crate::foundation::FocusFrame).
pub fn text_field<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextField<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_tf(TextFieldVariant::Outlined, value, on_change)
}

/// Filled tonal container text field driven by [`Theme`] and wrapped in a reactive [`FocusFrame`](crate::foundation::FocusFrame).
pub fn filled_text_field<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextField<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_tf(TextFieldVariant::Filled, value, on_change)
}

fn build_floating_label<State: 'static, Action: 'static>(
    theme: &Theme,
    label_text: Option<ArcStr>,
    is_empty: bool,
    is_error: bool,
    disabled: bool,
    label_color: masonry::peniko::Color,
) -> UiLabel<State, Action> {
    let s = &theme.scheme;
    let float_text = label_text.unwrap_or_default();
    ui_label(TypeRole::LabelSmall, float_text)
        .color(if is_error {
            s.error
        } else if !is_empty {
            theme.content_color(s.primary, disabled)
        } else {
            label_color
        })
        .line_breaking(LineBreaking::Clip)
}

fn wrap_outlined_floating_notch<State: 'static, Action: 'static>(
    surface: masonry::peniko::Color,
    input_widget: Box<AnyWidgetView<State, Action>>,
    float_lbl: UiLabel<State, Action>,
    is_error: bool,
) -> Box<AnyWidgetView<State, Action>> {
    let framed = sized_box(focus_frame(input_widget).error(is_error))
        .padding(Padding::top(7.px()))
        .alignment(UnitPoint::TOP_LEFT);
    let notch = sized_box(
        sized_box(clip_box(float_lbl))
            .padding(Padding::from_vh(0.px(), 4.px()))
            .background_color(surface),
    )
    .padding(Padding::left(12.px()))
    .alignment(UnitPoint::TOP_LEFT);
    zstack((framed, notch)).boxed()
}

fn build_bottom_row<State: 'static, Action: 'static>(
    theme: &Theme,
    error_text: Option<&ArcStr>,
    supporting_text: Option<&ArcStr>,
    char_count: usize,
    max_chars: Option<usize>,
) -> Option<Box<AnyWidgetView<State, Action>>> {
    let s = &theme.scheme;
    let is_error = error_text.is_some();
    let msg_text = error_text.or(supporting_text);
    let counter_text = max_chars.map(|max| format!("{char_count}/{max}"));

    if msg_text.is_none() && counter_text.is_none() {
        return None;
    }

    let msg_view = msg_text.map(|t| {
        let c = if is_error {
            s.error
        } else {
            s.on_surface_variant
        };
        ui_label(TypeRole::BodySmall, t.clone()).color(c)
    });
    let cnt_view = counter_text.map(|txt| {
        ui_label(TypeRole::LabelSmall, txt)
            .color(s.on_surface_variant)
            .line_breaking(LineBreaking::Clip)
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

/// Optional text submission callback invoked on Enter (when [`InsertNewline`] is `OnShiftEnter` or `Never`).
pub type TextSubmitCallback<State, Action> =
    Arc<dyn Fn(&mut State, String) -> Action + Send + Sync>;

/// Cross-theme multi-line text area with configurable newline behavior, vertical auto-growth bounds, scrolling, and focus chrome.
#[must_use = "View values do nothing unless provided to Brocado."]
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

    fn build_flex(&self, theme: &Theme) -> TextFieldFlex<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let s = &theme.scheme;
        let is_error = self.error_text.is_some();
        let is_floating = self.label_text.is_some();
        let base_class = match self.variant {
            TextFieldVariant::Outlined => classes::TEXT_FIELD_OUTLINED,
            TextFieldVariant::Filled => classes::TEXT_FIELD_FILLED,
        };

        let label_color = if is_error {
            s.error
        } else {
            theme.content_color(s.on_surface_variant, self.disabled)
        };

        let style = theme.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();
        let mut ti = text_input(self.value.clone(), move |st: &mut State, txt: String| {
            on_change(st, txt)
        })
        .insert_newline(self.insert_newline)
        .clip(false)
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(theme.content_color(s.on_surface, self.disabled))
        .font(theme.typography.resolved_font_family())
        .disabled(self.disabled);

        if let Some(ph) = &self.placeholder {
            ti = ti.placeholder(ph.clone());
        }
        if let Some(on_submit) = &self.on_submit {
            let on_submit = on_submit.clone();
            ti = ti.on_enter(move |st: &mut State, txt: String| on_submit(st, txt));
        }
        if let Some(on_cancel) = &self.on_cancel {
            let on_cancel = on_cancel.clone();
            ti = ti.on_cancel(move |st: &mut State, txt: String| on_cancel(st, txt));
        }

        let bounded_portal = constrained_box(
            scroll_view(ti.ui_class(classes::TEXT_FIELD_FLUSH))
                .constrain_horizontal(true)
                .must_fill(true)
                .scrollbar_visibility(ScrollBarVisibility::Never),
        )
        .min_height(self.min_height)
        .max_height(self.max_height);

        let frame_slot: Box<AnyWidgetView<State, Action>> = if is_floating {
            let float_lbl = build_floating_label(
                theme,
                self.label_text.clone(),
                self.value.is_empty(),
                is_error,
                self.disabled,
                label_color,
            );
            match self.variant {
                TextFieldVariant::Filled => {
                    let container = flex_col((clip_box(float_lbl), bounded_portal))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px())
                        .ui_class(classes::TEXT_FIELD_FILLED)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .padding(Padding::from_vh(8.px(), INPUT_PAD_H.px()))
                        .boxed();
                    focus_frame(container).error(is_error).boxed()
                }
                TextFieldVariant::Outlined => {
                    let input_widget = flex_col((bounded_portal,))
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .ui_class(base_class)
                        .ui_class_if(is_error, classes::TEXT_FIELD_ERROR)
                        .boxed();
                    wrap_outlined_floating_notch(s.surface, input_widget, float_lbl, is_error)
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
            theme,
            self.error_text.as_ref(),
            self.supporting_text.as_ref(),
            self.value.chars().count(),
            self.max_chars,
        );

        flex_col((frame_slot, bottom_row))
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
        let theme = current_theme_arc(ctx);
        let flex = self.build_flex(&theme);
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
        let theme = current_theme_arc(ctx);
        let next_flex = self.build_flex(&theme);
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

/// Outlined multi-line text area driven by [`Theme`] with scrollable vertical auto-growth and [`FocusFrame`](crate::foundation::FocusFrame).
pub fn text_area<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextArea<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_ta(TextFieldVariant::Outlined, value, on_change)
}

/// Filled tonal multi-line text area driven by [`Theme`] with scrollable vertical auto-growth and [`FocusFrame`](crate::foundation::FocusFrame).
pub fn filled_text_area<State: 'static, Action: 'static, F>(
    value: impl Into<String>,
    on_change: F,
) -> UiTextArea<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    make_ta(TextFieldVariant::Filled, value, on_change)
}
