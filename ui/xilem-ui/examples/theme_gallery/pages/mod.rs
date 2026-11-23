//! Modular showcase pages for `theme_gallery`.

pub mod components;
pub mod home;
pub mod layout;
pub mod overlays;
pub mod perf_and_spec;
pub mod platform_native;
pub mod workbench;

pub use components::components_page;
pub use home::home_page;
pub use layout::layout_page;
pub use overlays::overlays_page;
pub use perf_and_spec::perf_and_spec_page;
pub use platform_native::platform_native_page;
pub use workbench::workbench_page;
