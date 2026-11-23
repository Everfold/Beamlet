//! GNOME Libadwaita surface tokens and 9 named system accent colors.

use masonry::peniko::Color;
use xilem_ui_core::tokens::{Brightness, SemanticPalette};

/// GNOME Libadwaita system accent color presets (`org.gnome.desktop.interface accent-color`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AdwaitaAccent {
    /// GNOME Blue (`#3584E4` light / `#78AEED` dark).
    #[default]
    Blue,
    /// GNOME Teal (`#2190A4` / `#62C3D3`).
    Teal,
    /// GNOME Green (`#2EC27E` / `#57E389`).
    Green,
    /// GNOME Yellow (`#B58300` / `#F8E45C`).
    Yellow,
    /// GNOME Orange (`#E66100` / `#FFA348`).
    Orange,
    /// GNOME Red (`#E01B24` / `#F66151`).
    Red,
    /// GNOME Pink (`#D56199` / `#F48BB9`).
    Pink,
    /// GNOME Purple (`#9141AC` / `#DC8ADD`).
    Purple,
    /// GNOME Slate (`#6F8396` / `#99C1F1`).
    Slate,
}

impl AdwaitaAccent {
    /// Returns `(accent, accent_container)` for `brightness`.
    #[must_use]
    pub const fn colors(self, brightness: Brightness) -> (Color, Color) {
        match (self, brightness) {
            (Self::Blue, Brightness::Light) => (
                Color::from_rgb8(0x35, 0x84, 0xE4),
                Color::from_rgb8(0xDF, 0xEC, 0xFB),
            ),
            (Self::Blue, Brightness::Dark) => (
                Color::from_rgb8(0x78, 0xAE, 0xED),
                Color::from_rgb8(0x1E, 0x35, 0x52),
            ),
            (Self::Teal, Brightness::Light) => (
                Color::from_rgb8(0x21, 0x90, 0xA4),
                Color::from_rgb8(0xDC, 0xF0, 0xF4),
            ),
            (Self::Teal, Brightness::Dark) => (
                Color::from_rgb8(0x62, 0xC3, 0xD3),
                Color::from_rgb8(0x19, 0x3B, 0x42),
            ),
            (Self::Green, Brightness::Light) => (
                Color::from_rgb8(0x26, 0xA2, 0x69),
                Color::from_rgb8(0xDD, 0xF4, 0xE8),
            ),
            (Self::Green, Brightness::Dark) => (
                Color::from_rgb8(0x57, 0xE3, 0x89),
                Color::from_rgb8(0x18, 0x3D, 0x29),
            ),
            (Self::Yellow, Brightness::Light) => (
                Color::from_rgb8(0xAE, 0x7B, 0x03),
                Color::from_rgb8(0xF8, 0xF0, 0xD3),
            ),
            (Self::Yellow, Brightness::Dark) => (
                Color::from_rgb8(0xF8, 0xE4, 0x5C),
                Color::from_rgb8(0x42, 0x3A, 0x17),
            ),
            (Self::Orange, Brightness::Light) => (
                Color::from_rgb8(0xE6, 0x61, 0x00),
                Color::from_rgb8(0xFC, 0xE9, 0xD9),
            ),
            (Self::Orange, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0xA3, 0x48),
                Color::from_rgb8(0x47, 0x2B, 0x15),
            ),
            (Self::Red, Brightness::Light) => (
                Color::from_rgb8(0xE0, 0x1B, 0x24),
                Color::from_rgb8(0xFA, 0xE0, 0xE2),
            ),
            (Self::Red, Brightness::Dark) => (
                Color::from_rgb8(0xF6, 0x61, 0x51),
                Color::from_rgb8(0x48, 0x1C, 0x1A),
            ),
            (Self::Pink, Brightness::Light) => (
                Color::from_rgb8(0xD5, 0x61, 0x99),
                Color::from_rgb8(0xF8, 0xE7, 0xEF),
            ),
            (Self::Pink, Brightness::Dark) => (
                Color::from_rgb8(0xF4, 0x8B, 0xB9),
                Color::from_rgb8(0x46, 0x21, 0x34),
            ),
            (Self::Purple, Brightness::Light) => (
                Color::from_rgb8(0x91, 0x41, 0xAC),
                Color::from_rgb8(0xEF, 0xE2, 0xF4),
            ),
            (Self::Purple, Brightness::Dark) => (
                Color::from_rgb8(0xDC, 0x8A, 0xDD),
                Color::from_rgb8(0x3B, 0x22, 0x41),
            ),
            (Self::Slate, Brightness::Light) => (
                Color::from_rgb8(0x6F, 0x83, 0x96),
                Color::from_rgb8(0xE8, 0xEC, 0xF0),
            ),
            (Self::Slate, Brightness::Dark) => (
                Color::from_rgb8(0x99, 0xC1, 0xF1),
                Color::from_rgb8(0x28, 0x34, 0x42),
            ),
        }
    }
}

