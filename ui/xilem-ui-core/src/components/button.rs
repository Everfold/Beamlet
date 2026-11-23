//! Spec-driven push buttons returning concrete `Pod<widgets::Button>` views with open child slots.

use std::any::type_name;
use std::sync::Arc;

use masonry::core::{ArcStr, PointerButton};
use masonry::layout::AsUnit;
use masonry::widgets::{self, ButtonPress};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, flex_row};

use crate::foundation::ui_icon;
use crate::style_engine::{classes, current_spec_arc};
use crate::tokens::{DesignSpec, TypeRole};

const BUTTON_CONTENT_VIEW_ID: ViewId = ViewId::new(0x5549_3701);

/// Cross-theme semantic button emphasis variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// Highest-emphasis accent button (MD3 Filled / WinUI Accent / HIG Default / Adwaita Suggested).
    #[default]
    Primary,
    /// Medium-emphasis filled secondary container button.
    Secondary,
    /// Subtle elevated button.
    Elevated,
    /// Bordered outline button.
    Outlined,
    /// Text/flat/borderless button.
    Subtle,
    /// High-emphasis destructive / dangerous action button.
    Destructive,
}

impl ButtonVariant {
    const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => classes::BTN_PRIMARY,
            Self::Secondary => classes::BTN_SECONDARY,
            Self::Elevated => classes::BTN_ELEVATED,
            Self::Outlined => classes::BTN_OUTLINED,
            Self::Subtle => classes::BTN_SUBTLE,
            Self::Destructive => classes::BTN_DESTRUCTIVE,
        }
    }
}

/// Callback signature for [`UiButton`] and [`UiCustomButton`].
pub type ButtonCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

pub(crate) fn class_button_build<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    ctx: &mut ViewCtx,
    app_state: &mut State,
    class: &'static str,
    disabled: bool,
    child: &V,
) -> (Pod<widgets::Button>, V::ViewState) {
    let (child_pod, child_state) =
        ctx.with_id(BUTTON_CONTENT_VIEW_ID, |ctx| child.build(ctx, app_state));
    let pod = ctx.with_action_widget(|ctx| {
        let mut pod = ctx.create_pod(widgets::Button::new(child_pod.new_widget));
        pod.new_widget.classes.insert(class.to_string());
        pod.new_widget.options.disabled = disabled;
        if !disabled {
            crate::foundation::overlay::focus_bridge::record_modal_focusable(pod.new_widget.id());
        }
        pod
    });
    (pod, child_state)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Coordinates class swapping, disabled state, and child diffing on Button pods"
)]
pub(crate) fn class_button_rebuild<
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
>(
    ctx: &mut ViewCtx,
    mut element: Mut<'_, Pod<widgets::Button>>,
    app_state: &mut State,
    prev_class: &'static str,
    next_class: &'static str,
    prev_disabled: bool,
    next_disabled: bool,
    prev_child: &V,
    next_child: &V,
    child_state: &mut V::ViewState,
) {
    if prev_disabled != next_disabled {
        element.ctx.set_disabled(next_disabled);
    }
    if prev_class != next_class {
        element.ctx.remove_class(prev_class);
        element.ctx.add_class(next_class);
    }
    ctx.with_id(BUTTON_CONTENT_VIEW_ID, |ctx| {
        next_child.rebuild(
            prev_child,
            child_state,
            ctx,
            widgets::Button::child_mut(&mut element).downcast(),
            app_state,
        );
    });
}

pub(crate) fn class_button_teardown<
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
>(
    ctx: &mut ViewCtx,
    mut element: Mut<'_, Pod<widgets::Button>>,
    child: &V,
    child_state: &mut V::ViewState,
) {
    ctx.with_id(BUTTON_CONTENT_VIEW_ID, |ctx| {
        child.teardown(
            child_state,
            ctx,
            widgets::Button::child_mut(&mut element).downcast(),
        );
    });
    ctx.teardown_action_source(element);
}

pub(crate) fn class_button_message<
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
>(
    message: &mut MessageCtx,
    mut element: Mut<'_, Pod<widgets::Button>>,
    app_state: &mut State,
    child: &V,
    child_state: &mut V::ViewState,
    on_click: &ButtonCallback<State, Action>,
) -> MessageResult<Action> {
    match message.take_first() {
        Some(BUTTON_CONTENT_VIEW_ID) => child.message(
            child_state,
            message,
            widgets::Button::child_mut(&mut element).downcast(),
            app_state,
        ),
        None => match message.take_message::<ButtonPress>() {
            Some(press) => match press.button {
                None | Some(PointerButton::Primary) => MessageResult::Action(on_click(app_state)),
                _ => MessageResult::Nop,
            },
            None => {
                tracing::error!(
                    "Wrong message type in button::message: {message:?} expected {}",
                    type_name::<ButtonPress>()
                );
                MessageResult::Stale
            }
        },
        _ => MessageResult::Stale,
    }
}

