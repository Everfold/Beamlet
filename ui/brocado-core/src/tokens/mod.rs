//! Design system token protocols: locale, motion, layout, typography, and `DesignSpec`.

pub mod layout;
pub mod locale;
pub mod motion;
pub mod spec;
pub mod typography;

pub use layout::{
    Brightness, SPACE_2, SPACE_4, SPACE_6, SPACE_8, SPACE_12, SPACE_16, SPACE_20, SPACE_24,
    SPACE_32, WindowSizeClass,
};
pub use locale::UiLocale;
pub use motion::{MotionDuration, MotionEasing};
pub use spec::{
    ControlBorderStyle, ControlMetrics, ControlVisualGrammar, DesignSpec, DialogActionOrder,
    ElevationLevel, InputFocusStyle, NavIndicatorStyle, SemanticPalette, ShortcutConvention,
    StateFeedbackMode, TabIndicatorStyle, WindowBackdrop,
};
pub use typography::{TextStyleSpec, TypeRole, TypeScale};
