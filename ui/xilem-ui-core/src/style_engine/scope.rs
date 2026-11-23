//! Theme environment scope (`ui_scope`) and root window wrapper (`ui_window`).

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock, Mutex};

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::core::{DefaultProperties, FromDynWidget};
use xilem::masonry::properties::Dimensions;
use xilem::style::Style as _;
use xilem::view::sized_box;
use xilem::{Pod, ViewCtx, WidgetView, WindowId, WindowView, window};

use crate::style_engine::build_default_properties;
use crate::tokens::{Brightness, DesignSpec, WindowBackdrop};

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

static FALLBACK_LIGHT_SPEC: LazyLock<Arc<DesignSpec>> =
    LazyLock::new(|| Arc::new(DesignSpec::neutral_fallback(Brightness::Light)));

/// Reads a shared `Arc` handle to the ambient [`DesignSpec`] from the active [`ui_scope`]
/// stack without deep-cloning `DesignSpec`, falling back to a cached neutral fallback.
#[must_use]
pub fn current_spec_arc(_ctx: &ViewCtx) -> Arc<DesignSpec> {
    SPEC_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .map_or_else(|| FALLBACK_LIGHT_SPEC.clone(), Arc::clone)
    })
}

/// Reads the ambient [`DesignSpec`] from the active [`ui_scope`] stack, falling back to
/// [`DesignSpec::neutral_fallback`] if no enclosing [`ui_scope`] exists.
#[must_use]
pub fn current_spec(ctx: &ViewCtx) -> DesignSpec {
    (*current_spec_arc(ctx)).clone()
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

/// Wraps `child` inside a Xilem [`DesignSpec`] environment scope using a pre-allocated [`Arc<DesignSpec>`].
pub fn ui_scope_arc<State: 'static, Action: 'static, V>(
    spec: Arc<DesignSpec>,
    child: V,
) -> UiScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    UiScope {
        spec,
        child,
        phantom: PhantomData,
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
    ui_scope_arc(Arc::new(spec), child)
}

/// Resolves the root window backdrop fill for `backdrop`.
///
/// Translucent materials (`Mica`, `MicaAlt`, `Acrylic`, `MacosVibrancy`) require host OS
/// compositor support (`winit` transparent surface + Windows DWM / macOS `NSVisualEffectView`);
/// when rendered in standard opaque or headless surfaces, all modes fall back gracefully to
/// `spec.palette.window_bg`.
fn resolve_window_backdrop_fill(spec: &DesignSpec, backdrop: WindowBackdrop) -> xilem::Color {
    match backdrop {
        WindowBackdrop::Opaque
        | WindowBackdrop::Mica
        | WindowBackdrop::MicaAlt
        | WindowBackdrop::Acrylic
        | WindowBackdrop::MacosVibrancy => spec.palette.window_bg,
    }
}

/// Convenience helper constructing an opinionated desktop [`WindowView`] from a shared
/// [`Arc<DesignSpec>`] and explicit [`WindowBackdrop`] material.
pub fn ui_window_arc_with_backdrop<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    spec: &Arc<DesignSpec>,
    backdrop: WindowBackdrop,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    let bg = resolve_window_backdrop_fill(spec, backdrop);
    let default_props = cached_default_properties(spec);
    let root = sized_box(child)
        .dims(Dimensions::STRETCH)
        .background_color(bg);
    window(window_id, title.into(), ui_scope_arc(spec.clone(), root))
        .with_base_color(bg)
        .with_default_properties(default_props)
}

/// Convenience helper constructing an opinionated desktop [`WindowView`] from `spec` and an
/// explicit [`WindowBackdrop`] material.
pub fn ui_window_with_backdrop<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    spec: &DesignSpec,
    backdrop: WindowBackdrop,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    ui_window_arc_with_backdrop(window_id, title, &Arc::new(spec.clone()), backdrop, child)
}

