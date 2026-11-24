//! Shared 15-role typography ramp, line-height/tracking tokens, and platform font stacks.

use std::borrow::Cow;

use masonry::parley::style::{FontFamily, FontFamilyName, FontWeight};
use masonry::parley::{GenericFamily, LineHeight};
use xilem_masonry::view::Label;

/// The 15 semantic typography roles used across component specifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeRole {
    /// Hero display title (`57sp` baseline).
    DisplayLarge,
    /// Large display title (`45sp` baseline).
    DisplayMedium,
    /// Compact display title (`36sp` baseline).
    DisplaySmall,
    /// Page display headline (`32sp` baseline).
    HeadlineLarge,
    /// Dialog or section headline (`28sp` baseline).
    HeadlineMedium,
    /// Card or sub-section headline (`24sp` baseline).
    HeadlineSmall,
    /// Top app bar / window title (`22sp` baseline).
    TitleLarge,
    /// Emphasized card or list heading (`16sp` Medium baseline).
    TitleMedium,
    /// Dense group caption or tab title (`14sp` Medium baseline).
    TitleSmall,
    /// Primary interactive body text (`16sp` baseline).
    BodyLarge,
    /// Standard prose body text (`14sp` baseline).
    BodyMedium,
    /// Supporting / helper text (`12sp` baseline).
    BodySmall,
    /// Button and badge label (`14sp` Medium baseline).
    LabelLarge,
    /// Compact chip or tab label (`12sp` Medium baseline).
    LabelMedium,
    /// Overline or numeric badge text (`11sp` Medium baseline).
    LabelSmall,
}

/// Concrete font sizing, line-height, tracking, and weight parameters for a single [`TypeRole`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyleSpec {
    /// Logical font size in pixels.
    pub font_size: f32,
    /// Official target line height in logical pixels.
    pub line_height: f32,
    /// Tracking / letter spacing in logical pixels.
    pub letter_spacing: f32,
    /// Font weight.
    pub weight: FontWeight,
}

impl TextStyleSpec {
    /// Creates a [`TextStyleSpec`] with explicit size, line height, tracking, and weight.
    #[must_use]
    pub const fn new(
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        weight: FontWeight,
    ) -> Self {
        Self {
            font_size,
            line_height,
            letter_spacing,
            weight,
        }
    }
}

/// Full 15-slot typography scale shared by [`DesignSpec`](crate::tokens::DesignSpec).
#[derive(Clone, Debug, PartialEq)]
pub struct TypeScale {
    /// Display large style.
    pub display_large: TextStyleSpec,
    /// Display medium style.
    pub display_medium: TextStyleSpec,
    /// Display small style.
    pub display_small: TextStyleSpec,
    /// Headline large style.
    pub headline_large: TextStyleSpec,
    /// Headline medium style.
    pub headline_medium: TextStyleSpec,
    /// Headline small style.
    pub headline_small: TextStyleSpec,
    /// Title large style.
    pub title_large: TextStyleSpec,
    /// Title medium style.
    pub title_medium: TextStyleSpec,
    /// Title small style.
    pub title_small: TextStyleSpec,
    /// Body large style.
    pub body_large: TextStyleSpec,
    /// Body medium style.
    pub body_medium: TextStyleSpec,
    /// Body small style.
    pub body_small: TextStyleSpec,
    /// Label large style.
    pub label_large: TextStyleSpec,
    /// Label medium style.
    pub label_medium: TextStyleSpec,
    /// Label small style.
    pub label_small: TextStyleSpec,
    font_family: Option<FontFamily<'static>>,
}

impl Default for TypeScale {
    fn default() -> Self {
        Self::baseline()
    }
}

const MD3_FONT_STACK: &[FontFamilyName<'static>] = &[
    FontFamilyName::Named(Cow::Borrowed("Roboto")),
    FontFamilyName::Named(Cow::Borrowed("Sarasa UI SC")),
    FontFamilyName::Named(Cow::Borrowed("Microsoft YaHei UI")),
    FontFamilyName::Named(Cow::Borrowed("Noto Sans CJK SC")),
    FontFamilyName::Named(Cow::Borrowed("PingFang SC")),
    FontFamilyName::Generic(GenericFamily::SansSerif),
];

