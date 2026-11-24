//! Standalone offline headless preview harness, geometry/accessibility tree inspector,
//! and automated layout + WCAG contrast linter for any Brocado application.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod diff;
pub mod harness;
pub mod inspect;
