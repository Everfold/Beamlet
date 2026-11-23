//! Brightness polarity, responsive window breakpoints, and canonical spacing constants.

use masonry::layout::Length;

/// Color scheme brightness polarity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Brightness {
    /// Light background palette with dark ink.
    #[default]
    Light,
    /// Dark background palette with light ink.
    Dark,
}

impl Brightness {
    /// Returns the opposite brightness polarity.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

/// Responsive layout breakpoint classes (`Compact < 600`, `Medium < 840`, `Expanded >= 840`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowSizeClass {
    /// Narrow panes / phone-width (`< 600px`).
    Compact,
    /// Tablet / split-window (`600px..840px`).
    Medium,
    /// Standard desktop window (`>= 840px`).
    #[default]
    Expanded,
}

impl WindowSizeClass {
    /// Classifies logical viewport width into a [`WindowSizeClass`].
    #[must_use]
    pub fn from_width(width: f64) -> Self {
        if width < 600.0 {
            Self::Compact
        } else if width < 840.0 {
            Self::Medium
        } else {
            Self::Expanded
        }
    }

    /// Returns `true` when `Compact`.
    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    /// Returns `true` when `Medium`.
    #[must_use]
    pub const fn is_medium(self) -> bool {
        matches!(self, Self::Medium)
    }

    /// Returns `true` when `Expanded`.
    #[must_use]
    pub const fn is_expanded(self) -> bool {
        matches!(self, Self::Expanded)
    }
}

/// `2px` hairline layout spacing.
pub const SPACE_2: Length = Length::const_px(2.0);
/// `4px` micro spacing.
pub const SPACE_4: Length = Length::const_px(4.0);
/// `6px` dense desktop control spacing.
pub const SPACE_6: Length = Length::const_px(6.0);
/// `8px` compact spacing.
pub const SPACE_8: Length = Length::const_px(8.0);
/// `12px` standard intra-group spacing.
pub const SPACE_12: Length = Length::const_px(12.0);
/// `16px` section spacing.
pub const SPACE_16: Length = Length::const_px(16.0);
/// `20px` medium gutter spacing.
pub const SPACE_20: Length = Length::const_px(20.0);
/// `24px` container padding.
pub const SPACE_24: Length = Length::const_px(24.0);
/// `32px` wide layout spacing.
pub const SPACE_32: Length = Length::const_px(32.0);
