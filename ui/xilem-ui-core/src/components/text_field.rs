//! Outlined & Filled single-line text fields with reactive [`FocusFrame`](crate::foundation::FocusFrame) indicators.

use std::sync::Arc;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{ContentColor, Gap};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, Flex, FlexExt, Label, Prop, button, flex_col, flex_row, label, text_input,
};
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::foundation::{FocusFrameView, UiClassExt, focus_frame, icons, ui_icon};
use crate::style_engine::{classes, current_spec};
use crate::tokens::{DesignSpec, TypeRole};

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
            FocusFrameView<Box<AnyWidgetView<State, Action>>, State, Action>,
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
        let is_error = self.error_text.is_some();
        let class = if is_error {
            classes::TEXT_FIELD_ERROR
        } else {
            match self.variant {
                TextFieldVariant::Outlined => classes::TEXT_FIELD_OUTLINED,
                TextFieldVariant::Filled => classes::TEXT_FIELD_FILLED,
            }
        };

        let top_label = self.label_text.as_ref().map(|t| {
            let color = if is_error {
                p.error
            } else {
                spec.content_color(p.text_secondary, self.disabled)
            };
            spec.typography
                .style_label(TypeRole::LabelMedium, label(t.clone()))
                .color(color)
        });

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

        let input_widget: Box<AnyWidgetView<State, Action>> = if self.prefix_text.is_some()
            || self.suffix_text.is_some()
            || self.trailing_action.is_some()
        {
            let flush = ti.ui_class(classes::TEXT_FIELD_FLUSH);
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
            flex_row((prefix, flush.flex(1.0), suffix, trail, trail_err))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
                .ui_class(class)
                .boxed()
        } else {
            ti.ui_class(class).boxed()
        };

        let frame = focus_frame(input_widget).error(is_error);

        let msg_text = self.error_text.as_ref().or(self.supporting_text.as_ref());
        let counter_text = self
            .max_chars
            .map(|max| format!("{}/{}", self.value.chars().count(), max));

        let bottom_row = if msg_text.is_some() || counter_text.is_some() {
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
        } else {
            None
        };

        flex_col((top_label, frame, bottom_row))
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
        let spec = current_spec(ctx);
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
        let spec = current_spec(ctx);
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
