//! Unified [`DesignLanguage`], [`UiTheme`], and runtime theme-scoping helpers.

use std::sync::{Arc, LazyLock, Mutex};

use brocado_core::style_engine::scope::{UiScope, ui_scope_arc as core_ui_scope_arc};
use brocado_core::tokens::{Brightness, DesignSpec, TypeScale, WindowBackdrop};
use brocado_core::{FontFamily, FontFamilyName};
#[cfg(feature = "winit")]
use brocado_winit::{
    DesktopWindow, WindowId, desktop_window_arc, desktop_window_arc_with_backdrop,
};
use masonry::core::DefaultProperties;
use masonry::core::FromDynWidget;
use masonry::peniko::Color;
use xilem_masonry::WidgetView;

type ThemeSpecCache = Vec<(UiTheme, Arc<DesignSpec>)>;

static THEME_SPEC_CACHE: LazyLock<Mutex<ThemeSpecCache>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Supported cross-platform design systems.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DesignLanguage {
    /// Google Material Design 3 (OKLCH dynamic color, pill chrome, tinted state layers).
    #[default]
    Md3,
    /// Microsoft Windows 11 / WinUI 3 Fluent (Mica layers, `4px` radius, `2px` bottom focus bar).
    Fluent,
    /// Apple macOS Human Interface Guidelines (Aqua surfaces, compact `26px` chrome, `3px` halo focus).
    Hig,
    /// GNOME Libadwaita (Flat alpha veils, `12px` boxed lists, `46px` headerbar).
    Adwaita,
}

impl DesignLanguage {
    /// Ordered array of all four supported design languages.
    pub const ALL: [Self; 4] = [Self::Md3, Self::Fluent, Self::Hig, Self::Adwaita];

    /// Selects the host OS native design language at compile time (`Fluent` on Windows,
    /// `Hig` on macOS, `Adwaita` on Linux, falling back to `Md3`).
    #[must_use]
    pub const fn native() -> Self {
        if cfg!(target_os = "windows") {
            Self::Fluent
        } else if cfg!(target_os = "macos") {
            Self::Hig
        } else if cfg!(target_os = "linux") {
            Self::Adwaita
        } else {
            Self::Md3
        }
    }

    /// Human-readable display name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Md3 => "Material 3",
            Self::Fluent => "Windows Fluent",
            Self::Hig => "macOS HIG",
            Self::Adwaita => "GNOME Adwaita",
        }
    }
}

/// Unified accent preset mapped idiomatically onto every active backend.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum UiAccent {
    /// Theme-native default accent (`#6750A4` MD3 / Windows Blue / Apple Blue / GNOME Blue).
    #[default]
    Default,
    /// Blue accent.
    Blue,
    /// Teal accent.
    Teal,
    /// Green accent.
    Green,
    /// Purple accent.
    Purple,
    /// Pink / Rose accent.
    Pink,
    /// Orange / Amber accent.
    Orange,
    /// Red accent.
    Red,
    /// Arbitrary RGB seed color.
    Custom(Color),
}

impl UiAccent {
    /// Ordered named presets (excluding `Custom`) suitable for switcher bars.
    pub const PRESETS: [Self; 8] = [
        Self::Default,
        Self::Blue,
        Self::Teal,
        Self::Green,
        Self::Purple,
        Self::Pink,
        Self::Orange,
        Self::Red,
    ];

    /// Short display label for the preset.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Blue => "Blue",
            Self::Teal => "Teal",
            Self::Green => "Green",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Orange => "Orange",
            Self::Red => "Red",
            Self::Custom(_) => "Custom",
        }
    }

    #[cfg(feature = "md3")]
    const fn md3_seed(self) -> Color {
        match self {
            Self::Default | Self::Purple => Color::from_rgb8(0x67, 0x50, 0xA4),
            Self::Blue => Color::from_rgb8(0x1B, 0x6E, 0xF3),
            Self::Teal => Color::from_rgb8(0x00, 0x6A, 0x60),
            Self::Green => Color::from_rgb8(0x2E, 0x7D, 0x32),
            Self::Pink => Color::from_rgb8(0xC2, 0x18, 0x5B),
            Self::Orange => Color::from_rgb8(0xD8, 0x43, 0x15),
            Self::Red => Color::from_rgb8(0xC6, 0x28, 0x28),
            Self::Custom(c) => c,
        }
    }
}

