//! Spec-driven icon buttons returning concrete `Pod<widgets::Button>` views with optional custom glyph slots.

use std::sync::Arc;

use masonry::peniko::Color;
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::components::button::{
    ButtonCallback, class_button_build, class_button_message, class_button_rebuild,
    class_button_teardown,
};
use crate::foundation::ui_icon;
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::DesignSpec;

/// Visual emphasis style for [`UiIconButton`] and [`UiCustomIconButton`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconButtonVariant {
    /// Transparent background until hovered/selected.
    #[default]
    Standard,
    /// Accent background fill.
    Filled,
    /// Secondary container background fill.
    Tonal,
    /// Bordered outline ring.
    Outlined,
}

impl IconButtonVariant {
    const fn resolve(self, selected: bool, spec: &DesignSpec) -> (&'static str, Color) {
        let p = &spec.palette;
        match (self, selected) {
            (Self::Standard, false) => (classes::ICON_BTN_STD, p.text_secondary),
            (Self::Standard, true) => (classes::ICON_BTN_STD_SELECTED, p.accent),
            (Self::Filled, true) => (classes::ICON_BTN_FILLED, p.on_accent),
            (Self::Filled, false) => (classes::ICON_BTN_FILLED_UNSELECTED, p.accent),
            (Self::Tonal, true) => (classes::ICON_BTN_TONAL, p.on_secondary_container),
            (Self::Tonal, false) => (classes::ICON_BTN_TONAL_UNSELECTED, p.text_secondary),
            (Self::Outlined, false) => (classes::ICON_BTN_OUTLINED, p.text_secondary),
            (Self::Outlined, true) => (classes::ICON_BTN_OUTLINED_SELECTED, p.inverse_on_surface),
        }
    }

    const fn class_name(self, selected: bool) -> &'static str {
        match (self, selected) {
            (Self::Standard, false) => classes::ICON_BTN_STD,
            (Self::Standard, true) => classes::ICON_BTN_STD_SELECTED,
            (Self::Filled, true) => classes::ICON_BTN_FILLED,
            (Self::Filled, false) => classes::ICON_BTN_FILLED_UNSELECTED,
            (Self::Tonal, true) => classes::ICON_BTN_TONAL,
            (Self::Tonal, false) => classes::ICON_BTN_TONAL_UNSELECTED,
            (Self::Outlined, false) => classes::ICON_BTN_OUTLINED,
            (Self::Outlined, true) => classes::ICON_BTN_OUTLINED_SELECTED,
        }
    }
}

/// Callback signature for [`UiIconButton`].
pub type IconButtonCallback<State, Action> = ButtonCallback<State, Action>;

/// Spec-aware square icon button supporting toggle selection and direct `Pod<widgets::Button>` styling.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiIconButton<State, Action> {
    variant: IconButtonVariant,
    svg_path: &'static str,
    selected: bool,
    disabled: bool,
    on_click: IconButtonCallback<State, Action>,
}

impl<State, Action> UiIconButton<State, Action> {
    /// Marks this icon button as toggle-selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Disables activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn build_inner(&self, spec: &DesignSpec) -> (&'static str, Box<AnyWidgetView<State, Action>>)
    where
        State: 'static,
        Action: 'static,
    {
        let (cls, active_fg) = self.variant.resolve(self.selected, spec);
        let fg = spec.content_color(active_fg, self.disabled);
        let icon_size = (spec.metrics.icon_btn_size * 0.55).clamp(16.0, 22.0);
        (cls, ui_icon(self.svg_path, icon_size, fg).boxed())
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiIconButton<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiIconButton<State, Action> {
    type Element = Pod<widgets::Button>;
    type ViewState = (
        &'static str,
        Box<AnyWidgetView<State, Action>>,
        <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let (cls, inner) = self.build_inner(&spec);
        let (pod, child_state) = class_button_build(ctx, app_state, cls, self.disabled, &inner);
        (pod, (cls, inner, child_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (prev_cls, prev_inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let (next_cls, next_inner) = self.build_inner(&spec);
        class_button_rebuild(
            ctx,
            element,
            app_state,
            prev_cls,
            next_cls,
            prev.disabled,
            self.disabled,
            prev_inner,
            &next_inner,
            child_state,
        );
        *prev_cls = next_cls;
        *prev_inner = next_inner;
    }

    fn teardown(
        &self,
        (_, inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        class_button_teardown(ctx, element, inner, child_state);
    }

    fn message(
        &self,
        (_, inner, child_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        class_button_message(
            message,
            element,
            app_state,
            inner,
            child_state,
            &self.on_click,
        )
    }
}

/// Custom-content icon button view constructed by [`icon_button_view`], exposing `Pod<widgets::Button>` directly.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiCustomIconButton<V, State, Action> {
    variant: IconButtonVariant,
    selected: bool,
    disabled: bool,
    child: V,
    on_click: IconButtonCallback<State, Action>,
}

impl<V, State, Action> UiCustomIconButton<V, State, Action> {
    /// Marks this icon button as toggle-selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Disables activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<V, State, Action> ViewMarker for UiCustomIconButton<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiCustomIconButton<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = Pod<widgets::Button>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        class_button_build(
            ctx,
            app_state,
            self.variant.class_name(self.selected),
            self.disabled,
            &self.child,
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
        class_button_rebuild(
            ctx,
            element,
            app_state,
            prev.variant.class_name(prev.selected),
            self.variant.class_name(self.selected),
            prev.disabled,
            self.disabled,
            &prev.child,
            &self.child,
            view_state,
        );
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        class_button_teardown(ctx, element, &self.child, view_state);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        class_button_message(
            message,
            element,
            app_state,
            &self.child,
            view_state,
            &self.on_click,
        )
    }
}

/// Square icon button hosting arbitrary custom content `child` (e.g. a vector graphic or avatar).
pub fn icon_button_view<State: 'static, Action: 'static, V, F>(
    variant: IconButtonVariant,
    selected: bool,
    child: V,
    on_click: F,
) -> UiCustomIconButton<V, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiCustomIconButton {
        variant,
        selected,
        disabled: false,
        child,
        on_click: Arc::new(on_click),
    }
}

fn make_icon_button<State, Action, F>(
    variant: IconButtonVariant,
    svg_path: &'static str,
    selected_default: bool,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiIconButton {
        variant,
        svg_path,
        selected: selected_default,
        disabled: false,
        on_click: Arc::new(on_click),
    }
}

/// Standard icon button (unselected by default).
pub fn icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Standard, svg_path, false, on_click)
}

/// Filled high-emphasis icon button (defaults `selected = true`).
pub fn filled_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Filled, svg_path, true, on_click)
}

/// Tonal icon button (defaults `selected = true`).
pub fn tonal_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Tonal, svg_path, true, on_click)
}

/// Outlined icon button (unselected by default).
pub fn outlined_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Outlined, svg_path, false, on_click)
}
