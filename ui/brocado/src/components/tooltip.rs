//! Plain and rich tooltips backed by `HoverTooltipHost` and `TooltipLayer`.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::core::ArcStr;
use xilem_masonry::WidgetView;

use crate::foundation::overlay::{HoverTooltipView, TooltipContentSpec};
use crate::theme::Theme;
use crate::theme::metrics::{CARD_RADIUS, MENU_RADIUS};

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
        spec_builder: Arc::new(move |theme: &Theme| {
            let s = &theme.scheme;
            TooltipContentSpec {
                subhead: None,
                body: body.clone(),
                font_family: theme.typography.resolved_font_family(),
                bg: s.inverse_surface,
                subhead_color: s.inverse_on_surface,
                body_color: s.inverse_on_surface,
                shadow_color: s.shadow,
                corner_radius: (MENU_RADIUS * 0.5).max(4.0),
                is_rich: false,
                max_width: 240.0,
                hover_delay_ms: 450,
            }
        }),
        max_width: None,
        hover_delay_ms: None,
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
        spec_builder: Arc::new(move |theme: &Theme| {
            let s = &theme.scheme;
            TooltipContentSpec {
                subhead: sub.clone(),
                body: body.clone(),
                font_family: theme.typography.resolved_font_family(),
                bg: s.surface_container_high,
                subhead_color: s.on_surface,
                body_color: s.on_surface_variant,
                shadow_color: s.shadow,
                corner_radius: CARD_RADIUS,
                is_rich: true,
                max_width: 320.0,
                hover_delay_ms: 450,
            }
        }),
        max_width: None,
        hover_delay_ms: None,
        phantom: PhantomData,
    }
}
