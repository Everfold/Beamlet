//! Material Design 3 theme backend and signature Material 3 components for Xilem.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod color;
pub mod components;
pub mod theme;

pub use color::{ColorScheme, ContrastLevel, contrast_ratio, relative_luminance};
pub use components::{
    FabColor, TopAppBarVariant, bottom_app_bar, center_aligned_top_app_bar, extended_fab, fab,
    large_top_app_bar, medium_top_app_bar, top_app_bar,
};
pub use theme::Md3Theme;
pub use xilem_ui_core::components::*;
