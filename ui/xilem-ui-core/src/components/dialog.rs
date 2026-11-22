//! Centered modal dialog (`dialog` and `basic_dialog`) built atop `modal_shell`.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::kurbo::Vec2;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, MainAxisAlignment, flex_col, flex_row, label};
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::components::button::{destructive_button, primary_button, subtle_button};
use crate::foundation::overlay::{
    EscapeCatcher, ModalShellConfig, ModalShellState, modal_shell_build, modal_shell_message,
    modal_shell_rebuild, modal_shell_teardown,
};
use crate::foundation::ui_icon;
use crate::style_engine::classes;
use crate::style_engine::current_spec;
use crate::tokens::{DialogActionOrder, MotionDuration, TypeRole};

/// Callback invoked when a dialog closes via scrim, Escape, or action buttons.
pub type DialogCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Generic centered modal dialog view hosting arbitrary content.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiDialog<V, State, Action> {
    content: V,
    width: f64,
    dismiss_on_scrim: bool,
    on_dismiss: DialogCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State, Action> UiDialog<V, State, Action> {
    /// Overrides the modal dialog card width (defaults to `360px`).
    pub fn width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    /// Configures whether clicking the dimmed backdrop scrim triggers `on_dismiss`.
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    fn shell_config(&self) -> ModalShellConfig {
        ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            dims: Dimensions::width(self.width.px()),
            padding: Padding::all(24.px()),
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 20.0),
            duration: MotionDuration::Medium2,
            dismiss_on_scrim: self.dismiss_on_scrim,
        }
    }
}

impl<V, State, Action> ViewMarker for UiDialog<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiDialog<V, State, Action>
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

/// Wraps `content` in a centered modal dialog with scrim and Escape dismissal.
pub fn dialog<State: 'static, Action: 'static, V, F>(
    content: V,
    on_dismiss: F,
) -> UiDialog<V, State, Action>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiDialog {
        content,
        width: 360.0,
        dismiss_on_scrim: true,
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

/// Opinionated confirmation dialog builder supporting icon, title, body, and confirm/cancel buttons.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct BasicDialog<State, Action> {
    icon: Option<&'static str>,
    title: ArcStr,
    body: ArcStr,
    confirm_text: ArcStr,
    dismiss_text: Option<ArcStr>,
    width: f64,
    dismiss_on_scrim: bool,
    destructive: bool,
    on_confirm: DialogCallback<State, Action>,
    on_dismiss: DialogCallback<State, Action>,
}

impl<State, Action> BasicDialog<State, Action> {
    /// Sets an optional top header icon above the dialog title.
    pub fn icon(mut self, svg_path: &'static str) -> Self {
        self.icon = Some(svg_path);
        self
    }

    /// Sets an optional secondary cancel/dismiss action button label.
    pub fn dismiss_text(mut self, text: impl Into<ArcStr>) -> Self {
        self.dismiss_text = Some(text.into());
        self
    }

    /// Overrides the modal dialog card width (defaults to `380px`).
    pub fn width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    /// Configures whether clicking the dimmed backdrop scrim triggers `on_dismiss`.
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    /// Styles the confirmation action as a destructive error action (and tints the icon red).
    pub fn destructive(mut self, is_destructive: bool) -> Self {
        self.destructive = is_destructive;
        self
    }

    fn build_content(&self, spec: &crate::tokens::DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let icon_color = if self.destructive { p.error } else { p.accent };
        let ic = self.icon.map(|path| ui_icon(path, 24.0, icon_color));
        let title_v = spec
            .typography
            .style_label(TypeRole::HeadlineSmall, label(self.title.clone()))
            .color(p.text_primary);
        let body_v = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(self.body.clone()))
            .color(p.text_secondary);

        let on_dismiss = self.on_dismiss.clone();
        let on_confirm = self.on_confirm.clone();
        let cancel_btn = self
            .dismiss_text
            .as_ref()
            .map(|txt| subtle_button(txt.clone(), move |s: &mut State| on_dismiss(s)).boxed());
        let confirm_btn = if self.destructive {
            destructive_button(self.confirm_text.clone(), move |s: &mut State| {
                on_confirm(s)
            })
            .boxed()
        } else {
            primary_button(self.confirm_text.clone(), move |s: &mut State| {
                on_confirm(s)
            })
            .boxed()
        };
        let ordered_btns: Vec<Box<AnyWidgetView<State, Action>>> = match spec.dialog_action_order {
            DialogActionOrder::PrimaryLeading => {
                let mut v = vec![confirm_btn];
                if let Some(c) = cancel_btn {
                    v.push(c);
                }
                v
            }
            DialogActionOrder::PrimaryTrailing => {
                let mut v = Vec::new();
                if let Some(c) = cancel_btn {
                    v.push(c);
                }
                v.push(confirm_btn);
                v
            }
        };
        let actions = flex_row(ordered_btns)
            .main_axis_alignment(MainAxisAlignment::End)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px());

        flex_col((ic, title_v, body_v, actions))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(14.px())
            .boxed()
    }
}

impl<State: 'static, Action: 'static> ViewMarker for BasicDialog<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for BasicDialog<State, Action> {
    type Element = Pod<EscapeCatcher>;
    type ViewState = (
        Box<AnyWidgetView<State, Action>>,
        ModalShellState<
            State,
            Action,
            <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
        >,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec(ctx);
        let content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            dims: Dimensions::width(self.width.px()),
            padding: Padding::all(24.px()),
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 20.0),
            duration: MotionDuration::Medium2,
            dismiss_on_scrim: self.dismiss_on_scrim,
        };
        let (el, shell) = modal_shell_build(ctx, app_state, cfg, &content, self.on_dismiss.clone());
        (el, (content, shell))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_content, shell): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec(ctx);
        let next_content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            dims: Dimensions::width(self.width.px()),
            padding: Padding::all(24.px()),
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 20.0),
            duration: MotionDuration::Medium2,
            dismiss_on_scrim: self.dismiss_on_scrim,
        };
        modal_shell_rebuild(
            shell,
            ctx,
            element,
            app_state,
            cfg,
            prev_content,
            &next_content,
            self.on_dismiss.clone(),
        );
        *prev_content = next_content;
    }

    fn teardown(
        &self,
        (content, shell): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        modal_shell_teardown(shell, ctx, element, content);
    }

    fn message(
        &self,
        (content, shell): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        modal_shell_message(
            shell,
            message,
            element,
            app_state,
            content,
            &self.on_dismiss,
        )
    }
}

/// Standard modal dialog with title, body, confirm action, and optional cancel button.
pub fn basic_dialog<State: 'static, Action: 'static, F1, F2>(
    title: impl Into<ArcStr>,
    body: impl Into<ArcStr>,
    confirm_text: impl Into<ArcStr>,
    on_confirm: F1,
    on_dismiss: F2,
) -> BasicDialog<State, Action>
where
    F1: Fn(&mut State) -> Action + Send + Sync + 'static,
    F2: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    BasicDialog {
        icon: None,
        title: title.into(),
        body: body.into(),
        confirm_text: confirm_text.into(),
        dismiss_text: None,
        width: 380.0,
        dismiss_on_scrim: true,
        destructive: false,
        on_confirm: Arc::new(on_confirm),
        on_dismiss: Arc::new(on_dismiss),
    }
}
