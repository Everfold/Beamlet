//! `Md3Theme` constructing a complete Material Design 3 [`DesignSpec`].

use xilem::masonry::core::FromDynWidget;
use xilem::{Color, WidgetView, WindowId, WindowView};
use xilem_ui_core::style_engine::scope::{UiScope, ui_scope, ui_window};
use xilem_ui_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle,
    NavIndicatorStyle, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeScale,
};

use crate::color::{ColorScheme, ContrastLevel};

/// Configuration handle generating Material Design 3 [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct Md3Theme {
    /// Material 3 dynamic color scheme.
    pub scheme: ColorScheme,
    /// Typographic scale (defaults to Material 3 15-role official ramp).
    pub typography: TypeScale,
    /// Disables entrance/transition motion durations and spatial offsets when `true`.
    pub reduced_motion: bool,
}

impl Md3Theme {
    /// Default Material Design 3 Light theme (`#6750A4`).
    #[must_use]
    pub fn light() -> Self {
        Self {
            scheme: ColorScheme::baseline_light(),
            typography: TypeScale::md3(),
            reduced_motion: false,
        }
    }

    /// Default Material Design 3 Dark theme (`#6750A4`).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            scheme: ColorScheme::baseline_dark(),
            typography: TypeScale::md3(),
            reduced_motion: false,
        }
    }

    /// Builds an OKLCH dynamic theme from an arbitrary seed color.
    #[must_use]
    pub fn from_seed(seed: Color, brightness: Brightness) -> Self {
        Self {
            scheme: ColorScheme::from_seed(seed, brightness, ContrastLevel::Standard),
            typography: TypeScale::md3(),
            reduced_motion: false,
        }
    }

    /// Updates the contrast tier while preserving the hue seed and polarity.
    #[must_use]
    pub fn with_contrast(mut self, seed: Color, contrast: ContrastLevel) -> Self {
        self.scheme = ColorScheme::from_seed(seed, self.scheme.brightness, contrast);
        self
    }

    /// Overrides the typography scale.
    #[must_use]
    pub fn with_typography(mut self, typography: TypeScale) -> Self {
        self.typography = typography;
        self
    }

    /// Sets accessibility reduced-motion preference.
    #[must_use]
    pub const fn with_reduced_motion(mut self, reduced: bool) -> Self {
        self.reduced_motion = reduced;
        self
    }

    /// Material Design 3 control metrics (`40px` pill buttons, `28px` dialogs/sheets).
    #[must_use]
    pub const fn metrics() -> ControlMetrics {
        ControlMetrics {
            button_height: 40.0,
            button_radius: 999.0,
            button_pad_h: 24.0,
            icon_btn_size: 40.0,
            icon_btn_radius: 999.0,
            chip_height: 32.0,
            chip_radius: 8.0,
            input_radius: 8.0,
            input_pad_v: 12.0,
            input_pad_h: 16.0,
            search_height: 56.0,
            search_radius: 999.0,
            card_radius: 12.0,
            card_pad: 16.0,
            dialog_radius: 28.0,
            sheet_radius: 28.0,
            menu_radius: 8.0,
            top_bar_height: 64.0,
            bottom_bar_height: 80.0,
            nav_pill_radius: 999.0,
            sidebar_width: 260.0,
            switch_height: 32.0,
            switch_width: 52.0,
            checkbox_radius: 2.0,
        }
    }

    /// Converts this Material 3 theme into [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        DesignSpec {
            name: "Material 3",
            brightness: self.scheme.brightness,
            palette: self.scheme.to_semantic_palette(),
            metrics: Self::metrics(),
            feedback: StateFeedbackMode::TintedStateLayer {
                hover_alpha: 0.08,
                press_alpha: 0.12,
            },
            input_focus: InputFocusStyle::Outline2px,
            tab_indicator: TabIndicatorStyle::Underline { height: 3.0 },
            nav_indicator: NavIndicatorStyle::PillBackground,
            dialog_action_order: DialogActionOrder::PrimaryTrailing,
            shortcut_convention: ShortcutConvention::Control,
            control_border: ControlBorderStyle::UniformHairline,
            reduced_motion: self.reduced_motion,
            typography: self.typography.clone(),
        }
    }
}

impl Default for Md3Theme {
    fn default() -> Self {
        Self::light()
    }
}

/// Wraps `child` inside a live Material Design 3 [`UiScope`] matching `theme`.
pub fn md3_scope<State: 'static, Action: 'static, V>(
    theme: &Md3Theme,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    ui_scope(theme.to_spec(), child)
}

/// Wraps a standalone desktop [`WindowView`] with Material Design 3 backdrop, properties, and [`UiScope`].
pub fn md3_window<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    theme: &Md3Theme,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    ui_window(window_id, title, &theme.to_spec(), child)
}