const FLUENT_FONT_STACK: &[FontFamilyName<'static>] = &[
    FontFamilyName::Named(Cow::Borrowed("Segoe UI Variable")),
    FontFamilyName::Named(Cow::Borrowed("Segoe UI")),
    FontFamilyName::Named(Cow::Borrowed("Microsoft YaHei UI")),
    FontFamilyName::Named(Cow::Borrowed("Sarasa UI SC")),
    FontFamilyName::Named(Cow::Borrowed("Noto Sans CJK SC")),
    FontFamilyName::Named(Cow::Borrowed("PingFang SC")),
    FontFamilyName::Generic(GenericFamily::SansSerif),
];

const HIG_FONT_STACK: &[FontFamilyName<'static>] = &[
    FontFamilyName::Named(Cow::Borrowed("SF Pro Text")),
    FontFamilyName::Named(Cow::Borrowed("SF Pro Display")),
    FontFamilyName::Named(Cow::Borrowed("PingFang SC")),
    FontFamilyName::Named(Cow::Borrowed("Sarasa UI SC")),
    FontFamilyName::Named(Cow::Borrowed("Microsoft YaHei UI")),
    FontFamilyName::Named(Cow::Borrowed("Noto Sans CJK SC")),
    FontFamilyName::Generic(GenericFamily::SansSerif),
];

const ADWAITA_FONT_STACK: &[FontFamilyName<'static>] = &[
    FontFamilyName::Named(Cow::Borrowed("Adwaita Sans")),
    FontFamilyName::Named(Cow::Borrowed("Cantarell")),
    FontFamilyName::Named(Cow::Borrowed("Sarasa UI SC")),
    FontFamilyName::Named(Cow::Borrowed("Noto Sans CJK SC")),
    FontFamilyName::Named(Cow::Borrowed("Microsoft YaHei UI")),
    FontFamilyName::Named(Cow::Borrowed("PingFang SC")),
    FontFamilyName::Generic(GenericFamily::SansSerif),
];

impl TypeScale {
    /// Returns the baseline Material Design 3 15-role typography ramp (`16sp`/`14sp` body).
    #[must_use]
    pub const fn baseline() -> Self {
        Self::md3()
    }

