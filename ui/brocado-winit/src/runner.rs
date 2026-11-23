//! Desktop application event loop builder ([`AppRunner`]) and [`AppState`] trait.
//!
//! Upstream provenance: `AppRunner`, `AppState`, `ExitOnClose`, and `new_simple_with_tokio` are
//! adapted from upstream `linebender/xilem` (`xilem/src/app.rs`, `Xilem` / `ExitOnClose`).

use std::iter::Once;
use std::sync::Arc;

use masonry::core::DefaultProperties;
use masonry::peniko::{Blob, Color};
use masonry::theme::{BACKGROUND_COLOR, default_property_set};
use masonry_winit::app::{
    EventLoop, EventLoopBuilder, MasonryState, MasonryUserEvent, NewWindow, WgpuContext, WindowId,
    run_with,
};
use masonry_winit::winit::error::EventLoopError;
use tokio::runtime::Runtime as TokioRuntime;
use xilem_core::map_state;
use xilem_masonry::WidgetView;

use crate::config::WindowConfig;
use crate::driver::{DesktopDriver, OnStartCallback, OnWgpuReadyCallback};
use crate::window::DesktopWindow;

/// Trait implemented by multi-window application states to govern event-loop termination.
pub trait AppState {
    /// Returns `true` while the application event loop should keep running.
    fn keep_running(&self) -> bool;
}

/// Convenience wrapper used by [`AppRunner::new_simple`] that exits when its single window closes.
#[derive(Debug)]
pub struct ExitOnClose<S> {
    state: S,
    running: bool,
}

impl<S> ExitOnClose<S> {
    /// Creates an active [`ExitOnClose`] state wrapper.
    #[must_use]
    pub const fn new(state: S) -> Self {
        Self {
            state,
            running: true,
        }
    }

    /// Returns `true` if the application is still marked as running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Requests application shutdown on the next event-loop check.
    pub const fn stop(&mut self) {
        self.running = false;
    }

    /// Immutable reference to wrapped inner state.
    #[must_use]
    pub const fn inner(&self) -> &S {
        &self.state
    }
}

impl<S> AppState for ExitOnClose<S> {
    fn keep_running(&self) -> bool {
        self.running
    }
}

/// Top-level launcher driving [`DesktopWindow`] trees through `masonry_winit`.
#[must_use = "An AppRunner does nothing unless `.run()` or `.run_in(...)` is called."]
#[expect(
    missing_debug_implementations,
    reason = "Holds reactive logic closures, Tokio runtime handle, and lifecycle callbacks"
)]
pub struct AppRunner<State, Logic> {
    state: State,
    logic: Logic,
    runtime: Arc<TokioRuntime>,
    default_properties: Option<DefaultProperties>,
    default_base_color: Color,
    fonts: Vec<Blob<u8>>,
    on_start: Option<OnStartCallback>,
    on_wgpu_ready: Option<OnWgpuReadyCallback>,
}

type SimpleWindowFn<State> =
    Box<dyn FnMut(&mut ExitOnClose<State>) -> Once<DesktopWindow<ExitOnClose<State>>>>;

impl<State: 'static> AppRunner<ExitOnClose<State>, SimpleWindowFn<State>> {
    /// Builds a single-window launcher exiting automatically when the window is closed.
    pub fn new_simple<V>(
        state: State,
        logic: impl FnMut(&mut State) -> V + 'static,
        config: WindowConfig<State>,
    ) -> Self
    where
        V: WidgetView<State>,
    {
        Self::new_simple_with_tokio(
            state,
            logic,
            config,
            Arc::new(TokioRuntime::new().expect("failed to initialize Tokio runtime")),
        )
    }

    /// Variant of [`Self::new_simple`] reusing an existing Tokio runtime.
    pub fn new_simple_with_tokio<V>(
        state: State,
        mut logic: impl FnMut(&mut State) -> V + 'static,
        config: WindowConfig<State>,
        runtime: Arc<TokioRuntime>,
    ) -> Self
    where
        V: WidgetView<State>,
    {
        let window_id = WindowId::next();
        let on_close_cb = config.on_close.clone();
        let base_config = WindowConfig::<ExitOnClose<State>> {
            title: config.title,
            resizable: config.resizable,
            enabled_buttons: config.enabled_buttons,
            cursor: config.cursor,
            min_inner_size: config.min_inner_size,
            max_inner_size: config.max_inner_size,
            window_level: config.window_level,
            chrome: config.chrome,
            theme_override: config.theme_override,
            inner_size: config.inner_size,
            position: config.position,
            transparent: config.transparent,
            window_icon: config.window_icon,
            raw_attributes: config.raw_attributes,
            on_close: Some(Arc::new(move |wrapper: &mut ExitOnClose<State>| {
                wrapper.running = false;
                if let Some(cb) = &on_close_cb {
                    cb(&mut wrapper.state);
                }
            })),
        };
        AppRunner::new_with_tokio(
            ExitOnClose {
                state,
                running: true,
            },
            Box::new(move |wrapper: &mut ExitOnClose<State>| {
                let view = map_state(logic(&mut wrapper.state), |w: &mut ExitOnClose<State>| {
                    &mut w.state
                });
                let cfg = base_config.clone();
                std::iter::once(DesktopWindow::new(window_id, "", view).with_config(|_| cfg))
            }),
            runtime,
        )
    }
}

