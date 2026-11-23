//! View wrapper attaching Masonry `PropertyStack` style classes cleanly.

use std::marker::PhantomData;

use masonry::core::FromDynWidget;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

/// Wraps a [`WidgetView`] to attach a Masonry class selector without altering the widget hierarchy.
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct WithClass<V, State, Action> {
    child: V,
    class: Option<&'static str>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State, Action> ViewMarker for WithClass<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for WithClass<V, State, Action>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<V::Widget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (mut pod, view_state) = self.child.build(ctx, app_state);
        if let Some(class) = self.class {
            pod.new_widget.classes.insert(class.to_string());
        }
        (pod, view_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        if prev.class != self.class {
            if let Some(old_class) = prev.class {
                element.ctx.remove_class(old_class);
            }
            if let Some(new_class) = self.class {
                element.ctx.add_class(new_class);
            }
        }
        self.child
            .rebuild(&prev.child, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.child.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.child.message(view_state, message, element, app_state)
    }
}

/// Extension trait adding `.ui_class(...)` and `.ui_class_if(...)` to any [`WidgetView`].
pub trait UiClassExt<State: 'static, Action>: WidgetView<State, Action> + Sized {
    /// Attaches a Masonry class selector (e.g. `"ui.btn.primary"`).
    fn ui_class(self, class: &'static str) -> WithClass<Self, State, Action> {
        WithClass {
            child: self,
            class: Some(class),
            phantom: PhantomData,
        }
    }

    /// Conditionally attaches `class` when `enabled` is `true`.
    fn ui_class_if(self, enabled: bool, class: &'static str) -> WithClass<Self, State, Action> {
        WithClass {
            child: self,
            class: enabled.then_some(class),
            phantom: PhantomData,
        }
    }
}

impl<V, State: 'static, Action> UiClassExt<State, Action> for V where
    V: WidgetView<State, Action> + Sized
{
}