/// Convenience helper constructing an opinionated desktop [`WindowView`] from a shared [`Arc<DesignSpec>`].
pub fn ui_window_arc<State: 'static, V>(
    window_id: WindowId,
    title: impl Into<String>,
    spec: &Arc<DesignSpec>,
    child: V,
) -> WindowView<State>
where
    V: WidgetView<State>,
    V::Widget: FromDynWidget,
{
    ui_window_arc_with_backdrop(window_id, title, spec, spec.backdrop, child)
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
    ui_window_arc(window_id, title, &Arc::new(spec.clone()), child)
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

/// Memoizes a theme-aware subtree by both `data` (`PartialEq`) and the ambient [`DesignSpec`].
///
/// Rebuilds of enclosing views are skipped completely (neither `builder` nor child `rebuild`
/// runs) unless `data` changes, the enclosing `DesignSpec` changes, or a descendant explicitly
/// requested a rebuild via `MessageResult::RequestRebuild`.
pub fn memoize_with_spec<State, Action, Data, F, V>(
    data: Data,
    builder: F,
) -> MemoizeWithSpec<State, Action, Data, F, V>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + Send + Sync + 'static,
    F: Fn(&DesignSpec, &Data) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    MemoizeWithSpec {
        data,
        builder,
        phantom: PhantomData,
    }
}

type StatePhantom<State, Action, V> = PhantomData<fn(&mut State) -> (Action, V)>;

/// View created by [`memoize_with_spec`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(
    missing_debug_implementations,
    reason = "Contains closure trait generic"
)]
pub struct MemoizeWithSpec<State, Action, Data, F, V> {
    data: Data,
    builder: F,
    phantom: StatePhantom<State, Action, V>,
}

impl<State, Action, Data, F, V> ViewMarker for MemoizeWithSpec<State, Action, Data, F, V> {}

#[doc(hidden)]
#[expect(
    missing_debug_implementations,
    reason = "Internal ViewState for MemoizeWithSpec"
)]
pub struct MemoizeWithSpecState<V, VState> {
    spec: Arc<DesignSpec>,
    view: V,
    view_state: VState,
    dirty: bool,
}

impl<State, Action, Data, F, V> View<State, Action, ViewCtx>
    for MemoizeWithSpec<State, Action, Data, F, V>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + Send + Sync + 'static,
    F: Fn(&DesignSpec, &Data) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = MemoizeWithSpecState<V, V::ViewState>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let view = (self.builder)(&spec, &self.data);
        let (el, view_state) = view.build(ctx, app_state);
        (
            el,
            MemoizeWithSpecState {
                spec,
                view,
                view_state,
                dirty: false,
            },
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
        let next_spec = current_spec_arc(ctx);
        let spec_unchanged =
            Arc::ptr_eq(&view_state.spec, &next_spec) || *view_state.spec == *next_spec;
        let dirty = std::mem::take(&mut view_state.dirty);
        if !dirty && spec_unchanged && prev.data == self.data {
            return;
        }
        let next_view = (self.builder)(&next_spec, &self.data);
        next_view.rebuild(
            &view_state.view,
            &mut view_state.view_state,
            ctx,
            element,
            app_state,
        );
        view_state.spec = next_spec;
        view_state.view = next_view;
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        view_state
            .view
            .teardown(&mut view_state.view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let result =
            view_state
                .view
                .message(&mut view_state.view_state, message, element, app_state);
        if matches!(result, MessageResult::RequestRebuild) {
            view_state.dirty = true;
        }
        result
    }
}

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
    type ViewState = (Arc<DesignSpec>, V, V::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
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
        let next_spec = current_spec_arc(ctx);
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

    #[test]
    fn cached_default_properties_enforces_lru_capacity_bound() {
        let mut first = DesignSpec::neutral_fallback(Brightness::Light);
        first.metrics.button_height = 500.0;
        let first_arc = cached_default_properties(&first);

        for idx in 1..=20 {
            let mut other = DesignSpec::neutral_fallback(Brightness::Light);
            other.metrics.button_height = 500.0 + f64::from(idx);
            let _ = cached_default_properties(&other);
        }

        let cache_len = DEFAULT_PROPS_CACHE
            .lock()
            .expect("default properties cache mutex poisoned")
            .len();
        assert!(
            cache_len <= 16,
            "cache exceeded capacity bound 16: {cache_len}"
        );

        // First entry must have been evicted by 20 subsequent distinct inserts.
        let reloaded = cached_default_properties(&first);
        assert!(!Arc::ptr_eq(&first_arc, &reloaded));
    }
}
