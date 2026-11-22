//! Headless foundation, `DesignSpec` styling engine, and overlay primitives for multi-theme Xilem UIs.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod foundation;
pub mod style_engine;
pub mod tokens;

pub use foundation::{
    EscapeCatcher, FocusFrame, FocusFrameView, HoverTooltipHost, UiClassExt, focus_frame, icons,
    ui_icon, ui_transition,
};
pub use style_engine::{build_default_properties, current_spec, ui_scope, ui_window, with_spec};
pub use tokens::{
    Brightness, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel, InputFocusStyle,
    MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette, ShortcutConvention,
    StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale, WindowSizeClass,
};
