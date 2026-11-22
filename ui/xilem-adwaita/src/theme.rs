//! `AdwaitaTheme` constructing a GNOME Libadwaita [`DesignSpec`].

use xilem_ui_core::tokens::{
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle, NavIndicatorStyle,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeScale,
};

use crate::palette::{AdwaitaAccent, adwaita_palette};

/// Configuration handle generating GNOME Libadwaita [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct AdwaitaTheme {
    /// Light or Dark polarity.
    pub brightness: Brightness,
    /// GNOME system accent preset.
    pub accent: AdwaitaAccent,
    /// Desktop Adwaita typography ramp (`15px` body base).
    pub typography: TypeScale,
}

impl AdwaitaTheme {
    /// GNOME Libadwaita Light theme (GNOME Blue accent).
    #[must_use]
    pub fn light() -> Self {
        Self {
            brightness: Brightness::Light,
            accent: AdwaitaAccent::Blue,
            typography: TypeScale::compact_desktop(15.0),
        }
    }

    /// GNOME Libadwaita Dark theme (GNOME Blue accent).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            brightness: Brightness::Dark,
            accent: AdwaitaAccent::Blue,
            typography: TypeScale::compact_desktop(15.0),
        }
    }

    /// Overrides the GNOME accent preset.
    #[must_use]
    pub fn with_accent(mut self, accent: AdwaitaAccent) -> Self {
        self.accent = accent;
        self
    }

    /// Overrides the typography scale.
    #[must_use]
    pub fn with_typography(mut self, typography: TypeScale) -> Self {
        self.typography = typography;
        self
    }

    /// Libadwaita control geometry (`34px` buttons, `8px` controls, `12px` boxed lists/cards, `46px` headerbar).
    #[must_use]
    pub const fn metrics() -> ControlMetrics {
        ControlMetrics {
            button_height: 34.0,
            button_radius: 8.0,
            button_pad_h: 16.0,
            icon_btn_size: 34.0,
            icon_btn_radius: 8.0,
            chip_height: 30.0,
            chip_radius: 999.0,
            input_radius: 8.0,
            input_pad_v: 8.0,
            input_pad_h: 12.0,
            search_height: 36.0,
            search_radius: 999.0,
            card_radius: 12.0,
            card_pad: 14.0,
            dialog_radius: 14.0,
            sheet_radius: 14.0,
            menu_radius: 10.0,
            top_bar_height: 46.0,
            bottom_bar_height: 56.0,
            nav_pill_radius: 8.0,
            sidebar_width: 260.0,
            switch_height: 26.0,
            switch_width: 48.0,
            checkbox_radius: 4.0,
        }
    }

    /// Converts this GNOME Adwaita theme into [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        DesignSpec {
            name: "GNOME Adwaita",
            brightness: self.brightness,
            palette: adwaita_palette(self.accent, self.brightness),
            metrics: Self::metrics(),
            feedback: StateFeedbackMode::AlphaOverlay {
                hover_alpha: 0.07,
                press_alpha: 0.14,
            },
            input_focus: InputFocusStyle::Outline2px,
            tab_indicator: TabIndicatorStyle::SegmentedThumb,
            nav_indicator: NavIndicatorStyle::RoundedRow,
            dialog_action_order: DialogActionOrder::PrimaryTrailing,
            shortcut_convention: ShortcutConvention::Control,
            typography: self.typography.clone(),
        }
    }
}

impl Default for AdwaitaTheme {
    fn default() -> Self {
        Self::light()
    }
}
