//! Direct Material Design 3 Expressive [`Theme`], seed presets, and scoped theming helpers.

pub mod color;
pub mod metrics;

pub use color::{ColorScheme, ContrastLevel, DEFAULT_SEED, contrast_ratio, relative_luminance};
pub use metrics::ElevationLevel;

use std::sync::Arc;

use masonry::core::DefaultProperties;
use masonry::parley::{FontFamily, FontFamilyName};
use masonry::peniko::Color;
use masonry::properties::BoxShadow;
use xilem_masonry::WidgetView;

use crate::foundation::state_layer::with_opacity;
use crate::style_engine::{ThemeScope, theme_scope_arc};
use crate::tokens::{Brightness, TypeScale, UiLocale};

/// Curated Material 3 Expressive seed colors for quick theme creation, galleries, and previews.
pub mod seeds {
    use masonry::peniko::Color;

    /// Material 3 baseline purple seed (`#6750A4`).
    pub const PURPLE: Color = super::DEFAULT_SEED;
    /// Vibrant Cobalt Blue seed (`#1B6EF3`).
    pub const BLUE: Color = Color::from_rgb8(0x1B, 0x6E, 0xF3);
    /// Botanical Teal seed (`#006A60`).
    pub const TEAL: Color = Color::from_rgb8(0x00, 0x6A, 0x60);
    /// Forest Green seed (`#2E7D32`).
    pub const GREEN: Color = Color::from_rgb8(0x2E, 0x7D, 0x32);
    /// Magenta Rose seed (`#C2185B`).
    pub const PINK: Color = Color::from_rgb8(0xC2, 0x18, 0x5B);
    /// Persimmon Orange seed (`#D84315`).
    pub const ORANGE: Color = Color::from_rgb8(0xD8, 0x43, 0x15);
    /// Crimson Red seed (`#C62828`).
    pub const RED: Color = Color::from_rgb8(0xC6, 0x28, 0x28);
    /// Deep Indigo seed (`#3F51B5`).
    pub const INDIGO: Color = Color::from_rgb8(0x3F, 0x51, 0xB5);

    /// Ordered preset list (`(label, seed_color)`) for theme switchers and galleries.
    pub const PRESETS: [(&str, Color); 8] = [
        ("Purple", PURPLE),
        ("Blue", BLUE),
        ("Teal", TEAL),
        ("Green", GREEN),
        ("Pink", PINK),
        ("Orange", ORANGE),
        ("Red", RED),
        ("Indigo", INDIGO),
    ];
}

/// Complete Material Design 3 Expressive runtime theme passed through [`theme_scope`].
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Light or Dark polarity.
    pub brightness: Brightness,
    /// Dynamic contrast tier (`Standard`, `Medium`, `High`).
    pub contrast: ContrastLevel,
    /// Source OKLCH seed color used to derive [`scheme`](Self::scheme).
    pub seed: Color,
    /// Full Material 3 Expressive color roles.
    pub scheme: ColorScheme,
    /// 15-role Material 3 Expressive typography scale and optional font family.
    pub typography: TypeScale,
    /// Accessibility preference disabling entrance slide/fade offsets and spring motion.
    pub reduced_motion: bool,
    /// Localization labels and calendar/time conventions.
    pub locale: UiLocale,
}

impl Theme {
    /// Default Material 3 Expressive Light theme (`DEFAULT_SEED`, `Standard` contrast).
    #[must_use]
    pub fn light() -> Self {
        Self::from_seed(DEFAULT_SEED, Brightness::Light)
    }

    /// Default Material 3 Expressive Dark theme (`DEFAULT_SEED`, `Standard` contrast).
    #[must_use]
    pub fn dark() -> Self {
        Self::from_seed(DEFAULT_SEED, Brightness::Dark)
    }

