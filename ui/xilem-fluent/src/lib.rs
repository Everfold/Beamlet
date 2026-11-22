//! Windows 11 / WinUI 3 Fluent Design theme backend and signature WinUI components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{
    InfoBarSeverity, breadcrumb_bar, command_bar, expander, info_badge, info_bar, navigation_view,
    number_box, settings_card, settings_expander,
};
pub use palette::{FluentAccent, fluent_palette};
pub use theme::{FluentTheme, fluent_scope, fluent_window};
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
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel, InputFocusStyle,
    MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette, ShortcutConvention,
    StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale, WindowSizeClass,
};
