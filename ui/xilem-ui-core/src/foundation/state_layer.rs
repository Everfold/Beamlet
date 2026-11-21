//! Alpha blending and interactive state-color synthesis across `StateFeedbackMode`s.

use xilem::Color;
use xilem::masonry::palette::css::TRANSPARENT;

use crate::tokens::{Brightness, StateFeedbackMode};

/// Returns `color` with its alpha multiplied by `alpha` (`0.0..=1.0`).
#[must_use]
pub fn with_opacity(color: Color, alpha: f32) -> Color {
    color.multiply_alpha(alpha.clamp(0.0, 1.0))
}

/// Alpha-composites `overlay` (scaled by `overlay_alpha`) onto `base`.
#[must_use]
pub fn blend_over(base: Color, overlay: Color, overlay_alpha: f32) -> Color {
    let fg = overlay.to_rgba8();
    let bg = base.to_rgba8();
    let a = (f32::from(fg.a) / 255.0) * overlay_alpha.clamp(0.0, 1.0);
    let bg_a = f32::from(bg.a) / 255.0;
    let out_a = a + bg_a * (1.0 - a);
    if out_a <= 1e-6 {
        return TRANSPARENT;
    }

    let blend_chan = |fg_c: u8, bg_c: u8| -> u8 {
        let f = f32::from(fg_c) / 255.0;
        let b = f32::from(bg_c) / 255.0;
        let out = (f * a + b * bg_a * (1.0 - a)) / out_a;
        (out.clamp(0.0, 1.0) * 255.0).round() as u8
    };

    Color::from_rgba8(
        blend_chan(fg.r, bg.r),
        blend_chan(fg.g, bg.g),
        blend_chan(fg.b, bg.b),
        (out_a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Resolved `(rest, hover, active, disabled)` background colors for an interactive control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteractiveColors {
    /// Resting background fill.
    pub rest: Color,
    /// Hovered background fill.
    pub hover: Color,
    /// Active/pressed background fill.
    pub pressed: Color,
    /// Disabled background fill.
    pub disabled: Color,
}

/// Computes `(rest, hover, pressed, disabled)` fill colors for a control under `mode`.
#[must_use]
pub fn resolve_interactive_colors(
    base: Color,
    state_tint: Color,
    disabled_bg: Color,
    brightness: Brightness,
    mode: StateFeedbackMode,
) -> InteractiveColors {
    let neutral_overlay = match brightness {
        Brightness::Light => Color::BLACK,
        Brightness::Dark => Color::WHITE,
    };
    let (hover, pressed) = match mode {
        StateFeedbackMode::TintedStateLayer {
            hover_alpha,
            press_alpha,
        } => (
            blend_over(base, state_tint, hover_alpha),
            blend_over(base, state_tint, press_alpha),
        ),
        StateFeedbackMode::SubtleSurfaceAndBorder => {
            let subtle = match brightness {
                Brightness::Light => Color::from_rgb8(0x00, 0x00, 0x00),
                Brightness::Dark => Color::from_rgb8(0xFF, 0xFF, 0xFF),
            };
            (
                blend_over(base, subtle, 0.04),
                blend_over(base, subtle, 0.09),
            )
        }
        StateFeedbackMode::CrispShadowAndRing => (
            blend_over(base, neutral_overlay, 0.06),
            blend_over(base, neutral_overlay, 0.14),
        ),
        StateFeedbackMode::AlphaOverlay {
            hover_alpha,
            press_alpha,
        } => (
            blend_over(base, neutral_overlay, hover_alpha),
            blend_over(base, neutral_overlay, press_alpha),
        ),
    };
    InteractiveColors {
        rest: base,
        hover,
        pressed,
        disabled: disabled_bg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_over_handles_transparent_and_opaque_bases() {
        let transparent_blend = blend_over(TRANSPARENT, Color::WHITE, 0.5);
        let rgba = transparent_blend.to_rgba8();
        assert!(rgba.a > 120 && rgba.a < 135);
        assert_eq!((rgba.r, rgba.g, rgba.b), (255, 255, 255));

        let opaque_blend = blend_over(Color::WHITE, Color::BLACK, 0.10).to_rgba8();
        assert_eq!(opaque_blend.a, 255);
        assert!(opaque_blend.r < 235 && opaque_blend.r > 220);
    }

    #[test]
    fn resolve_interactive_colors_produces_distinct_states() {
        let mode = StateFeedbackMode::TintedStateLayer {
            hover_alpha: 0.08,
            press_alpha: 0.12,
        };
        let colors = resolve_interactive_colors(
            Color::WHITE,
            Color::BLACK,
            Color::from_rgba8(0, 0, 0, 30),
            Brightness::Light,
            mode,
        );
        assert_ne!(colors.rest, colors.hover);
        assert_ne!(colors.hover, colors.pressed);
    }
}