/// Multi-theme button view reading typography and colors from [`DesignSpec`] while exposing `Pod<widgets::Button>` directly.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiButton<State, Action> {
    variant: ButtonVariant,
    text: ArcStr,
    leading_icon: Option<&'static str>,
    trailing_icon: Option<&'static str>,
    disabled: bool,
    on_click: ButtonCallback<State, Action>,
}

impl<State, Action> UiButton<State, Action> {
    /// Adds a `16px` leading SVG icon before the button label.
    pub fn leading_icon(mut self, svg_path: &'static str) -> Self {
        self.leading_icon = Some(svg_path);
        self
    }

    /// Adds a `16px` trailing SVG icon after the button label.
    pub fn trailing_icon(mut self, svg_path: &'static str) -> Self {
        self.trailing_icon = Some(svg_path);
        self
    }

    /// Disables pointer and keyboard activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn build_inner(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let active_fg = match self.variant {
            ButtonVariant::Primary => p.on_accent,
            ButtonVariant::Secondary => p.on_secondary_container,
            ButtonVariant::Elevated | ButtonVariant::Subtle => p.accent,
            ButtonVariant::Outlined => p.text_primary,
            ButtonVariant::Destructive => p.on_error,
        };
        let fg = spec.content_color(active_fg, self.disabled);
        let lbl = spec
            .typography
            .style_label(
                TypeRole::LabelLarge,
                xilem_masonry::view::label(self.text.clone()),
            )
            .color(fg);

        if self.leading_icon.is_some() || self.trailing_icon.is_some() {
            let lead = self.leading_icon.map(|path| ui_icon(path, 16.0, fg));
            let trail = self.trailing_icon.map(|path| ui_icon(path, 16.0, fg));
            flex_row((lead, lbl, trail))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
                .boxed()
        } else {
            lbl.boxed()
        }
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiButton<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiButton<State, Action> {
    type Element = Pod<widgets::Button>;
    type ViewState = (
        Box<AnyWidgetView<State, Action>>,
        <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let inner = self.build_inner(&spec);
        let (pod, child_state) = class_button_build(
            ctx,
            app_state,
            self.variant.class_name(),
            self.disabled,
            &inner,
        );
        (pod, (inner, child_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (prev_inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_inner = self.build_inner(&spec);
        class_button_rebuild(
            ctx,
            element,
            app_state,
            prev.variant.class_name(),
            self.variant.class_name(),
            prev.disabled,
            self.disabled,
            prev_inner,
            &next_inner,
            child_state,
        );
        *prev_inner = next_inner;
    }

    fn teardown(
        &self,
        (inner, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        class_button_teardown(ctx, element, inner, child_state);
    }

    fn message(
        &self,
        (inner, child_state): &mut Self::ViewState,
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

/// Custom-content spec-styled button constructed by [`button_view`], exposing `Pod<widgets::Button>` directly.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiCustomButton<V, State, Action> {
    variant: ButtonVariant,
    child: V,
    disabled: bool,
    on_click: ButtonCallback<State, Action>,
}

impl<V, State, Action> UiCustomButton<V, State, Action> {
    /// Disables pointer and keyboard activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<V, State, Action> ViewMarker for UiCustomButton<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiCustomButton<V, State, Action>
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
            self.variant.class_name(),
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
            prev.variant.class_name(),
            self.variant.class_name(),
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

/// Creates a spec-styled push button wrapping arbitrary non-interactive custom content `child`.
pub fn button_view<State: 'static, Action: 'static, V, F>(
    variant: ButtonVariant,
    child: V,
    on_click: F,
) -> UiCustomButton<V, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiCustomButton {
        variant,
        child,
        disabled: false,
        on_click: Arc::new(on_click),
    }
}

fn make_button<State, Action, F>(
    variant: ButtonVariant,
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiButton {
        variant,
        text: text.into(),
        leading_icon: None,
        trailing_icon: None,
        disabled: false,
        on_click: Arc::new(on_click),
    }
}

/// Primary accent button.
pub fn primary_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Primary, text, on_click)
}

/// Secondary container button.
pub fn secondary_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Secondary, text, on_click)
}

/// Subtle elevated button.
pub fn elevated_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Elevated, text, on_click)
}

/// Outlined bordered button.
pub fn outlined_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Outlined, text, on_click)
}

/// Low-emphasis flat/text button.
pub fn subtle_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Subtle, text, on_click)
}

/// High-emphasis destructive / dangerous confirmation button styled with `palette.error`.
pub fn destructive_button<State: 'static, Action: 'static, F>(
    text: impl Into<ArcStr>,
    on_click: F,
) -> UiButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_button(ButtonVariant::Destructive, text, on_click)
}
