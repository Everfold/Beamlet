//! Elevated, filled, outlined, grouped, and arbitrary tonal-elevation surface containers.

use std::marker::PhantomData;

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Background, BoxShadow, CornerRadius, Padding};
use xilem::masonry::widgets;
use xilem::view::sized_box;

use crate::foundation::UiClassExt;
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
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(child).ui_class(variant.class_name())
}

/// Returns an elevated card container wrapping `child`.
pub fn elevated_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    card(CardVariant::Elevated, child)
}

/// Returns a filled container card wrapping `child`.
pub fn filled_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    card(CardVariant::Filled, child)
}

/// Returns an outlined bordered card wrapping `child`.
pub fn outlined_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    card(CardVariant::Outlined, child)
}

/// Returns a grouped card surface suitable for Fluent SettingsCards / HIG FormGroups / Adwaita PreferencesGroups.
pub fn grouped_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    card(CardVariant::Grouped, child)
}

/// Tonal surface container rendered with explicit [`ElevationLevel`] tint and drop shadow.
#[must_use = "View values do nothing unless provided to Xilem."]
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

impl<V, State, Action> xilem::core::ViewMarker for SurfaceBox<V, State, Action> {}

impl<V, State, Action> xilem::core::View<State, Action, xilem::ViewCtx>
    for SurfaceBox<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = xilem::Pod<xilem::masonry::widgets::SizedBox>;
    type ViewState = V::ViewState;

    fn build(
        &self,
        ctx: &mut xilem::ViewCtx,
        app_state: &mut State,
    ) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec(ctx);
        let p = &spec.palette;
        let bg = self.elevation.tinted_surface(p.surface, p.accent);
        let rad = self.radius.unwrap_or(spec.metrics.card_radius);
        let pad = self.padding.unwrap_or(12.0);
        let (child_pod, child_state) = self.child.build(ctx, app_state);
        let mut pod = ctx.create_pod(xilem::masonry::widgets::SizedBox::new(
            child_pod.new_widget.erased(),
        ));
        pod.new_widget.properties.insert(Background::Color(bg));
        pod.new_widget
            .properties
            .insert(CornerRadius { radius: rad.px() });
        pod.new_widget.properties.insert(Padding::all(pad.px()));
        pod.new_widget
            .properties
            .insert(spec.box_shadow(self.elevation));
        (pod, child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut xilem::ViewCtx,
        mut element: xilem::core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = crate::style_engine::current_spec(ctx);
        let p = &spec.palette;
        let bg = self.elevation.tinted_surface(p.surface, p.accent);
        let rad = self.radius.unwrap_or(spec.metrics.card_radius);
        let pad = self.padding.unwrap_or(12.0);
        let id = element.id();
        ctx.mark_prop_changed::<Background>(id);
        ctx.mark_prop_changed::<CornerRadius>(id);
        ctx.mark_prop_changed::<Padding>(id);
        ctx.mark_prop_changed::<BoxShadow>(id);
        element.insert_prop(Background::Color(bg));
        element.insert_prop(CornerRadius { radius: rad.px() });
        element.insert_prop(Padding::all(pad.px()));
        element.insert_prop(spec.box_shadow(self.elevation));

        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
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
        view_state: &mut Self::ViewState,
        ctx: &mut xilem::ViewCtx,
        mut element: xilem::core::Mut<'_, Self::Element>,
    ) {
        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("SurfaceBox SizedBox always holds a child");
        self.child.teardown(view_state, ctx, child_dyn.downcast());
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut xilem::core::MessageCtx,
        mut element: xilem::core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> xilem::core::MessageResult<Action> {
        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
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