impl<State, Logic, WindowIter> AppRunner<State, Logic>
where
    State: AppState + 'static,
    Logic: FnMut(&mut State) -> WindowIter + 'static,
    WindowIter: Iterator<Item = DesktopWindow<State>>,
{
    /// Builds a multi-window desktop runner backed by a fresh Tokio runtime.
    pub fn new(state: State, logic: Logic) -> Self {
        Self::new_with_tokio(
            state,
            logic,
            Arc::new(TokioRuntime::new().expect("failed to initialize Tokio runtime")),
        )
    }

    /// Builds a multi-window desktop runner sharing an caller-managed Tokio runtime.
    pub fn new_with_tokio(state: State, logic: Logic, runtime: Arc<TokioRuntime>) -> Self {
        Self {
            state,
            logic,
            runtime,
            default_properties: None,
            default_base_color: BACKGROUND_COLOR,
            fonts: Vec::new(),
            on_start: None,
            on_wgpu_ready: None,
        }
    }

    /// Registers an embedded font blob loaded for all windows on startup.
    pub fn with_font(mut self, data: impl Into<Blob<u8>>) -> Self {
        self.fonts.push(data.into());
        self
    }

    /// Assigns initial widget tree default properties (`UiTheme::default_properties()`).
    pub fn with_default_properties(mut self, props: DefaultProperties) -> Self {
        self.default_properties = Some(props);
        self
    }

    /// Sets the fallback window clear color for windows without an explicit base color.
    pub const fn with_default_base_color(mut self, color: Color) -> Self {
        self.default_base_color = color;
        self
    }

    /// Registers a hook executed once windows are initialized on the event loop.
    pub fn with_on_start(mut self, callback: impl FnOnce(&mut MasonryState) + 'static) -> Self {
        self.on_start = Some(Box::new(callback));
        self
    }

    /// Registers a hook executed once `wgpu` device & queue initialization finishes.
    pub fn with_on_wgpu_ready(mut self, callback: impl FnMut(&WgpuContext<'_>) + 'static) -> Self {
        self.on_wgpu_ready = Some(Box::new(callback));
        self
    }

    /// Runs the application on a default `EventLoop::with_user_event()`.
    pub fn run(self) -> Result<(), EventLoopError> {
        self.run_in(EventLoop::with_user_event())
    }

    /// Runs the application inside a customized [`EventLoopBuilder`].
    pub fn run_in(mut self, mut event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
        let event_loop = event_loop.build()?;
        let proxy = event_loop.create_proxy();
        let default_props = self
            .default_properties
            .take()
            .unwrap_or_else(default_property_set);
        let (driver, windows) =
            self.into_driver_and_windows(move |event| proxy.send_event(event).map_err(|err| err.0));
        run_with(event_loop, windows, driver, default_props)
    }

    /// Consumes the runner into its raw [`DesktopDriver`] and initial [`NewWindow`] list for embedding.
    pub fn into_driver_and_windows(
        self,
        proxy: impl Fn(MasonryUserEvent) -> Result<(), MasonryUserEvent> + Send + Sync + 'static,
    ) -> (DesktopDriver<State, Logic>, Vec<NewWindow>) {
        DesktopDriver::new(
            self.state,
            self.logic,
            proxy,
            self.runtime,
            self.default_base_color,
            self.fonts,
            self.on_start,
            self.on_wgpu_ready,
        )
    }
}
