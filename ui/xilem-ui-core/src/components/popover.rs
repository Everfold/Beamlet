//! Spec-styled arbitrary interactive popover overlay surface (`NSPopover` / `GtkPopover` / Fluent `Flyout`).

use xilem::kurbo::Vec2;
use xilem::masonry::layout::Dim;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, button, flex_col, label, sized_box, zstack};
use xilem::{WidgetView, masonry::layout::AsUnit};

use crate::foundation::{PopoverAnchorRect, UiClassExt, escape_catcher, popover_place};
use crate::style_engine::classes;

/// Spec-styled light-dismiss interactive popover surface placed adjacent to `anchor_rect`.
///
/// Clicking outside the popover or pressing `Escape` invokes `on_dismiss`. Inside `content`,
/// arbitrary reactive Xilem widgets (buttons, switches, sliders, inputs) behave normally.
pub fn popover_surface<State, Action, ContentV, OnDismiss>(
    anchor_rect: PopoverAnchorRect,
    content: ContentV,
    on_dismiss: OnDismiss,
) -> impl WidgetView<State, Action>
where
    State: 'static,
    Action: 'static,
    ContentV: WidgetView<State, Action>,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + Clone + 'static,
{
    let scrim_cb = on_dismiss.clone();
    let scrim = button(label(""), move |s: &mut State| scrim_cb(s))
        .ui_class(classes::SCRIM_TRANSPARENT)
        .dims(Dimensions::STRETCH);

    let panel = sized_box(
        flex_col((content,))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .padding(Padding::all(12.px())),
    )
    .ui_class(classes::POPOVER_SURFACE)
    .dims(Dimensions::new(Dim::MaxContent, Dim::MaxContent));

    let placed = popover_place(anchor_rect, Vec2::new(0.0, 6.0), panel).dims(Dimensions::STRETCH);

    escape_catcher(zstack((scrim, placed)), on_dismiss).dims(Dimensions::STRETCH)
}
