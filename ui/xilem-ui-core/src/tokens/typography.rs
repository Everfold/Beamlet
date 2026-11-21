//! Shared 15-role typography ramp and custom font-family support.

use xilem::FontWeight;
use xilem::masonry::parley::GenericFamily;
use xilem::masonry::parley::style::FontFamily;
use xilem::view::Label;

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

/// Concrete font sizing parameters for a single [`TypeRole`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyleSpec {
    /// Logical font size in pixels.
    pub font_size: f32,
    /// Font weight.
    pub weight: FontWeight,
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
    /// Returns the baseline `16sp`/`14sp` readable typography ramp.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            display_large: TextStyleSpec {
                font_size: 57.0,
                weight: FontWeight::NORMAL,
            },
            display_medium: TextStyleSpec {
                font_size: 45.0,
                weight: FontWeight::NORMAL,
            },
            display_small: TextStyleSpec {
                font_size: 36.0,
                weight: FontWeight::NORMAL,
            },
            headline_large: TextStyleSpec {
                font_size: 32.0,
                weight: FontWeight::NORMAL,
            },
            headline_medium: TextStyleSpec {
                font_size: 28.0,
                weight: FontWeight::NORMAL,
            },
            headline_small: TextStyleSpec {
                font_size: 24.0,
                weight: FontWeight::NORMAL,
            },
            title_large: TextStyleSpec {
                font_size: 22.0,
                weight: FontWeight::NORMAL,
            },
            title_medium: TextStyleSpec {
                font_size: 16.0,
                weight: FontWeight::MEDIUM,
            },
            title_small: TextStyleSpec {
                font_size: 14.0,
                weight: FontWeight::MEDIUM,
            },
            body_large: TextStyleSpec {
                font_size: 15.0,
                weight: FontWeight::NORMAL,
            },
            body_medium: TextStyleSpec {
                font_size: 14.0,
                weight: FontWeight::NORMAL,
            },
            body_small: TextStyleSpec {
                font_size: 12.0,
                weight: FontWeight::NORMAL,
            },
            label_large: TextStyleSpec {
                font_size: 14.0,
                weight: FontWeight::MEDIUM,
            },
            label_medium: TextStyleSpec {
                font_size: 12.0,
                weight: FontWeight::MEDIUM,
            },
            label_small: TextStyleSpec {
                font_size: 11.0,
                weight: FontWeight::MEDIUM,
            },
            font_family: None,
        }
    }

    /// Returns a pointer-dense desktop ramp (e.g., `14px` for Fluent/Adwaita or `13px` for macOS HIG).
    #[must_use]
    pub fn compact_desktop(base_body_px: f32) -> Self {
        let base = base_body_px.clamp(12.0, 16.0);
        Self {
            display_large: TextStyleSpec {
                font_size: base + 26.0,
                weight: FontWeight::SEMI_BOLD,
            },
            display_medium: TextStyleSpec {
                font_size: base + 18.0,
                weight: FontWeight::SEMI_BOLD,
            },
            display_small: TextStyleSpec {
                font_size: base + 12.0,
                weight: FontWeight::SEMI_BOLD,
            },
            headline_large: TextStyleSpec {
                font_size: base + 10.0,
                weight: FontWeight::SEMI_BOLD,
            },
            headline_medium: TextStyleSpec {
                font_size: base + 6.0,
                weight: FontWeight::SEMI_BOLD,
            },
            headline_small: TextStyleSpec {
                font_size: base + 4.0,
                weight: FontWeight::SEMI_BOLD,
            },
            title_large: TextStyleSpec {
                font_size: base + 4.0,
                weight: FontWeight::SEMI_BOLD,
            },
            title_medium: TextStyleSpec {
                font_size: base + 1.0,
                weight: FontWeight::SEMI_BOLD,
            },
            title_small: TextStyleSpec {
                font_size: base,
                weight: FontWeight::SEMI_BOLD,
            },
            body_large: TextStyleSpec {
                font_size: base,
                weight: FontWeight::NORMAL,
            },
            body_medium: TextStyleSpec {
                font_size: base,
                weight: FontWeight::NORMAL,
            },
            body_small: TextStyleSpec {
                font_size: (base - 2.0).max(11.0),
                weight: FontWeight::NORMAL,
            },
            label_large: TextStyleSpec {
                font_size: base,
                weight: FontWeight::MEDIUM,
            },
            label_medium: TextStyleSpec {
                font_size: (base - 2.0).max(11.0),
                weight: FontWeight::MEDIUM,
            },
            label_small: TextStyleSpec {
                font_size: (base - 3.0).max(10.0),
                weight: FontWeight::MEDIUM,
            },
            font_family: None,
        }
    }

    /// Sets an explicit font family (useful for bundling CJK/UI fonts).
    #[must_use]
    pub fn with_font_family(mut self, family: FontFamily<'static>) -> Self {
        self.font_family = Some(family);
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

    /// Applies `role` sizing, weight, and configured font family onto a Xilem [`Label`].
    pub fn style_label(&self, role: TypeRole, label: Label) -> Label {
        let style = self.get(role);
        let styled = label.text_size(style.font_size).weight(style.weight);
        if let Some(font) = &self.font_family {
            styled.font(font.clone())
        } else {
            styled
        }
    }
}
