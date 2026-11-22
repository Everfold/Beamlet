//! Typographic text components bound to the active [`DesignSpec`](crate::tokens::DesignSpec) scale.

use std::marker::PhantomData;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::core::ArcStr;
use xilem::masonry::properties::ContentColor;
use xilem::masonry::widgets;
use xilem::view::{Label, Prose, label, prose};
use xilem::{Color, Pod, ViewCtx};

use crate::style_engine::current_spec;
use crate::tokens::TypeRole;

/// Label view styled via a semantic [`TypeRole`] from the active [`DesignSpec`](crate::tokens::DesignSpec).
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

    fn inner_view(&self, ctx: &ViewCtx) -> Label {
        let spec = current_spec(ctx);
        spec.typography
            .style_label(self.role, label(self.text.clone()))
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiLabel<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiLabel<State, Action> {
    type Element = Pod<widgets::Label>;
    type ViewState = (Label, Option<Color>);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let inner = self.inner_view(ctx);
        let (mut pod, ()) = View::<State, Action, ViewCtx>::build(&inner, ctx, app_state);
        if let Some(c) = self.color {
            pod.new_widget.properties.insert(ContentColor::new(c));
        }
        (pod, (inner, self.color))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_inner, prev_color): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let next_inner = self.inner_view(ctx);
        View::<State, Action, ViewCtx>::rebuild(
            &next_inner,
            prev_inner,
            &mut (),
            ctx,
            element.reborrow_mut(),
            app_state,
        );
        if *prev_color != self.color {
            ctx.mark_prop_changed::<ContentColor>(element.id());
            if let Some(c) = self.color {
                element.insert_prop(ContentColor::new(c));
            }
            *prev_color = self.color;
        }
        *prev_inner = next_inner;
    }

    fn teardown(
        &self,
        (prev_inner, _): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        View::<State, Action, ViewCtx>::teardown(prev_inner, &mut (), ctx, element);
    }

    fn message(
        &self,
        (prev_inner, _): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        View::<State, Action, ViewCtx>::message(prev_inner, &mut (), message, element, app_state)
    }
}

/// Creates a semantic label styled using `role` from the active [`DesignSpec`](crate::tokens::DesignSpec).
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

/// Multi-line wrapping prose view styled via [`TypeRole`] from the active [`DesignSpec`](crate::tokens::DesignSpec).
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

    fn inner_view(&self, ctx: &ViewCtx) -> Prose<State, Action> {
        let spec = current_spec(ctx);
        let style = spec.typography.get(self.role);
        let view: Prose<State, Action> = prose(self.text.clone())
            .text_size(style.font_size)
            .weight(style.weight);
        if let Some(c) = self.color {
            view.text_color(c)
        } else {
            view
        }
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiProse<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiProse<State, Action> {
    type Element = Pod<widgets::Prose>;
    type ViewState = Prose<State, Action>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let inner = self.inner_view(ctx);
        let (pod, ()) = inner.build(ctx, app_state);
        (pod, inner)
    }

    fn rebuild(
        &self,
        _prev: &Self,
        prev_inner: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let next_inner = self.inner_view(ctx);
        next_inner.rebuild(prev_inner, &mut (), ctx, element, app_state);
        *prev_inner = next_inner;
    }

    fn teardown(
        &self,
        prev_inner: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_inner.teardown(&mut (), ctx, element);
    }

    fn message(
        &self,
        prev_inner: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_inner.message(&mut (), message, element, app_state)
    }
}

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
