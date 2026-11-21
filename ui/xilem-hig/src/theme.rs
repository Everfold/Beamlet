//! `HigTheme` constructing an Apple Human Interface Guidelines [`DesignSpec`].

use xilem_ui_core::tokens::{
    Brightness, ControlMetrics, DesignSpec, InputFocusStyle, NavIndicatorStyle, StateFeedbackMode,
    TabIndicatorStyle, TypeScale,
};

use crate::palette::{HigAccent, hig_palette};

/// Configuration handle generating macOS Apple HIG [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct HigTheme {
    /// Light or Dark Aqua polarity.
    pub brightness: Brightness,
    /// Apple system accent color.
    pub accent: HigAccent,
    /// Compact `13px` macOS typography ramp.
    pub typography: TypeScale,
}

impl HigTheme {
    /// macOS Aqua Light theme (System Blue accent).
    #[must_use]
    pub fn light() -> Self {
        Self {
            brightness: Brightness::Light,
            accent: HigAccent::Blue,
            typography: TypeScale::compact_desktop(13.0),
        }
    }

    /// macOS Aqua Dark theme (System Blue accent).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            brightness: Brightness::Dark,
            accent: HigAccent::Blue,
            typography: TypeScale::compact_desktop(13.0),
        }
    }

    /// Overrides the Apple accent color.
    #[must_use]
    pub fn with_accent(mut self, accent: HigAccent) -> Self {
        self.accent = accent;
        self
    }

    /// Overrides the typography ramp.
    #[must_use]
    pub fn with_typography(mut self, typography: TypeScale) -> Self {
        self.typography = typography;
        self
    }

    /// Apple HIG control metrics (`26px` buttons, `6px` radii, `10px` grouped forms, `38px` toolbar).
    #[must_use]
    pub const fn metrics() -> ControlMetrics {
        ControlMetrics {
            button_height: 26.0,
            button_radius: 6.0,
            button_pad_h: 12.0,
            icon_btn_size: 26.0,
            icon_btn_radius: 6.0,
            chip_height: 22.0,
            chip_radius: 6.0,
            input_radius: 6.0,
            input_pad_v: 4.0,
            input_pad_h: 8.0,
            search_height: 28.0,
            search_radius: 6.0,
            card_radius: 10.0,
            card_pad: 12.0,
            dialog_radius: 12.0,
            sheet_radius: 12.0,
            menu_radius: 6.0,
            top_bar_height: 38.0,
            bottom_bar_height: 56.0,
            nav_pill_radius: 6.0,
            sidebar_width: 230.0,
            switch_height: 20.0,
            switch_width: 38.0,
            checkbox_radius: 5.0,
        }
    }

    /// Converts this macOS HIG theme into [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        DesignSpec {
            name: "Apple HIG",
            brightness: self.brightness,
            palette: hig_palette(self.accent, self.brightness),
            metrics: Self::metrics(),
            feedback: StateFeedbackMode::CrispShadowAndRing,
            input_focus: InputFocusStyle::OuterHalo3px,
            tab_indicator: TabIndicatorStyle::SegmentedThumb,
            nav_indicator: NavIndicatorStyle::RoundedRow,
            typography: self.typography.clone(),
        }
    }
}

impl Default for HigTheme {
    fn default() -> Self {
        Self::light()
    }
}
