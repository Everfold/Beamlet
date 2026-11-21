//! Spec-colored horizontal, vertical, and inset dividers.

use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{divider_h, divider_v};
use xilem::{AnyWidgetView, WidgetView};

use crate::style_engine::with_spec;
use crate::tokens::DesignSpec;

fn divider_color(spec: &DesignSpec) -> xilem::Color {
    spec.palette.divider
}

/// Full-bleed horizontal hairline divider colored by the active theme's `divider` token.
pub fn horizontal_divider<State: 'static, Action: 'static>() -> impl WidgetView<State, Action> {
    with_spec(|spec, _| divider_h().color(divider_color(spec)))
}

/// Full-height vertical hairline divider colored by the active theme's `divider` token.
pub fn vertical_divider<State: 'static, Action: 'static>() -> impl WidgetView<State, Action> {
    with_spec(|spec, _| divider_v().color(divider_color(spec)))
}

/// Horizontal divider padded on the left/right edges.
pub fn inset_divider<State: 'static, Action: 'static>(
    left_inset: f64,
    right_inset: f64,
) -> Box<AnyWidgetView<State, Action>> {
    with_spec(move |spec, _| {
        divider_h()
            .color(divider_color(spec))
            .padding(xilem::masonry::properties::Padding {
                left: left_inset.px(),
                right: right_inset.px(),
                top: 0.px(),
                bottom: 0.px(),
            })
            .boxed()
    })
    .boxed()
}

fn _touch_as_unit() {
    let _ = 1.px();
}
