//! Convenience prelude exposing concrete cross-theme components, focus primitives, and per-platform APIs.

pub use crate::components::*;
pub use crate::theme::{DesignLanguage, UiAccent, UiTheme, ui_scope, ui_window};
pub use crate::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    FocusFrame, FocusFrameView, InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle,
    SemanticPalette, ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale,
    UiClassExt, WindowSizeClass, build_default_properties, cached_default_properties, classes,
    current_spec, focus_frame, icons, ui_icon, ui_transition, with_spec,
};

#[cfg(feature = "md3")]
pub use xilem_md3::{
    ColorScheme, ContrastLevel, FabColor, FabSize, Md3Theme, TopAppBarVariant, bottom_app_bar,
    center_aligned_top_app_bar, extended_fab, fab, floating_toolbar, large_fab, large_top_app_bar,
    md3_scope, md3_window, medium_top_app_bar, navigation_rail_with_fab, sized_fab, small_fab,
    top_app_bar, top_app_bar_slots,
};

#[cfg(feature = "fluent")]
pub use xilem_fluent::{
    FluentAccent, FluentTheme, InfoBarSeverity, breadcrumb_bar, command_bar, expander,
    fluent_scope, fluent_window, info_badge, info_bar, navigation_view, number_box, settings_card,
    settings_expander,
};

#[cfg(feature = "hig")]
pub use xilem_hig::{
    HigAccent, HigTheme, disclosure_group, form_group, form_row, help_button, hig_scope,
    hig_window, inspector_panel, macos_toolbar, macos_toolbar_with_center, path_control,
    source_list_sidebar, stepper,
};

#[cfg(feature = "adwaita")]
pub use xilem_adwaita::{
    AdwAlertDialog, AdwBreakpointBin, AdwClamp, AdwaitaAccent, AdwaitaTheme, BreakpointBin,
    ClampView, action_row, adwaita_scope, adwaita_window, alert_dialog, banner, breakpoint_bin,
    clamp, clickable_action_row, combo_row, entry_row, expander_row, header_bar,
    header_bar_with_title, preferences_group, status_page, switch_row, view_switcher,
    view_switcher_bar,
};
