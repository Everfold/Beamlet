//! Multi-theme design system specification protocol (`DesignSpec`).

use xilem::Color;
use xilem::kurbo::Point;
use xilem::masonry::layout::Length;
use xilem::masonry::properties::BoxShadow;

use crate::foundation::state_layer::{blend_over, with_opacity};
use crate::tokens::{Brightness, TypeScale};

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

    /// Blends an MD3-style primary surface tint overlay (`0%..14%`) onto `surface`.
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

/// Platform-agnostic semantic color palette consumed by all shared components and Masonry style rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SemanticPalette {
    /// Primary brand/accent fill color.
    pub accent: Color,
    /// Ink painted over [`accent`](Self::accent).
    pub on_accent: Color,
    /// Subtle container tint derived from `accent`.
    pub accent_container: Color,
    /// Ink painted over [`accent_container`](Self::accent_container).
    pub on_accent_container: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Ink painted over [`secondary`](Self::secondary).
    pub on_secondary: Color,
    /// Active indicator pill / selected chip container fill.
    pub secondary_container: Color,
    /// Ink painted over [`secondary_container`](Self::secondary_container).
    pub on_secondary_container: Color,
    /// Tertiary highlight fill (e.g. badges, time AM/PM active state).
    pub tertiary_container: Color,
    /// Ink over [`tertiary_container`](Self::tertiary_container).
    pub on_tertiary_container: Color,
    /// Backdrop window background (`Mica` / Aqua window / Adwaita window).
    pub window_bg: Color,
    /// Primary widget content surface.
    pub surface: Color,
    /// Low-emphasis card / drawer surface.
    pub surface_low: Color,
    /// Default toolbar / bar / container surface.
    pub surface_container: Color,
    /// Raised dialog / picker / search surface.
    pub surface_high: Color,
    /// Input filled background / inactive slider track.
    pub surface_highest: Color,
    /// Top header/toolbar surface color.
    pub header_bar_bg: Color,
    /// Navigation sidebar/rail surface color.
    pub sidebar_bg: Color,
    /// Floating menu / select popup surface color.
    pub popup_bg: Color,
    /// Primary body and heading text color.
    pub text_primary: Color,
    /// Secondary caption and placeholder text color.
    pub text_secondary: Color,
    /// Hairline subtle border for cards and inputs.
    pub border: Color,
    /// Strong border for switches, checkboxes, outlined buttons.
    pub border_strong: Color,
    /// Hairline horizontal/vertical divider color.
    pub divider: Color,
    /// Destructive / validation error color.
    pub error: Color,
    /// Ink over [`error`](Self::error).
    pub on_error: Color,
    /// High-contrast toast / plain tooltip background.
    pub inverse_surface: Color,
    /// Ink over [`inverse_surface`](Self::inverse_surface).
    pub inverse_on_surface: Color,
    /// Modal scrim base color (rendered with partial alpha).
    pub scrim: Color,
    /// Elevation shadow base color.
    pub shadow: Color,
}

