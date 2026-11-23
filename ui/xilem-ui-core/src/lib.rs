//! Headless foundation, `DesignSpec` styling engine, and overlay primitives for multi-theme Xilem UIs.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod foundation;
pub mod style_engine;
pub mod tokens;

pub use foundation::{
    BeveledBorder, BeveledBorderView, EscapeCatcher, FocusFrame, FocusFrameView, HoverTooltipHost,
    RippleLayer, RippleLayerView, UiClassExt, WrapState, WrapView, WrapWidget, beveled_surface,
    focus_frame, icons, ripple_layer, ui_icon, ui_transition, wrap,
};
pub use style_engine::{
    MemoizeWithSpec, build_default_properties, current_spec, memoize_with_spec, ui_scope,
    ui_window, ui_window_with_backdrop, with_spec,
};
pub use tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale, WindowBackdrop,
    WindowSizeClass,
};
