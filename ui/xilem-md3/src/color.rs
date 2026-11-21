//! Perceptual OKLCH dynamic Material Design 3 `ColorScheme` generator and contrast helpers.

use xilem::Color;
use xilem_ui_core::tokens::{Brightness, SemanticPalette};

/// Accessibility contrast tier for dynamic Material 3 palettes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ContrastLevel {
    /// Default Material 3 tonal contrast.
    #[default]
    Standard,
    /// Boosted mid-tier contrast (~4.5:1 -> ~6.0:1).
    Medium,
    /// High-contrast accessibility tier (~7.0:1+ primary pairs).
    High,
}

/// Material Design 3 semantic color roles derived from an arbitrary seed color via OKLCH.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
    /// Polarity (Light or Dark).
    pub brightness: Brightness,
    /// Contrast tier.
    pub contrast: ContrastLevel,
    /// Primary brand color.
    pub primary: Color,
    /// Ink painted on [`primary`](Self::primary).
    pub on_primary: Color,
    /// Tonal container derived from `primary`.
    pub primary_container: Color,
    /// Ink painted on [`primary_container`](Self::primary_container).
    pub on_primary_container: Color,
    /// Secondary accent role.
    pub secondary: Color,
    /// Ink painted on [`secondary`](Self::secondary).
    pub on_secondary: Color,
    /// Active indicator pill / selected state fill.
    pub secondary_container: Color,
    /// Ink painted on [`secondary_container`](Self::secondary_container).
    pub on_secondary_container: Color,
    /// Complementary tertiary highlight role.
    pub tertiary: Color,
    /// Ink painted on [`tertiary`](Self::tertiary).
    pub on_tertiary: Color,
    /// Tertiary tonal container fill.
    pub tertiary_container: Color,
    /// Ink painted on [`tertiary_container`](Self::tertiary_container).
    pub on_tertiary_container: Color,
    /// Validation / destructive error color.
    pub error: Color,
    /// Ink painted on [`error`](Self::error).
    pub on_error: Color,
    /// Error container fill.
    pub error_container: Color,
    /// Ink on error container.
    pub on_error_container: Color,
    /// Base canvas surface.
    pub surface: Color,
    /// Lowest elevation surface container.
    pub surface_container_lowest: Color,
    /// Low elevation surface container.
    pub surface_container_low: Color,
    /// Standard elevation surface container.
    pub surface_container: Color,
    /// Elevated container fill (cards, dialogs, sheets).
    pub surface_container_high: Color,
    /// Highest container fill (filled text inputs, slider tracks).
    pub surface_container_highest: Color,
    /// Primary ink on surfaces.
    pub on_surface: Color,
    /// Secondary muted ink on surfaces.
    pub on_surface_variant: Color,
    /// Strong structural outline (outlined inputs, switches).
    pub outline: Color,
    /// Subtle structural hairline divider/border.
    pub outline_variant: Color,
    /// High-contrast inverse fill (tooltips, snackbars).
    pub inverse_surface: Color,
    /// Ink painted on [`inverse_surface`](Self::inverse_surface).
    pub inverse_on_surface: Color,
    /// Modal scrim base.
    pub scrim: Color,
    /// Drop shadow base.
    pub shadow: Color,
}

