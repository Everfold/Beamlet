//! Windows 11 / WinUI 3 Fluent Design theme backend and signature WinUI components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{
    InfoBarSeverity, breadcrumb_bar, command_bar, command_bar_with_overflow, expander, info_badge,
    info_bar, navigation_view, navigation_view_collapsible, number_box, settings_card,
    settings_expander,
};
pub use palette::{FluentAccent, fluent_palette};
pub use theme::{FluentTheme, fluent_scope, fluent_window};
pub use xilem_ui_core::components::*;
pub use xilem_ui_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, BeveledBorder, BeveledBorderView, EscapeCatcher,
    EscapeCatcherView, EscapePressed, FocusFrame, FocusFrameView, HoverTooltipHost,
    HoverTooltipView, InteractiveColors, PopupAction, PopupEntrySpec, PopupLayer, PopupLayerSpec,
    SlideTransition, TooltipContentSpec, UiClassExt, UiTransition, WithClass, beveled_surface,
    blend_over, escape_catcher, focus_frame, icons, resolve_interactive_colors, ui_icon,
    ui_transition, with_opacity,
};
pub use xilem_ui_core::style_engine::{
    MemoizeWithSpec, build_default_properties, cached_default_properties, classes, current_spec,
    memoize_with_spec, with_spec,
};
pub use xilem_ui_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale,
    WindowSizeClass,
};
