//! `Md3Theme` constructing a complete Material Design 3 [`DesignSpec`].

use xilem::Color;
use xilem_ui_core::tokens::{
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle, NavIndicatorStyle,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeScale,
};

use crate::color::{ColorScheme, ContrastLevel};

/// Configuration handle generating Material Design 3 [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct Md3Theme {
    /// Material 3 dynamic color scheme.
    pub scheme: ColorScheme,
    /// Typographic scale (defaults to Material 3 15-role baseline scale).
    pub typography: TypeScale,
}

impl Md3Theme {
    /// Default Material Design 3 Light theme (`#6750A4`).
    #[must_use]
    pub fn light() -> Self {
        Self {
            scheme: ColorScheme::baseline_light(),
            typography: TypeScale::baseline(),
        }
    }

    /// Default Material Design 3 Dark theme (`#6750A4`).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            scheme: ColorScheme::baseline_dark(),
            typography: TypeScale::baseline(),
        }
    }

    /// Builds an OKLCH dynamic theme from an arbitrary seed color.
    #[must_use]
    pub fn from_seed(seed: Color, brightness: Brightness) -> Self {
        Self {
            scheme: ColorScheme::from_seed(seed, brightness, ContrastLevel::Standard),
            typography: TypeScale::baseline(),
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
            search_height: 52.0,
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
            typography: self.typography.clone(),
        }
    }
}

impl Default for Md3Theme {
    fn default() -> Self {
        Self::light()
    }
}
