//! Spec-colored horizontal, vertical, and inset dividers.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{divider_h, divider_v};

use crate::style_engine::with_spec;

/// Full-bleed horizontal hairline divider colored by the active theme's `divider` token.
pub fn horizontal_divider<State: 'static, Action: 'static>()
-> impl WidgetView<State, Action, Widget = widgets::Divider> {
    with_spec(|spec, _| divider_h().color(spec.palette.divider))
}

/// Full-height vertical hairline divider colored by the active theme's `divider` token.
pub fn vertical_divider<State: 'static, Action: 'static>()
-> impl WidgetView<State, Action, Widget = widgets::Divider> {
    with_spec(|spec, _| divider_v().color(spec.palette.divider))
}

/// Horizontal divider padded on the left/right edges.
pub fn inset_divider<State: 'static, Action: 'static>(
    left_inset: f64,
    right_inset: f64,
) -> impl WidgetView<State, Action, Widget = widgets::Divider> {
    with_spec(move |spec, _| {
        divider_h().color(spec.palette.divider).padding(Padding {
            left: left_inset.px(),
            right: right_inset.px(),
            top: 0.px(),
            bottom: 0.px(),
        })
    })
}
