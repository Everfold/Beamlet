//! `FluentTheme` constructing a Windows 11 / WinUI 3 [`DesignSpec`].

use brocado_core::style_engine::scope::{UiScope, ui_scope};
use brocado_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, InputFocusStyle,
    MotionScheme, NavIndicatorStyle, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle,
    TypeScale, WindowBackdrop,
};
use masonry::core::FromDynWidget;
use xilem_masonry::WidgetView;

use crate::palette::{FluentAccent, fluent_palette};

/// Configuration handle generating Windows 11 / WinUI 3 [`DesignSpec`] instances.
#[derive(Clone, Debug, PartialEq)]
pub struct FluentTheme {
    /// Light or Dark Mica polarity.
    pub brightness: Brightness,
    /// Accent color preset.
    pub accent: FluentAccent,
    /// Fluent 2 desktop type ramp.
    pub typography: TypeScale,
    /// Disables entrance/transition motion durations and spatial offsets when `true`.
    pub reduced_motion: bool,
}

impl FluentTheme {
    /// Windows 11 Light Mica theme with default Windows Blue accent.
    #[must_use]
    pub fn light() -> Self {
        Self {
            brightness: Brightness::Light,
            accent: FluentAccent::WindowsBlue,
            typography: TypeScale::fluent2(),
            reduced_motion: false,
        }
    }

    /// Windows 11 Dark Mica theme with default Windows Blue accent.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            brightness: Brightness::Dark,
            accent: FluentAccent::WindowsBlue,
            typography: TypeScale::fluent2(),
            reduced_motion: false,
        }
    }

    /// Overrides the Windows accent color.
    #[must_use]
    pub fn with_accent(mut self, accent: FluentAccent) -> Self {
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

    /// WinUI 3 control geometry (`32px` buttons, `4px` corner radius, `8px` cards/dialogs).
    #[must_use]
    pub const fn metrics() -> ControlMetrics {
        ControlMetrics {
            button_height: 32.0,
            button_radius: 4.0,
            button_pad_h: 12.0,
            icon_btn_size: 32.0,
            icon_btn_radius: 4.0,
            chip_height: 28.0,
            chip_radius: 4.0,
            input_radius: 4.0,
            input_pad_v: 6.0,
            input_pad_h: 10.0,
            search_height: 34.0,
            search_radius: 4.0,
            card_radius: 8.0,
            card_pad: 14.0,
            dialog_radius: 8.0,
            sheet_radius: 8.0,
            menu_radius: 6.0,
            top_bar_height: 48.0,
            bottom_bar_height: 56.0,
            nav_pill_radius: 4.0,
            sidebar_width: 280.0,
            switch_height: 20.0,
            switch_width: 40.0,
            checkbox_radius: 4.0,
        }
    }

    /// Converts this Fluent theme into [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        DesignSpec {
            name: "Windows Fluent",
            brightness: self.brightness,
            backdrop: WindowBackdrop::Mica,
            palette: fluent_palette(self.accent, self.brightness),
            metrics: Self::metrics(),
            feedback: StateFeedbackMode::SubtleSurfaceAndBorder,
            input_focus: InputFocusStyle::BottomAccentLine2px,
            tab_indicator: TabIndicatorStyle::ShortPill,
            nav_indicator: NavIndicatorStyle::LeadingAccentBar,
            dialog_action_order: DialogActionOrder::PrimaryLeading,
            shortcut_convention: ShortcutConvention::Control,
            control_border: ControlBorderStyle::BeveledElevation,
            motion_scheme: MotionScheme::Standard,
            active_button_radius: None,
            reduced_motion: self.reduced_motion,
            typography: self.typography.clone(),
            locale: Default::default(),
        }
    }
}

impl Default for FluentTheme {
    fn default() -> Self {
        Self::light()
    }
}

/// Wraps `child` inside a live Windows Fluent [`UiScope`] matching `theme`.
pub fn fluent_scope<State: 'static, Action: 'static, V>(
    theme: &FluentTheme,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    ui_scope(theme.to_spec(), child)
}
