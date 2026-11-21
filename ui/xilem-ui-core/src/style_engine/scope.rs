//! Theme environment scope (`ui_scope`), root window wrapper (`ui_window`), and `impl_spec_view!`.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock, Mutex};

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::core::{DefaultProperties, FromDynWidget};
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::sized_box;
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView, WindowId, WindowView, window};

use crate::style_engine::build_default_properties;
use crate::tokens::{Brightness, DesignSpec};

thread_local! {
    static SPEC_STACK: RefCell<Vec<Arc<DesignSpec>>> = const { RefCell::new(Vec::new()) };
}

struct SpecGuard;

impl SpecGuard {
    fn push(spec: Arc<DesignSpec>) -> Self {
        SPEC_STACK.with(|stack| stack.borrow_mut().push(spec));
        Self
    }
}

impl Drop for SpecGuard {
    fn drop(&mut self) {
        SPEC_STACK.with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

type DefaultPropsCache = Vec<(DesignSpec, Arc<DefaultProperties>)>;

static DEFAULT_PROPS_CACHE: LazyLock<Mutex<DefaultPropsCache>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Returns a cached [`Arc<DefaultProperties>`] for `spec`, preserving `Arc::ptr_eq` across
/// rebuilds when `spec` has not changed while producing a fresh `Arc` on theme changes.
#[must_use]
pub fn cached_default_properties(spec: &DesignSpec) -> Arc<DefaultProperties> {
    let mut cache = DEFAULT_PROPS_CACHE
        .lock()
        .expect("default properties cache mutex poisoned");
    if let Some((_, props)) = cache.iter().find(|(cached_spec, _)| cached_spec == spec) {
        return props.clone();
    }
    let props = Arc::new(build_default_properties(spec));
    if cache.len() >= 16 {
        cache.remove(0);
    }
    cache.push((spec.clone(), props.clone()));
    props
}

/// Reads the ambient [`DesignSpec`] from the active [`ui_scope`] stack, falling back to
/// [`DesignSpec::neutral_fallback`] if no enclosing [`ui_scope`] exists.
#[must_use]
pub fn current_spec(_ctx: &ViewCtx) -> DesignSpec {
    SPEC_STACK.with(|stack| {
        stack.borrow().last().map_or_else(
            || DesignSpec::neutral_fallback(Brightness::Light),
            |s| (**s).clone(),
        )
    })
}

/// View created by [`ui_scope`] pushing an [`Arc<DesignSpec>`] around child build and rebuild.
#[must_use = "View values do nothing unless provided to Xilem."]
#[derive(Debug)]
pub struct UiScope<State, Action, V> {
    spec: Arc<DesignSpec>,
    child: V,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<State, Action, V> ViewMarker for UiScope<State, Action, V> {}

impl<State, Action, V> View<State, Action, ViewCtx> for UiScope<State, Action, V>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let _guard = SpecGuard::push(self.spec.clone());
        self.child.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let _guard = SpecGuard::push(self.spec.clone());
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

/// Wraps `child` inside a Xilem [`DesignSpec`] environment scope supporting live updates.
pub fn ui_scope<State: 'static, Action: 'static, V>(
    spec: DesignSpec,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    UiScope {
        spec: Arc::new(spec),
        child,
        phantom: PhantomData,
    }
}

/// Convenience helper constructing an opinionated desktop [`WindowView`] whose backdrop,
/// Masonry [`DefaultProperties`], and Xilem ambient [`DesignSpec`] stay synced with `spec`.
pub fn ui_window<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    spec: &DesignSpec,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    let bg = spec.palette.window_bg;
    let default_props = cached_default_properties(spec);
    let root = sized_box(child)
        .dims(Dimensions::STRETCH)
        .background_color(bg);
    window(window_id, title.into(), ui_scope(spec.clone(), root))
        .with_base_color(bg)
        .with_default_properties(default_props)
}

/// Creates a lazily evaluated view that reads the active [`DesignSpec`] on every build/rebuild.
pub fn with_spec<State, Action, F, V>(builder: F) -> WithSpec<State, Action, F, V>
where
    State: 'static,
    Action: 'static,
    F: Fn(&DesignSpec, &mut State) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
{
    WithSpec {
        builder,
        phantom: PhantomData,
    }
}

type StatePhantom<State, Action, V> = PhantomData<fn(&mut State) -> (Action, V)>;

/// View created by [`with_spec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains closure trait generic"
)]
pub struct WithSpec<State, Action, F, V> {
    builder: F,
    phantom: StatePhantom<State, Action, V>,
}

