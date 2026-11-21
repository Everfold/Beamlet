//! Masonry `DefaultProperties` synthesizer and `DesignSpec` environment scope.

pub mod builder;
pub mod classes;
pub mod scope;

pub use builder::build_default_properties;
pub use scope::{
    WithSpec, cached_default_properties, current_spec, ui_scope, ui_window, with_spec,
};
