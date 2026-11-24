//! Convenience prelude exposing concrete cross-theme components, focus primitives, and per-platform APIs.

pub use crate::components::*;
pub use crate::theme::{DesignLanguage, UiAccent, UiTheme, ui_scope};
#[cfg(feature = "winit")]
pub use crate::theme::{themed_window, themed_window_with_backdrop};
pub use crate::{
    AdaptivePairView, AdaptivePairWidget, AnyWidgetView, BeveledBorder, BeveledBorderView,
    Brightness, ClipBoxView, ClipBoxWidget, Color, ContextMenuHost, ContextMenuView,
    ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    EscapeCatcher, FocusFrame, FocusFrameView, FontWeight, HoverTooltipHost, InputFocusStyle,
    InsertNewline, MotionDuration, MotionEasing, NavIndicatorStyle, Pod, PopoverAnchorRect,
    PopoverAnchorView, PopoverAnchorWidget, RippleLayer, RippleLayerView, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TextAlign, TypeRole, TypeScale,
    UiClassExt, ViewCtx, WidgetView, WidgetViewSequence, WindowBackdrop, WindowSizeClass,
    WrapState, WrapView, WrapWidget, adaptive_pair, beveled_surface, build_default_properties,
    cached_default_properties, classes, clip_box, current_spec, focus_frame, icons,
    memoize_with_spec, popover_anchor, popover_place, ripple_layer, ui_icon, ui_transition,
    with_spec, wrap,
};

#[cfg(feature = "winit")]
pub use crate::{
    AppRunner, AppState, DesktopWindow, EventLoop, EventLoopBuilder, ExitOnClose, WindowChrome,
    WindowConfig, WindowId, desktop_window, desktop_window_arc,
};

#[cfg(feature = "md3")]
pub use brocado_md3::{
    ColorScheme, ContrastLevel, FabColor, FabSize, Md3Theme, TopAppBarVariant, bottom_app_bar,
    center_aligned_top_app_bar, extended_fab, fab, floating_toolbar, large_fab, large_top_app_bar,
    md3_scope, medium_top_app_bar, navigation_rail_with_fab, sized_fab, small_fab, top_app_bar,
    top_app_bar_slots,
};

#[cfg(feature = "fluent")]
pub use brocado_fluent::{
    FluentAccent, FluentTheme, InfoBarSeverity, breadcrumb_bar, command_bar,
    command_bar_with_overflow, expander, fluent_scope, info_badge, info_bar, navigation_view,
    navigation_view_collapsible, number_box, settings_card, settings_expander,
};

#[cfg(feature = "hig")]
pub use brocado_hig::{
    HigAccent, HigTheme, MacosAlertDialog, disclosure_group, form_group, form_row, help_button,
    hig_scope, inspector_panel, macos_alert_dialog, macos_toolbar, macos_toolbar_with_center,
    path_control, popup_button, source_list_sidebar, stepper,
};

#[cfg(feature = "adwaita")]
pub use brocado_adwaita::{
    AdwAlertDialog, AdwBreakpointBin, AdwClamp, AdwaitaAccent, AdwaitaTheme, BreakpointBin,
    ClampView, action_row, adwaita_scope, alert_dialog, banner, breakpoint_bin, clamp,
    clickable_action_row, combo_row, entry_row, expander_row, header_bar, header_bar_with_title,
    preferences_group, status_page, switch_row, view_switcher, view_switcher_bar,
};