impl ColorScheme {
    /// Generates a full Material 3 color scheme from `seed`, `brightness`, and `contrast`.
    #[must_use]
    pub fn from_seed(seed: Color, brightness: Brightness, contrast: ContrastLevel) -> Self {
        let (_, seed_c, hue) = srgb_to_oklch(seed);
        let c_primary = seed_c.clamp(0.11, 0.22);
        let c_secondary = (c_primary * 0.40).clamp(0.03, 0.08);
        let c_tertiary = (c_primary * 0.72).clamp(0.06, 0.16);
        let hue_tertiary = (hue + 60.0).rem_euclid(360.0);
        let c_neutral = (c_primary * 0.08).clamp(0.008, 0.025);
        let c_neutral_var = (c_primary * 0.14).clamp(0.014, 0.045);

        let boost: f64 = match contrast {
            ContrastLevel::Standard => 0.0,
            ContrastLevel::Medium => 0.06,
            ContrastLevel::High => 0.12,
        };

        let (
            primary,
            on_primary,
            primary_container,
            on_primary_container,
            secondary,
            on_secondary,
            secondary_container,
            on_secondary_container,
            tertiary,
            on_tertiary,
            tertiary_container,
            on_tertiary_container,
            error,
            on_error,
            error_container,
            on_error_container,
            surface,
            surface_lowest,
            surface_low,
            surface_container,
            surface_high,
            surface_highest,
            on_surface,
            on_surface_variant,
            outline,
            outline_variant,
            inverse_surface,
            inverse_on_surface,
        ) = match brightness {
            Brightness::Light => (
                oklch((0.50 - boost).max(0.32), c_primary, hue),
                Color::WHITE,
                oklch(0.90, c_primary * 0.45, hue),
                oklch((0.24 - boost).max(0.10), c_primary * 0.85, hue),
                oklch((0.52 - boost).max(0.34), c_secondary, hue),
                Color::WHITE,
                oklch(0.91, c_secondary * 0.75, hue),
                oklch((0.24 - boost).max(0.10), c_secondary * 0.9, hue),
                oklch((0.52 - boost).max(0.34), c_tertiary, hue_tertiary),
                Color::WHITE,
                oklch(0.91, c_tertiary * 0.48, hue_tertiary),
                oklch((0.24 - boost).max(0.10), c_tertiary * 0.85, hue_tertiary),
                oklch((0.51 - boost).max(0.34), 0.18, 25.0),
                Color::WHITE,
                oklch(0.92, 0.05, 25.0),
                oklch(0.24, 0.14, 25.0),
                oklch(0.985, c_neutral, hue),
                Color::WHITE,
                oklch(0.968, c_neutral, hue),
                oklch(0.950, c_neutral, hue),
                oklch(0.932, c_neutral, hue),
                oklch(0.912, c_neutral, hue),
                oklch((0.21 - boost).max(0.08), c_neutral_var, hue),
                oklch((0.43 - boost).max(0.24), c_neutral_var, hue),
                oklch((0.56 - boost).max(0.34), c_neutral_var, hue),
                oklch((0.81 - boost * 0.5).max(0.55), c_neutral_var, hue),
                oklch(0.26, c_neutral, hue),
                oklch(0.96, c_neutral, hue),
            ),
            Brightness::Dark => (
                oklch((0.78 + boost * 0.6).min(0.92), c_primary * 0.82, hue),
                oklch((0.23 - boost * 0.5).max(0.10), c_primary * 0.75, hue),
                oklch((0.36 + boost * 0.2).min(0.45), c_primary * 0.72, hue),
                oklch(0.92, c_primary * 0.40, hue),
                oklch((0.79 + boost * 0.6).min(0.92), c_secondary, hue),
                oklch((0.24 - boost * 0.5).max(0.10), c_secondary, hue),
                oklch(0.35, c_secondary * 1.1, hue),
                oklch(0.92, c_secondary * 0.7, hue),
                oklch(
                    (0.80 + boost * 0.6).min(0.92),
                    c_tertiary * 0.78,
                    hue_tertiary,
                ),
                oklch(
                    (0.24 - boost * 0.5).max(0.10),
                    c_tertiary * 0.7,
                    hue_tertiary,
                ),
                oklch(0.36, c_tertiary * 0.65, hue_tertiary),
                oklch(0.92, c_tertiary * 0.40, hue_tertiary),
                oklch((0.77 + boost * 0.5).min(0.90), 0.14, 25.0),
                oklch(0.22, 0.12, 25.0),
                oklch(0.36, 0.13, 25.0),
                oklch(0.93, 0.05, 25.0),
                oklch(0.18, c_neutral, hue),
                oklch(0.14, c_neutral, hue),
                oklch(0.21, c_neutral, hue),
                oklch(0.24, c_neutral, hue),
                oklch(0.27, c_neutral, hue),
                oklch(0.31, c_neutral, hue),
                oklch((0.92 + boost * 0.4).min(0.98), c_neutral, hue),
                oklch((0.76 + boost * 0.5).min(0.90), c_neutral_var, hue),
                oklch((0.58 + boost * 0.6).min(0.78), c_neutral_var, hue),
                oklch((0.36 + boost * 0.5).min(0.55), c_neutral_var, hue),
                oklch(0.92, c_neutral, hue),
                oklch(0.24, c_neutral, hue),
            ),
        };

        Self {
            brightness,
            contrast,
            primary,
            on_primary,
            primary_container,
            on_primary_container,
            secondary,
            on_secondary,
            secondary_container,
            on_secondary_container,
            tertiary,
            on_tertiary,
            tertiary_container,
            on_tertiary_container,
            error,
            on_error,
            error_container,
            on_error_container,
            surface,
            surface_container_lowest: surface_lowest,
            surface_container_low: surface_low,
            surface_container,
            surface_container_high: surface_high,
            surface_container_highest: surface_highest,
            on_surface,
            on_surface_variant,
            outline,
            outline_variant,
            inverse_surface,
            inverse_on_surface,
            scrim: Color::BLACK,
            shadow: Color::BLACK,
        }
    }

