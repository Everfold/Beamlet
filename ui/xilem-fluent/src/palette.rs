//! Windows 11 Mica surface layers and Fluent accent color presets.

use xilem::Color;
use xilem_ui_core::foundation::state_layer::blend_over;
use xilem_ui_core::tokens::{Brightness, SemanticPalette};

/// Windows 11 system accent presets and custom RGB option.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FluentAccent {
    /// Default Windows 11 Cobalt Blue (`#0067C0` light / `#60CDFF` dark).
    #[default]
    WindowsBlue,
    /// Fluent Seafoam Teal (`#038387` / `#4CC2C4`).
    Teal,
    /// Fluent Iris Purple (`#744DA9` / `#B4A0FF`).
    Purple,
    /// Fluent Rose (`#C30052` / `#FF6B9C`).
    Rose,
    /// Fluent Forest Green (`#107C10` / `#6CCB5F`).
    Green,
    /// Fluent Amber (`#C15800` / `#FCE100`).
    Amber,
    /// Caller-supplied accent seed color.
    Custom(Color),
}

impl FluentAccent {
    /// Returns the resolved Light/Dark accent color pair `(primary_accent, container_tint)`.
    #[must_use]
    pub fn colors(self, brightness: Brightness) -> (Color, Color) {
        match (self, brightness) {
            (Self::WindowsBlue, Brightness::Light) => (
                Color::from_rgb8(0x00, 0x67, 0xC0),
                Color::from_rgb8(0xE6, 0xF1, 0xFB),
            ),
            (Self::WindowsBlue, Brightness::Dark) => (
                Color::from_rgb8(0x60, 0xCD, 0xFF),
                Color::from_rgb8(0x1D, 0x3A, 0x4C),
            ),
            (Self::Teal, Brightness::Light) => (
                Color::from_rgb8(0x03, 0x83, 0x87),
                Color::from_rgb8(0xE0, 0xF3, 0xF3),
            ),
            (Self::Teal, Brightness::Dark) => (
                Color::from_rgb8(0x4C, 0xC2, 0xC4),
                Color::from_rgb8(0x1A, 0x3C, 0x3D),
            ),
            (Self::Purple, Brightness::Light) => (
                Color::from_rgb8(0x74, 0x4D, 0xA9),
                Color::from_rgb8(0xF0, 0xEA, 0xF8),
            ),
            (Self::Purple, Brightness::Dark) => (
                Color::from_rgb8(0xB4, 0xA0, 0xFF),
                Color::from_rgb8(0x32, 0x2B, 0x49),
            ),
            (Self::Rose, Brightness::Light) => (
                Color::from_rgb8(0xC3, 0x00, 0x52),
                Color::from_rgb8(0xFA, 0xE5, 0xEE),
            ),
            (Self::Rose, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0x6B, 0x9C),
                Color::from_rgb8(0x49, 0x1E, 0x2F),
            ),
            (Self::Green, Brightness::Light) => (
                Color::from_rgb8(0x10, 0x7C, 0x10),
                Color::from_rgb8(0xE5, 0xF3, 0xE5),
            ),
            (Self::Green, Brightness::Dark) => (
                Color::from_rgb8(0x6C, 0xCB, 0x5F),
                Color::from_rgb8(0x1E, 0x3B, 0x1E),
            ),
            (Self::Amber, Brightness::Light) => (
                Color::from_rgb8(0xC1, 0x58, 0x00),
                Color::from_rgb8(0xFB, 0xED, 0xE1),
            ),
            (Self::Amber, Brightness::Dark) => (
                Color::from_rgb8(0xF9, 0xB8, 0x4A),
                Color::from_rgb8(0x44, 0x33, 0x17),
            ),
            (Self::Custom(c), Brightness::Light) => (c, blend_over(Color::WHITE, c, 0.14)),
            (Self::Custom(c), Brightness::Dark) => {
                let bright = blend_over(c, Color::WHITE, 0.35);
                (
                    bright,
                    blend_over(Color::from_rgb8(0x20, 0x20, 0x20), c, 0.24),
                )
            }
        }
    }
}

