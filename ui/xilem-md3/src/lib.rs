//! Material Design 3 theme backend and signature Material 3 components for Xilem.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod color;
pub mod components;
pub mod theme;

pub use color::{ColorScheme, ContrastLevel, contrast_ratio, relative_luminance};
pub use components::{
    FabColor, TopAppBarVariant, bottom_app_bar, center_aligned_top_app_bar, extended_fab, fab,
    large_top_app_bar, medium_top_app_bar, top_app_bar, top_app_bar_slots,
};
pub use theme::{Md3Theme, md3_scope, md3_window};
pub use xilem_ui_core::components::*;
pub use xilem_ui_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, EscapeCatcher, EscapeCatcherView, EscapePressed,
    FocusFrame, FocusFrameView, HoverTooltipHost, HoverTooltipView, InteractiveColors, PopupAction,
    PopupEntrySpec, PopupLayer, PopupLayerSpec, SlideTransition, TooltipContentSpec, UiClassExt,
    UiTransition, WithClass, blend_over, escape_catcher, focus_frame, icons,
    resolve_interactive_colors, ui_icon, ui_transition, with_opacity,
};
pub use xilem_ui_core::style_engine::{
    cached_default_properties, classes, current_spec, with_spec,
};
pub use xilem_ui_core::tokens::{
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel, InputFocusStyle,
    MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette, ShortcutConvention,
    StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale, WindowSizeClass,
};