    /// Baseline Material 3 light scheme from Material purple seed `#6750A4`.
    #[must_use]
    pub fn baseline_light() -> Self {
        Self::from_seed(
            Color::from_rgb8(0x67, 0x50, 0xA4),
            Brightness::Light,
            ContrastLevel::Standard,
        )
    }

    /// Baseline Material 3 dark scheme from Material purple seed `#6750A4`.
    #[must_use]
    pub fn baseline_dark() -> Self {
        Self::from_seed(
            Color::from_rgb8(0x67, 0x50, 0xA4),
            Brightness::Dark,
            ContrastLevel::Standard,
        )
    }

    /// Converts this MD3 scheme into the cross-theme [`SemanticPalette`].
    #[must_use]
    pub fn to_semantic_palette(&self) -> SemanticPalette {
        SemanticPalette {
            accent: self.primary,
            on_accent: self.on_primary,
            accent_container: self.primary_container,
            on_accent_container: self.on_primary_container,
            secondary: self.secondary,
            on_secondary: self.on_secondary,
            secondary_container: self.secondary_container,
            on_secondary_container: self.on_secondary_container,
            tertiary_container: self.tertiary_container,
            on_tertiary_container: self.on_tertiary_container,
            window_bg: self.surface,
            surface: self.surface,
            surface_low: self.surface_container_low,
            surface_container: self.surface_container,
            surface_high: self.surface_container_high,
            surface_highest: self.surface_container_highest,
            header_bar_bg: self.surface_container,
            sidebar_bg: self.surface_container_low,
            popup_bg: self.surface_container_high,
            text_primary: self.on_surface,
            text_secondary: self.on_surface_variant,
            border: self.outline_variant,
            border_strong: self.outline,
            divider: self.outline_variant,
            error: self.error,
            on_error: self.on_error,
            inverse_surface: self.inverse_surface,
            inverse_on_surface: self.inverse_on_surface,
            scrim: self.scrim,
            shadow: self.shadow,
        }
    }
}

/// Computes WCAG 2.1 relative luminance (`0.0..=1.0`).
#[must_use]
pub fn relative_luminance(color: Color) -> f64 {
    let rgba = color.to_rgba8();
    let lin = |c: u8| {
        let s = f64::from(c) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgba.r) + 0.7152 * lin(rgba.g) + 0.0722 * lin(rgba.b)
}

/// Computes WCAG 2.1 contrast ratio (`1.0..=21.0`) between `fg` and `bg`.
#[must_use]
pub fn contrast_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn srgb_channel_to_linear(c: u8) -> f64 {
    let s = f64::from(c) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(v: f64) -> u8 {
    let clamped = v.clamp(0.0, 1.0);
    let s = if clamped <= 0.003_130_8 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn srgb_to_oklch(color: Color) -> (f64, f64, f64) {
    let rgba = color.to_rgba8();
    let r = srgb_channel_to_linear(rgba.r);
    let g = srgb_channel_to_linear(rgba.g);
    let b = srgb_channel_to_linear(rgba.b);

    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();

    let ok_l = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let ok_a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let ok_b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;

    let chroma = ok_a.hypot(ok_b);
    let hue = ok_b.atan2(ok_a).to_degrees().rem_euclid(360.0);
    (ok_l, chroma, hue)
}

fn oklch(l: f64, c: f64, h_deg: f64) -> Color {
    let h = h_deg.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();

    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let b_ch = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3;

    Color::from_rgb8(
        linear_to_srgb_u8(r),
        linear_to_srgb_u8(g),
        linear_to_srgb_u8(b_ch),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_light_and_dark_meet_wcag_aa() {
        let light = ColorScheme::baseline_light();
        let dark = ColorScheme::baseline_dark();

        assert!(contrast_ratio(light.on_surface, light.surface) >= 4.5);
        assert!(contrast_ratio(light.on_primary, light.primary) >= 4.5);
        assert!(contrast_ratio(light.on_error, light.error) >= 4.5);
        assert!(contrast_ratio(dark.on_surface, dark.surface) >= 4.5);
        assert!(contrast_ratio(dark.on_primary, dark.primary) >= 4.5);
    }

    #[test]
    fn high_contrast_increases_foreground_background_separation() {
        let seed = Color::from_rgb8(0x00, 0x6A, 0x60);
        let std = ColorScheme::from_seed(seed, Brightness::Light, ContrastLevel::Standard);
        let high = ColorScheme::from_seed(seed, Brightness::Light, ContrastLevel::High);

        assert!(
            contrast_ratio(high.on_primary, high.primary)
                > contrast_ratio(std.on_primary, std.primary)
        );
        assert!(
            contrast_ratio(high.on_surface, high.surface)
                > contrast_ratio(std.on_surface, std.surface)
        );
    }
}
