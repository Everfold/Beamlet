//! Unified multi-theme UI facade supporting Material Design 3, Windows Fluent, macOS HIG, and GNOME Adwaita.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod prelude;
pub mod theme;

pub use theme::{DesignLanguage, UiAccent, UiTheme, ui_scope, ui_window};
pub use xilem_ui_core as core;
pub use xilem_ui_core::components;
pub use xilem_ui_core::foundation::{UiClassExt, icons, ui_icon, ui_transition};
pub use xilem_ui_core::style_engine::{
    build_default_properties, cached_default_properties, classes, current_spec, with_spec,
};
pub use xilem_ui_core::tokens::{
    Brightness, ControlMetrics, DesignSpec, ElevationLevel, InputFocusStyle, MotionDuration,
    MotionEasing, NavIndicatorStyle, SemanticPalette, StateFeedbackMode, TabIndicatorStyle,
    TypeRole, TypeScale, WindowSizeClass,
};

#[cfg(feature = "md3")]
pub use xilem_md3 as md3;

#[cfg(feature = "fluent")]
pub use xilem_fluent as fluent;

#[cfg(feature = "hig")]
pub use xilem_hig as hig;

#[cfg(feature = "adwaita")]
pub use xilem_adwaita as adwaita;
