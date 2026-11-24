//! Elevated, filled, outlined, grouped, and arbitrary tonal-elevation surface containers.

use std::marker::PhantomData;

use masonry::layout::AsUnit;
use masonry::properties::{Background, BoxShadow, CornerRadius, Padding};
use xilem_masonry::WidgetView;

use crate::foundation::{BoundedX, ContainerView, UiClassExt, WithClass, container};
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, ElevationLevel};

/// Cross-theme card surface variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CardVariant {
    /// Raised card with subtle border + elevation shadow.
    #[default]
    Elevated,
    /// Tonal container fill without outline.
    Filled,
    /// Flat card with 1px structural border.
    Outlined,
    /// Grouped settings / boxed-list container card.
    Grouped,
}

impl CardVariant {
    const fn class_name(self) -> &'static str {
        match self {
            Self::Elevated => classes::CARD_ELEVATED,
            Self::Filled => classes::CARD_FILLED,
            Self::Outlined => classes::CARD_OUTLINED,
            Self::Grouped => classes::GROUPED_CARD,
        }
    }
}

/// Wraps `child` in a card container styled with `variant`.
pub fn card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    variant: CardVariant,
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    container(child).ui_class(variant.class_name())
}

/// Compile-time horizontally bounded card wrapper requiring `child: BoundedX`.
pub fn bounded_card<State: 'static, Action: 'static, V: WidgetView<State, Action> + BoundedX>(
    variant: CardVariant,
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    card(variant, child)
}

/// Returns an elevated card container wrapping `child`.
pub fn elevated_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    card(CardVariant::Elevated, child)
}

/// Returns a filled container card wrapping `child`.
pub fn filled_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    card(CardVariant::Filled, child)
}

/// Returns an outlined bordered card wrapping `child`.
pub fn outlined_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    card(CardVariant::Outlined, child)
}

/// Returns a grouped card surface suitable for Fluent SettingsCards / HIG FormGroups / Adwaita PreferencesGroups.
pub fn grouped_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> WithClass<ContainerView<V, State, Action>, State, Action> {
    card(CardVariant::Grouped, child)
}

/// Tonal surface container rendered with explicit [`ElevationLevel`] tint and drop shadow.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct SurfaceBox<V, State, Action> {
    elevation: ElevationLevel,
    radius: Option<f64>,
    padding: Option<f64>,
    child: V,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State, Action> SurfaceBox<V, State, Action> {
    /// Overrides the corner radius (defaults to `spec.metrics.card_radius`).
    pub fn radius(mut self, radius: f64) -> Self {
        self.radius = Some(radius);
        self
    }

    /// Sets uniform inner padding in logical pixels.
    pub fn padding(mut self, padding: f64) -> Self {
        self.padding = Some(padding);
        self
    }
}

impl<V, State, Action> xilem_core::ViewMarker for SurfaceBox<V, State, Action> {}

impl<V, State, Action> xilem_core::View<State, Action, xilem_masonry::ViewCtx>
    for SurfaceBox<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = xilem_masonry::Pod<masonry::widgets::SizedBox>;
    type ViewState = ((masonry::peniko::Color, f64, f64, BoxShadow), V::ViewState);

    fn build(
        &self,
        ctx: &mut xilem_masonry::ViewCtx,
        app_state: &mut State,
    ) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec_arc(ctx);
        let p = &spec.palette;
        let bg = self.elevation.tinted_surface(p.surface, p.accent);
        let rad = self.radius.unwrap_or(spec.metrics.card_radius);
        let pad = self.padding.unwrap_or(12.0);
        let shadow = spec.box_shadow(self.elevation);
        let (child_pod, child_state) = self.child.build(ctx, app_state);
        let mut pod = ctx.create_pod(masonry::widgets::SizedBox::new(
            child_pod.new_widget.erased(),
        ));
        pod.new_widget.properties.insert(Background::Color(bg));
        pod.new_widget
            .properties
            .insert(CornerRadius { radius: rad.px() });
        pod.new_widget.properties.insert(Padding::all(pad.px()));
        pod.new_widget.properties.insert(shadow);
        (pod, ((bg, rad, pad, shadow), child_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        ((prev_bg, prev_rad, prev_pad, prev_shadow), view_state): &mut Self::ViewState,
        ctx: &mut xilem_masonry::ViewCtx,
        mut element: xilem_core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = crate::style_engine::current_spec_arc(ctx);
        let p = &spec.palette;
        let bg = self.elevation.tinted_surface(p.surface, p.accent);
        let rad = self.radius.unwrap_or(spec.metrics.card_radius);
        let pad = self.padding.unwrap_or(12.0);
        let shadow = spec.box_shadow(self.elevation);
        let id = element.id();
        if *prev_bg != bg {
            ctx.mark_prop_changed::<Background>(id);
            element.insert_prop(Background::Color(bg));
            *prev_bg = bg;
        }
        if (*prev_rad - rad).abs() > f64::EPSILON {
            ctx.mark_prop_changed::<CornerRadius>(id);
            element.insert_prop(CornerRadius { radius: rad.px() });
            *prev_rad = rad;
        }
        if (*prev_pad - pad).abs() > f64::EPSILON {
            ctx.mark_prop_changed::<Padding>(id);
            element.insert_prop(Padding::all(pad.px()));
            *prev_pad = pad;
        }
        if *prev_shadow != shadow {
            ctx.mark_prop_changed::<BoxShadow>(id);
            element.insert_prop(shadow);
            *prev_shadow = shadow;
        }

        let mut child_dyn = masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("SurfaceBox SizedBox always holds a child");
        self.child.rebuild(
            &prev.child,
            view_state,
            ctx,
            child_dyn.downcast(),
            app_state,
        );
    }

    fn teardown(
        &self,
        (_props_cache, view_state): &mut Self::ViewState,
        ctx: &mut xilem_masonry::ViewCtx,
        mut element: xilem_core::Mut<'_, Self::Element>,
    ) {
        let mut child_dyn = masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("SurfaceBox SizedBox always holds a child");
        self.child.teardown(view_state, ctx, child_dyn.downcast());
    }

    fn message(
        &self,
        (_props_cache, view_state): &mut Self::ViewState,
        message: &mut xilem_core::MessageCtx,
        mut element: xilem_core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> xilem_core::MessageResult<Action> {
        let mut child_dyn = masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("SurfaceBox SizedBox always holds a child");
        self.child
            .message(view_state, message, child_dyn.downcast(), app_state)
    }
}

/// Wraps `child` in a surface container with tonal elevation + shadow.
pub fn surface_box<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    elevation: ElevationLevel,
    child: V,
) -> SurfaceBox<V, State, Action> {
    let _ = DesignSpec::neutral_fallback;
    SurfaceBox {
        elevation,
        radius: None,
        padding: None,
        child,
        phantom: PhantomData,
    }
}
