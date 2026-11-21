//! Windows 11 / WinUI 3 Fluent Design theme backend and signature WinUI components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{InfoBarSeverity, command_bar, info_bar, settings_card, settings_expander};
pub use palette::{FluentAccent, fluent_palette};
pub use theme::FluentTheme;
pub use xilem_ui_core::components::*;
