//! `Md3Theme` constructing a complete Material Design 3 [`DesignSpec`].

use crate::style_engine::scope::{UiScope, ui_scope};
use crate::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle,
    MotionScheme, NavIndicatorStyle, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle,
    TypeScale, WindowBackdrop,
};
use masonry::core::FromDynWidget;
use masonry::peniko::Color;
use xilem_masonry::WidgetView;

use super::color::{ColorScheme, ContrastLevel, Md3ColorVariant};

/// Configuration handle generating Material Design 3 [`DesignSpec`] instances.
/// Material Design 3 visual style generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Md3Style {
    /// Original Material 3 specification (baseline typography, standard motion, 20dp pill buttons).
    Baseline,
    /// Material 3 Expressive specification (emphasized typography, spring motion, full-pill buttons with `:active` morph).
    #[default]
    Expressive,
}

/// Complete Material Design 3 theme definition bundling dynamic color, typography, style generation, and motion preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct Md3Theme {
    /// Material 3 dynamic color scheme.
    pub scheme: ColorScheme,
    /// Visual style generation (`Expressive` by default, with `Baseline` fallback).
    pub style: Md3Style,
    /// Typographic scale (defaults to Material 3 Expressive 15-role ramp).
    pub typography: TypeScale,
    /// Disables entrance/transition motion durations and spatial offsets when `true`.
    pub reduced_motion: bool,
}

impl Md3Theme {
    /// Default Material Design 3 Expressive Light theme (`#6750A4`).
    #[must_use]
    pub fn light() -> Self {
        Self::from_seed(Color::from_rgb8(0x67, 0x50, 0xA4), Brightness::Light)
    }

    /// Default Material Design 3 Expressive Dark theme (`#6750A4`).
    #[must_use]
    pub fn dark() -> Self {
        Self::from_seed(Color::from_rgb8(0x67, 0x50, 0xA4), Brightness::Dark)
    }

    /// Baseline (pre-Expressive) Material Design 3 Light theme (`#6750A4`).
    #[must_use]
    pub fn baseline_light() -> Self {
        Self::light().with_style(Color::from_rgb8(0x67, 0x50, 0xA4), Md3Style::Baseline)
    }

    /// Baseline (pre-Expressive) Material Design 3 Dark theme (`#6750A4`).
    #[must_use]
    pub fn baseline_dark() -> Self {
        Self::dark().with_style(Color::from_rgb8(0x67, 0x50, 0xA4), Md3Style::Baseline)
    }

    /// Builds an OKLCH dynamic Material 3 Expressive theme from an arbitrary seed color.
    #[must_use]
    pub fn from_seed(seed: Color, brightness: Brightness) -> Self {
        Self {
            scheme: ColorScheme::from_seed_with_variant(
                seed,
                brightness,
                ContrastLevel::Standard,
                Md3ColorVariant::Expressive,
            ),
            style: Md3Style::Expressive,
            typography: TypeScale::md3_expressive(),
            reduced_motion: false,
        }
    }

    /// Switches between [`Md3Style::Baseline`] and [`Md3Style::Expressive`], updating palette, typography, and motion tokens.
    #[must_use]
    pub fn with_style(mut self, seed: Color, style: Md3Style) -> Self {
        self.style = style;
        let variant = match style {
            Md3Style::Baseline => Md3ColorVariant::Baseline,
            Md3Style::Expressive => Md3ColorVariant::Expressive,
        };
        self.scheme = ColorScheme::from_seed_with_variant(
            seed,
            self.scheme.brightness,
            self.scheme.contrast,
            variant,
        );
        self.typography = match style {
            Md3Style::Baseline => TypeScale::md3(),
            Md3Style::Expressive => TypeScale::md3_expressive(),
        };
        self
    }

    /// Updates the contrast tier while preserving the hue seed, style generation, and polarity.
    #[must_use]
    pub fn with_contrast(mut self, seed: Color, contrast: ContrastLevel) -> Self {
        let variant = match self.style {
            Md3Style::Baseline => Md3ColorVariant::Baseline,
            Md3Style::Expressive => Md3ColorVariant::Expressive,
        };
        self.scheme =
            ColorScheme::from_seed_with_variant(seed, self.scheme.brightness, contrast, variant);
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

    /// Material Design 3 Expressive control metrics (`16px` cards, `12px` inputs/menus, `40px` full-pill buttons).
    #[must_use]
    pub const fn expressive_metrics() -> ControlMetrics {
        let mut metrics = Self::metrics();
        metrics.card_radius = 16.0;
        metrics.input_radius = 12.0;
        metrics.menu_radius = 12.0;
        metrics
    }

    /// Converts this Material 3 theme into [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        let (name, metrics, motion_scheme, active_button_radius) = match self.style {
            Md3Style::Baseline => ("Material 3", Self::metrics(), MotionScheme::Standard, None),
            Md3Style::Expressive => (
                "Material 3 Expressive",
                Self::expressive_metrics(),
                MotionScheme::Expressive,
                Some(12.0),
            ),
        };
        DesignSpec {
            name,
            brightness: self.scheme.brightness,
            backdrop: WindowBackdrop::Opaque,
            palette: self.scheme.to_semantic_palette(),
            metrics,
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
            motion_scheme,
            active_button_radius,
            reduced_motion: self.reduced_motion,
            typography: self.typography.clone(),
            locale: Default::default(),
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
