//! Convenience prelude exposing concrete cross-theme components, focus primitives, and per-platform APIs.

pub use crate::components::*;
pub use crate::theme::{DesignLanguage, UiAccent, UiTheme, ui_scope, ui_window};
pub use crate::{
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel, FocusFrame,
    FocusFrameView, InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle,
    SemanticPalette, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale,
    UiClassExt, WindowSizeClass, build_default_properties, cached_default_properties, classes,
    current_spec, focus_frame, icons, ui_icon, ui_transition, with_spec,
};

#[cfg(feature = "md3")]
pub use xilem_md3::{
    ColorScheme, ContrastLevel, FabColor, Md3Theme, TopAppBarVariant, bottom_app_bar,
    center_aligned_top_app_bar, extended_fab, fab, large_top_app_bar, md3_scope, md3_window,
    medium_top_app_bar, top_app_bar, top_app_bar_slots,
};

#[cfg(feature = "fluent")]
pub use xilem_fluent::{
    FluentAccent, FluentTheme, InfoBarSeverity, command_bar, fluent_scope, fluent_window, info_bar,
    navigation_view, settings_card, settings_expander,
};

#[cfg(feature = "hig")]
pub use xilem_hig::{
    HigAccent, HigTheme, form_group, form_row, help_button, hig_scope, hig_window, macos_toolbar,
    macos_toolbar_with_center, source_list_sidebar,
};

#[cfg(feature = "adwaita")]
pub use xilem_adwaita::{
    AdwaitaAccent, AdwaitaTheme, action_row, adwaita_scope, adwaita_window, banner,
    clickable_action_row, header_bar, header_bar_with_title, preferences_group, status_page,
    switch_row,
};
