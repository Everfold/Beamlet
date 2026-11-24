//! Headless theme environment scope (`theme_scope`) and reactive theme helpers.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock, Mutex};

use masonry::core::{DefaultProperties, FromDynWidget};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::{Pod, ViewCtx, WidgetView};

use crate::style_engine::build_default_properties;
use crate::theme::Theme;

thread_local! {
    static THEME_STACK: RefCell<Vec<Arc<Theme>>> = const { RefCell::new(Vec::new()) };
}

struct ThemeGuard;

impl ThemeGuard {
    fn push(theme: Arc<Theme>) -> Self {
        THEME_STACK.with(|stack| stack.borrow_mut().push(theme));
        Self
    }
}

impl Drop for ThemeGuard {
    fn drop(&mut self) {
        THEME_STACK.with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

type DefaultPropsCache = Vec<(Theme, Arc<DefaultProperties>)>;

static DEFAULT_PROPS_CACHE: LazyLock<Mutex<DefaultPropsCache>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Returns a cached [`Arc<DefaultProperties>`] for `theme`, preserving `Arc::ptr_eq` across
/// rebuilds when `theme` has not changed while producing a fresh `Arc` on theme changes.
#[must_use]
pub fn cached_default_properties(theme: &Theme) -> Arc<DefaultProperties> {
    let mut cache = DEFAULT_PROPS_CACHE
        .lock()
        .expect("default properties cache mutex poisoned");
    if let Some((_, props)) = cache.iter().find(|(cached_theme, _)| cached_theme == theme) {
        return props.clone();
    }
    let props = Arc::new(build_default_properties(theme));
    if cache.len() >= 16 {
        cache.remove(0);
    }
    cache.push((theme.clone(), props.clone()));
    props
}

static FALLBACK_LIGHT_THEME: LazyLock<Arc<Theme>> = LazyLock::new(|| Arc::new(Theme::light()));

/// Reads a shared `Arc` handle to the ambient [`Theme`] from the active [`theme_scope`]
/// stack without deep-cloning [`Theme`], falling back to [`Theme::light()`].
#[must_use]
pub fn current_theme_arc(_ctx: &ViewCtx) -> Arc<Theme> {
    THEME_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .map_or_else(|| FALLBACK_LIGHT_THEME.clone(), Arc::clone)
    })
}

/// Reads the ambient [`Theme`] from the active [`theme_scope`] stack, falling back to
/// [`Theme::light()`] if no enclosing [`theme_scope`] exists.
#[must_use]
pub fn current_theme(ctx: &ViewCtx) -> Theme {
    (*current_theme_arc(ctx)).clone()
}

/// View created by [`theme_scope_arc`] pushing an [`Arc<Theme>`] around child build and rebuild.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct ThemeScope<State, Action, V> {
    theme: Arc<Theme>,
    child: V,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<State, Action, V> ViewMarker for ThemeScope<State, Action, V> {}

impl<State, Action, V> View<State, Action, ViewCtx> for ThemeScope<State, Action, V>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let _guard = ThemeGuard::push(self.theme.clone());
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
        let _guard = ThemeGuard::push(self.theme.clone());
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

/// Wraps `child` inside a Brocado [`Theme`] environment scope using a pre-allocated [`Arc<Theme>`].
pub fn theme_scope_arc<State: 'static, Action: 'static, V>(
    theme: Arc<Theme>,
    child: V,
) -> ThemeScope<State, Action, V>
where
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    ThemeScope {
        theme,
        child,
        phantom: PhantomData,
    }
}

/// Creates a lazily evaluated view that reads the active [`Theme`] on every build/rebuild.
pub fn with_theme<State, Action, F, V>(builder: F) -> WithTheme<State, Action, F, V>
where
    State: 'static,
    Action: 'static,
    F: Fn(&Theme, &mut State) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
{
    WithTheme {
        builder,
        phantom: PhantomData,
    }
}

/// Memoizes a theme-aware subtree by both `data` (`PartialEq`) and the ambient [`Theme`].
///
/// Rebuilds of enclosing views are skipped completely (neither `builder` nor child `rebuild`
/// runs) unless `data` changes, the enclosing `Theme` changes, or a descendant explicitly
/// requested a rebuild via `MessageResult::RequestRebuild`.
pub fn memoize_with_theme<State, Action, Data, F, V>(
    data: Data,
    builder: F,
) -> MemoizeWithTheme<State, Action, Data, F, V>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + Send + Sync + 'static,
    F: Fn(&Theme, &Data) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    MemoizeWithTheme {
        data,
        builder,
        phantom: PhantomData,
    }
}

type StatePhantom<State, Action, V> = PhantomData<fn(&mut State) -> (Action, V)>;

