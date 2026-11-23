//! Self-owned desktop Winit + `masonry_winit` host runtime for Beamlet UI.
//!
//! Replaces the monolithic top-level `xilem` runner shell with direct control over
//! [`WindowChrome`], compositor [`WindowBackdrop`](xilem_ui_core::tokens::WindowBackdrop) hints,
//! raw Winit attributes, and reactive [`DesktopWindow`] lifecycles.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod config;
pub mod driver;
pub mod runner;
pub mod window;

pub use config::{WindowChrome, WindowConfig, brightness_to_winit_theme};
pub use driver::{DesktopDriver, OnStartCallback, OnWgpuReadyCallback, async_action};
pub use masonry_winit::app::{
    EventLoop, EventLoopBuilder, EventLoopProxy, MasonryState, MasonryUserEvent, NewWindow,
    WgpuContext, WgpuLimits, WindowId,
};
pub use masonry_winit::winit;
pub use runner::{AppRunner, AppState, ExitOnClose};
pub use window::{
    DesktopWindow, PodWindow, desktop_window, desktop_window_arc, desktop_window_arc_with_backdrop,
    resolve_window_backdrop_fill,
};

#[cfg(test)]
mod tests {
    use masonry::dpi::LogicalSize;
    use masonry_winit::winit::window::Theme as WinitTheme;
    use xilem_masonry::view::label;
    use xilem_ui_core::tokens::{Brightness, DesignSpec, WindowBackdrop};

    use super::*;

    #[test]
    fn window_chrome_maps_decorations_flag() {
        assert!(WindowChrome::NativeTitleBar.uses_decorations());
        assert!(WindowChrome::UnifiedTitleBar.uses_decorations());
        assert!(!WindowChrome::Borderless.uses_decorations());
    }

    #[test]
    fn backdrop_drives_initial_transparency_and_can_be_overridden() {
        let cfg: WindowConfig<()> = WindowConfig::new("Demo")
            .with_chrome(WindowChrome::Borderless)
            .with_initial_inner_size(LogicalSize::new(960.0, 640.0));

        let opaque_attrs =
            cfg.build_initial_attributes(WindowBackdrop::Opaque, Some(Brightness::Light));
        assert!(!opaque_attrs.transparent);
        assert!(!opaque_attrs.decorations);
        assert_eq!(opaque_attrs.title, "Demo");
        assert_eq!(opaque_attrs.preferred_theme, Some(WinitTheme::Light));

        let mica_attrs = cfg.build_initial_attributes(WindowBackdrop::Mica, Some(Brightness::Dark));
        assert!(mica_attrs.transparent);
        assert_eq!(mica_attrs.preferred_theme, Some(WinitTheme::Dark));

        let forced_opaque = WindowConfig::<()>::new("Override")
            .with_transparent(false)
            .build_initial_attributes(WindowBackdrop::Acrylic, Some(Brightness::Light));
        assert!(!forced_opaque.transparent);
    }

    #[test]
    fn raw_attributes_hook_runs_at_initial_build() {
        let cfg: WindowConfig<()> = WindowConfig::new("Custom")
            .with_raw_attributes(|attrs| attrs.with_title("HookedTitle").with_resizable(false));
        let attrs = cfg.build_initial_attributes(WindowBackdrop::Opaque, None);
        assert_eq!(attrs.title, "HookedTitle");
        assert!(!attrs.resizable);
    }

    #[test]
    fn desktop_window_attaches_spec_default_properties_and_backdrop() {
        struct State {
            running: bool,
        }
        impl AppState for State {
            fn keep_running(&self) -> bool {
                self.running
            }
        }

        let spec = DesignSpec::neutral_fallback(Brightness::Dark);
        let mut win = desktop_window::<State, _>(WindowId::next(), "Host", &spec, label("Body"))
            .with_chrome(WindowChrome::UnifiedTitleBar)
            .with_backdrop(WindowBackdrop::MacosVibrancy);
        assert_eq!(win.backdrop(), WindowBackdrop::MacosVibrancy);
        assert_eq!(win.config().chrome, WindowChrome::UnifiedTitleBar);
        assert!(win.default_properties.is_some());

        let mut exit_state = ExitOnClose::new(());
        assert!(exit_state.keep_running());
        exit_state.stop();
        assert!(!exit_state.keep_running());
        win = win.on_close(|s| s.running = false);
        let mut s = State { running: true };
        win.on_close_requested(&mut s);
        assert!(!s.keep_running());
    }
}
