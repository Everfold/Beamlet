//! Shared 15-role typography ramp, line-height/tracking tokens, and developer-configurable font fallback support.
//!
//! By default, typography scales defer to the operating system's default UI font
//! (`GenericFamily::SystemUi`) rather than hardcoding proprietary or locale-biased font stacks.
//! Applications may override or extend the font family / fallback chain on [`TypeScale`] or `UiTheme`.

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

    /// Scales font size and line height proportionally by `factor`.
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        Self {
            font_size: self.font_size * factor,
            line_height: self.line_height * factor,
            ..self
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

impl TypeScale {
    /// Returns the baseline Material Design 3 15-role typography ramp (`16sp`/`14sp` body).
    #[must_use]
    pub const fn baseline() -> Self {
        Self::md3()
    }

    /// Returns the official Material Design 3 typography ramp (size, line height, tracking, weight).
    ///
    /// Defaults to the OS system UI font (`GenericFamily::SystemUi`) unless overridden.
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
            font_family: None,
        }
    }

    /// Returns the Material Design 3 Expressive typography ramp with emphasized weights and
    /// tightened display/headline tracking.
    #[must_use]
    pub const fn md3_expressive() -> Self {
        Self {
            display_large: TextStyleSpec::new(57.0, 64.0, -0.4, FontWeight::BOLD),
            display_medium: TextStyleSpec::new(45.0, 52.0, -0.25, FontWeight::BOLD),
            display_small: TextStyleSpec::new(36.0, 44.0, -0.15, FontWeight::SEMI_BOLD),
            headline_large: TextStyleSpec::new(32.0, 40.0, -0.1, FontWeight::SEMI_BOLD),
            headline_medium: TextStyleSpec::new(28.0, 36.0, 0.0, FontWeight::SEMI_BOLD),
            headline_small: TextStyleSpec::new(24.0, 32.0, 0.0, FontWeight::SEMI_BOLD),
            title_large: TextStyleSpec::new(22.0, 28.0, 0.0, FontWeight::SEMI_BOLD),
            title_medium: TextStyleSpec::new(16.0, 24.0, 0.1, FontWeight::SEMI_BOLD),
            title_small: TextStyleSpec::new(14.0, 20.0, 0.1, FontWeight::SEMI_BOLD),
            body_large: TextStyleSpec::new(16.0, 24.0, 0.5, FontWeight::NORMAL),
            body_medium: TextStyleSpec::new(14.0, 20.0, 0.25, FontWeight::NORMAL),
            body_small: TextStyleSpec::new(12.0, 16.0, 0.4, FontWeight::NORMAL),
            label_large: TextStyleSpec::new(14.0, 20.0, 0.1, FontWeight::SEMI_BOLD),
            label_medium: TextStyleSpec::new(12.0, 16.0, 0.4, FontWeight::SEMI_BOLD),
            label_small: TextStyleSpec::new(11.0, 16.0, 0.4, FontWeight::SEMI_BOLD),
            font_family: None,
        }
    }

    /// Returns the Windows 11 / WinUI 3 Fluent 2 typography ramp (size, line height, tracking, weight).
    ///
    /// Defaults to the OS system UI font (`GenericFamily::SystemUi`) unless overridden.
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
            font_family: None,
        }
    }

    /// Returns the Apple macOS HIG desktop typography ramp (`13pt` body baseline).
    ///
    /// Defaults to the OS system UI font (`GenericFamily::SystemUi`) unless overridden.
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
            font_family: None,
        }
    }

    /// Returns the GNOME HIG / Libadwaita 1.x typography ramp (`15px`/`11pt` body baseline).
    ///
    /// Defaults to the OS system UI font (`GenericFamily::SystemUi`) unless overridden.
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
            font_family: None,
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

    /// Parses a CSS-style font fallback stack string (e.g. `"Inter, system-ui, sans-serif"`).
    ///
    /// Returns `None` when the input contains no non-empty font family names.
    #[must_use]
    pub fn parse_font_stack(stack: &str) -> Option<FontFamily<'static>> {
        let items: Vec<FontFamilyName<'static>> = stack
            .split(',')
            .map(|part| part.trim().trim_matches('\'').trim_matches('"').trim())
            .filter(|part| !part.is_empty())
            .map(|name| {
                if let Some(generic) = GenericFamily::parse(name) {
                    FontFamilyName::Generic(generic)
                } else {
                    FontFamilyName::Named(Cow::Owned(name.to_owned()))
                }
            })
            .collect();
        match items.len() {
            0 => None,
            1 => Some(FontFamily::Single(items.into_iter().next().unwrap())),
            _ => Some(FontFamily::List(Cow::Owned(items))),
        }
    }

    /// Sets an explicit font family or fallback list (useful for app-bundled or custom UI fonts).
    #[must_use]
    pub fn with_font_family(mut self, family: FontFamily<'static>) -> Self {
        self.font_family = Some(family);
        self
    }

    /// Parses and assigns a CSS-style font fallback stack string (e.g. `"Inter, system-ui, sans-serif"`).
    ///
    /// An empty/whitespace-only string resets back to the OS system UI default.
    #[must_use]
    pub fn with_font_stack(mut self, stack: &str) -> Self {
        self.font_family = Self::parse_font_stack(stack);
        self
    }

    /// Replaces the active font stack with an explicit ordered sequence of [`FontFamilyName`] items.
    ///
    /// An empty sequence resets back to the OS system UI default.
    #[must_use]
    pub fn with_font_fallbacks<I>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = FontFamilyName<'static>>,
    {
        let items: Vec<FontFamilyName<'static>> = families.into_iter().collect();
        self.font_family = match items.len() {
            0 => None,
            1 => Some(FontFamily::Single(items.into_iter().next().unwrap())),
            _ => Some(FontFamily::List(Cow::Owned(items))),
        };
        self
    }

    /// Appends a fallback font family to the end of the current stack (or after `GenericFamily::SystemUi` when unset).
    #[must_use]
    pub fn append_font_fallback(mut self, fallback: FontFamilyName<'static>) -> Self {
        let mut items = match self.font_family.take() {
            None => vec![FontFamilyName::Generic(GenericFamily::SystemUi)],
            Some(FontFamily::Single(existing)) => vec![existing],
            Some(FontFamily::List(existing)) => existing.into_owned(),
            Some(FontFamily::Source(src)) => match Self::parse_font_stack(&src) {
                Some(FontFamily::Single(existing)) => vec![existing],
                Some(FontFamily::List(existing)) => existing.into_owned(),
                _ => vec![FontFamilyName::Generic(GenericFamily::SystemUi)],
            },
        };
        items.push(fallback);
        self.font_family = Some(FontFamily::List(Cow::Owned(items)));
        self
    }

    /// Resets the font selection back to the operating system's default UI font (`GenericFamily::SystemUi`).
    #[must_use]
    pub fn with_system_font(mut self) -> Self {
        self.font_family = None;
        self
    }

    /// Convenience helper selecting the generic sans-serif stack (`FontFamily::from(GenericFamily::SansSerif)`).
    #[must_use]
    pub fn with_sans_serif(mut self) -> Self {
        self.font_family = Some(FontFamily::from(GenericFamily::SansSerif));
        self
    }

    /// Returns the explicit font family override, or `None` when deferring to OS `system-ui`.
    #[must_use]
    pub fn font_family(&self) -> Option<&FontFamily<'static>> {
        self.font_family.as_ref()
    }

    /// Returns the effective font family used when building text views (defaults to `GenericFamily::SystemUi`).
    #[must_use]
    pub fn resolved_font_family(&self) -> FontFamily<'static> {
        self.font_family
            .clone()
            .unwrap_or(FontFamily::Single(FontFamilyName::Generic(
                GenericFamily::SystemUi,
            )))
    }

    /// Uniformly scales font sizes and line heights across all 15 roles by `factor` (clamped to `0.5..=3.0`),
    /// preserving any configured font family fallback chain.
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        let f = factor.clamp(0.5, 3.0);
        Self {
            display_large: self.display_large.scaled(f),
            display_medium: self.display_medium.scaled(f),
            display_small: self.display_small.scaled(f),
            headline_large: self.headline_large.scaled(f),
            headline_medium: self.headline_medium.scaled(f),
            headline_small: self.headline_small.scaled(f),
            title_large: self.title_large.scaled(f),
            title_medium: self.title_medium.scaled(f),
            title_small: self.title_small.scaled(f),
            body_large: self.body_large.scaled(f),
            body_medium: self.body_medium.scaled(f),
            body_small: self.body_small.scaled(f),
            label_large: self.label_large.scaled(f),
            label_medium: self.label_medium.scaled(f),
            label_small: self.label_small.scaled(f),
            font_family: self.font_family,
        }
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

    /// Applies `role` sizing, weight, line height, tracking, and resolved font family onto a Brocado [`Label`].
    pub fn style_label(&self, role: TypeRole, label: Label) -> Label {
        let style = self.get(role);
        label
            .text_size(style.font_size)
            .weight(style.weight)
            .line_height(LineHeight::Absolute(style.line_height))
            .letter_spacing(style.letter_spacing)
            .font(self.resolved_font_family())
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
    fn type_scales_default_to_system_ui_and_allow_fallback_customization() {
        let system_ui = FontFamily::Single(FontFamilyName::Generic(GenericFamily::SystemUi));
        for ramp in [
            TypeScale::md3(),
            TypeScale::fluent2(),
            TypeScale::macos_hig(),
            TypeScale::adwaita(),
            TypeScale::compact_desktop(14.0),
        ] {
            assert!(ramp.font_family().is_none());
            assert_eq!(ramp.resolved_font_family(), system_ui);
        }

        let single = TypeScale::md3().with_font_stack("sans-serif");
        assert_eq!(
            single.font_family(),
            Some(&FontFamily::Single(FontFamilyName::Generic(
                GenericFamily::SansSerif
            )))
        );

        let runtime_string = String::from(" 'Custom UI' , system-ui , \"Fallback Serif\" , serif ");
        let list = TypeScale::fluent2().with_font_stack(&runtime_string);
        assert_eq!(
            list.font_family(),
            Some(&FontFamily::List(Cow::Owned(vec![
                FontFamilyName::Named(Cow::Owned(String::from("Custom UI"))),
                FontFamilyName::Generic(GenericFamily::SystemUi),
                FontFamilyName::Named(Cow::Owned(String::from("Fallback Serif"))),
                FontFamilyName::Generic(GenericFamily::Serif),
            ])))
        );

        let cleared = list.clone().with_font_stack("  ");
        assert!(cleared.font_family().is_none());
        assert_eq!(cleared.resolved_font_family(), system_ui);

        let appended_default = TypeScale::adwaita()
            .append_font_fallback(FontFamilyName::Named(Cow::Borrowed("AppFallback")));
        assert_eq!(
            appended_default.font_family(),
            Some(&FontFamily::List(Cow::Owned(vec![
                FontFamilyName::Generic(GenericFamily::SystemUi),
                FontFamilyName::Named(Cow::Borrowed("AppFallback")),
            ])))
        );

        let appended_existing = TypeScale::macos_hig()
            .with_font_fallbacks([FontFamilyName::Named(Cow::Borrowed("AppPrimary"))])
            .append_font_fallback(FontFamilyName::Generic(GenericFamily::SansSerif));
        assert_eq!(
            appended_existing.font_family(),
            Some(&FontFamily::List(Cow::Owned(vec![
                FontFamilyName::Named(Cow::Borrowed("AppPrimary")),
                FontFamilyName::Generic(GenericFamily::SansSerif),
            ])))
        );

        let reset = appended_existing.with_system_font();
        assert!(reset.font_family().is_none());
        assert_eq!(reset.resolved_font_family(), system_ui);
    }

    #[test]
    fn type_scale_scaled_scales_sizes_and_line_heights() {
        let base = TypeScale::md3().with_font_stack("sans-serif");
        let scaled = base.clone().scaled(1.5);
        assert_eq!(
            scaled.body_medium.font_size,
            base.body_medium.font_size * 1.5
        );
        assert_eq!(
            scaled.body_medium.line_height,
            base.body_medium.line_height * 1.5
        );
        assert_eq!(
            scaled.title_large.font_size,
            base.title_large.font_size * 1.5
        );
        assert_eq!(scaled.font_family(), base.font_family());

        let clamped = TypeScale::md3().scaled(10.0);
        assert_eq!(
            clamped.body_medium.font_size,
            base.body_medium.font_size * 3.0
        );
    }

    #[test]
    fn md3_expressive_type_scale_uses_emphasized_weights() {
        let base = TypeScale::md3();
        let exp = TypeScale::md3_expressive();
        assert!(exp.font_family().is_none());
        assert_eq!(exp.display_large.weight, FontWeight::BOLD);
        assert_eq!(exp.display_medium.weight, FontWeight::BOLD);
        assert_eq!(exp.headline_large.weight, FontWeight::SEMI_BOLD);
        assert_eq!(exp.title_medium.weight, FontWeight::SEMI_BOLD);
        assert_eq!(exp.label_large.weight, FontWeight::SEMI_BOLD);
        assert_eq!(exp.body_medium.weight, FontWeight::NORMAL);
        assert!(exp.display_large.weight > base.display_large.weight);
        assert!(exp.headline_large.weight > base.headline_large.weight);
        assert!(exp.label_large.weight > base.label_large.weight);
    }
}
