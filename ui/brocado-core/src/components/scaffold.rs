//! Application screen scaffold coordinating top bar, body, bottom bar, FAB, sheets, dialogs, and toasts.

use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::Padding;
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, sized_box, zstack};

/// Screen layout scaffold holding body content plus optional chrome and modal overlay slots.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Holds type-erased child views"
)]
pub struct UiScaffold<State: 'static, Action: 'static> {
    body: Box<AnyWidgetView<State, Action>>,
    top_bar: Option<Box<AnyWidgetView<State, Action>>>,
    bottom_bar: Option<Box<AnyWidgetView<State, Action>>>,
    floating_action: Option<Box<AnyWidgetView<State, Action>>>,
    modal_drawer: Option<Box<AnyWidgetView<State, Action>>>,
    sheet: Option<Box<AnyWidgetView<State, Action>>>,
    dialog: Option<Box<AnyWidgetView<State, Action>>>,
    toast: Option<Box<AnyWidgetView<State, Action>>>,
}

impl<State: 'static, Action: 'static> UiScaffold<State, Action> {
    /// Places a top app bar or platform header bar at the top edge.
    pub fn top_bar<V: WidgetView<State, Action>>(mut self, view: V) -> Self {
        self.top_bar = Some(view.boxed());
        self
    }

    /// Places a bottom navigation bar or bottom bar along the bottom edge.
    pub fn bottom_bar<V: WidgetView<State, Action>>(mut self, view: V) -> Self {
        self.bottom_bar = Some(view.boxed());
        self
    }

    /// Anchors a floating action view at the bottom-right corner above the bottom bar.
    pub fn floating_action<V: WidgetView<State, Action>>(mut self, view: V) -> Self {
        self.floating_action = Some(view.boxed());
        self
    }

    /// Anchors an optional modal drawer overlay above the main scaffold content.
    pub fn modal_drawer<V: WidgetView<State, Action>>(mut self, view: Option<V>) -> Self {
        self.modal_drawer = view.map(|v| v.boxed());
        self
    }

    /// Anchors an optional modal bottom/side sheet overlay.
    pub fn sheet<V: WidgetView<State, Action>>(mut self, view: Option<V>) -> Self {
        self.sheet = view.map(|v| v.boxed());
        self
    }

    /// Anchors an optional modal dialog overlay.
    pub fn dialog<V: WidgetView<State, Action>>(mut self, view: Option<V>) -> Self {
        self.dialog = view.map(|v| v.boxed());
        self
    }

    /// Anchors a toast/snackbar host banner along the bottom center.
    pub fn toast<V: WidgetView<State, Action>>(mut self, view: V) -> Self {
        self.toast = Some(view.boxed());
        self
    }

    /// Finalizes the scaffold into a single composable [`WidgetView`].
    pub fn build(self) -> impl WidgetView<State, Action, Widget = widgets::ZStack> {
        let fab_layer = self.floating_action.map(|fab| {
            sized_box(fab)
                .padding(Padding::all(16.px()))
                .alignment(UnitPoint::BOTTOM_RIGHT)
        });

        let toast_layer = self.toast.map(|t| {
            sized_box(t)
                .padding(Padding::all(16.px()))
                .alignment(UnitPoint::BOTTOM)
        });

        // Host FAB and toast inside the flexible body stack so they automatically sit above `bottom_bar`.
        let body_area = zstack((
            self.body.alignment(UnitPoint::CENTER),
            fab_layer,
            toast_layer,
        ));

        let base_col = flex_col((self.top_bar, body_area.flex(1.0), self.bottom_bar))
            .cross_axis_alignment(CrossAxisAlignment::Stretch);

        let drawer_layer = self.modal_drawer.map(|d| d.alignment(UnitPoint::LEFT));
        let sheet_layer = self.sheet.map(|s| s.alignment(UnitPoint::CENTER));
        let dialog_layer = self.dialog.map(|d| d.alignment(UnitPoint::CENTER));

        zstack((
            base_col.alignment(UnitPoint::CENTER),
            drawer_layer,
            sheet_layer,
            dialog_layer,
        ))
    }
}

/// Starts building a [`UiScaffold`] around `body`.
pub fn scaffold<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    body: V,
) -> UiScaffold<State, Action> {
    UiScaffold {
        body: body.boxed(),
        top_bar: None,
        bottom_bar: None,
        floating_action: None,
        modal_drawer: None,
        sheet: None,
        dialog: None,
        toast: None,
    }
}
