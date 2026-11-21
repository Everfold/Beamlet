//! GNOME Libadwaita theme backend and signature GTK/Adwaita components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use components::{action_row, banner, header_bar, preferences_group, status_page, switch_row};
pub use palette::{AdwaitaAccent, adwaita_palette};
pub use theme::AdwaitaTheme;
pub use xilem_ui_core::components::*;