    /// Creates a Material 3 Expressive theme generated from `seed` and `brightness`.
    #[must_use]
    pub fn from_seed(seed: Color, brightness: Brightness) -> Self {
        Self {
            brightness,
            contrast: ContrastLevel::Standard,
            seed,
            scheme: ColorScheme::from_seed(seed, brightness, ContrastLevel::Standard),
            typography: TypeScale::expressive(),
            reduced_motion: false,
            locale: UiLocale::english(),
        }
    }

    /// Re-derives [`scheme`](Self::scheme) for a new OKLCH `seed` color.
    #[must_use]
    pub fn with_seed(mut self, seed: Color) -> Self {
        self.seed = seed;
        self.scheme = ColorScheme::from_seed(self.seed, self.brightness, self.contrast);
        self
    }

    /// Updates [`brightness`](Self::brightness) and re-derives [`scheme`](Self::scheme).
    #[must_use]
    pub fn with_brightness(mut self, brightness: Brightness) -> Self {
        self.brightness = brightness;
        self.scheme = ColorScheme::from_seed(self.seed, self.brightness, self.contrast);
        self
    }

    /// Updates [`contrast`](Self::contrast) and re-derives [`scheme`](Self::scheme).
    #[must_use]
    pub fn with_contrast(mut self, contrast: ContrastLevel) -> Self {
        self.contrast = contrast;
        self.scheme = ColorScheme::from_seed(self.seed, self.brightness, self.contrast);
        self
    }

    /// Convenience toggle selecting `ContrastLevel::High` (`true`) or `ContrastLevel::Standard` (`false`).
    #[must_use]
    pub fn with_high_contrast(self, high_contrast: bool) -> Self {
        self.with_contrast(if high_contrast {
            ContrastLevel::High
        } else {
            ContrastLevel::Standard
        })
    }

    /// Enables or disables accessibility reduced-motion behavior.
    #[must_use]
    pub const fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    /// Replaces the typography scale wholesale.
    #[must_use]
    pub fn with_typography(mut self, typography: TypeScale) -> Self {
        self.typography = typography;
        self
    }

    /// Overrides the theme's font family / fallback chain explicitly.
    #[must_use]
    pub fn with_font_family(mut self, family: FontFamily<'static>) -> Self {
        self.typography = self.typography.with_font_family(family);
        self
    }

    /// Parses and assigns a CSS-style font fallback stack string (e.g. `"Inter, system-ui, sans-serif"`).
    ///
    /// An empty or whitespace-only string resets to the OS system UI default.
    #[must_use]
    pub fn with_font_stack(mut self, stack: &str) -> Self {
        self.typography = self.typography.with_font_stack(stack);
        self
    }

    /// Replaces the font stack with an explicit ordered sequence of [`FontFamilyName`] items.
    #[must_use]
    pub fn with_font_fallbacks<I>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = FontFamilyName<'static>>,
    {
        self.typography = self.typography.with_font_fallbacks(families);
        self
    }

    /// Appends a fallback font family to the end of the current stack.
    #[must_use]
    pub fn append_font_fallback(mut self, fallback: FontFamilyName<'static>) -> Self {
        self.typography = self.typography.append_font_fallback(fallback);
        self
    }

    /// Resets the font configuration back to the OS default UI font (`GenericFamily::SystemUi`).
    #[must_use]
    pub fn with_system_font(mut self) -> Self {
        self.typography = self.typography.with_system_font();
        self
    }

    /// Uniformly scales typography sizes and line heights (clamped to `0.5..=3.0`).
    #[must_use]
    pub fn with_font_scale(mut self, scale: f32) -> Self {
        self.typography = self.typography.scaled(scale);
        self
    }

    /// Assigns localized calendar, picker, dialog, and search copy overrides.
    #[must_use]
    pub fn with_locale(mut self, locale: UiLocale) -> Self {
        self.locale = locale;
        self
    }

    /// Toggles `Light <-> Dark` polarity in-place and re-derives [`scheme`](Self::scheme).
    pub fn toggle_brightness(&mut self) {
        self.brightness = self.brightness.toggled();
        self.scheme = ColorScheme::from_seed(self.seed, self.brightness, self.contrast);
    }