/// Top-level runtime theme handle driving multi-theme Brocado applications.
#[derive(Clone, Debug, PartialEq)]
pub struct UiTheme {
    /// Active platform design language.
    pub language: DesignLanguage,
    /// Light or Dark polarity.
    pub brightness: Brightness,
    /// Brand/system accent color selection.
    pub accent: UiAccent,
    /// Elevated contrast tier (mainly strengthens MD3 tonal contrast).
    pub high_contrast: bool,
    /// Disables entrance/transition motion durations and spatial offsets when `true`.
    pub reduced_motion: bool,
    /// Optional font family / fallback stack overriding the OS system UI default (`None` uses `system-ui`).
    pub font_family: Option<FontFamily<'static>>,
}

impl UiTheme {
    /// Native host OS theme in Light mode.
    #[must_use]
    pub fn native_light() -> Self {
        Self::new(DesignLanguage::native(), Brightness::Light)
    }

    /// Native host OS theme in Dark mode.
    #[must_use]
    pub fn native_dark() -> Self {
        Self::new(DesignLanguage::native(), Brightness::Dark)
    }

    /// Creates a [`UiTheme`] with explicit [`DesignLanguage`] and [`Brightness`].
    #[must_use]
    pub const fn new(language: DesignLanguage, brightness: Brightness) -> Self {
        Self {
            language,
            brightness,
            accent: UiAccent::Default,
            high_contrast: false,
            reduced_motion: false,
            font_family: None,
        }
    }

    /// Sets the accent preset.
    #[must_use]
    pub const fn with_accent(mut self, accent: UiAccent) -> Self {
        self.accent = accent;
        self
    }

    /// Enables or disables high-contrast palette adjustment.
    #[must_use]
    pub const fn with_high_contrast(mut self, high_contrast: bool) -> Self {
        self.high_contrast = high_contrast;
        self
    }

    /// Sets accessibility reduced-motion preference.
    #[must_use]
    pub const fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    /// Overrides the theme's font family / fallback chain explicitly.
    #[must_use]
    pub fn with_font_family(mut self, family: FontFamily<'static>) -> Self {
        self.font_family = Some(family);
        self
    }

    /// Parses and assigns a CSS-style font fallback stack string (e.g. `"Inter, system-ui, sans-serif"`).
    ///
    /// An empty or whitespace-only string resets to the OS system UI default.
    #[must_use]
    pub fn with_font_stack(mut self, stack: &str) -> Self {
        self.font_family = TypeScale::parse_font_stack(stack);
        self
    }

    /// Replaces the font stack with an explicit ordered sequence of [`FontFamilyName`] items.
    ///
    /// An empty sequence resets to the OS system UI default.
    #[must_use]
    pub fn with_font_fallbacks<I>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = FontFamilyName<'static>>,
    {
        self.font_family = TypeScale::baseline()
            .with_font_fallbacks(families)
            .font_family()
            .cloned();
        self
    }

    /// Appends a fallback font family to the current font stack (or after `GenericFamily::SystemUi` when unset).
    #[must_use]
    pub fn append_font_fallback(mut self, fallback: FontFamilyName<'static>) -> Self {
        let base = match self.font_family.take() {
            Some(f) => TypeScale::baseline().with_font_family(f),
            None => TypeScale::baseline(),
        };
        self.font_family = base.append_font_fallback(fallback).font_family().cloned();
        self
    }

    /// Resets the font configuration back to the OS default UI font (`GenericFamily::SystemUi`).
    #[must_use]
    pub fn with_system_font(mut self) -> Self {
        self.font_family = None;
        self
    }

    /// Toggles `Light <-> Dark` polarity in-place.
    pub fn toggle_brightness(&mut self) {
        self.brightness = match self.brightness {
            Brightness::Light => Brightness::Dark,
            Brightness::Dark => Brightness::Light,
        };
    }

