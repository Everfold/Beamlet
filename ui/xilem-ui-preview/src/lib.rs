//! Offline headless GUI preview harness, layout/contrast linter, and snapshot regression helpers.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod harness;
pub mod inspect;