/// Geometry and density parameters distinguishing MD3, Fluent, macOS HIG, and Adwaita.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlMetrics {
    /// Minimum button height (`40` MD3, `32` Fluent, `26` HIG, `34` Adwaita).
    pub button_height: f64,
    /// Standard button corner radius.
    pub button_radius: f64,
    /// Standard button horizontal padding.
    pub button_pad_h: f64,
    /// Standard icon button square size (`40` MD3, `32` Fluent/Adwaita, `26` HIG).
    pub icon_btn_size: f64,
    /// Icon button corner radius.
    pub icon_btn_radius: f64,
    /// Chip height (`32` MD3, `28` Fluent, `22` HIG, `30` Adwaita).
    pub chip_height: f64,
    /// Chip corner radius.
    pub chip_radius: f64,
    /// Text input box corner radius.
    pub input_radius: f64,
    /// Text input vertical padding.
    pub input_pad_v: f64,
    /// Text input horizontal padding.
    pub input_pad_h: f64,
    /// Pill / desktop search bar height.
    pub search_height: f64,
    /// Search bar corner radius.
    pub search_radius: f64,
    /// Standard card / grouped list container corner radius.
    pub card_radius: f64,
    /// Standard card interior padding.
    pub card_pad: f64,
    /// Modal dialog card corner radius.
    pub dialog_radius: f64,
    /// Bottom/side sheet corner radius.
    pub sheet_radius: f64,
    /// Dropdown menu and popup corner radius.
    pub menu_radius: f64,
    /// Top app bar / header bar / toolbar height (`64` MD3, `48` Fluent, `38` HIG, `46` Adwaita).
    pub top_bar_height: f64,
    /// Bottom navigation bar height (`80` MD3, `56` desktop themes).
    pub bottom_bar_height: f64,
    /// Navigation item highlight corner radius.
    pub nav_pill_radius: f64,
    /// Navigation drawer / source list sidebar width (`260` MD3, `280` Fluent, `230` HIG, `260` Adwaita).
    pub sidebar_width: f64,
    /// Toggle switch track height (`32` MD3, `20` Fluent/HIG, `26` Adwaita).
    pub switch_height: f64,
    /// Toggle switch track width (`52` MD3, `40` Fluent, `38` HIG, `48` Adwaita).
    pub switch_width: f64,
    /// Checkbox box corner radius (`2` MD3, `4` Fluent/Adwaita, `5` HIG).
    pub checkbox_radius: f64,
}

/// Hover and press visual grammar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StateFeedbackMode {
    /// Material 3 tinted state layer blended over base fill (`8%` hover, `12%` active).
    TintedStateLayer {
        /// Opacity blended on pointer hover.
        hover_alpha: f32,
        /// Opacity blended on pointer press.
        press_alpha: f32,
    },
    /// Windows Fluent subtle white/black tint with beveled bottom border emphasis.
    SubtleSurfaceAndBorder,
    /// macOS HIG crisp surface with soft elevation and darkened solid accent on press.
    CrispShadowAndRing,
    /// GNOME Adwaita flat current-color alpha veil (`7%` hover, `14%` active).
    AlphaOverlay {
        /// Opacity blended on pointer hover.
        hover_alpha: f32,
        /// Opacity blended on pointer press.
        press_alpha: f32,
    },
}

/// Focused text-input border grammar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputFocusStyle {
    /// Material 3 / Adwaita `2px` perimeter outline.
    #[default]
    Outline2px,
    /// Windows Fluent `2px` accent bar flush along the bottom edge of the input container.
    BottomAccentLine2px,
    /// macOS HIG soft `3px` semi-transparent outer accent halo ring.
    OuterHalo3px,
}

/// Tab selection indicator grammar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabIndicatorStyle {
    /// Material 3 / Adwaita full-width underline indicator.
    Underline {
        /// Indicator stroke thickness (`3px` primary, `2px` secondary).
        height: f64,
    },
    /// Windows Fluent centered `3px` short accent pill.
    ShortPill,
    /// macOS HIG / Adwaita segmented capsule thumb behind the active tab label.
    SegmentedThumb,
}

impl Default for TabIndicatorStyle {
    fn default() -> Self {
        Self::Underline { height: 3.0 }
    }
}

/// Sidebar/drawer navigation active indicator grammar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NavIndicatorStyle {
    /// Material 3 full rounded capsule (`secondary_container` fill).
    #[default]
    PillBackground,
    /// Windows Fluent `NavigationViewItem`: subtle row fill + `3px × 16px` leading accent bar.
    LeadingAccentBar,
    /// macOS Sidebar / Adwaita soft rounded selection row.
    RoundedRow,
}

