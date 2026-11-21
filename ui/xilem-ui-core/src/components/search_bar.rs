//! Pill/rounded search bar honoring `search_height` and `search_radius` tokens.

use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, button, flex_row, sized_box, text_input};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Callback invoked when the search query text changes.
pub type SearchChangeCallback<State, Action> =
    Arc<dyn Fn(&mut State, String) -> Action + Send + Sync>;

/// Callback invoked when the user clears or submits the search bar.
pub type SearchActionCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Spec-aware search bar with leading search icon and optional clear action.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiSearchBar<State, Action> {
    query: String,
    placeholder: ArcStr,
    on_change: SearchChangeCallback<State, Action>,
    on_clear: Option<SearchActionCallback<State, Action>>,
}

impl<State, Action> UiSearchBar<State, Action> {
    /// Overrides the placeholder string (defaults to `"Search..."`).
    pub fn placeholder(mut self, text: impl Into<ArcStr>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Displays a clear button when `query` is non-empty and invokes `on_clear`.
    pub fn on_clear<F>(mut self, on_clear: F) -> Self
    where
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        self.on_clear = Some(Arc::new(on_clear));
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let m = &spec.metrics;
        let style = spec.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();

        let mut ti = text_input(self.query.clone(), move |s: &mut State, val: String| {
            on_change(s, val)
        })
        .placeholder(self.placeholder.clone())
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(p.text_primary);
        if let Some(font) = spec.typography.font_family() {
            ti = ti.font(font.clone());
        }
        let clear_btn = if !self.query.is_empty()
            && let Some(on_clear) = &self.on_clear
        {
            let on_clear = on_clear.clone();
            Some(
                button(ui_icon(icons::CLOSE, 16.0, p.text_secondary), move |s| {
                    on_clear(s)
                })
                .ui_class(classes::ICON_BTN_STD),
            )
        } else {
            None
        };

        sized_box(
            flex_row((
                ui_icon(icons::SEARCH, 18.0, p.text_secondary),
                ti.ui_class(classes::TEXT_FIELD_FLUSH).flex(1.0),
                clear_btn,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
            .padding(Padding::from_vh(0.px(), 14.px())),
        )
        .dims(Dimensions::height(m.search_height.px()))
        .background_color(p.surface_highest)
        .border(p.border, 1.px())
        .corner_radius(m.search_radius.px())
        .boxed()
    }
}

crate::impl_spec_view!(UiSearchBar<State, Action>);

/// Creates a spec-styled search input bar.
pub fn search_bar<State: 'static, Action: 'static, F>(
    query: impl Into<String>,
    on_change: F,
) -> UiSearchBar<State, Action>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    UiSearchBar {
        query: query.into(),
        placeholder: "Search...".into(),
        on_change: Arc::new(on_change),
        on_clear: None,
    }
}