    /// Standard Material 3 disabled foreground/ink color (`38%` `on_surface`).
    #[must_use]
    pub fn disabled_content(&self) -> Color {
        with_opacity(self.scheme.on_surface, 0.38)
    }

    /// Standard Material 3 disabled container background fill (`12%` `on_surface`).
    #[must_use]
    pub fn disabled_container(&self) -> Color {
        with_opacity(self.scheme.on_surface, 0.12)
    }

    /// Resolves `active_color` or `disabled_content()` when `disabled` is `true`.
    #[must_use]
    pub fn content_color(&self, active_color: Color, disabled: bool) -> Color {
        if disabled {
            self.disabled_content()
        } else {
            active_color
        }
    }

    /// Computes box shadow for `level` using `self.scheme.shadow`.
    #[must_use]
    pub fn box_shadow(&self, level: ElevationLevel) -> BoxShadow {
        level.box_shadow(self.scheme.shadow)
    }

    /// Builds Masonry [`DefaultProperties`] corresponding to this [`Theme`].
    #[must_use]
    pub fn default_properties(&self) -> DefaultProperties {
        crate::style_engine::build_default_properties(self)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

/// Formats a primary-modifier shortcut chord such as `"S"` into `"Ctrl+S"`.
#[must_use]
pub fn format_shortcut_primary(key: &str) -> String {
    format!("Ctrl+{key}")
}

/// Formats a primary+shift shortcut chord such as `"Z"` into `"Ctrl+Shift+Z"`.
#[must_use]
pub fn format_shortcut_primary_shift(key: &str) -> String {
    format!("Ctrl+Shift+{key}")
}

/// Wraps `child` inside a live [`Theme`] scope.
pub fn theme_scope<State: 'static, Action: 'static, V>(
    theme: &Theme,
    child: V,
) -> ThemeScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: masonry::core::FromDynWidget,
{
    theme_scope_arc(Arc::new(theme.clone()), child)
}

/// Creates a themed [`DesktopWindow`](crate::host::DesktopWindow) preconfigured with `theme` polarity and surface background.
#[cfg(feature = "winit")]
pub fn themed_window<State: 'static, V>(
    id: masonry_winit::app::WindowId,
    title: impl Into<String>,
    theme: &Theme,
    root_view: V,
) -> crate::host::DesktopWindow<State>
where
    V: WidgetView<State>,
{
    crate::host::desktop_window(id, title, theme, root_view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use masonry::parley::GenericFamily;

    #[test]
    fn theme_font_builders_configure_and_reset_stack() {
        let themed = Theme::light()
            .with_font_stack("Inter, system-ui, sans-serif")
            .with_font_scale(1.15);
        assert!(themed.typography.font_family().is_some());
        assert!((themed.typography.body_medium.font_size - 16.1).abs() < 0.05);

        let reset = themed.with_system_font();
        assert!(reset.typography.font_family().is_none());

        let appended =
            Theme::light().append_font_fallback(FontFamilyName::Generic(GenericFamily::Monospace));
        assert!(matches!(
            appended.typography.font_family(),
            Some(FontFamily::List(_))
        ));
    }

    #[test]
    fn shortcut_helpers_format_deterministically() {
        assert_eq!(format_shortcut_primary("S"), "Ctrl+S");
        assert_eq!(format_shortcut_primary_shift("Z"), "Ctrl+Shift+Z");
    }

    #[test]
    fn theme_toggle_and_seed_update_scheme() {
        let mut theme = Theme::from_seed(seeds::TEAL, Brightness::Light);
        let light_surface = theme.scheme.surface;
        theme.toggle_brightness();
        assert_eq!(theme.brightness, Brightness::Dark);
        assert_ne!(theme.scheme.surface, light_surface);
        assert_eq!(theme.with_high_contrast(true).contrast, ContrastLevel::High);
    }
}