impl<State, Action, F, V> ViewMarker for WithSpec<State, Action, F, V> {}

impl<State, Action, F, V> View<State, Action, ViewCtx> for WithSpec<State, Action, F, V>
where
    State: 'static,
    Action: 'static,
    F: Fn(&DesignSpec, &mut State) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: xilem::masonry::core::FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = (DesignSpec, V, V::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec(ctx);
        let view = (self.builder)(&spec, app_state);
        let (el, state) = view.build(ctx, app_state);
        (el, (spec, view, state))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_spec, prev_view, view_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let next_spec = current_spec(ctx);
        let next_view = (self.builder)(&next_spec, app_state);
        next_view.rebuild(prev_view, view_state, ctx, element, app_state);
        *prev_spec = next_spec;
        *prev_view = next_view;
    }

    fn teardown(
        &self,
        (_spec, prev_view, view_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_view.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        (_spec, prev_view, view_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        prev_view.message(view_state, message, element, app_state)
    }
}

/// Internal runtime state stored by [`impl_spec_view!`].
#[doc(hidden)]
#[expect(
    missing_debug_implementations,
    reason = "Holds type-erased child view and ViewState"
)]
pub struct SpecViewState<State: 'static, Action: 'static> {
    spec: DesignSpec,
    view: Box<AnyWidgetView<State, Action>>,
    state: <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
}

#[doc(hidden)]
pub fn spec_view_build<State: 'static, Action: 'static>(
    ctx: &mut ViewCtx,
    app_state: &mut State,
    render: impl FnOnce(&DesignSpec) -> Box<AnyWidgetView<State, Action>>,
) -> (
    Pod<xilem::masonry::widgets::Passthrough>,
    SpecViewState<State, Action>,
) {
    let spec = current_spec(ctx);
    let view = render(&spec);
    let (el, state) = view.build(ctx, app_state);
    (el, SpecViewState { spec, view, state })
}

#[doc(hidden)]
pub fn spec_view_rebuild<State: 'static, Action: 'static>(
    view_state: &mut SpecViewState<State, Action>,
    ctx: &mut ViewCtx,
    element: Mut<'_, Pod<xilem::masonry::widgets::Passthrough>>,
    app_state: &mut State,
    render: impl FnOnce(&DesignSpec) -> Box<AnyWidgetView<State, Action>>,
) {
    let next_spec = current_spec(ctx);
    let next_view = render(&next_spec);
    next_view.rebuild(
        &view_state.view,
        &mut view_state.state,
        ctx,
        element,
        app_state,
    );
    view_state.spec = next_spec;
    view_state.view = next_view;
}

#[doc(hidden)]
pub fn spec_view_teardown<State: 'static, Action: 'static>(
    view_state: &mut SpecViewState<State, Action>,
    ctx: &mut ViewCtx,
    element: Mut<'_, Pod<xilem::masonry::widgets::Passthrough>>,
) {
    view_state
        .view
        .teardown(&mut view_state.state, ctx, element);
}

#[doc(hidden)]
pub fn spec_view_message<State: 'static, Action: 'static>(
    view_state: &mut SpecViewState<State, Action>,
    message: &mut MessageCtx,
    element: Mut<'_, Pod<xilem::masonry::widgets::Passthrough>>,
    app_state: &mut State,
) -> MessageResult<Action> {
    view_state
        .view
        .message(&mut view_state.state, message, element, app_state)
}

