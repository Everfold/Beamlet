//! Material Design 3 Expressive geometry constants, state-layer opacities, and elevation tokens.

use masonry::kurbo::Point;
use masonry::layout::Length;
use masonry::peniko::Color;
use masonry::properties::BoxShadow;

use crate::foundation::state_layer::{blend_over, with_opacity};

/// Minimum standard button height (`40px`).
pub const BUTTON_HEIGHT: f64 = 40.0;
/// Resting Material 3 Expressive pill button corner radius (`999px`).
pub const BUTTON_RADIUS: f64 = 999.0;
/// Active/pressed Material 3 Expressive button corner radius morph (`12px`).
pub const ACTIVE_BUTTON_RADIUS: f64 = 12.0;
/// Standard horizontal padding inside buttons (`24px`).
pub const BUTTON_PAD_H: f64 = 24.0;

/// Standard icon button box size (`40px`).
pub const ICON_BTN_SIZE: f64 = 40.0;
/// Standard circular icon button corner radius (`999px`).
pub const ICON_BTN_RADIUS: f64 = 999.0;

/// Standard chip height (`32px`).
pub const CHIP_HEIGHT: f64 = 32.0;
/// Standard chip corner radius (`8px`).
pub const CHIP_RADIUS: f64 = 8.0;

/// Text input corner radius (`12px`).
pub const INPUT_RADIUS: f64 = 12.0;
/// Text input vertical padding (`12px`).
pub const INPUT_PAD_V: f64 = 12.0;
/// Text input horizontal padding (`16px`).
pub const INPUT_PAD_H: f64 = 16.0;

/// Search bar height (`56px`).
pub const SEARCH_HEIGHT: f64 = 56.0;
/// Search bar pill corner radius (`999px`).
pub const SEARCH_RADIUS: f64 = 999.0;

/// Card corner radius (`16px`).
pub const CARD_RADIUS: f64 = 16.0;
/// Standard card interior padding (`16px`).
pub const CARD_PAD: f64 = 16.0;

/// Modal dialog corner radius (`28px`).
pub const DIALOG_RADIUS: f64 = 28.0;
/// Bottom/side sheet corner radius (`28px`).
pub const SHEET_RADIUS: f64 = 28.0;
/// Popup menu / select popover corner radius (`12px`).
pub const MENU_RADIUS: f64 = 12.0;

/// Compact top app bar height (`64px`).
pub const TOP_BAR_HEIGHT: f64 = 64.0;
/// Bottom navigation / app bar height (`80px`).
pub const BOTTOM_BAR_HEIGHT: f64 = 80.0;
/// Active navigation indicator pill corner radius (`999px`).
pub const NAV_PILL_RADIUS: f64 = 999.0;
/// Default expanded navigation drawer / sidebar width (`260px`).
pub const SIDEBAR_WIDTH: f64 = 260.0;

/// Switch track height (`32px`).
pub const SWITCH_HEIGHT: f64 = 32.0;
/// Switch track width (`52px`).
pub const SWITCH_WIDTH: f64 = 52.0;
/// Checkbox corner radius (`2px`).
pub const CHECKBOX_RADIUS: f64 = 2.0;

/// Material 3 hover state-layer opacity (`8%`).
pub const HOVER_ALPHA: f64 = 0.08;
/// Material 3 press/active state-layer opacity (`12%`).
pub const PRESS_ALPHA: f64 = 0.12;

/// Surface elevation levels (`Level0..Level5`) producing tonal fill & soft shadows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElevationLevel {
    /// Flat `0px` elevation.
    #[default]
    Level0,
    /// `1px` subtle lift (cards, toolbars).
    Level1,
    /// `3px` menu / popover / tooltip elevation.
    Level2,
    /// `6px` modal dialog elevation.
    Level3,
    /// `8px` elevated navigation / overlay elevation.
    Level4,
    /// `12px` prominent floating elevation.
    Level5,
}

impl ElevationLevel {
    /// Returns the Masonry [`BoxShadow`] approximation for this elevation level.
    #[must_use]
    pub fn box_shadow(self, shadow_color: Color) -> BoxShadow {
        match self {
            Self::Level0 => BoxShadow::default(),
            Self::Level1 => BoxShadow::new(with_opacity(shadow_color, 0.14), Point::new(0.0, 1.0))
                .blur(Length::px(3.0)),
            Self::Level2 => BoxShadow::new(with_opacity(shadow_color, 0.18), Point::new(0.0, 2.0))
                .blur(Length::px(6.0)),
            Self::Level3 => BoxShadow::new(with_opacity(shadow_color, 0.22), Point::new(0.0, 4.0))
                .blur(Length::px(10.0)),
            Self::Level4 => BoxShadow::new(with_opacity(shadow_color, 0.24), Point::new(0.0, 6.0))
                .blur(Length::px(14.0)),
            Self::Level5 => BoxShadow::new(with_opacity(shadow_color, 0.26), Point::new(0.0, 8.0))
                .blur(Length::px(18.0)),
        }
    }

    /// Blends a Material 3 primary surface tint overlay (`0%..14%`) onto `surface`.
    #[must_use]
    pub fn tinted_surface(self, surface: Color, accent: Color) -> Color {
        let opacity = match self {
            Self::Level0 => 0.0,
            Self::Level1 => 0.05,
            Self::Level2 => 0.08,
            Self::Level3 => 0.11,
            Self::Level4 => 0.12,
            Self::Level5 => 0.14,
        };
        if opacity == 0.0 {
            surface
        } else {
            blend_over(surface, accent, opacity)
        }
    }
}
