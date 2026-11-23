//! `HigTheme` constructing an Apple Human Interface Guidelines [`DesignSpec`].

use xilem::masonry::core::FromDynWidget;
use xilem::{WidgetView, WindowId, WindowView};
use xilem_ui_core::style_engine::scope::{UiScope, ui_scope, ui_window};
use xilem_ui_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle,
    NavIndicatorStyle, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeScale,
    WindowBackdrop,
};

use crate::palette::{HigAccent, hig_palette};

/// Configuration handle generating macOS Apple HIG [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct HigTheme {
    /// Light or Dark Aqua polarity.
    pub brightness: Brightness,
    /// Apple system accent color.
    pub accent: HigAccent,
    /// Compact `13px` macOS HIG typography ramp.
    pub typography: TypeScale,
    /// Disables entrance/transition motion durations and spatial offsets when `true`.
    pub reduced_motion: bool,
}

impl HigTheme {
    /// macOS Aqua Light theme (System Blue accent).
    #[must_use]
    pub fn light() -> Self {
        Self {
            brightness: Brightness::Light,
            accent: HigAccent::Blue,
            typography: TypeScale::macos_hig(),
            reduced_motion: false,
        }
    }

    /// macOS Aqua Dark theme (System Blue accent).
    #[must_use]
    pub fn dark() -> Self {
        Self {
            brightness: Brightness::Dark,
            accent: HigAccent::Blue,
            typography: TypeScale::macos_hig(),
            reduced_motion: false,
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

    /// Sets accessibility reduced-motion preference.
    #[must_use]
    pub const fn with_reduced_motion(mut self, reduced: bool) -> Self {
        self.reduced_motion = reduced;
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
            backdrop: WindowBackdrop::MacosVibrancy,
            palette: hig_palette(self.accent, self.brightness),
            metrics: Self::metrics(),
            feedback: StateFeedbackMode::CrispShadowAndRing,
            input_focus: InputFocusStyle::OuterHalo3px,
            tab_indicator: TabIndicatorStyle::SegmentedThumb,
            nav_indicator: NavIndicatorStyle::RoundedRow,
            dialog_action_order: DialogActionOrder::PrimaryTrailing,
            shortcut_convention: ShortcutConvention::Command,
            control_border: ControlBorderStyle::CrispElevation,
            reduced_motion: self.reduced_motion,
            typography: self.typography.clone(),
        }
    }
}

impl Default for HigTheme {
    fn default() -> Self {
        Self::light()
    }
}

/// Wraps `child` inside a live macOS Apple HIG [`UiScope`] matching `theme`.
pub fn hig_scope<State: 'static, Action: 'static, V>(
    theme: &HigTheme,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    ui_scope(theme.to_spec(), child)
}

/// Wraps a standalone desktop [`WindowView`] with macOS Aqua backdrop, properties, and [`UiScope`].
pub fn hig_window<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    theme: &HigTheme,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    ui_window(window_id, title, &theme.to_spec(), child)
}