    /// Synthesizes the complete cross-theme [`DesignSpec`].
    #[must_use]
    pub fn to_spec(&self) -> DesignSpec {
        let mut spec = match self.language {
            DesignLanguage::Md3 => {
                #[cfg(feature = "md3")]
                {
                    let seed = self.accent.md3_seed();
                    let contrast = if self.high_contrast {
                        brocado_md3::ContrastLevel::High
                    } else {
                        brocado_md3::ContrastLevel::Standard
                    };
                    brocado_md3::Md3Theme::from_seed(seed, self.brightness)
                        .with_contrast(seed, contrast)
                        .with_reduced_motion(self.reduced_motion)
                        .to_spec()
                }
                #[cfg(not(feature = "md3"))]
                {
                    DesignSpec::neutral_fallback(self.brightness)
                }
            }
            DesignLanguage::Fluent => {
                #[cfg(feature = "fluent")]
                {
                    let accent = match self.accent {
                        UiAccent::Default | UiAccent::Blue => {
                            brocado_fluent::FluentAccent::WindowsBlue
                        }
                        UiAccent::Teal => brocado_fluent::FluentAccent::Teal,
                        UiAccent::Green => brocado_fluent::FluentAccent::Green,
                        UiAccent::Purple => brocado_fluent::FluentAccent::Purple,
                        UiAccent::Pink | UiAccent::Red => brocado_fluent::FluentAccent::Rose,
                        UiAccent::Orange => brocado_fluent::FluentAccent::Amber,
                        UiAccent::Custom(c) => brocado_fluent::FluentAccent::Custom(c),
                    };
                    brocado_fluent::FluentTheme {
                        brightness: self.brightness,
                        accent,
                        reduced_motion: self.reduced_motion,
                        ..brocado_fluent::FluentTheme::light()
                    }
                    .to_spec()
                }
                #[cfg(not(feature = "fluent"))]
                {
                    DesignSpec::neutral_fallback(self.brightness)
                }
            }
            DesignLanguage::Hig => {
                #[cfg(feature = "hig")]
                {
                    let accent = match self.accent {
                        UiAccent::Default | UiAccent::Blue | UiAccent::Custom(_) => {
                            brocado_hig::HigAccent::Blue
                        }
                        UiAccent::Teal | UiAccent::Green => brocado_hig::HigAccent::Green,
                        UiAccent::Purple => brocado_hig::HigAccent::Purple,
                        UiAccent::Pink => brocado_hig::HigAccent::Pink,
                        UiAccent::Orange => brocado_hig::HigAccent::Orange,
                        UiAccent::Red => brocado_hig::HigAccent::Red,
                    };
                    brocado_hig::HigTheme {
                        brightness: self.brightness,
                        accent,
                        reduced_motion: self.reduced_motion,
                        ..brocado_hig::HigTheme::light()
                    }
                    .to_spec()
                }
                #[cfg(not(feature = "hig"))]
                {
                    DesignSpec::neutral_fallback(self.brightness)
                }
            }
            DesignLanguage::Adwaita => {
                #[cfg(feature = "adwaita")]
                {
                    let accent = match self.accent {
                        UiAccent::Default | UiAccent::Blue | UiAccent::Custom(_) => {
                            brocado_adwaita::AdwaitaAccent::Blue
                        }
                        UiAccent::Teal => brocado_adwaita::AdwaitaAccent::Teal,
                        UiAccent::Green => brocado_adwaita::AdwaitaAccent::Green,
                        UiAccent::Purple => brocado_adwaita::AdwaitaAccent::Purple,
                        UiAccent::Pink => brocado_adwaita::AdwaitaAccent::Pink,
                        UiAccent::Orange => brocado_adwaita::AdwaitaAccent::Orange,
                        UiAccent::Red => brocado_adwaita::AdwaitaAccent::Red,
                    };
                    brocado_adwaita::AdwaitaTheme {
                        brightness: self.brightness,
                        accent,
                        reduced_motion: self.reduced_motion,
                        ..brocado_adwaita::AdwaitaTheme::light()
                    }
                    .to_spec()
                }
                #[cfg(not(feature = "adwaita"))]
                {
                    DesignSpec::neutral_fallback(self.brightness)
                }
            }
        };
        if let Some(font) = &self.font_family {
            spec.typography = spec.typography.with_font_family(font.clone());
        }
        spec
    }

    /// Returns a cached [`Arc<DesignSpec>`] for this theme, avoiding repeated palette
    /// synthesis and cloning across high-frequency rebuilds (e.g. slider drags).
    #[must_use]
    pub fn cached_spec(&self) -> Arc<DesignSpec> {
        let mut cache = THEME_SPEC_CACHE
            .lock()
            .expect("theme spec cache mutex poisoned");
        if let Some((_, spec)) = cache.iter().find(|(cached, _)| cached == self) {
            return spec.clone();
        }
        let spec = Arc::new(self.to_spec());
        if cache.len() >= 32 {
            cache.remove(0);
        }
        cache.push((self.clone(), spec.clone()));
        spec
    }

