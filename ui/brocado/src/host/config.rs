//! Desktop window configuration, chrome presets, and reactive attribute diffing.
//!
//! Upstream provenance: reactive vs initial `WindowAttributes` diffing structure is adapted from
//! upstream `linebender/xilem` (`xilem/src/window_options.rs`, `WindowOptions`), extended with
//! Brocado `WindowChrome`, `Brightness` theme sync, and `raw_attributes` hook.

use std::sync::Arc;

use crate::tokens::Brightness;
use masonry_winit::winit::dpi::{Position, Size};
use masonry_winit::winit::window::{
    Cursor, Icon, Theme as WinitTheme, Window as WinitWindow, WindowAttributes, WindowButtons,
    WindowLevel,
};

/// Window frame chrome strategy decoupling native OS decorations from custom titlebars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowChrome {
    /// Standard OS-drawn titlebar and window borders (`decorations = true`).
    #[default]
    NativeTitleBar,
    /// Client-drawn borderless window (`decorations = false`).
    Borderless,
    /// Full-bleed unified titlebar / transparent titlebar chrome where supported by the OS.
    UnifiedTitleBar,
}

impl WindowChrome {
    /// Returns whether OS window decorations are enabled for this chrome mode.
    #[must_use]
    pub const fn uses_decorations(self) -> bool {
        match self {
            Self::NativeTitleBar | Self::UnifiedTitleBar => true,
            Self::Borderless => false,
        }
    }
}

/// Converts theme [`Brightness`] polarity to the corresponding OS Winit window theme.
#[must_use]
pub const fn brightness_to_winit_theme(brightness: Brightness) -> WinitTheme {
    match brightness {
        Brightness::Light => WinitTheme::Light,
        Brightness::Dark => WinitTheme::Dark,
    }
}

pub(crate) type CloseCallback<State> = Arc<dyn Fn(&mut State) + Send + Sync>;
pub(crate) type RawAttributesHook = Arc<dyn Fn(WindowAttributes) -> WindowAttributes + Send + Sync>;

/// Attributes, chrome settings, and close callbacks for a desktop [`DesktopWindow`](crate::DesktopWindow).
#[expect(
    missing_debug_implementations,
    reason = "Contains closure callbacks for close lifecycle and raw attribute hooks"
)]
pub struct WindowConfig<State> {
    pub(crate) title: String,
    pub(crate) resizable: bool,
    pub(crate) enabled_buttons: WindowButtons,
    pub(crate) cursor: Cursor,
    pub(crate) min_inner_size: Option<Size>,
    pub(crate) max_inner_size: Option<Size>,
    pub(crate) window_level: WindowLevel,
    pub(crate) chrome: WindowChrome,
    pub(crate) theme_override: Option<WinitTheme>,
    pub(crate) inner_size: Option<Size>,
    pub(crate) position: Option<Position>,
    pub(crate) transparent: Option<bool>,
    pub(crate) window_icon: Option<Icon>,
    pub(crate) raw_attributes: Option<RawAttributesHook>,
    pub(crate) on_close: Option<CloseCallback<State>>,
}

impl<State> Clone for WindowConfig<State> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            resizable: self.resizable,
            enabled_buttons: self.enabled_buttons,
            cursor: self.cursor.clone(),
            min_inner_size: self.min_inner_size,
            max_inner_size: self.max_inner_size,
            window_level: self.window_level,
            chrome: self.chrome,
            theme_override: self.theme_override,
            inner_size: self.inner_size,
            position: self.position,
            transparent: self.transparent,
            window_icon: self.window_icon.clone(),
            raw_attributes: self.raw_attributes.clone(),
            on_close: self.on_close.clone(),
        }
    }
}

