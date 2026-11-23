//! Masonry `DefaultProperties` synthesizer and `DesignSpec` environment scope.

pub mod builder;
pub mod classes;
pub mod scope;

pub use builder::build_default_properties;
pub use scope::{
    MemoizeWithSpec, WithSpec, cached_default_properties, current_spec, current_spec_arc,
    memoize_with_spec, ui_scope, ui_scope_arc, ui_window, ui_window_arc, with_spec,
};
