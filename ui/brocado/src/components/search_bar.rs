//! Pill/rounded search bar backed directly by a reactive [`FocusFrame`].

use std::sync::Arc;

use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::properties::Padding;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt, button, flex_row, text_input};

use crate::foundation::{
    FocusFrame, FocusFrameView, UiClassExt, WithClass, focus_frame, icons, ui_icon,
};
use crate::style_engine::{classes, current_theme_arc};
use crate::theme::Theme;
use crate::theme::metrics::SEARCH_RADIUS;
use crate::tokens::TypeRole;

/// Callback invoked when the search query text changes.
pub type SearchChangeCallback<State, Action> =
    Arc<dyn Fn(&mut State, String) -> Action + Send + Sync>;

/// Callback invoked when the user clears or submits the search bar.
pub type SearchActionCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

type SearchBarFrame<State, Action> =
    WithClass<FocusFrameView<Box<AnyWidgetView<State, Action>>, State, Action>, State, Action>;

/// Spec-aware search bar exposing `Pod<FocusFrame>` directly for styling and reactive focus indicators.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiSearchBar<State, Action> {
    query: String,
    placeholder: Option<ArcStr>,
    on_change: SearchChangeCallback<State, Action>,
    on_clear: Option<SearchActionCallback<State, Action>>,
}

impl<State, Action> UiSearchBar<State, Action> {
    /// Overrides the placeholder string (defaults to `theme.locale.search_placeholder`).
    pub fn placeholder(mut self, text: impl Into<ArcStr>) -> Self {
        self.placeholder = Some(text.into());
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

    fn build_frame(&self, theme: &Theme) -> SearchBarFrame<State, Action>
    where
        State: 'static,
        Action: 'static,
    {
        let s = &theme.scheme;
        let style = theme.typography.get(TypeRole::BodyMedium);
        let on_change = self.on_change.clone();

        let ti = text_input(self.query.clone(), move |st: &mut State, val: String| {
            on_change(st, val)
        })
        .placeholder(
            self.placeholder
                .clone()
                .unwrap_or_else(|| theme.locale.search_placeholder.clone()),
        )
        .text_size(style.font_size)
        .weight(style.weight)
        .text_color(s.on_surface)
        .font(theme.typography.resolved_font_family());
        let clear_btn = if !self.query.is_empty()
            && let Some(on_clear) = &self.on_clear
        {
            let on_clear = on_clear.clone();
            Some(
                button(
                    ui_icon(icons::CLOSE, 16.0, s.on_surface_variant),
                    move |st| on_clear(st),
                )
                .ui_class(classes::ICON_BTN_STD),
            )
        } else {
            None
        };

        let inner = flex_row((
            ui_icon(icons::SEARCH, 18.0, s.on_surface_variant),
            ti.ui_class(classes::TEXT_FIELD_FLUSH).flex(1.0),
            clear_btn,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px())
        .padding(Padding::from_vh(0.px(), 10.px()))
        .boxed();

        focus_frame(inner)
            .radius(SEARCH_RADIUS)
            .ui_class(classes::SEARCH_BAR)
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiSearchBar<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiSearchBar<State, Action> {
    type Element = Pod<FocusFrame>;
    type ViewState = (
        SearchBarFrame<State, Action>,
        <SearchBarFrame<State, Action> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let frame = self.build_frame(&theme);
        let (pod, st) = frame.build(ctx, app_state);
        (pod, (frame, st))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_frame, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let theme = current_theme_arc(ctx);
        let next_frame = self.build_frame(&theme);
        next_frame.rebuild(prev_frame, st, ctx, element, app_state);
        *prev_frame = next_frame;
    }

    fn teardown(
        &self,
        (prev_frame, st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_frame.teardown(st, ctx, element);
    }

    fn message(
        &self,
        (prev_frame, st): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_frame.message(st, message, element, app_state)
    }
}

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
        placeholder: None,
        on_change: Arc::new(on_change),
        on_clear: None,
    }
}
