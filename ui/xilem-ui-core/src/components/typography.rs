//! Typographic text components bound to the active [`DesignSpec`] scale.

use std::marker::PhantomData;

use xilem::Color;
use xilem::masonry::core::ArcStr;
use xilem::style::Style as _;
use xilem::view::{Prose, label, prose};
use xilem::{AnyWidgetView, WidgetView};

use crate::tokens::{DesignSpec, TypeRole};

/// Label view styled via a semantic [`TypeRole`] from the active [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiLabel<State, Action> {
    role: TypeRole,
    text: ArcStr,
    color: Option<Color>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiLabel<State, Action> {
    /// Overrides the default foreground ink color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let styled = spec
            .typography
            .style_label(self.role, label(self.text.clone()));
        if let Some(c) = self.color {
            styled.color(c).boxed()
        } else {
            styled.boxed()
        }
    }
}

crate::impl_spec_view!(UiLabel<State, Action>);

/// Creates a semantic label styled using `role` from the active [`DesignSpec`].
pub fn ui_label<State: 'static, Action: 'static>(
    role: TypeRole,
    text: impl Into<ArcStr>,
) -> UiLabel<State, Action> {
    UiLabel {
        role,
        text: text.into(),
        color: None,
        phantom: PhantomData,
    }
}

/// Multi-line wrapping prose view styled via [`TypeRole`] from the active [`DesignSpec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiProse<State, Action> {
    role: TypeRole,
    text: ArcStr,
    color: Option<Color>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiProse<State, Action> {
    /// Overrides the default foreground ink color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let style = spec.typography.get(self.role);
        let mut view: Prose<State, Action> = prose(self.text.clone())
            .text_size(style.font_size)
            .weight(style.weight);
        if let Some(c) = self.color {
            view = view.text_color(c);
        }
        view.boxed()
    }
}

crate::impl_spec_view!(UiProse<State, Action>);

/// Creates wrapping prose text styled using `role`.
pub fn ui_prose<State: 'static, Action: 'static>(
    role: TypeRole,
    text: impl Into<ArcStr>,
) -> UiProse<State, Action> {
    UiProse {
        role,
        text: text.into(),
        color: None,
        phantom: PhantomData,
    }
}

macro_rules! role_fn {
    ($fn_name:ident, $role:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $fn_name<State: 'static, Action: 'static>(
            text: impl Into<ArcStr>,
        ) -> UiLabel<State, Action> {
            ui_label($role, text)
        }
    };
}

role_fn!(
    display_large,
    TypeRole::DisplayLarge,
    "Large display text (`DisplayLarge`)."
);
role_fn!(
    display_medium,
    TypeRole::DisplayMedium,
    "Medium display text (`DisplayMedium`)."
);
role_fn!(
    display_small,
    TypeRole::DisplaySmall,
    "Small display text (`DisplaySmall`)."
);
role_fn!(
    headline_large,
    TypeRole::HeadlineLarge,
    "Large headline (`HeadlineLarge`)."
);
role_fn!(
    headline_medium,
    TypeRole::HeadlineMedium,
    "Medium headline (`HeadlineMedium`)."
);
role_fn!(
    headline_small,
    TypeRole::HeadlineSmall,
    "Small headline (`HeadlineSmall`)."
);
role_fn!(
    title_large,
    TypeRole::TitleLarge,
    "Large title (`TitleLarge`)."
);
role_fn!(
    title_medium,
    TypeRole::TitleMedium,
    "Medium title (`TitleMedium`)."
);
role_fn!(
    title_small,
    TypeRole::TitleSmall,
    "Small title (`TitleSmall`)."
);
role_fn!(
    body_large,
    TypeRole::BodyLarge,
    "Large body text (`BodyLarge`)."
);
role_fn!(
    body_medium,
    TypeRole::BodyMedium,
    "Medium body text (`BodyMedium`)."
);
role_fn!(
    body_small,
    TypeRole::BodySmall,
    "Small body text (`BodySmall`)."
);
role_fn!(
    label_large,
    TypeRole::LabelLarge,
    "Large label text (`LabelLarge`)."
);
role_fn!(
    label_medium,
    TypeRole::LabelMedium,
    "Medium label text (`LabelMedium`)."
);
role_fn!(
    label_small,
    TypeRole::LabelSmall,
    "Small label text (`LabelSmall`)."
);
