//! Elevated, filled, outlined, grouped, and arbitrary tonal-elevation surface containers.

use std::marker::PhantomData;

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{CornerRadius, Padding};
use xilem::style::Style as _;
use xilem::view::sized_box;
use xilem::{AnyWidgetView, WidgetView};

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

/// Returns an elevated card container wrapping `child`.
pub fn elevated_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action> {
    sized_box(child)
        .padding(Padding::all(16.px()))
        .ui_class(CardVariant::Elevated.class_name())
}

/// Returns a filled container card wrapping `child`.
pub fn filled_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action> {
    sized_box(child)
        .padding(Padding::all(16.px()))
        .ui_class(CardVariant::Filled.class_name())
}

/// Returns an outlined bordered card wrapping `child`.
pub fn outlined_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action> {
    sized_box(child)
        .padding(Padding::all(16.px()))
        .ui_class(CardVariant::Outlined.class_name())
}

/// Returns a grouped card surface suitable for Fluent SettingsCards / HIG FormGroups / Adwaita PreferencesGroups.
pub fn grouped_card<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> impl WidgetView<State, Action> {
    sized_box(child).ui_class(CardVariant::Grouped.class_name())
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
    V: WidgetView<State, Action> + Clone,
{
    type Element = xilem::Pod<xilem::masonry::widgets::Passthrough>;
    type ViewState = crate::style_engine::scope::SpecViewState<State, Action>;

    fn build(
        &self,
        ctx: &mut xilem::ViewCtx,
        app_state: &mut State,
    ) -> (Self::Element, Self::ViewState) {
        crate::style_engine::scope::spec_view_build(ctx, app_state, |spec| {
            self.render_with_spec(spec)
        })
    }

    fn rebuild(
        &self,
        _prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut xilem::ViewCtx,
        element: xilem::core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        crate::style_engine::scope::spec_view_rebuild(
            view_state,
            ctx,
            element,
            app_state,
            |spec| self.render_with_spec(spec),
        );
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut xilem::ViewCtx,
        element: xilem::core::Mut<'_, Self::Element>,
    ) {
        crate::style_engine::scope::spec_view_teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut xilem::core::MessageCtx,
        element: xilem::core::Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> xilem::core::MessageResult<Action> {
        crate::style_engine::scope::spec_view_message(view_state, message, element, app_state)
    }
}

impl<V, State, Action> SurfaceBox<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action> + Clone,
{
    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>> {
        let p = &spec.palette;
        let bg = self.elevation.tinted_surface(p.surface, p.accent);
        let rad = self.radius.unwrap_or(spec.metrics.card_radius);
        let pad = self.padding.unwrap_or(12.0);
        sized_box(self.child.clone())
            .background_color(bg)
            .corner_radius(rad.px())
            .padding(Padding::all(pad.px()))
            .box_shadow(spec.box_shadow(self.elevation))
            .boxed()
    }
}

/// Wraps `child` in a surface container with tonal elevation + shadow.
pub fn surface_box<State: 'static, Action: 'static, V: WidgetView<State, Action> + Clone>(
    elevation: ElevationLevel,
    child: V,
) -> SurfaceBox<V, State, Action> {
    let _ = CornerRadius { radius: 0.px() };
    SurfaceBox {
        elevation,
        radius: None,
        padding: None,
        child,
        phantom: PhantomData,
    }
}