/// Implements [`View`] + [`ViewMarker`] for any component view struct defining
/// `fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>`.
#[macro_export]
macro_rules! impl_spec_view {
    ($ty:ident < State, Action $(, $gen:ident )* > $(where $($bounds:tt)+)?) => {
        impl<State, Action $(, $gen)*> xilem::core::ViewMarker for $ty<State, Action $(, $gen)*>
        $(where $($bounds)+)?
        {}

        impl<State, Action $(, $gen)*> xilem::core::View<State, Action, xilem::ViewCtx>
            for $ty<State, Action $(, $gen)*>
        where
            State: 'static,
            Action: 'static,
            $($($bounds)+)?
        {
            type Element = xilem::Pod<xilem::masonry::widgets::Passthrough>;
            type ViewState = $crate::style_engine::scope::SpecViewState<State, Action>;

            fn build(
                &self,
                ctx: &mut xilem::ViewCtx,
                app_state: &mut State,
            ) -> (Self::Element, Self::ViewState) {
                $crate::style_engine::scope::spec_view_build(ctx, app_state, |spec| {
                    self.render_with_spec(spec)
                })
            }

            fn rebuild(
                &self,
                _prev: &Self,
                view_state: &mut Self::ViewState,
                ctx: &mut xilem::ViewCtx,
                element: xilem::core::Mut<'_, Self::Element>,
                app_state: &mut State,
            ) {
                $crate::style_engine::scope::spec_view_rebuild(
                    view_state,
                    ctx,
                    element,
                    app_state,
                    |spec| self.render_with_spec(spec),
                );
            }

            fn teardown(
                &self,
                view_state: &mut Self::ViewState,
                ctx: &mut xilem::ViewCtx,
                element: xilem::core::Mut<'_, Self::Element>,
            ) {
                $crate::style_engine::scope::spec_view_teardown(view_state, ctx, element);
            }

            fn message(
                &self,
                view_state: &mut Self::ViewState,
                message: &mut xilem::core::MessageCtx,
                element: xilem::core::Mut<'_, Self::Element>,
                app_state: &mut State,
            ) -> xilem::core::MessageResult<Action> {
                $crate::style_engine::scope::spec_view_message(
                    view_state, message, element, app_state,
                )
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_stack_spec() -> DesignSpec {
        SPEC_STACK.with(|stack| {
            stack.borrow().last().map_or_else(
                || DesignSpec::neutral_fallback(Brightness::Light),
                |s| (**s).clone(),
            )
        })
    }

    #[test]
    fn spec_stack_nests_and_restores_on_drop() {
        assert_eq!(read_stack_spec().brightness, Brightness::Light);
        let dark = Arc::new(DesignSpec::neutral_fallback(Brightness::Dark));
        let outer = SpecGuard::push(dark);
        assert_eq!(read_stack_spec().brightness, Brightness::Dark);

        {
            let mut inner_spec = DesignSpec::neutral_fallback(Brightness::Light);
            inner_spec.name = "Nested";
            let _inner = SpecGuard::push(Arc::new(inner_spec));
            assert_eq!(read_stack_spec().name, "Nested");
            assert_eq!(read_stack_spec().brightness, Brightness::Light);
        }

        assert_eq!(read_stack_spec().brightness, Brightness::Dark);
        drop(outer);
        assert_eq!(read_stack_spec().brightness, Brightness::Light);
    }

    #[test]
    fn cached_default_properties_reuses_arc_for_equal_spec() {
        let light = DesignSpec::neutral_fallback(Brightness::Light);
        let dark = DesignSpec::neutral_fallback(Brightness::Dark);
        let a = cached_default_properties(&light);
        let b = cached_default_properties(&light);
        let c = cached_default_properties(&dark);
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
    }
}
