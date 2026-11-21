//! Unified [`DesignLanguage`], [`UiTheme`], and runtime theme-scoping helpers.

use xilem::masonry::core::DefaultProperties;
use xilem::masonry::core::FromDynWidget;
use xilem::{Color, WidgetView, WindowId, WindowView};
use xilem_ui_core::style_engine::scope::{
    UiScope, ui_scope as core_ui_scope, ui_window as core_ui_window,
};
use xilem_ui_core::tokens::{Brightness, DesignSpec};

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

/// Top-level runtime theme handle driving multi-theme Xilem applications.
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
        match self.language {
            DesignLanguage::Md3 => {
                #[cfg(feature = "md3")]
                {
                    let seed = self.accent.md3_seed();
                    let contrast = if self.high_contrast {
                        xilem_md3::ContrastLevel::High
                    } else {
                        xilem_md3::ContrastLevel::Standard
                    };
                    xilem_md3::Md3Theme::from_seed(seed, self.brightness)
                        .with_contrast(seed, contrast)
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
                            xilem_fluent::FluentAccent::WindowsBlue
                        }
                        UiAccent::Teal => xilem_fluent::FluentAccent::Teal,
                        UiAccent::Green => xilem_fluent::FluentAccent::Green,
                        UiAccent::Purple => xilem_fluent::FluentAccent::Purple,
                        UiAccent::Pink | UiAccent::Red => xilem_fluent::FluentAccent::Rose,
                        UiAccent::Orange => xilem_fluent::FluentAccent::Amber,
                        UiAccent::Custom(c) => xilem_fluent::FluentAccent::Custom(c),
                    };
                    xilem_fluent::FluentTheme {
                        brightness: self.brightness,
                        accent,
                        ..xilem_fluent::FluentTheme::light()
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
                            xilem_hig::HigAccent::Blue
                        }
                        UiAccent::Teal | UiAccent::Green => xilem_hig::HigAccent::Green,
                        UiAccent::Purple => xilem_hig::HigAccent::Purple,
                        UiAccent::Pink => xilem_hig::HigAccent::Pink,
                        UiAccent::Orange => xilem_hig::HigAccent::Orange,
                        UiAccent::Red => xilem_hig::HigAccent::Red,
                    };
                    xilem_hig::HigTheme {
                        brightness: self.brightness,
                        accent,
                        ..xilem_hig::HigTheme::light()
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
                            xilem_adwaita::AdwaitaAccent::Blue
                        }
                        UiAccent::Teal => xilem_adwaita::AdwaitaAccent::Teal,
                        UiAccent::Green => xilem_adwaita::AdwaitaAccent::Green,
                        UiAccent::Purple => xilem_adwaita::AdwaitaAccent::Purple,
                        UiAccent::Pink => xilem_adwaita::AdwaitaAccent::Pink,
                        UiAccent::Orange => xilem_adwaita::AdwaitaAccent::Orange,
                        UiAccent::Red => xilem_adwaita::AdwaitaAccent::Red,
                    };
                    xilem_adwaita::AdwaitaTheme {
                        brightness: self.brightness,
                        accent,
                        ..xilem_adwaita::AdwaitaTheme::light()
                    }
                    .to_spec()
                }
                #[cfg(not(feature = "adwaita"))]
                {
                    DesignSpec::neutral_fallback(self.brightness)
                }
            }
        }
    }

    /// Builds Masonry [`DefaultProperties`] corresponding to [`to_spec`](Self::to_spec).
    #[must_use]
    pub fn default_properties(&self) -> DefaultProperties {
        xilem_ui_core::style_engine::build_default_properties(&self.to_spec())
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
    core_ui_scope(theme.to_spec(), child)
}

/// Wraps an entire Xilem desktop window with backdrop color, Masonry `DefaultProperties`, and `UiScope` synced to `theme`.
pub fn ui_window<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    theme: &UiTheme,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    core_ui_window(window_id, title, &theme.to_spec(), child)
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
}
