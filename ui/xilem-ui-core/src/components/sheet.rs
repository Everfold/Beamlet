//! Modal bottom sheet and trailing side sheet anchored via `modal_shell`.

use std::marker::PhantomData;
use std::sync::Arc;

use masonry::kurbo::Vec2;
use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::{Dimensions, Padding};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::foundation::overlay::{
    EscapeCatcher, ModalShellConfig, ModalShellState, modal_shell_build, modal_shell_message,
    modal_shell_rebuild, modal_shell_teardown,
};
use crate::style_engine::classes;
use crate::tokens::MotionDuration;

/// Callback invoked when a sheet is dismissed.
pub type SheetDismissCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SheetEdge {
    Bottom,
    Side,
}

/// Modal sheet overlay (`bottom_sheet` or `side_sheet`).
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiSheet<V, State, Action> {
    edge: SheetEdge,
    size: f64,
    content: V,
    on_dismiss: SheetDismissCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State, Action> UiSheet<V, State, Action> {
    /// Overrides the sheet's height (bottom sheet) or width (side sheet).
    pub fn extent(mut self, size: f64) -> Self {
        self.size = size;
        self
    }

    fn shell_config(&self) -> ModalShellConfig {
        match self.edge {
            SheetEdge::Bottom => ModalShellConfig {
                class: classes::SHEET_SURFACE,
                dims: Dimensions::STRETCH.with_height(self.size.px()),
                padding: Padding::all(20.px()),
                alignment: UnitPoint::BOTTOM,
                enter_offset: Vec2::new(0.0, 48.0),
                duration: MotionDuration::Medium3,
                dismiss_on_scrim: true,
            },
            SheetEdge::Side => ModalShellConfig {
                class: classes::SHEET_SURFACE,
                dims: Dimensions::STRETCH.with_width(self.size.px()),
                padding: Padding::all(20.px()),
                alignment: UnitPoint::RIGHT,
                enter_offset: Vec2::new(48.0, 0.0),
                duration: MotionDuration::Medium3,
                dismiss_on_scrim: true,
            },
        }
    }
}

impl<V, State, Action> ViewMarker for UiSheet<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiSheet<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = Pod<EscapeCatcher>;
    type ViewState = ModalShellState<State, Action, V::ViewState>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        modal_shell_build(
            ctx,
            app_state,
            self.shell_config(),
            &self.content,
            self.on_dismiss.clone(),
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        modal_shell_rebuild(
            view_state,
            ctx,
            element,
            app_state,
            self.shell_config(),
            &prev.content,
            &self.content,
            self.on_dismiss.clone(),
        );
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        modal_shell_teardown(view_state, ctx, element, &self.content);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        modal_shell_message(
            view_state,
            message,
            element,
            app_state,
            &self.content,
            &self.on_dismiss,
        )
    }
}

/// Modal sheet sliding up from the bottom edge of the window.
pub fn bottom_sheet<State: 'static, Action: 'static, V, F>(
    content: V,
    on_dismiss: F,
) -> UiSheet<V, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiSheet {
        edge: SheetEdge::Bottom,
        size: 320.0,
        content,
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

/// Modal sheet sliding in from the trailing right edge of the window.
pub fn side_sheet<State: 'static, Action: 'static, V, F>(
    content: V,
    on_dismiss: F,
) -> UiSheet<V, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiSheet {
        edge: SheetEdge::Side,
        size: 360.0,
        content,
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}