/// Synthesizes a Windows 11 Mica/Acrylic [`SemanticPalette`] for `accent` and `brightness`.
#[must_use]
pub fn fluent_palette(accent: FluentAccent, brightness: Brightness) -> SemanticPalette {
    let (acc, acc_container) = accent.colors(brightness);
    match brightness {
        Brightness::Light => SemanticPalette {
            accent: acc,
            on_accent: Color::WHITE,
            accent_container: acc_container,
            on_accent_container: blend_over(Color::BLACK, acc, 0.55),
            secondary: Color::from_rgb8(0x4A, 0x55, 0x68),
            on_secondary: Color::WHITE,
            secondary_container: Color::from_rgb8(0xEA, 0xEA, 0xEA),
            on_secondary_container: Color::from_rgb8(0x1A, 0x1A, 0x1A),
            tertiary_container: acc_container,
            on_tertiary_container: Color::from_rgb8(0x1A, 0x1A, 0x1A),
            window_bg: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            surface: Color::from_rgb8(0xFB, 0xFB, 0xFB),
            surface_low: Color::from_rgb8(0xF7, 0xF7, 0xF7),
            surface_container: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            surface_high: Color::WHITE,
            surface_highest: Color::from_rgb8(0xEC, 0xEC, 0xEC),
            header_bar_bg: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            sidebar_bg: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            popup_bg: Color::from_rgb8(0xFC, 0xFC, 0xFC),
            text_primary: Color::from_rgb8(0x1A, 0x1A, 0x1A),
            text_secondary: Color::from_rgb8(0x5F, 0x5F, 0x5F),
            border: Color::from_rgb8(0xE5, 0xE5, 0xE5),
            border_strong: Color::from_rgb8(0x8A, 0x8A, 0x8A),
            divider: Color::from_rgb8(0xE5, 0xE5, 0xE5),
            error: Color::from_rgb8(0xC4, 0x2B, 0x1C),
            on_error: Color::WHITE,
            inverse_surface: Color::from_rgb8(0x2B, 0x2B, 0x2B),
            inverse_on_surface: Color::WHITE,
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
        Brightness::Dark => SemanticPalette {
            accent: acc,
            on_accent: Color::from_rgb8(0x00, 0x1E, 0x2E),
            accent_container: acc_container,
            on_accent_container: Color::from_rgb8(0xF4, 0xF4, 0xF4),
            secondary: Color::from_rgb8(0xAD, 0xB5, 0xBD),
            on_secondary: Color::from_rgb8(0x1C, 0x1C, 0x1C),
            secondary_container: Color::from_rgb8(0x37, 0x37, 0x37),
            on_secondary_container: Color::WHITE,
            tertiary_container: acc_container,
            on_tertiary_container: Color::WHITE,
            window_bg: Color::from_rgb8(0x20, 0x20, 0x20),
            surface: Color::from_rgb8(0x2B, 0x2B, 0x2B),
            surface_low: Color::from_rgb8(0x24, 0x24, 0x24),
            surface_container: Color::from_rgb8(0x27, 0x27, 0x27),
            surface_high: Color::from_rgb8(0x32, 0x32, 0x32),
            surface_highest: Color::from_rgb8(0x3C, 0x3C, 0x3C),
            header_bar_bg: Color::from_rgb8(0x20, 0x20, 0x20),
            sidebar_bg: Color::from_rgb8(0x20, 0x20, 0x20),
            popup_bg: Color::from_rgb8(0x2C, 0x2C, 0x2C),
            text_primary: Color::WHITE,
            text_secondary: Color::from_rgb8(0xCE, 0xCE, 0xCE),
            border: Color::from_rgb8(0x3D, 0x3D, 0x3D),
            border_strong: Color::from_rgb8(0x9A, 0x9A, 0x9A),
            divider: Color::from_rgb8(0x3D, 0x3D, 0x3D),
            error: Color::from_rgb8(0xFF, 0x99, 0xA4),
            on_error: Color::from_rgb8(0x3B, 0x0A, 0x10),
            inverse_surface: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            inverse_on_surface: Color::from_rgb8(0x1A, 0x1A, 0x1A),
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluent_light_and_dark_use_win11_mica_surfaces() {
        let light = fluent_palette(FluentAccent::WindowsBlue, Brightness::Light);
        let dark = fluent_palette(FluentAccent::WindowsBlue, Brightness::Dark);

        assert_eq!(light.window_bg, Color::from_rgb8(0xF3, 0xF3, 0xF3));
        assert_eq!(light.surface, Color::from_rgb8(0xFB, 0xFB, 0xFB));
        assert_eq!(dark.window_bg, Color::from_rgb8(0x20, 0x20, 0x20));
        assert_eq!(dark.surface, Color::from_rgb8(0x2B, 0x2B, 0x2B));
        assert_ne!(light.accent, dark.accent);
    }

    #[test]
    fn custom_accent_builds_distinct_container_and_foreground() {
        let c = Color::from_rgb8(0x2A, 0x6F, 0x97);
        let pal = fluent_palette(FluentAccent::Custom(c), Brightness::Light);
        assert_eq!(pal.accent, c);
        assert_ne!(pal.accent_container, c);
    }
}
