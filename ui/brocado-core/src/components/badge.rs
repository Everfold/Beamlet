//! Notification dot and counter badges styled from [`DesignSpec`](crate::tokens::DesignSpec).

use masonry::core::ArcStr;
use masonry::layout::UnitPoint;
use masonry::parley::LineHeight;
use masonry::properties::LineBreaking;
use masonry::widgets;
use xilem_masonry::WidgetView;
use xilem_masonry::view::{ZStackExt, badged_optional, label, sized_box, zstack};

use crate::components::typography::ui_label;
use crate::foundation::UiClassExt;
use crate::style_engine::{classes, with_spec};
use crate::tokens::TypeRole;

/// Returns a small status dot badge (`6×6`).
pub fn small_badge<State: 'static, Action: 'static>()
-> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(label("")).ui_class(classes::BADGE_SMALL)
}

/// Returns a numeric/text pill badge.
pub fn large_badge<State: 'static, Action: 'static>(
    text: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let text = text.into();
    with_spec(move |spec, _| {
        let txt = ui_label(TypeRole::LabelSmall, text.clone())
            .line_height(LineHeight::MetricsRelative(1.0))
            .color(spec.palette.on_error)
            .line_breaking(LineBreaking::Clip)
            .alignment(UnitPoint::CENTER);
        sized_box(zstack((txt,))).ui_class(classes::BADGE_LARGE)
    })
}

/// Anchors an optional badge over the top-right corner of `anchor`.
pub fn with_badge<State: 'static, Action: 'static, A, B>(
    anchor: A,
    badge: Option<B>,
) -> impl WidgetView<State, Action>
where
    A: WidgetView<State, Action>,
    B: WidgetView<State, Action>,
{
    badged_optional(anchor, badge)
}
