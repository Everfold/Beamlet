//! Spec-driven 1/2/3-line list items (static and clickable) with slots and dividers.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, button, flex_col, flex_row, label};
use xilem::{AnyWidgetView, WidgetView};

use crate::components::divider::horizontal_divider;
use crate::foundation::UiClassExt;
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Slot view factory stored in [`UiListItem`].
pub type ListSlotFactory<State, Action> =
    Arc<dyn Fn(&DesignSpec) -> Box<AnyWidgetView<State, Action>> + Send + Sync>;

/// List click handler type.
pub type ListClickCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Reusable 1/2/3-line list row component.
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

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
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
                xilem::Color::TRANSPARENT
            };
            row.padding(Padding::from_vh(8.px(), 12.px()))
                .background_color(bg)
                .corner_radius(spec.metrics.nav_pill_radius.px())
                .boxed()
        };

        if self.show_divider {
            flex_col((item, horizontal_divider())).boxed()
        } else {
            item
        }
    }
}

crate::impl_spec_view!(UiListItem<State, Action>);

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
