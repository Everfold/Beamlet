//! Custom [`AppDriver`] connecting [`DesktopWindow`]s with [`masonry_winit`].
//!
//! Upstream provenance: `DesktopDriver`, `WindowProxy`, `async_action`, and `AppDriver` dispatch/lifecycle
//! are adapted from upstream `linebender/xilem` (`xilem/src/driver.rs`, `MasonryDriver`).

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;

use masonry::core::{ErasedAction, WidgetId};
use masonry::peniko::{Blob, Color};
use masonry_winit::app::{
    AppDriver, DriverCtx, MasonryState, MasonryUserEvent, NewWindow, WgpuContext, WindowId,
};
use tokio::runtime::Runtime as TokioRuntime;
use xilem_core::{
    DynMessage, MessageCtx, MessageResult, ProxyError, RawProxy, SendMessage, View, ViewId,
    ViewPathTracker,
};
use xilem_masonry::ViewCtx;

use super::runner::AppState;
use super::window::{DesktopWindow, DesktopWindowViewState};

type MessagePackage = (Arc<[ViewId]>, SendMessage);

/// Wraps an async task payload for submission into `masonry_winit`'s event loop.
#[must_use]
pub fn async_action(path: Arc<[ViewId]>, message: SendMessage) -> ErasedAction {
    Box::<MessagePackage>::new((path, message))
}

struct EventSink(Box<dyn Fn(MasonryUserEvent) -> Result<(), MasonryUserEvent> + Send + Sync>);

impl EventSink {
    fn send_message(
        &self,
        window_id: WindowId,
        path: Arc<[ViewId]>,
        message: SendMessage,
    ) -> Result<(), ProxyError> {
        let event = MasonryUserEvent::AsyncAction(window_id, async_action(path, message));
        match (self.0)(event) {
            Ok(()) => Ok(()),
            Err(err) => {
                let MasonryUserEvent::AsyncAction(_, payload) = err else {
                    unreachable!("unexpected user event variant returned from sink");
                };
                Err(ProxyError::DriverFinished(
                    payload.downcast::<MessagePackage>().unwrap().1,
                ))
            }
        }
    }
}

impl Debug for EventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EventSink").finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct WindowProxy {
    window_id: WindowId,
    sink: Arc<EventSink>,
}

impl RawProxy for WindowProxy {
    fn send_message(&self, path: Arc<[ViewId]>, message: SendMessage) -> Result<(), ProxyError> {
        self.sink.send_message(self.window_id, path, message)
    }

    fn dyn_debug(&self) -> &dyn Debug {
        self
    }
}

struct ManagedWindow<State: 'static> {
    view: DesktopWindow<State>,
    view_ctx: ViewCtx,
    view_state: DesktopWindowViewState,
}

/// Callback invoked once the Masonry event loop starts and initial windows exist.
pub type OnStartCallback = Box<dyn FnOnce(&mut MasonryState)>;
/// Callback invoked once `wgpu` device and queue initialization finishes.
pub type OnWgpuReadyCallback = Box<dyn FnMut(&WgpuContext<'_>)>;

/// Self-owned [`AppDriver`] driving reactive [`DesktopWindow`] trees without depending on `xilem`.
#[expect(
    missing_debug_implementations,
    reason = "Holds reactive application state, Tokio runtime handle, and lifecycle callbacks"
)]
pub struct DesktopDriver<State: 'static, Logic> {
    state: State,
    logic: Logic,
    windows: HashMap<WindowId, ManagedWindow<State>>,
    sink: Arc<EventSink>,
    runtime: Arc<TokioRuntime>,
    default_base_color: Color,
    fonts: Vec<Blob<u8>>,
    on_start: Option<OnStartCallback>,
    on_wgpu_ready: Option<OnWgpuReadyCallback>,
}

impl<State, Logic, WindowIter> DesktopDriver<State, Logic>
where
    State: 'static,
    Logic: FnMut(&mut State) -> WindowIter,
    WindowIter: Iterator<Item = DesktopWindow<State>>,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "Internal constructor wiring runtime, fonts, and lifecycle hooks from AppRunner"
    )]
    pub(crate) fn new(
        state: State,
        logic: Logic,
        event_sink: impl Fn(MasonryUserEvent) -> Result<(), MasonryUserEvent> + Send + Sync + 'static,
        runtime: Arc<TokioRuntime>,
        default_base_color: Color,
        fonts: Vec<Blob<u8>>,
        on_start: Option<OnStartCallback>,
        on_wgpu_ready: Option<OnWgpuReadyCallback>,
    ) -> (Self, Vec<NewWindow>) {
        let mut driver = Self {
            state,
            logic,
            windows: HashMap::new(),
            sink: Arc::new(EventSink(Box::new(event_sink))),
            runtime,
            default_base_color,
            fonts,
            on_start,
            on_wgpu_ready,
        };
        let initial: Vec<_> = (driver.logic)(&mut driver.state)
            .map(|view| driver.build_window(view))
            .collect();
        (driver, initial)
    }

    fn build_window(&mut self, mut window_view: DesktopWindow<State>) -> NewWindow {
        window_view
            .base_color
            .get_or_insert(self.default_base_color);

        let mut view_ctx = ViewCtx::new(
            Arc::new(WindowProxy {
                window_id: window_view.id,
                sink: self.sink.clone(),
            }),
            self.runtime.clone(),
        );
        let (new_window, view_state) = window_view.build(&mut view_ctx, &mut self.state);
        self.windows.insert(
            window_view.id,
            ManagedWindow {
                view: window_view,
                view_ctx,
                view_state,
            },
        );
        new_window.0
    }

    fn create_window(&mut self, ctx: &mut DriverCtx<'_>, view: DesktopWindow<State>) {
        ctx.create_window(self.build_window(view));
    }

    fn close_window(&mut self, window_id: WindowId, ctx: &mut DriverCtx<'_>) {
        let managed = self.windows.get_mut(&window_id).unwrap();
        managed.view.teardown(
            &mut managed.view_state,
            &mut managed.view_ctx,
            ctx.window(window_id),
        );
        self.windows.remove(&window_id);
        ctx.close_window(window_id);
    }

    fn run_logic(&mut self, ctx: &mut DriverCtx<'_>) {
        let mut live_ids = HashSet::new();
        for next_view in (self.logic)(&mut self.state) {
            if !live_ids.insert(next_view.id) {
                tracing::error!(
                    window_id = next_view.id.trace(),
                    "app logic returned duplicate WindowId; ignoring duplicate window"
                );
                continue;
            }
            match self.windows.get_mut(&next_view.id) {
                Some(ManagedWindow {
                    view,
                    view_ctx,
                    view_state,
                }) => {
                    next_view.rebuild(
                        view,
                        view_state,
                        view_ctx,
                        ctx.window(next_view.id),
                        &mut self.state,
                    );
                    *view = next_view;
                }
                None => self.create_window(ctx, next_view),
            }
        }

        let to_close: Vec<_> = self
            .windows
            .keys()
            .copied()
            .filter(|id| !live_ids.contains(id))
            .collect();
        for window_id in to_close {
            self.close_window(window_id, ctx);
        }
    }
}