/// View created by [`memoize_with_theme`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Contains closure trait generic"
)]
pub struct MemoizeWithTheme<State, Action, Data, F, V> {
    data: Data,
    builder: F,
    phantom: StatePhantom<State, Action, V>,
}

impl<State, Action, Data, F, V> ViewMarker for MemoizeWithTheme<State, Action, Data, F, V> {}

#[doc(hidden)]
#[expect(
    missing_debug_implementations,
    reason = "Internal ViewState for MemoizeWithTheme"
)]
pub struct MemoizeWithThemeState<V, VState> {
    theme: Arc<Theme>,
    view: V,
    view_state: VState,
    dirty: bool,
}

impl<State, Action, Data, F, V> View<State, Action, ViewCtx>
    for MemoizeWithTheme<State, Action, Data, F, V>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + Send + Sync + 'static,
    F: Fn(&Theme, &Data) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = MemoizeWithThemeState<V, V::ViewState>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let view = (self.builder)(&theme, &self.data);
        let (el, view_state) = view.build(ctx, app_state);
        (
            el,
            MemoizeWithThemeState {
                theme,
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
        let next_theme = current_theme_arc(ctx);
        let theme_unchanged =
            Arc::ptr_eq(&view_state.theme, &next_theme) || *view_state.theme == *next_theme;
        let dirty = std::mem::take(&mut view_state.dirty);
        if !dirty && theme_unchanged && prev.data == self.data {
            return;
        }
        let next_view = (self.builder)(&next_theme, &self.data);
        next_view.rebuild(
            &view_state.view,
            &mut view_state.view_state,
            ctx,
            element,
            app_state,
        );
        view_state.theme = next_theme;
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

/// View created by [`with_theme`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Contains closure trait generic"
)]
pub struct WithTheme<State, Action, F, V> {
    builder: F,
    phantom: StatePhantom<State, Action, V>,
}

impl<State, Action, F, V> ViewMarker for WithTheme<State, Action, F, V> {}

impl<State, Action, F, V> View<State, Action, ViewCtx> for WithTheme<State, Action, F, V>
where
    State: 'static,
    Action: 'static,
    F: Fn(&Theme, &mut State) -> V + Send + Sync + 'static,
    V: WidgetView<State, Action>,
    V::Widget: FromDynWidget,
{
    type Element = Pod<V::Widget>;
    type ViewState = (Arc<Theme>, V, V::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let view = (self.builder)(&theme, app_state);
        let (el, state) = view.build(ctx, app_state);
        (el, (theme, view, state))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_theme, prev_view, view_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let next_theme = current_theme_arc(ctx);
        let next_view = (self.builder)(&next_theme, app_state);
        next_view.rebuild(prev_view, view_state, ctx, element, app_state);
        *prev_theme = next_theme;
        *prev_view = next_view;
    }

    fn teardown(
        &self,
        (_theme, prev_view, view_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        prev_view.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        (_theme, prev_view, view_state): &mut Self::ViewState,
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
    use crate::tokens::Brightness;

    fn read_stack_theme() -> Theme {
        THEME_STACK.with(|stack| {
            stack
                .borrow()
                .last()
                .map_or_else(Theme::light, |t| (**t).clone())
        })
    }

    #[test]
    fn theme_stack_nests_and_restores_on_drop() {
        assert_eq!(read_stack_theme().brightness, Brightness::Light);
        let dark = Arc::new(Theme::dark());
        let outer = ThemeGuard::push(dark);
        assert_eq!(read_stack_theme().brightness, Brightness::Dark);

        {
            let inner_theme = Theme::light().with_seed(crate::theme::seeds::TEAL);
            let _inner = ThemeGuard::push(Arc::new(inner_theme.clone()));
            assert_eq!(read_stack_theme().seed, crate::theme::seeds::TEAL);
            assert_eq!(read_stack_theme().brightness, Brightness::Light);
        }

        assert_eq!(read_stack_theme().brightness, Brightness::Dark);
        drop(outer);
        assert_eq!(read_stack_theme().brightness, Brightness::Light);
    }

    #[test]
    fn cached_default_properties_reuses_arc_for_equal_theme() {
        let light = Theme::light();
        let dark = Theme::dark();
        let a = cached_default_properties(&light);
        let b = cached_default_properties(&light);
        let c = cached_default_properties(&dark);
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
    }

    #[test]
    fn cached_default_properties_enforces_lru_capacity_bound() {
        let first = Theme::light().with_font_scale(1.01);
        let first_arc = cached_default_properties(&first);

        for idx in 2_u16..=22 {
            let other = Theme::light().with_font_scale(1.0 + f32::from(idx) * 0.01);
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

        // First entry must have been evicted by subsequent distinct inserts.
        let reloaded = cached_default_properties(&first);
        assert!(!Arc::ptr_eq(&first_arc, &reloaded));
    }
}
