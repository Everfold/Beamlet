//! Masonry `DefaultProperties` synthesizer and `Theme` environment scope.
//!
//! ## Three-Layer Styling Boundary
//!
//! Brocado intentionally rejects arbitrary CSS cascade / deep descendant combinators and keeps
//! theme synthesis split into three strictly scoped layers:
//!
//! 1. **Semantic design tokens** ([`Theme`](crate::theme::Theme)): M3 Expressive [`ColorScheme`](crate::theme::ColorScheme),
//!    type scale, geometry constants, elevation, and motion timings.
//! 2. **Shallow curated `PropertyStack` rules** ([`builder::build_default_properties`]): single
//!    semantic class selectors plus Masonry's native pseudo-states (`hovered`, `active`,
//!    `focused`, `disabled`), memoized behind [`cached_default_properties`].
//! 3. **Structural decorator widgets** (`FocusFrame`, `RippleLayer`): dedicated
//!    widgets for custom focus halos and clipped ink ripples that
//!    cannot be represented cleanly by flat box properties.

pub mod builder;
pub mod classes;
pub mod scope;

pub use builder::build_default_properties;
pub use scope::{
    MemoizeWithTheme, ThemeScope, WithTheme, cached_default_properties, current_theme,
    current_theme_arc, memoize_with_theme, theme_scope_arc, with_theme,
};
