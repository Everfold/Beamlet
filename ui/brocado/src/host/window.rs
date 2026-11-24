//! Reactive [`DesktopWindow`] view wrapping [`MasonryRoot`] and OS window attributes.
//!
//! Upstream provenance: `PodWindow` and `DesktopWindow`'s `View<State, (), ViewCtx>` delegation
//! to `MasonryRoot` are adapted from upstream `linebender/xilem` (`xilem/src/window_view.rs`,
//! `PodWindow` / `WindowView`), extended with Brocado [`Theme`] and OS theme polarity sync.

use std::sync::Arc;

use crate::style_engine::{cached_default_properties, theme_scope_arc};
use crate::theme::Theme;
use crate::tokens::Brightness;
use masonry::core::{DefaultProperties, FromDynWidget};
use masonry::peniko::Color;
use masonry::properties::Dimensions;
use masonry::theme::BACKGROUND_COLOR;
use masonry_winit::app::{NewWindow, Window, WindowId};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewElement, ViewMarker};
use xilem_masonry::style::Style as _;
use xilem_masonry::view::sized_box;
use xilem_masonry::{InitialRootWidget, MasonryRoot, ViewCtx, WidgetView};

use super::config::{WindowChrome, WindowConfig};

/// View element wrapper around [`masonry_winit::app::NewWindow`].
#[expect(
    missing_debug_implementations,
    reason = "Wraps masonry_winit NewWindow whose inner widget tree is type-erased"
)]
pub struct PodWindow(pub NewWindow);

impl ViewElement for PodWindow {
    type Mut<'a> = &'a mut Window;
}

pub(crate) type DesktopWindowViewState = <MasonryRoot<()> as View<(), (), ViewCtx>>::ViewState;

/// Reactive desktop window view backed directly by [`MasonryRoot`] and [`masonry_winit`].
#[must_use = "Window views do nothing unless returned from AppRunner."]
#[expect(
    missing_debug_implementations,
    reason = "Holds reactive MasonryRoot view and window callbacks"
)]
pub struct DesktopWindow<State: 'static> {
    pub(crate) id: WindowId,
    pub(crate) config: WindowConfig<State>,
    pub(crate) masonry_root: MasonryRoot<State>,
    pub(crate) brightness: Option<Brightness>,
    pub(crate) base_color: Option<Color>,
    pub(crate) default_properties: Option<Arc<DefaultProperties>>,
}

impl<State: 'static> DesktopWindow<State> {
    /// Creates an unthemed desktop window with `title` and `root_view`.
    pub fn new(id: WindowId, title: impl Into<String>, root_view: impl WidgetView<State>) -> Self {
        Self {
            id,
            config: WindowConfig::new(title),
            masonry_root: MasonryRoot::new(root_view),
            brightness: None,
            base_color: None,
            default_properties: None,
        }
    }

    /// Updates [`WindowConfig`] using builder closure `f`.
    pub fn with_config(
        mut self,
        f: impl FnOnce(WindowConfig<State>) -> WindowConfig<State>,
    ) -> Self {
        self.config = f(self.config);
        self
    }

    /// Registers a close handler executed when the user closes this window.
    pub fn on_close(mut self, callback: impl Fn(&mut State) + Send + Sync + 'static) -> Self {
        self.config = self.config.on_close(callback);
        self
    }

    /// Sets window frame chrome ([`WindowChrome::NativeTitleBar`], [`WindowChrome::Borderless`], etc.).
    pub fn with_chrome(mut self, chrome: WindowChrome) -> Self {
        self.config = self.config.with_chrome(chrome);
        self
    }

    /// Overrides the window surface clear color.
    pub fn with_base_color(mut self, color: Color) -> Self {
        self.base_color = Some(color);
        self
    }

    /// Assigns memoized Masonry [`DefaultProperties`] synchronized across theme swaps.
    pub fn with_default_properties(mut self, props: Arc<DefaultProperties>) -> Self {
        self.default_properties = Some(props);
        self
    }

    /// Returns the window's persistent [`WindowId`].
    #[must_use]
    pub const fn id(&self) -> WindowId {
        self.id
    }

    /// Returns the active [`WindowConfig`].
    #[must_use]
    pub const fn config(&self) -> &WindowConfig<State> {
        &self.config
    }

    pub(crate) fn on_close_requested(&self, state: &mut State) {
        if let Some(cb) = &self.config.on_close {
            cb(state);
        }
    }
}

/// Constructs a themed [`DesktopWindow`] synchronized with a shared [`Arc<Theme>`].
pub fn desktop_window_arc<State: 'static, V>(
    id: WindowId,
    title: impl Into<String>,
    theme: &Arc<Theme>,
    child: V,
) -> DesktopWindow<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    let bg = theme.scheme.surface;
    let default_props = cached_default_properties(theme);
    let root = sized_box(child)
        .dims(Dimensions::STRETCH)
        .background_color(bg);
    let mut win = DesktopWindow::new(id, title, theme_scope_arc(theme.clone(), root))
        .with_base_color(bg)
        .with_default_properties(default_props);
    win.brightness = Some(theme.brightness);
    win
}

/// Constructs a themed [`DesktopWindow`] whose ambient [`Theme`], Masonry `DefaultProperties`,
/// and OS theme polarity stay synced with `theme`.
pub fn desktop_window<State: 'static, V>(
    id: WindowId,
    title: impl Into<String>,
    theme: &Theme,
    child: V,
) -> DesktopWindow<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    desktop_window_arc(id, title, &Arc::new(theme.clone()), child)
}

impl<State: 'static> ViewMarker for DesktopWindow<State> {}

impl<State: 'static> View<State, (), ViewCtx> for DesktopWindow<State> {
    type Element = PodWindow;
    type ViewState = DesktopWindowViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (InitialRootWidget(root_widget), view_state) = self.masonry_root.build(ctx, app_state);
        let attrs = self.config.build_initial_attributes(self.brightness);
        let base_color = self.base_color.unwrap_or(BACKGROUND_COLOR);
        (
            PodWindow(
                NewWindow::new_with_id(self.id, attrs, root_widget.new_widget.erased())
                    .with_base_color(base_color),
            ),
            view_state,
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        root_widget_view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        window: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.config.sync_window(
            &prev.config,
            window.handle(),
            self.brightness,
            prev.brightness,
        );
        if self.base_color != prev.base_color
            && let Some(base_color) = self.base_color
        {
            *window.base_color() = base_color;
        }
        if let Some(props) = &self.default_properties
            && prev
                .default_properties
                .as_ref()
                .is_none_or(|p| !Arc::ptr_eq(p, props))
        {
            window.render_root().set_default_properties(props.clone());
        }
        self.masonry_root.rebuild(
            &prev.masonry_root,
            root_widget_view_state,
            ctx,
            window.render_root(),
            app_state,
        );
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        window: Mut<'_, Self::Element>,
    ) {
        self.masonry_root
            .teardown(view_state, ctx, window.render_root());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        window: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<()> {
        self.masonry_root
            .message(view_state, message, window.render_root(), app_state)
    }
}
