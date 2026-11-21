//! macOS Apple Human Interface Guidelines (Aqua Light & Dark) theme backend and native components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{form_group, form_row, help_button, macos_toolbar, source_list_sidebar};
pub use palette::{HigAccent, hig_palette};
pub use theme::HigTheme;
pub use xilem_ui_core::components::*;
