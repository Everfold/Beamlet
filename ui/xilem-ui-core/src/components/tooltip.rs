//! Plain and rich tooltips backed by `HoverTooltipHost` and `TooltipLayer`.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::WidgetView;
use xilem::masonry::core::ArcStr;

use crate::foundation::overlay::{HoverTooltipView, TooltipContentSpec};
use crate::tokens::DesignSpec;

/// Wraps `child` with a compact plain tooltip shown on hover after a short dwell delay.
pub fn plain_tooltip<State, Action, V>(
    text: impl Into<ArcStr>,
    child: V,
) -> HoverTooltipView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    let body: ArcStr = text.into();
    HoverTooltipView {
        child,
        spec_builder: Arc::new(move |spec: &DesignSpec| TooltipContentSpec {
            subhead: None,
            body: body.clone(),
            bg: spec.palette.inverse_surface,
            subhead_color: spec.palette.inverse_on_surface,
            body_color: spec.palette.inverse_on_surface,
            shadow_color: spec.palette.shadow,
            corner_radius: (spec.metrics.menu_radius * 0.5).max(4.0),
            is_rich: false,
        }),
        phantom: PhantomData,
    }
}

/// Wraps `child` with an elevated rich tooltip card featuring an optional subhead and body.
pub fn rich_tooltip<State, Action, V>(
    subhead: Option<&str>,
    body: impl Into<ArcStr>,
    child: V,
) -> HoverTooltipView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    let sub: Option<ArcStr> = subhead.map(ArcStr::from);
    let body: ArcStr = body.into();
    HoverTooltipView {
        child,
        spec_builder: Arc::new(move |spec: &DesignSpec| TooltipContentSpec {
            subhead: sub.clone(),
            body: body.clone(),
            bg: spec.palette.surface_high,
            subhead_color: spec.palette.text_primary,
            body_color: spec.palette.text_secondary,
            shadow_color: spec.palette.shadow,
            corner_radius: spec.metrics.card_radius,
            is_rich: true,
        }),
        phantom: PhantomData,
    }
}
