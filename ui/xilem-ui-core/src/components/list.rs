//! Spec-driven 1/2/3-line list items and open-content list row helpers.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::peniko::Color;
use masonry::properties::Padding;
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, Flex, FlexExt, button, flex_col, flex_row, label, sized_box,
};

use crate::components::divider::horizontal_divider;
use crate::foundation::UiClassExt;
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::{DesignSpec, TypeRole};

/// Slot view factory stored in [`UiListItem`].
pub type ListSlotFactory<State, Action> =
    Arc<dyn Fn(&DesignSpec) -> Box<AnyWidgetView<State, Action>> + Send + Sync>;

/// List click handler type.
pub type ListClickCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

type ListOuterFlex<State, Action> = Flex<
    (
        Box<AnyWidgetView<State, Action>>,
        Option<Box<AnyWidgetView<State, Action>>>,
    ),
    State,
    Action,
>;

/// Reusable 1/2/3-line list row component exposing `Pod<widgets::Flex>` directly.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains slot view closures")]
pub struct UiListItem<State, Action> {
    headline: ArcStr,
    overline: Option<ArcStr>,
    supporting_text: Option<ArcStr>,
    leading: Option<ListSlotFactory<State, Action>>,
    trailing: Option<ListSlotFactory<State, Action>>,
    selected: bool,
    show_divider: bool,
    on_click: Option<ListClickCallback<State, Action>>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action> UiListItem<State, Action> {
    /// Sets small uppercase/overline caption above the headline.
    pub fn overline(mut self, text: impl Into<ArcStr>) -> Self {
        self.overline = Some(text.into());
        self
    }

    /// Sets supporting subtitle text below the headline.
    pub fn supporting_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.supporting_text = Some(text.into());
        self
    }

    /// Attaches an arbitrary leading slot view.
    pub fn leading<F, V>(mut self, f: F) -> Self
    where
        F: Fn(&DesignSpec) -> V + Send + Sync + 'static,
        V: WidgetView<State, Action>,
        State: 'static,
        Action: 'static,
    {
        self.leading = Some(Arc::new(move |spec| f(spec).boxed()));
        self
    }

    /// Attaches an arbitrary trailing slot view.
    pub fn trailing<F, V>(mut self, f: F) -> Self
    where
        F: Fn(&DesignSpec) -> V + Send + Sync + 'static,
        V: WidgetView<State, Action>,
        State: 'static,
        Action: 'static,
    {
        self.trailing = Some(Arc::new(move |spec| f(spec).boxed()));
        self
    }

    /// Highlights the row as selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Appends a hairline divider below the row.
    pub fn show_divider(mut self, show: bool) -> Self {
        self.show_divider = show;
        self
    }

    fn build_flex(&self, spec: &DesignSpec) -> ListOuterFlex<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let ink = if self.selected {
            p.on_secondary_container
        } else {
            p.text_primary
        };
        let sub_ink = if self.selected {
            p.on_secondary_container
        } else {
            p.text_secondary
        };

        let overline = self.overline.as_ref().map(|t| {
            spec.typography
                .style_label(TypeRole::LabelSmall, label(t.clone()))
                .color(sub_ink)
        });
        let head = spec
            .typography
            .style_label(TypeRole::BodyLarge, label(self.headline.clone()))
            .color(ink);
        let supporting = self.supporting_text.as_ref().map(|t| {
            spec.typography
                .style_label(TypeRole::BodyMedium, label(t.clone()))
                .color(sub_ink)
        });

        let text_col = flex_col((overline, head, supporting))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(2.px());
        let lead = self.leading.as_ref().map(|f| f(spec));
        let trail = self.trailing.as_ref().map(|f| f(spec));

        let row = flex_row((lead, text_col.flex(1.0), trail))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px());

        let item: Box<AnyWidgetView<State, Action>> = if let Some(on_click) = &self.on_click {
            let on_click = on_click.clone();
            let cls = if self.selected {
                classes::LIST_ITEM_SELECTED
            } else {
                classes::LIST_ITEM
            };
            button(row, move |state: &mut State| on_click(state))
                .ui_class(cls)
                .boxed()
        } else {
            let bg = if self.selected {
                p.secondary_container
            } else {
                Color::TRANSPARENT
            };
            row.padding(Padding::from_vh(8.px(), 12.px()))
                .background_color(bg)
                .corner_radius(spec.metrics.nav_pill_radius.px())
                .boxed()
        };

        let div = self.show_divider.then(|| horizontal_divider().boxed());
        flex_col((item, div)).cross_axis_alignment(CrossAxisAlignment::Stretch)
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiListItem<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiListItem<State, Action> {
    type Element = Pod<widgets::Flex>;
    type ViewState = (
        ListOuterFlex<State, Action>,
        <ListOuterFlex<State, Action> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let flex = self.build_flex(&spec);
        let (pod, st) = flex.build(ctx, app_state);
        (pod, (flex, st))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_flex = self.build_flex(&spec);
        next_flex.rebuild(prev_flex, st, ctx, element, app_state);
        *prev_flex = next_flex;
    }

    fn teardown(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_flex.teardown(st, ctx, element);
    }

    fn message(
        &self,
        (prev_flex, st): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_flex.message(st, message, element, app_state)
    }
}

/// Non-interactive list item row.
pub fn list_item<State: 'static, Action: 'static>(
    headline: impl Into<ArcStr>,
) -> UiListItem<State, Action> {
    UiListItem {
        headline: headline.into(),
        overline: None,
        supporting_text: None,
        leading: None,
        trailing: None,
        selected: false,
        show_divider: false,
        on_click: None,
        phantom: PhantomData,
    }
}

/// Interactive list item row firing `on_click` on press.
pub fn clickable_list_item<State: 'static, Action: 'static, F>(
    headline: impl Into<ArcStr>,
    on_click: F,
) -> UiListItem<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiListItem {
        headline: headline.into(),
        overline: None,
        supporting_text: None,
        leading: None,
        trailing: None,
        selected: false,
        show_divider: false,
        on_click: Some(Arc::new(on_click)),
        phantom: PhantomData,
    }
}

/// Non-interactive list row hosting arbitrary custom view content with standard list padding and radius.
pub fn list_item_view<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    content: V,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(content).padding(Padding::from_vh(8.px(), 12.px()))
}

/// Interactive list row hosting arbitrary custom view content with native theme hover/press feedback.
pub fn clickable_list_item_view<State: 'static, Action: 'static, V, F>(
    content: V,
    selected: bool,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cls = if selected {
        classes::LIST_ITEM_SELECTED
    } else {
        classes::LIST_ITEM
    };
    button(content, on_click).ui_class(cls)
}

/// Virtualized lazy-mounted vertical list backed by Xilem's
/// [`virtual_scroll`](xilem_masonry::view::virtual_scroll).
///
/// Only rows intersecting the visible viewport (plus a small scroll buffer) are built and mounted,
/// keeping view build, layout, and paint proportional to the visible window (`O(viewport)`)
/// even for `10_000+` items.
#[expect(
    clippy::type_complexity,
    reason = "Preserves VirtualScroll builder methods while keeping Xilem's opaque default scroll handler"
)]
pub fn virtual_list<State, Action, V, F>(
    item_count: usize,
    row_builder: F,
) -> xilem_masonry::view::VirtualScroll<
    State,
    Action,
    V,
    F,
    impl Fn(&mut State, std::ops::Range<usize>) -> MessageResult<Action> + Send + Sync + 'static,
>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State, usize) -> V + 'static,
    State: 'static,
    Action: 'static,
{
    xilem_masonry::view::virtual_scroll(item_count, row_builder)
}
