//! Masonry `DefaultProperties` synthesizer and `DesignSpec` environment scope.
//!
//! ## Three-Layer Styling Boundary
//!
//! Brocado intentionally rejects arbitrary CSS cascade / deep descendant combinators and keeps
//! theme synthesis split into three strictly scoped layers:
//!
//! 1. **Semantic design tokens** ([`DesignSpec`](crate::tokens::DesignSpec)): palette, type scale,
//!    control metrics, elevation, and motion timings.
//! 2. **Shallow curated `PropertyStack` rules** ([`builder::build_default_properties`]): single
//!    semantic class selectors plus Masonry's native pseudo-states (`hovered`, `active`,
//!    `focused`, `disabled`), memoized behind [`cached_default_properties`].
//! 3. **Structural decorator widgets** (`FocusFrame`, `BeveledBorder`, `RippleLayer`): dedicated
//!    widgets for custom multi-pass focus halos, etched bevels, and clipped ink ripples that
//!    cannot be represented cleanly by flat box properties.

pub mod builder;
pub mod classes;
pub mod scope;

pub use builder::build_default_properties;
pub use scope::{
    MemoizeWithSpec, WithSpec, cached_default_properties, current_spec, current_spec_arc,
    memoize_with_spec, ui_scope, ui_scope_arc, with_spec,
};
