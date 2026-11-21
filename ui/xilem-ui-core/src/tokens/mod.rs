//! Design system token protocols: motion, layout, typography, and `DesignSpec`.

pub mod layout;
pub mod motion;
pub mod spec;
pub mod typography;

pub use layout::{
    Brightness, SPACE_2, SPACE_4, SPACE_6, SPACE_8, SPACE_12, SPACE_16, SPACE_20, SPACE_24,
    SPACE_32, WindowSizeClass,
};
pub use motion::{MotionDuration, MotionEasing};
pub use spec::{
    ControlMetrics, DesignSpec, ElevationLevel, InputFocusStyle, NavIndicatorStyle,
    SemanticPalette, StateFeedbackMode, TabIndicatorStyle,
};
pub use typography::{TextStyleSpec, TypeRole, TypeScale};
