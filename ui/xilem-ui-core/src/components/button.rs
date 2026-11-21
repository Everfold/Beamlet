//! Spec-driven push buttons adapting height, corner radius, and state layer per theme.

use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, button, flex_row, sized_box};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::{UiClassExt, ui_icon};
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Cross-theme semantic button emphasis variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// Highest-emphasis accent button (MD3 Filled / WinUI Accent / HIG Default / Adwaita Suggested).
    #[default]
    Primary,
    /// Medium-emphasis filled secondary container button.
    Secondary,
    /// Subtle elevated button.
    Elevated,
    /// Bordered outline button.
    Outlined,
    /// Text/flat/borderless button.
    Subtle,
}

impl ButtonVariant {
    const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => classes::BTN_PRIMARY,
            Self::Secondary => classes::BTN_SECONDARY,
            Self::Elevated => classes::BTN_ELEVATED,
            Self::Outlined => classes::BTN_OUTLINED,
            Self::Subtle => classes::BTN_SUBTLE,
        }
    }
}

/// Callback signature for [`UiButton`].
pub type ButtonCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Multi-theme button view reading geometry and colors from [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiButton<State, Action> {
    variant: ButtonVariant,
    text: ArcStr,
    leading_icon: Option<&'static str>,
    trailing_icon: Option<&'static str>,
    disabled: bool,
    on_click: ButtonCallback<State, Action>,
}

impl<State, Action> UiButton<State, Action> {
    /// Adds an `18px` leading SVG icon before the button label.
    pub fn leading_icon(mut self, svg_path: &'static str) -> Self {
        self.leading_icon = Some(svg_path);
        self
    }

    /// Adds an `18px` trailing SVG icon after the button label.
    pub fn trailing_icon(mut self, svg_path: &'static str) -> Self {
        self.trailing_icon = Some(svg_path);
        self
    }

    /// Disables pointer and keyboard activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let active_fg = match self.variant {
            ButtonVariant::Primary => p.on_accent,
            ButtonVariant::Secondary => p.on_secondary_container,
            ButtonVariant::Elevated | ButtonVariant::Subtle => p.accent,
            ButtonVariant::Outlined => p.text_primary,
        };
        let fg = spec.content_color(active_fg, self.disabled);
        let lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, xilem::view::label(self.text.clone()))
            .color(fg);

        let content: Box<AnyWidgetView<State, Action>> =
            if self.leading_icon.is_some() || self.trailing_icon.is_some() {
                let lead = self.leading_icon.map(|path| ui_icon(path, 16.0, fg));
                let trail = self.trailing_icon.map(|path| ui_icon(path, 16.0, fg));
                flex_row((lead, lbl, trail))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px())
                    .boxed()
            } else {
                lbl.boxed()
            };

        let on_click = self.on_click.clone();
        sized_box(
            button(content, move |state: &mut State| on_click(state))
                .disabled(self.disabled)
                .ui_class(self.variant.class_name()),
        )
        .dims(Dimensions::height(spec.metrics.button_height.px()))
        .boxed()
    }
}

crate::impl_spec_view!(UiButton<State, Action>);

fn make_button<State, Action, F>(
    variant: ButtonVariant,
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiButton {
        variant,
        text: text.into(),
        leading_icon: None,
        trailing_icon: None,
        disabled: false,
        on_click: Arc::new(on_click),
    }
}

/// Primary accent button.
pub fn primary_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Primary, text, on_click)
}

/// Secondary container button.
pub fn secondary_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Secondary, text, on_click)
}

/// Subtle elevated button.
pub fn elevated_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Elevated, text, on_click)
}

/// Outlined bordered button.
pub fn outlined_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Outlined, text, on_click)
}

/// Low-emphasis flat/text button.
pub fn subtle_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Subtle, text, on_click)
}