    /// Returns the official Material Design 3 typography ramp (size, line height, tracking, weight).
    #[must_use]
    pub const fn md3() -> Self {
        Self {
            display_large: TextStyleSpec::new(57.0, 64.0, -0.25, FontWeight::NORMAL),
            display_medium: TextStyleSpec::new(45.0, 52.0, 0.0, FontWeight::NORMAL),
            display_small: TextStyleSpec::new(36.0, 44.0, 0.0, FontWeight::NORMAL),
            headline_large: TextStyleSpec::new(32.0, 40.0, 0.0, FontWeight::NORMAL),
            headline_medium: TextStyleSpec::new(28.0, 36.0, 0.0, FontWeight::NORMAL),
            headline_small: TextStyleSpec::new(24.0, 32.0, 0.0, FontWeight::NORMAL),
            title_large: TextStyleSpec::new(22.0, 28.0, 0.0, FontWeight::NORMAL),
            title_medium: TextStyleSpec::new(16.0, 24.0, 0.15, FontWeight::MEDIUM),
            title_small: TextStyleSpec::new(14.0, 20.0, 0.1, FontWeight::MEDIUM),
            body_large: TextStyleSpec::new(16.0, 24.0, 0.5, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(14.0, 20.0, 0.25, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(12.0, 16.0, 0.4, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(14.0, 20.0, 0.1, FontWeight::MEDIUM),
            label_medium: TextStyleSpec::new(12.0, 16.0, 0.5, FontWeight::MEDIUM),
            label_small: TextStyleSpec::new(11.0, 16.0, 0.5, FontWeight::MEDIUM),
            font_family: Some(FontFamily::List(Cow::Borrowed(MD3_FONT_STACK))),
        }
    }

    /// Returns the Windows 11 / WinUI 3 Fluent 2 type ramp (`Segoe UI Variable` scale).
    #[must_use]
    pub const fn fluent2() -> Self {
        Self {
            display_large: TextStyleSpec::new(68.0, 92.0, -0.5, FontWeight::SEMI_BOLD),
            display_medium: TextStyleSpec::new(48.0, 60.0, -0.25, FontWeight::SEMI_BOLD),
            display_small: TextStyleSpec::new(40.0, 52.0, 0.0, FontWeight::SEMI_BOLD),
            headline_large: TextStyleSpec::new(32.0, 42.0, 0.0, FontWeight::SEMI_BOLD),
            headline_medium: TextStyleSpec::new(28.0, 36.0, 0.0, FontWeight::SEMI_BOLD),
            headline_small: TextStyleSpec::new(24.0, 32.0, 0.0, FontWeight::SEMI_BOLD),
            title_large: TextStyleSpec::new(20.0, 28.0, 0.0, FontWeight::SEMI_BOLD),
            title_medium: TextStyleSpec::new(16.0, 22.0, 0.0, FontWeight::SEMI_BOLD),
            title_small: TextStyleSpec::new(14.0, 20.0, 0.0, FontWeight::SEMI_BOLD),
            body_large: TextStyleSpec::new(18.0, 24.0, 0.0, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(14.0, 20.0, 0.0, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(12.0, 16.0, 0.0, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(14.0, 20.0, 0.0, FontWeight::MEDIUM),
            label_medium: TextStyleSpec::new(12.0, 16.0, 0.0, FontWeight::MEDIUM),
            label_small: TextStyleSpec::new(11.0, 14.0, 0.0, FontWeight::MEDIUM),
            font_family: Some(FontFamily::List(Cow::Borrowed(FLUENT_FONT_STACK))),
        }
    }

    /// Returns the Apple macOS HIG desktop typography ramp (`13pt` SF Pro body baseline).
    #[must_use]
    pub const fn macos_hig() -> Self {
        Self {
            display_large: TextStyleSpec::new(40.0, 48.0, -0.3, FontWeight::BOLD),
            display_medium: TextStyleSpec::new(34.0, 41.0, -0.2, FontWeight::BOLD),
            display_small: TextStyleSpec::new(28.0, 34.0, -0.1, FontWeight::BOLD),
            headline_large: TextStyleSpec::new(26.0, 32.0, 0.0, FontWeight::BOLD),
            headline_medium: TextStyleSpec::new(22.0, 26.0, 0.0, FontWeight::BOLD),
            headline_small: TextStyleSpec::new(17.0, 22.0, 0.0, FontWeight::SEMI_BOLD),
            title_large: TextStyleSpec::new(15.0, 20.0, 0.0, FontWeight::SEMI_BOLD),
            title_medium: TextStyleSpec::new(13.0, 16.0, 0.0, FontWeight::BOLD),
            title_small: TextStyleSpec::new(12.0, 16.0, 0.0, FontWeight::MEDIUM),
            body_large: TextStyleSpec::new(13.0, 18.0, 0.0, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(13.0, 16.0, 0.0, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(11.0, 14.0, 0.0, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(13.0, 16.0, 0.0, FontWeight::MEDIUM),
            label_medium: TextStyleSpec::new(11.0, 14.0, 0.0, FontWeight::MEDIUM),
            label_small: TextStyleSpec::new(10.0, 13.0, 0.0, FontWeight::MEDIUM),
            font_family: Some(FontFamily::List(Cow::Borrowed(HIG_FONT_STACK))),
        }
    }

    /// Returns the GNOME HIG / Libadwaita 1.x typography ramp (`15px`/`11pt` Adwaita Sans baseline).
    #[must_use]
    pub const fn adwaita() -> Self {
        Self {
            display_large: TextStyleSpec::new(40.0, 48.0, -0.2, FontWeight::BOLD),
            display_medium: TextStyleSpec::new(34.0, 42.0, -0.1, FontWeight::BOLD),
            display_small: TextStyleSpec::new(28.0, 36.0, 0.0, FontWeight::BOLD),
            headline_large: TextStyleSpec::new(27.0, 34.0, 0.0, FontWeight::BOLD),
            headline_medium: TextStyleSpec::new(24.0, 30.0, 0.0, FontWeight::BOLD),
            headline_small: TextStyleSpec::new(20.0, 26.0, 0.0, FontWeight::BOLD),
            title_large: TextStyleSpec::new(20.0, 26.0, 0.0, FontWeight::SEMI_BOLD),
            title_medium: TextStyleSpec::new(17.0, 22.0, 0.0, FontWeight::BOLD),
            title_small: TextStyleSpec::new(15.0, 20.0, 0.0, FontWeight::BOLD),
            body_large: TextStyleSpec::new(15.0, 22.0, 0.0, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(15.0, 20.0, 0.0, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(12.0, 16.0, 0.0, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(15.0, 20.0, 0.0, FontWeight::SEMI_BOLD),
            label_medium: TextStyleSpec::new(12.0, 16.0, 0.0, FontWeight::BOLD),
            label_small: TextStyleSpec::new(11.0, 14.0, 0.0, FontWeight::MEDIUM),
            font_family: Some(FontFamily::List(Cow::Borrowed(ADWAITA_FONT_STACK))),
        }
    }

    /// Returns a pointer-dense desktop ramp (e.g., `14px` for Fluent/Adwaita or `13px` for macOS HIG).
    #[must_use]
    pub fn compact_desktop(base_body_px: f32) -> Self {
        let base = base_body_px.clamp(12.0, 16.0);
        let lh = |sz: f32| (sz * 1.35).round();
        let body_sm = (base - 2.0).max(11.0);
        let lbl_sm = (base - 3.0).max(10.0);
        Self {
            display_large: TextStyleSpec::new(
                base + 26.0,
                lh(base + 26.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            display_medium: TextStyleSpec::new(
                base + 18.0,
                lh(base + 18.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            display_small: TextStyleSpec::new(
                base + 12.0,
                lh(base + 12.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            headline_large: TextStyleSpec::new(
                base + 10.0,
                lh(base + 10.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            headline_medium: TextStyleSpec::new(
                base + 6.0,
                lh(base + 6.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            headline_small: TextStyleSpec::new(
                base + 4.0,
                lh(base + 4.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            title_large: TextStyleSpec::new(base + 4.0, lh(base + 4.0), 0.0, FontWeight::SEMI_BOLD),
            title_medium: TextStyleSpec::new(
                base + 1.0,
                lh(base + 1.0),
                0.0,
                FontWeight::SEMI_BOLD,
            ),
            title_small: TextStyleSpec::new(base, lh(base), 0.0, FontWeight::SEMI_BOLD),
            body_large: TextStyleSpec::new(base, lh(base), 0.0, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(base, lh(base), 0.0, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(body_sm, lh(body_sm), 0.0, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(base, lh(base), 0.0, FontWeight::MEDIUM),
            label_medium: TextStyleSpec::new(body_sm, lh(body_sm), 0.0, FontWeight::MEDIUM),
            label_small: TextStyleSpec::new(lbl_sm, lh(lbl_sm), 0.0, FontWeight::MEDIUM),
            font_family: None,
        }
    }

    /// Sets an explicit font family (useful for bundling CJK/UI fonts).
    #[must_use]
    pub fn with_font_family(mut self, family: FontFamily<'static>) -> Self {
        self.font_family = Some(family);
        self
    }

    /// Parses and assigns a CSS-style font fallback stack string (e.g. `"Roboto, sans-serif"`).
    #[must_use]
    pub fn with_font_stack(mut self, stack: &'static str) -> Self {
        let items: Vec<FontFamilyName<'static>> = stack
            .split(',')
            .map(|part| part.trim().trim_matches('\'').trim_matches('"'))
            .filter(|part| !part.is_empty())
            .map(|name| match name {
                "sans-serif" => FontFamilyName::Generic(GenericFamily::SansSerif),
                "serif" => FontFamilyName::Generic(GenericFamily::Serif),
                "monospace" => FontFamilyName::Generic(GenericFamily::Monospace),
                "system-ui" => FontFamilyName::Generic(GenericFamily::SystemUi),
                other => FontFamilyName::Named(Cow::Borrowed(other)),
            })
            .collect();
        if let [single] = items.as_slice() {
            self.font_family = Some(FontFamily::Single(single.clone()));
        } else if !items.is_empty() {
            self.font_family = Some(FontFamily::List(Cow::Owned(items)));
        } else {
            self.font_family = Some(FontFamily::from(stack));
        }
        self
    }

    /// Selects the canonical Material Design 3 Roboto + CJK sans fallback stack.
    #[must_use]
    pub fn with_md3_font_stack(mut self) -> Self {
        self.font_family = Some(FontFamily::List(Cow::Borrowed(MD3_FONT_STACK)));
        self
    }

    /// Selects the canonical Windows 11 Segoe UI Variable + YaHei + cross-platform CJK fallback stack.
    #[must_use]
    pub fn with_fluent_font_stack(mut self) -> Self {
        self.font_family = Some(FontFamily::List(Cow::Borrowed(FLUENT_FONT_STACK)));
        self
    }

    /// Selects the canonical Apple macOS SF Pro + PingFang + cross-platform CJK fallback stack.
    #[must_use]
    pub fn with_hig_font_stack(mut self) -> Self {
        self.font_family = Some(FontFamily::List(Cow::Borrowed(HIG_FONT_STACK)));
        self
    }

    /// Selects the canonical GNOME Adwaita Sans + Cantarell + cross-platform CJK fallback stack.
    #[must_use]
    pub fn with_adwaita_font_stack(mut self) -> Self {
        self.font_family = Some(FontFamily::List(Cow::Borrowed(ADWAITA_FONT_STACK)));
        self
    }

    /// Convenience helper selecting the generic sans-serif stack (`FontFamily::from(GenericFamily::SansSerif)`).
    #[must_use]
    pub fn with_sans_serif(mut self) -> Self {
        self.font_family = Some(FontFamily::from(GenericFamily::SansSerif));
        self
    }

    /// Returns the configured font family, if overridden.
    #[must_use]
    pub fn font_family(&self) -> Option<&FontFamily<'static>> {
        self.font_family.as_ref()
    }

    /// Looks up the [`TextStyleSpec`] for a given [`TypeRole`].
    #[must_use]
    pub const fn get(&self, role: TypeRole) -> TextStyleSpec {
        match role {
            TypeRole::DisplayLarge => self.display_large,
            TypeRole::DisplayMedium => self.display_medium,
            TypeRole::DisplaySmall => self.display_small,
            TypeRole::HeadlineLarge => self.headline_large,
            TypeRole::HeadlineMedium => self.headline_medium,
            TypeRole::HeadlineSmall => self.headline_small,
            TypeRole::TitleLarge => self.title_large,
            TypeRole::TitleMedium => self.title_medium,
            TypeRole::TitleSmall => self.title_small,
            TypeRole::BodyLarge => self.body_large,
            TypeRole::BodyMedium => self.body_medium,
            TypeRole::BodySmall => self.body_small,
            TypeRole::LabelLarge => self.label_large,
            TypeRole::LabelMedium => self.label_medium,
            TypeRole::LabelSmall => self.label_small,
        }
    }

    /// Applies `role` sizing, weight, line height, tracking, and configured font family onto a Brocado [`Label`].
    pub fn style_label(&self, role: TypeRole, label: Label) -> Label {
        let style = self.get(role);
        let styled = label
            .text_size(style.font_size)
            .weight(style.weight)
            .line_height(LineHeight::Absolute(style.line_height))
            .letter_spacing(style.letter_spacing);
        if let Some(font) = &self.font_family {
            styled.font(font.clone())
        } else {
            styled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_type_scales_match_platform_metrics() {
        let md3 = TypeScale::md3();
        assert_eq!(md3.body_medium.font_size, 14.0);
        assert_eq!(md3.body_medium.line_height, 20.0);
        assert_eq!(md3.display_large.font_size, 57.0);
        assert_eq!(md3.display_large.line_height, 64.0);

        let fluent = TypeScale::fluent2();
        assert_eq!(fluent.body_medium.font_size, 14.0);
        assert_eq!(fluent.title_large.font_size, 20.0);
        assert_eq!(fluent.display_large.font_size, 68.0);
        assert_eq!(fluent.display_large.line_height, 92.0);

        let hig = TypeScale::macos_hig();
        assert_eq!(hig.body_medium.font_size, 13.0);
        assert_eq!(hig.body_medium.line_height, 16.0);
        assert_eq!(hig.label_small.font_size, 10.0);

        let adw = TypeScale::adwaita();
        assert_eq!(adw.body_medium.font_size, 15.0);
        assert_eq!(adw.body_medium.line_height, 20.0);
        assert_eq!(adw.headline_large.font_size, 27.0);

        for ramp in [md3, fluent, hig, adw] {
            for role in [
                TypeRole::DisplayLarge,
                TypeRole::HeadlineMedium,
                TypeRole::TitleMedium,
                TypeRole::BodyMedium,
                TypeRole::LabelSmall,
            ] {
                let s = ramp.get(role);
                assert!(s.line_height > s.font_size);
            }
        }
    }

    #[test]
    fn font_stack_helpers_assign_family() {
        assert!(
            TypeScale::md3()
                .with_md3_font_stack()
                .font_family()
                .is_some()
        );
        assert!(
            TypeScale::fluent2()
                .with_fluent_font_stack()
                .font_family()
                .is_some()
        );
        assert!(
            TypeScale::macos_hig()
                .with_hig_font_stack()
                .font_family()
                .is_some()
        );
        assert!(
            TypeScale::adwaita()
                .with_adwaita_font_stack()
                .font_family()
                .is_some()
        );
    }
}
