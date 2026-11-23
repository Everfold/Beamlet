//! macOS Apple System Colors & Aqua Light/Dark palette presets.

use brocado_core::tokens::{Brightness, SemanticPalette};
use masonry::peniko::Color;

/// Apple System Settings accent color presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HigAccent {
    /// Apple System Blue (`#007AFF` light / `#0A84FF` dark).
    #[default]
    Blue,
    /// Apple System Purple (`#AF52DE` / `#BF5AF2`).
    Purple,
    /// Apple System Pink (`#FF2D55` / `#FF375F`).
    Pink,
    /// Apple System Red (`#FF3B30` / `#FF453A`).
    Red,
    /// Apple System Orange (`#FF9500` / `#FF9F0A`).
    Orange,
    /// Apple System Yellow (`#B58500` / `#FFD60A`).
    Yellow,
    /// Apple System Green (`#28CD41` / `#30D158`).
    Green,
    /// Apple System Graphite (`#8E8E93` / `#98989D`).
    Graphite,
}

impl HigAccent {
    /// Returns `(accent, accent_container)` tuned for macOS `brightness`.
    #[must_use]
    pub const fn colors(self, brightness: Brightness) -> (Color, Color) {
        match (self, brightness) {
            (Self::Blue, Brightness::Light) => (
                Color::from_rgb8(0x00, 0x7A, 0xFF),
                Color::from_rgb8(0xE1, 0xF0, 0xFF),
            ),
            (Self::Blue, Brightness::Dark) => (
                Color::from_rgb8(0x0A, 0x84, 0xFF),
                Color::from_rgb8(0x13, 0x35, 0x57),
            ),
            (Self::Purple, Brightness::Light) => (
                Color::from_rgb8(0xAF, 0x52, 0xDE),
                Color::from_rgb8(0xF5, 0xEA, 0xFA),
            ),
            (Self::Purple, Brightness::Dark) => (
                Color::from_rgb8(0xBF, 0x5A, 0xF2),
                Color::from_rgb8(0x3B, 0x20, 0x4A),
            ),
            (Self::Pink, Brightness::Light) => (
                Color::from_rgb8(0xFF, 0x2D, 0x55),
                Color::from_rgb8(0xFF, 0xE6, 0xEB),
            ),
            (Self::Pink, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0x37, 0x5F),
                Color::from_rgb8(0x4D, 0x19, 0x25),
            ),
            (Self::Red, Brightness::Light) => (
                Color::from_rgb8(0xFF, 0x3B, 0x30),
                Color::from_rgb8(0xFF, 0xE8, 0xE6),
            ),
            (Self::Red, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0x45, 0x3A),
                Color::from_rgb8(0x4B, 0x1B, 0x19),
            ),
            (Self::Orange, Brightness::Light) => (
                Color::from_rgb8(0xFF, 0x95, 0x00),
                Color::from_rgb8(0xFF, 0xF2, 0xDF),
            ),
            (Self::Orange, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0x9F, 0x0A),
                Color::from_rgb8(0x49, 0x2F, 0x0A),
            ),
            (Self::Yellow, Brightness::Light) => (
                Color::from_rgb8(0xB5, 0x85, 0x00),
                Color::from_rgb8(0xFC, 0xF3, 0xD6),
            ),
            (Self::Yellow, Brightness::Dark) => (
                Color::from_rgb8(0xFF, 0xD6, 0x0A),
                Color::from_rgb8(0x49, 0x3F, 0x0A),
            ),
            (Self::Green, Brightness::Light) => (
                Color::from_rgb8(0x28, 0xCD, 0x41),
                Color::from_rgb8(0xE4, 0xFA, 0xE8),
            ),
            (Self::Green, Brightness::Dark) => (
                Color::from_rgb8(0x30, 0xD1, 0x58),
                Color::from_rgb8(0x16, 0x40, 0x22),
            ),
            (Self::Graphite, Brightness::Light) => (
                Color::from_rgb8(0x6E, 0x6E, 0x73),
                Color::from_rgb8(0xEC, 0xEC, 0xEE),
            ),
            (Self::Graphite, Brightness::Dark) => (
                Color::from_rgb8(0x98, 0x98, 0x9D),
                Color::from_rgb8(0x34, 0x34, 0x37),
            ),
        }
    }
}