impl<State, Logic, WindowIter> DesktopDriver<State, Logic>
where
    State: AppState + 'static,
    Logic: FnMut(&mut State) -> WindowIter,
    WindowIter: Iterator<Item = DesktopWindow<State>>,
{
    fn dispatch_message(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_>,
        id_path: Vec<ViewId>,
        message: DynMessage,
    ) -> MessageResult<()> {
        let Some(managed) = self.windows.get_mut(&window_id) else {
            unreachable!("window presence checked before dispatch");
        };

        let mut msg_ctx = MessageCtx::new(
            std::mem::take(managed.view_ctx.environment()),
            id_path,
            message,
        );
        let result = managed.view.message(
            &mut managed.view_state,
            &mut msg_ctx,
            ctx.window(window_id),
            &mut self.state,
        );
        let (env, _id_path, _message) = msg_ctx.finish();
        *managed.view_ctx.environment() = env;
        result
    }

    fn handle_message_result(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_>,
        result: MessageResult<()>,
    ) {
        let Some(managed) = self.windows.get_mut(&window_id) else {
            unreachable!("window presence checked before handling message result");
        };

        match result {
            MessageResult::Action(()) => {
                self.run_logic(ctx);
                if !self.state.keep_running() {
                    ctx.exit();
                }
            }
            MessageResult::RequestRebuild => {
                managed.view.masonry_root.rebuild(
                    &managed.view.masonry_root,
                    &mut managed.view_state,
                    &mut managed.view_ctx,
                    ctx.render_root(window_id),
                    &mut self.state,
                );
            }
            MessageResult::Nop => {}
            MessageResult::Stale => {
                tracing::info!("discarding stale UI message");
            }
        }
    }
}

impl<State, Logic, WindowIter> AppDriver for DesktopDriver<State, Logic>
where
    State: AppState + 'static,
    Logic: FnMut(&mut State) -> WindowIter,
    WindowIter: Iterator<Item = DesktopWindow<State>>,
{
    fn on_action(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_>,
        widget_id: WidgetId,
        action: ErasedAction,
    ) {
        let Some(managed) = self.windows.get_mut(&window_id) else {
            tracing::warn!(
                window_id = window_id.trace(),
                "called on_action for unknown window"
            );
            return;
        };

        let Some(id_path) = managed.view_ctx.get_id_path(widget_id).cloned() else {
            tracing::error!(
                "received action {action:?} for unknown widget id; missing `with_action_widget`?"
            );
            return;
        };

        let result = self.dispatch_message(window_id, ctx, id_path, DynMessage(action));
        self.handle_message_result(window_id, ctx, result);
    }

    fn on_async_action(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_>,
        action: ErasedAction,
    ) {
        if !self.windows.contains_key(&window_id) {
            tracing::warn!(
                window_id = window_id.trace(),
                "called on_async_action for unknown window"
            );
            return;
        }

        let (id_path, message) = *action.downcast::<MessagePackage>().unwrap();
        let result = self.dispatch_message(window_id, ctx, Vec::from(&*id_path), message.into());
        self.handle_message_result(window_id, ctx, result);
    }

    fn on_start(&mut self, state: &mut MasonryState) {
        let fonts = std::mem::take(&mut self.fonts);
        for root in state.roots() {
            if let Some(root_widget) = root
                .get_subwindow_root(0)
                .downcast::<masonry::widgets::Passthrough>()
            {
                let fallback = root_widget.inner().inner_id();
                root.set_focus_fallback(Some(fallback));
            }
            for font in &fonts {
                drop(root.register_fonts(font.clone()));
            }
        }
        if let Some(cb) = self.on_start.take() {
            cb(state);
        }
    }

    fn on_close_requested(&mut self, window_id: WindowId, ctx: &mut DriverCtx<'_>) {
        if let Some(managed) = self.windows.get(&window_id) {
            managed.view.on_close_requested(&mut self.state);
        }
        self.run_logic(ctx);
        if !self.state.keep_running() {
            ctx.exit();
        }
    }

    fn on_wgpu_ready(&mut self, wgpu: &WgpuContext<'_>) {
        if let Some(cb) = &mut self.on_wgpu_ready {
            cb(wgpu);
        }
    }
}
