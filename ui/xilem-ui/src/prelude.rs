//! Convenience prelude for building multi-theme Xilem applications cleanly.

pub use crate::components::*;
pub use crate::theme::{DesignLanguage, UiAccent, UiTheme, ui_scope, ui_window};
pub use crate::{
    Brightness, ControlMetrics, DesignSpec, ElevationLevel, InputFocusStyle, MotionDuration,
    MotionEasing, NavIndicatorStyle, SemanticPalette, StateFeedbackMode, TabIndicatorStyle,
    TypeRole, TypeScale, UiClassExt, WindowSizeClass, build_default_properties,
    cached_default_properties, classes, current_spec, icons, ui_icon, ui_transition, with_spec,
};

#[cfg(feature = "md3")]
pub use xilem_md3::{
    ColorScheme, ContrastLevel, FabColor, Md3Theme, TopAppBarVariant, bottom_app_bar,
    center_aligned_top_app_bar, extended_fab, fab, large_top_app_bar, medium_top_app_bar,
};

#[cfg(feature = "fluent")]
pub use xilem_fluent::{
    FluentAccent, FluentTheme, InfoBarSeverity, command_bar, info_bar, settings_card,
    settings_expander,
};

#[cfg(feature = "hig")]
pub use xilem_hig::{
    HigAccent, HigTheme, form_group, form_row, help_button, macos_toolbar, source_list_sidebar,
};

#[cfg(feature = "adwaita")]
pub use xilem_adwaita::{
    AdwaitaAccent, AdwaitaTheme, action_row, banner, header_bar, preferences_group, status_page,
    switch_row,
};