/// Synthesizes an Apple HIG Aqua [`SemanticPalette`] for `accent` and `brightness`.
#[must_use]
pub fn hig_palette(accent: HigAccent, brightness: Brightness) -> SemanticPalette {
    let (acc, acc_container) = accent.colors(brightness);
    match brightness {
        Brightness::Light => SemanticPalette {
            accent: acc,
            on_accent: Color::WHITE,
            accent_container: acc_container,
            on_accent_container: Color::from_rgb8(0x0A, 0x30, 0x62),
            secondary: Color::from_rgb8(0x6E, 0x6E, 0x73),
            on_secondary: Color::WHITE,
            secondary_container: Color::from_rgb8(0xDF, 0xDF, 0xE2),
            on_secondary_container: Color::from_rgb8(0x1D, 0x1D, 0x1F),
            tertiary_container: acc_container,
            on_tertiary_container: Color::from_rgb8(0x1D, 0x1D, 0x1F),
            window_bg: Color::from_rgb8(0xEC, 0xEC, 0xEC),
            surface: Color::WHITE,
            surface_low: Color::from_rgb8(0xF5, 0xF5, 0xF7),
            surface_container: Color::from_rgb8(0xEE, 0xEE, 0xF0),
            surface_high: Color::WHITE,
            surface_highest: Color::from_rgb8(0xE5, 0xE5, 0xEA),
            header_bar_bg: Color::from_rgb8(0xF6, 0xF6, 0xF6),
            sidebar_bg: Color::from_rgb8(0xE9, 0xE9, 0xEB),
            popup_bg: Color::from_rgb8(0xFC, 0xFC, 0xFD),
            text_primary: Color::from_rgb8(0x1D, 0x1D, 0x1F),
            text_secondary: Color::from_rgb8(0x6E, 0x6E, 0x73),
            border: Color::from_rgb8(0xD8, 0xD8, 0xDC),
            border_strong: Color::from_rgb8(0xAE, 0xAE, 0xB2),
            divider: Color::from_rgb8(0xE2, 0xE2, 0xE6),
            success: Color::from_rgb8(0x24, 0x8A, 0x3D),
            on_success: Color::WHITE,
            warning: Color::from_rgb8(0xC9, 0x34, 0x00),
            on_warning: Color::WHITE,
            error: Color::from_rgb8(0xFF, 0x3B, 0x30),
            on_error: Color::WHITE,
            inverse_surface: Color::from_rgb8(0x24, 0x24, 0x26),
            inverse_on_surface: Color::WHITE,
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
        Brightness::Dark => SemanticPalette {
            accent: acc,
            on_accent: Color::WHITE,
            accent_container: acc_container,
            on_accent_container: Color::from_rgb8(0xF5, 0xF5, 0xF7),
            secondary: Color::from_rgb8(0x98, 0x98, 0x9D),
            on_secondary: Color::from_rgb8(0x1E, 0x1E, 0x1E),
            secondary_container: Color::from_rgb8(0x3A, 0x3A, 0x3C),
            on_secondary_container: Color::WHITE,
            tertiary_container: acc_container,
            on_tertiary_container: Color::WHITE,
            window_bg: Color::from_rgb8(0x1E, 0x1E, 0x1E),
            surface: Color::from_rgb8(0x28, 0x28, 0x2A),
            surface_low: Color::from_rgb8(0x24, 0x24, 0x26),
            surface_container: Color::from_rgb8(0x2C, 0x2C, 0x2E),
            surface_high: Color::from_rgb8(0x32, 0x32, 0x35),
            surface_highest: Color::from_rgb8(0x3A, 0x3A, 0x3D),
            header_bar_bg: Color::from_rgb8(0x2B, 0x2B, 0x2D),
            sidebar_bg: Color::from_rgb8(0x25, 0x25, 0x27),
            popup_bg: Color::from_rgb8(0x30, 0x30, 0x33),
            text_primary: Color::from_rgb8(0xF5, 0xF5, 0xF7),
            text_secondary: Color::from_rgb8(0xA1, 0xA1, 0xA6),
            border: Color::from_rgb8(0x3D, 0x3D, 0x41),
            border_strong: Color::from_rgb8(0x6E, 0x6E, 0x73),
            divider: Color::from_rgb8(0x38, 0x38, 0x3B),
            success: Color::from_rgb8(0x30, 0xD1, 0x58),
            on_success: Color::from_rgb8(0x06, 0x29, 0x0E),
            warning: Color::from_rgb8(0xFF, 0x9F, 0x0A),
            on_warning: Color::from_rgb8(0x2E, 0x19, 0x00),
            error: Color::from_rgb8(0xFF, 0x45, 0x3A),
            on_error: Color::WHITE,
            inverse_surface: Color::from_rgb8(0xEC, 0xEC, 0xEC),
            inverse_on_surface: Color::from_rgb8(0x1D, 0x1D, 0x1F),
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hig_light_and_dark_match_aqua_surfaces() {
        let light = hig_palette(HigAccent::Blue, Brightness::Light);
        let dark = hig_palette(HigAccent::Blue, Brightness::Dark);
        assert_eq!(light.window_bg, Color::from_rgb8(0xEC, 0xEC, 0xEC));
        assert_eq!(light.accent, Color::from_rgb8(0x00, 0x7A, 0xFF));
        assert_eq!(dark.window_bg, Color::from_rgb8(0x1E, 0x1E, 0x1E));
        assert_eq!(dark.accent, Color::from_rgb8(0x0A, 0x84, 0xFF));
    }

    #[test]
    fn all_accents_yield_distinct_colors() {
        let blue = HigAccent::Blue.colors(Brightness::Light);
        let green = HigAccent::Green.colors(Brightness::Light);
        let pink = HigAccent::Pink.colors(Brightness::Light);
        assert_ne!(blue.0, green.0);
        assert_ne!(blue.0, pink.0);
    }
}
