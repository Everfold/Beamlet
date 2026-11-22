//! GNOME Libadwaita theme backend and signature GTK/Adwaita components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod clamp;
pub mod components;
pub mod palette;
pub mod theme;

pub use clamp::{AdwClamp, ClampView, clamp};
pub use components::{
    action_row, banner, clickable_action_row, combo_row, entry_row, expander_row, header_bar,
    header_bar_with_title, preferences_group, status_page, switch_row, view_switcher,
};
pub use palette::{AdwaitaAccent, adwaita_palette};
pub use theme::{AdwaitaTheme, adwaita_scope, adwaita_window};
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