impl<State> WindowConfig<State> {
    /// Creates default desktop window configuration with `title`.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            resizable: true,
            enabled_buttons: WindowButtons::all(),
            cursor: Cursor::default(),
            min_inner_size: None,
            max_inner_size: None,
            window_level: WindowLevel::default(),
            chrome: WindowChrome::default(),
            theme_override: None,
            inner_size: None,
            position: None,
            transparent: None,
            window_icon: None,
            raw_attributes: None,
            on_close: None,
        }
    }

    /// Sets the window titlebar string (updated reactively across rebuilds).
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the initial client area size used on window creation.
    #[must_use]
    pub fn with_initial_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.inner_size = Some(size.into());
        self
    }

    /// Sets the initial outer window position used on window creation.
    #[must_use]
    pub fn with_initial_position(mut self, position: impl Into<Position>) -> Self {
        self.position = Some(position.into());
        self
    }

    /// Sets the minimum allowed client area size.
    #[must_use]
    pub fn with_min_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.min_inner_size = Some(size.into());
        self
    }

    /// Sets the maximum allowed client area size.
    #[must_use]
    pub fn with_max_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.max_inner_size = Some(size.into());
        self
    }

    /// Controls whether the user can resize the window.
    #[must_use]
    pub const fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Controls which OS window frame buttons are enabled.
    #[must_use]
    pub const fn with_enabled_buttons(mut self, buttons: WindowButtons) -> Self {
        self.enabled_buttons = buttons;
        self
    }

    /// Sets the window cursor icon.
    #[must_use]
    pub fn with_cursor(mut self, cursor: impl Into<Cursor>) -> Self {
        self.cursor = cursor.into();
        self
    }

    /// Sets window stacking level (`Normal`, `AlwaysOnTop`, `AlwaysOnBottom`).
    #[must_use]
    pub const fn with_window_level(mut self, level: WindowLevel) -> Self {
        self.window_level = level;
        self
    }

    /// Selects frame chrome mode ([`WindowChrome::NativeTitleBar`], [`WindowChrome::Borderless`], etc.).
    #[must_use]
    pub const fn with_chrome(mut self, chrome: WindowChrome) -> Self {
        self.chrome = chrome;
        self
    }

    /// Explicitly overrides window surface transparency (defaults to `backdrop.is_translucent()`).
    #[must_use]
    pub const fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = Some(transparent);
        self
    }

    /// Assigns an initial window icon.
    #[must_use]
    pub fn with_window_icon(mut self, icon: Option<Icon>) -> Self {
        self.window_icon = icon;
        self
    }

    /// Explicitly sets an OS Winit window theme override (`Light` / `Dark`), overriding [`Theme`](crate::theme::Theme) polarity.
    #[must_use]
    pub const fn with_theme_override(mut self, theme: Option<WinitTheme>) -> Self {
        self.theme_override = theme;
        self
    }

    /// Installs an escape-hatch hook to customize raw [`WindowAttributes`] at creation time.
    #[must_use]
    pub fn with_raw_attributes(
        mut self,
        hook: impl Fn(WindowAttributes) -> WindowAttributes + Send + Sync + 'static,
    ) -> Self {
        self.raw_attributes = Some(Arc::new(hook));
        self
    }

    /// Registers a callback executed when the OS requests closing this window.
    #[must_use]
    pub fn on_close(mut self, callback: impl Fn(&mut State) + Send + Sync + 'static) -> Self {
        self.on_close = Some(Arc::new(callback));
        self
    }

    /// Builds initial [`WindowAttributes`] incorporating chrome, transparency, and theme.
    #[must_use]
    pub fn build_initial_attributes(&self, brightness: Option<Brightness>) -> WindowAttributes {
        let transparent = self.transparent.unwrap_or(false);
        let resolved_theme = self
            .theme_override
            .or_else(|| brightness.map(brightness_to_winit_theme));

        let mut attrs = WinitWindow::default_attributes()
            .with_title(self.title.clone())
            .with_resizable(self.resizable)
            .with_enabled_buttons(self.enabled_buttons)
            .with_window_level(self.window_level)
            .with_decorations(self.chrome.uses_decorations())
            .with_transparent(transparent)
            .with_theme(resolved_theme);

        if let Some(size) = self.inner_size {
            attrs = attrs.with_inner_size(size);
        }
        if let Some(pos) = self.position {
            attrs = attrs.with_position(pos);
        }
        if let Some(min) = self.min_inner_size {
            attrs = attrs.with_min_inner_size(min);
        }
        if let Some(max) = self.max_inner_size {
            attrs = attrs.with_max_inner_size(max);
        }
        if let Some(icon) = &self.window_icon {
            attrs = attrs.with_window_icon(Some(icon.clone()));
        }

        #[cfg(target_os = "macos")]
        if self.chrome == WindowChrome::UnifiedTitleBar {
            use masonry_winit::winit::platform::macos::WindowAttributesExtMacOS;
            attrs = attrs
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true);
        }

        if let Some(hook) = &self.raw_attributes {
            attrs = hook(attrs);
        }
        attrs
    }

    /// Diffs reactive window properties against `prev` and applies updates to `window`.
    pub fn sync_window(
        &self,
        prev: &Self,
        window: &WinitWindow,
        brightness: Option<Brightness>,
        prev_brightness: Option<Brightness>,
    ) {
        if self.title != prev.title {
            window.set_title(&self.title);
        }
        if self.resizable != prev.resizable {
            window.set_resizable(self.resizable);
        }
        if self.enabled_buttons != prev.enabled_buttons {
            window.set_enabled_buttons(self.enabled_buttons);
        }
        if self.cursor != prev.cursor {
            window.set_cursor(self.cursor.clone());
        }
        if self.min_inner_size != prev.min_inner_size {
            window.set_min_inner_size(self.min_inner_size);
        }
        if self.max_inner_size != prev.max_inner_size {
            window.set_max_inner_size(self.max_inner_size);
        }
        if self.window_level != prev.window_level {
            window.set_window_level(self.window_level);
        }
        if self.chrome.uses_decorations() != prev.chrome.uses_decorations() {
            window.set_decorations(self.chrome.uses_decorations());
        }

        let next_theme = self
            .theme_override
            .or_else(|| brightness.map(brightness_to_winit_theme));
        let prev_theme = prev
            .theme_override
            .or_else(|| prev_brightness.map(brightness_to_winit_theme));
        if next_theme != prev_theme {
            window.set_theme(next_theme);
        }
    }
}