/// Synthesizes a GNOME Libadwaita [`SemanticPalette`] for `accent` and `brightness`.
#[must_use]
pub fn adwaita_palette(accent: AdwaitaAccent, brightness: Brightness) -> SemanticPalette {
    let (acc, acc_container) = accent.colors(brightness);
    match brightness {
        Brightness::Light => SemanticPalette {
            accent: acc,
            on_accent: Color::WHITE,
            accent_container: acc_container,
            on_accent_container: Color::from_rgb8(0x1C, 0x2B, 0x3E),
            secondary: Color::from_rgb8(0x5E, 0x5C, 0x64),
            on_secondary: Color::WHITE,
            secondary_container: Color::from_rgb8(0xE1, 0xE1, 0xE3),
            on_secondary_container: Color::from_rgb8(0x24, 0x1F, 0x31),
            tertiary_container: acc_container,
            on_tertiary_container: Color::from_rgb8(0x24, 0x1F, 0x31),
            window_bg: Color::from_rgb8(0xFA, 0xFA, 0xFA),
            surface: Color::WHITE,
            surface_low: Color::from_rgb8(0xF6, 0xF5, 0xF4),
            surface_container: Color::from_rgb8(0xEB, 0xEB, 0xEB),
            surface_high: Color::WHITE,
            surface_highest: Color::from_rgb8(0xE2, 0xE2, 0xE4),
            header_bar_bg: Color::from_rgb8(0xEB, 0xEB, 0xEB),
            sidebar_bg: Color::from_rgb8(0xF3, 0xF3, 0xF3),
            popup_bg: Color::from_rgb8(0xFC, 0xFC, 0xFC),
            text_primary: Color::from_rgb8(0x24, 0x1F, 0x31),
            text_secondary: Color::from_rgb8(0x5E, 0x5C, 0x64),
            border: Color::from_rgb8(0xDA, 0xD8, 0xDC),
            border_strong: Color::from_rgb8(0x9A, 0x99, 0x96),
            divider: Color::from_rgb8(0xE4, 0xE3, 0xE6),
            success: Color::from_rgb8(0x26, 0xA2, 0x69),
            on_success: Color::WHITE,
            warning: Color::from_rgb8(0xAE, 0x7B, 0x03),
            on_warning: Color::WHITE,
            error: Color::from_rgb8(0xC0, 0x1C, 0x28),
            on_error: Color::WHITE,
            inverse_surface: Color::from_rgb8(0x2E, 0x2E, 0x32),
            inverse_on_surface: Color::WHITE,
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
        Brightness::Dark => SemanticPalette {
            accent: acc,
            on_accent: Color::from_rgb8(0x0E, 0x16, 0x24),
            accent_container: acc_container,
            on_accent_container: Color::from_rgb8(0xF6, 0xF5, 0xF4),
            secondary: Color::from_rgb8(0x9A, 0x99, 0x96),
            on_secondary: Color::from_rgb8(0x24, 0x24, 0x24),
            secondary_container: Color::from_rgb8(0x3D, 0x3D, 0x3F),
            on_secondary_container: Color::WHITE,
            tertiary_container: acc_container,
            on_tertiary_container: Color::WHITE,
            window_bg: Color::from_rgb8(0x24, 0x24, 0x24),
            surface: Color::from_rgb8(0x30, 0x30, 0x30),
            surface_low: Color::from_rgb8(0x2A, 0x2A, 0x2A),
            surface_container: Color::from_rgb8(0x34, 0x34, 0x34),
            surface_high: Color::from_rgb8(0x38, 0x38, 0x38),
            surface_highest: Color::from_rgb8(0x44, 0x44, 0x44),
            header_bar_bg: Color::from_rgb8(0x30, 0x30, 0x30),
            sidebar_bg: Color::from_rgb8(0x2B, 0x2B, 0x2B),
            popup_bg: Color::from_rgb8(0x36, 0x36, 0x36),
            text_primary: Color::WHITE,
            text_secondary: Color::from_rgb8(0xB8, 0xB8, 0xBC),
            border: Color::from_rgb8(0x45, 0x45, 0x48),
            border_strong: Color::from_rgb8(0x77, 0x76, 0x7B),
            divider: Color::from_rgb8(0x3F, 0x3F, 0x42),
            success: Color::from_rgb8(0x57, 0xE3, 0x89),
            on_success: Color::from_rgb8(0x08, 0x2D, 0x18),
            warning: Color::from_rgb8(0xF8, 0xE4, 0x5C),
            on_warning: Color::from_rgb8(0x2D, 0x24, 0x04),
            error: Color::from_rgb8(0xF6, 0x61, 0x51),
            on_error: Color::from_rgb8(0x3B, 0x0A, 0x10),
            inverse_surface: Color::from_rgb8(0xFA, 0xFA, 0xFA),
            inverse_on_surface: Color::from_rgb8(0x24, 0x1F, 0x31),
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adwaita_surfaces_match_libadwaita_defaults() {
        let light = adwaita_palette(AdwaitaAccent::Blue, Brightness::Light);
        let dark = adwaita_palette(AdwaitaAccent::Blue, Brightness::Dark);
        assert_eq!(light.window_bg, Color::from_rgb8(0xFA, 0xFA, 0xFA));
        assert_eq!(light.header_bar_bg, Color::from_rgb8(0xEB, 0xEB, 0xEB));
        assert_eq!(dark.window_bg, Color::from_rgb8(0x24, 0x24, 0x24));
        assert_eq!(dark.header_bar_bg, Color::from_rgb8(0x30, 0x30, 0x30));
    }

    #[test]
    fn accent_presets_distinguish_hues() {
        assert_ne!(
            AdwaitaAccent::Blue.colors(Brightness::Light).0,
            AdwaitaAccent::Teal.colors(Brightness::Light).0
        );
        assert_ne!(
            AdwaitaAccent::Orange.colors(Brightness::Dark).0,
            AdwaitaAccent::Purple.colors(Brightness::Dark).0
        );
    }
}