    /// Builds Masonry [`DefaultProperties`] corresponding to [`to_spec`](Self::to_spec).
    #[must_use]
    pub fn default_properties(&self) -> DefaultProperties {
        brocado_core::style_engine::build_default_properties(&self.cached_spec())
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::native_light()
    }
}

/// Wraps `child` inside a live [`UiTheme`] scope.
pub fn ui_scope<State: 'static, Action: 'static, V>(
    theme: &UiTheme,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    core_ui_scope_arc(theme.cached_spec(), child)
}

/// Wraps an entire desktop [`DesktopWindow`] with backdrop color, Masonry `DefaultProperties`, OS theme polarity, and `UiScope` synced to `theme`.
#[cfg(feature = "winit")]
pub fn themed_window<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    theme: &UiTheme,
    child: V,
) -> DesktopWindow<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    desktop_window_arc(window_id, title, &theme.cached_spec(), child)
}

/// Wraps an entire desktop [`DesktopWindow`] with an explicit [`WindowBackdrop`] override, Masonry
/// `DefaultProperties`, OS theme polarity, and `UiScope` synced to `theme`.
#[cfg(feature = "winit")]
pub fn themed_window_with_backdrop<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    theme: &UiTheme,
    backdrop: WindowBackdrop,
    child: V,
) -> DesktopWindow<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    desktop_window_arc_with_backdrop(window_id, title, &theme.cached_spec(), backdrop, child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_theme_generates_distinct_specs_per_language_and_polarity() {
        for lang in DesignLanguage::ALL {
            let light = UiTheme::new(lang, Brightness::Light).to_spec();
            let dark = UiTheme::new(lang, Brightness::Dark).to_spec();
            assert_eq!(light.brightness, Brightness::Light);
            assert_eq!(dark.brightness, Brightness::Dark);
            assert_ne!(light.palette.window_bg, dark.palette.window_bg);
            assert_ne!(light.palette.text_primary, dark.palette.text_primary);
        }
    }

    #[test]
    fn ui_theme_assigns_expected_platform_backdrops() {
        assert_eq!(
            UiTheme::new(DesignLanguage::Md3, Brightness::Light)
                .to_spec()
                .backdrop,
            WindowBackdrop::Opaque
        );
        assert_eq!(
            UiTheme::new(DesignLanguage::Fluent, Brightness::Light)
                .to_spec()
                .backdrop,
            WindowBackdrop::Mica
        );
        assert_eq!(
            UiTheme::new(DesignLanguage::Hig, Brightness::Light)
                .to_spec()
                .backdrop,
            WindowBackdrop::MacosVibrancy
        );
        assert_eq!(
            UiTheme::new(DesignLanguage::Adwaita, Brightness::Light)
                .to_spec()
                .backdrop,
            WindowBackdrop::Opaque
        );
    }

    #[test]
    fn ui_theme_defaults_to_system_font_and_propagates_custom_fallbacks() {
        use std::borrow::Cow;
        let system_ui = FontFamily::Single(FontFamilyName::Generic(
            brocado_core::GenericFamily::SystemUi,
        ));

        for lang in DesignLanguage::ALL {
            let theme = UiTheme::new(lang, Brightness::Light);
            assert!(theme.font_family.is_none());
            assert!(theme.to_spec().typography.font_family().is_none());
            assert_eq!(theme.to_spec().typography.resolved_font_family(), system_ui);
        }

        let stacked =
            UiTheme::new(DesignLanguage::Md3, Brightness::Dark).with_font_stack("CustomUI, serif");
        assert_eq!(
            stacked.cached_spec().typography.font_family(),
            Some(&FontFamily::List(Cow::Owned(vec![
                FontFamilyName::Named(Cow::Owned(String::from("CustomUI"))),
                FontFamilyName::Generic(brocado_core::GenericFamily::Serif),
            ])))
        );

        let appended = UiTheme::new(DesignLanguage::Hig, Brightness::Light)
            .append_font_fallback(FontFamilyName::Named(Cow::Borrowed("AppEmoji")));
        assert_eq!(
            appended.cached_spec().typography.font_family(),
            Some(&FontFamily::List(Cow::Owned(vec![
                FontFamilyName::Generic(brocado_core::GenericFamily::SystemUi),
                FontFamilyName::Named(Cow::Borrowed("AppEmoji")),
            ])))
        );

        let reset = appended.with_system_font();
        assert!(reset.font_family.is_none());
        assert_eq!(
            reset.cached_spec().typography.resolved_font_family(),
            system_ui
        );
    }
}
