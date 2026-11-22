//! macOS Apple Human Interface Guidelines (Aqua Light & Dark) theme backend and native components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{
    MacosAlertDialog, disclosure_group, form_group, form_row, help_button, inspector_panel,
    macos_alert_dialog, macos_toolbar, macos_toolbar_with_center, path_control, popup_button,
    source_list_sidebar, stepper,
};
pub use palette::{HigAccent, hig_palette};
pub use theme::{HigTheme, hig_scope, hig_window};
pub use xilem_ui_core::components::*;
pub use xilem_ui_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, EscapeCatcher, EscapeCatcherView, EscapePressed,
    FocusFrame, FocusFrameView, HoverTooltipHost, HoverTooltipView, InteractiveColors, PopupAction,
    PopupEntrySpec, PopupLayer, PopupLayerSpec, SlideTransition, TooltipContentSpec, UiClassExt,
    UiTransition, WithClass, blend_over, escape_catcher, focus_frame, icons,
    resolve_interactive_colors, ui_icon, ui_transition, with_opacity,
};
pub use xilem_ui_core::style_engine::{
    build_default_properties, cached_default_properties, classes, current_spec, with_spec,
};
pub use xilem_ui_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale,
    WindowSizeClass,
};
