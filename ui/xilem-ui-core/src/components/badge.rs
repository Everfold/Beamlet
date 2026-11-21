//! Notification dot and counter badges styled from [`DesignSpec`].

use std::marker::PhantomData;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::UnitPoint;
use xilem::style::Style as _;
use xilem::view::{badged_optional, label, sized_box};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::UiClassExt;
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Small `6px` status dot badge view.
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiSmallBadge<State, Action> {
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiSmallBadge<State, Action> {
    fn render_with_spec(&self, _spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        sized_box(label("")).ui_class(classes::BADGE_SMALL).boxed()
    }
}

crate::impl_spec_view!(UiSmallBadge<State, Action>);

/// Returns a small status dot badge.
pub fn small_badge<State: 'static, Action: 'static>() -> UiSmallBadge<State, Action> {
    UiSmallBadge {
        phantom: PhantomData,
    }
}

/// Pill counter badge view.
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiLargeBadge<State, Action> {
    text: ArcStr,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiLargeBadge<State, Action> {
    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let txt = spec
            .typography
            .style_label(TypeRole::LabelSmall, label(self.text.clone()))
            .color(spec.palette.on_error);
        sized_box(txt).ui_class(classes::BADGE_LARGE).boxed()
    }
}

crate::impl_spec_view!(UiLargeBadge<State, Action>);

/// Returns a numeric/text pill badge.
pub fn large_badge<State: 'static, Action: 'static>(
    text: impl Into<ArcStr>,
) -> UiLargeBadge<State, Action> {
    UiLargeBadge {
        text: text.into(),
        phantom: PhantomData,
    }
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
    let _ = UnitPoint::TOP_RIGHT;
    badged_optional(anchor, badge)
}
