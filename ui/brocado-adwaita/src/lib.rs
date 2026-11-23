//! GNOME Libadwaita theme backend and signature GTK/Adwaita components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod breakpoint;
pub mod clamp;
pub mod components;
pub mod palette;
pub mod theme;

pub use breakpoint::{AdwBreakpointBin, BreakpointBin, breakpoint_bin};
pub use brocado_core::components::*;
pub use brocado_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, EscapeCatcher, EscapeCatcherView, EscapePressed,
    FocusFrame, FocusFrameView, HoverTooltipHost, HoverTooltipView, InteractiveColors, PopupAction,
    PopupEntrySpec, PopupLayer, PopupLayerSpec, SlideTransition, TooltipContentSpec, UiClassExt,
    UiTransition, WithClass, blend_over, escape_catcher, focus_frame, icons,
    resolve_interactive_colors, ui_icon, ui_transition, with_opacity,
};
pub use brocado_core::style_engine::{
    MemoizeWithSpec, build_default_properties, cached_default_properties, classes, current_spec,
    memoize_with_spec, with_spec,
};
pub use brocado_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale,
    WindowBackdrop, WindowSizeClass,
};
pub use clamp::{AdwClamp, ClampView, clamp};
pub use components::{
    AdwAlertDialog, action_row, alert_dialog, banner, clickable_action_row, combo_row, entry_row,
    expander_row, header_bar, header_bar_with_title, preferences_group, status_page, switch_row,
    view_switcher, view_switcher_bar,
};
pub use palette::{AdwaitaAccent, adwaita_palette};
pub use theme::{AdwaitaTheme, adwaita_scope};