/// Complete cross-theme design specification passed through [`ui_scope`](crate::style_engine::ui_scope).
#[derive(Clone, Debug, PartialEq)]
pub struct DesignSpec {
    /// Human-readable theme identifier (e.g. `"Material 3"`, `"Windows Fluent"`, `"Apple HIG"`, `"GNOME Adwaita"`).
    pub name: &'static str,
    /// Light or Dark polarity.
    pub brightness: Brightness,
    /// Semantic color tokens.
    pub palette: SemanticPalette,
    /// Control geometry & density tokens.
    pub metrics: ControlMetrics,
    /// Hover/press state synthesis strategy.
    pub feedback: StateFeedbackMode,
    /// Text-input focus indicator strategy.
    pub input_focus: InputFocusStyle,
    /// Tab selection indicator strategy.
    pub tab_indicator: TabIndicatorStyle,
    /// Sidebar active item indicator strategy.
    pub nav_indicator: NavIndicatorStyle,
    /// 15-role typography ramp and optional font family.
    pub typography: TypeScale,
}

impl DesignSpec {
    /// Neutral fallback specification used if a component renders outside [`ui_scope`](crate::style_engine::ui_scope).
    #[must_use]
    pub fn neutral_fallback(brightness: Brightness) -> Self {
        let palette = match brightness {
            Brightness::Light => SemanticPalette {
                accent: Color::from_rgb8(0x2F, 0x6F, 0xED),
                on_accent: Color::WHITE,
                accent_container: Color::from_rgb8(0xDC, 0xE7, 0xFF),
                on_accent_container: Color::from_rgb8(0x0B, 0x26, 0x5C),
                secondary: Color::from_rgb8(0x52, 0x60, 0x70),
                on_secondary: Color::WHITE,
                secondary_container: Color::from_rgb8(0xE2, 0xE8, 0xF0),
                on_secondary_container: Color::from_rgb8(0x16, 0x1D, 0x26),
                tertiary_container: Color::from_rgb8(0xEB, 0xDE, 0xFF),
                on_tertiary_container: Color::from_rgb8(0x29, 0x13, 0x4D),
                window_bg: Color::from_rgb8(0xF6, 0xF7, 0xF9),
                surface: Color::WHITE,
                surface_low: Color::from_rgb8(0xF3, 0xF5, 0xF8),
                surface_container: Color::from_rgb8(0xEB, 0xEE, 0xF3),
                surface_high: Color::from_rgb8(0xFA, 0xFB, 0xFD),
                surface_highest: Color::from_rgb8(0xE3, 0xE7, 0xEE),
                header_bar_bg: Color::from_rgb8(0xEE, 0xF1, 0xF5),
                sidebar_bg: Color::from_rgb8(0xF2, 0xF4, 0xF8),
                popup_bg: Color::WHITE,
                text_primary: Color::from_rgb8(0x18, 0x1A, 0x1E),
                text_secondary: Color::from_rgb8(0x59, 0x60, 0x6A),
                border: Color::from_rgb8(0xD5, 0xDA, 0xE2),
                border_strong: Color::from_rgb8(0x88, 0x90, 0x9B),
                divider: Color::from_rgb8(0xE0, 0xE4, 0xEC),
                error: Color::from_rgb8(0xC6, 0x28, 0x28),
                on_error: Color::WHITE,
                inverse_surface: Color::from_rgb8(0x27, 0x2A, 0x30),
                inverse_on_surface: Color::from_rgb8(0xF3, 0xF5, 0xF8),
                scrim: Color::BLACK,
                shadow: Color::BLACK,
            },
            Brightness::Dark => SemanticPalette {
                accent: Color::from_rgb8(0x78, 0xA8, 0xFF),
                on_accent: Color::from_rgb8(0x09, 0x1E, 0x42),
                accent_container: Color::from_rgb8(0x1E, 0x3A, 0x6B),
                on_accent_container: Color::from_rgb8(0xDF, 0xEA, 0xFF),
                secondary: Color::from_rgb8(0xA7, 0xB4, 0xC4),
                on_secondary: Color::from_rgb8(0x1A, 0x20, 0x28),
                secondary_container: Color::from_rgb8(0x30, 0x38, 0x45),
                on_secondary_container: Color::from_rgb8(0xE4, 0xE9, 0xF0),
                tertiary_container: Color::from_rgb8(0x43, 0x2C, 0x66),
                on_tertiary_container: Color::from_rgb8(0xEF, 0xE5, 0xFF),
                window_bg: Color::from_rgb8(0x18, 0x1A, 0x1E),
                surface: Color::from_rgb8(0x20, 0x23, 0x28),
                surface_low: Color::from_rgb8(0x24, 0x27, 0x2D),
                surface_container: Color::from_rgb8(0x29, 0x2C, 0x33),
                surface_high: Color::from_rgb8(0x2E, 0x32, 0x3A),
                surface_highest: Color::from_rgb8(0x38, 0x3C, 0x45),
                header_bar_bg: Color::from_rgb8(0x23, 0x26, 0x2B),
                sidebar_bg: Color::from_rgb8(0x1E, 0x21, 0x26),
                popup_bg: Color::from_rgb8(0x2C, 0x30, 0x37),
                text_primary: Color::from_rgb8(0xEE, 0xF1, 0xF5),
                text_secondary: Color::from_rgb8(0xA6, 0xAD, 0xB8),
                border: Color::from_rgb8(0x3A, 0x3F, 0x48),
                border_strong: Color::from_rgb8(0x6E, 0x76, 0x84),
                divider: Color::from_rgb8(0x35, 0x39, 0x42),
                error: Color::from_rgb8(0xF2, 0x8B, 0x82),
                on_error: Color::from_rgb8(0x42, 0x0A, 0x0A),
                inverse_surface: Color::from_rgb8(0xE9, 0xEC, 0xF2),
                inverse_on_surface: Color::from_rgb8(0x1E, 0x20, 0x24),
                scrim: Color::BLACK,
                shadow: Color::BLACK,
            },
        };
        Self {
            name: "Neutral",
            brightness,
            palette,
            metrics: ControlMetrics {
                button_height: 36.0,
                button_radius: 8.0,
                button_pad_h: 16.0,
                icon_btn_size: 36.0,
                icon_btn_radius: 8.0,
                chip_height: 30.0,
                chip_radius: 8.0,
                input_radius: 6.0,
                input_pad_v: 8.0,
                input_pad_h: 12.0,
                search_height: 44.0,
                search_radius: 22.0,
                card_radius: 10.0,
                card_pad: 16.0,
                dialog_radius: 12.0,
                sheet_radius: 14.0,
                menu_radius: 8.0,
                top_bar_height: 52.0,
                bottom_bar_height: 64.0,
                nav_pill_radius: 8.0,
                sidebar_width: 260.0,
                switch_height: 26.0,
                switch_width: 46.0,
                checkbox_radius: 4.0,
            },
            feedback: StateFeedbackMode::AlphaOverlay {
                hover_alpha: 0.08,
                press_alpha: 0.14,
            },
            input_focus: InputFocusStyle::Outline2px,
            tab_indicator: TabIndicatorStyle::Underline { height: 2.0 },
            nav_indicator: NavIndicatorStyle::RoundedRow,
            typography: TypeScale::baseline(),
        }
    }

    /// Standard disabled foreground/ink color (`38%` `text_primary`).
    #[must_use]
    pub fn disabled_content(&self) -> Color {
        with_opacity(self.palette.text_primary, 0.38)
    }

    /// Standard disabled container background fill (`12%` `text_primary`).
    #[must_use]
    pub fn disabled_container(&self) -> Color {
        with_opacity(self.palette.text_primary, 0.12)
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

    /// Computes box shadow for `level` using `self.palette.shadow`.
    #[must_use]
    pub fn box_shadow(&self, level: ElevationLevel) -> BoxShadow {
        level.box_shadow(self.palette.shadow)
    }
}
