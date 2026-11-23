//! Shared modal overlay shell composing `EscapeCatcher`, scrim backdrop, `ZStack`, and `SlideTransition`.

use std::sync::Arc;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewPathTracker as _};
use xilem::kurbo::Vec2;
use xilem::masonry::core::{CollectionWidget, NewWidget};
use xilem::masonry::layout::UnitPoint;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::masonry::widgets::{self, Passthrough, ZStack};
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::foundation::overlay::focus_bridge::collect_modal_focusables;
use crate::foundation::overlay::{EscapeCatcher, EscapePressed, ui_scrim};
use crate::foundation::transition::SlideTransition;
use crate::style_engine::current_spec_arc;
use crate::tokens::{MotionDuration, MotionEasing};

const MODAL_SCRIM_ID: ViewId = ViewId::new(0x5549_3430);
const MODAL_CONTENT_ID: ViewId = ViewId::new(0x5549_3431);

/// Configuration describing a modal surface hosted over the window root.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalShellConfig {
    /// Masonry surface class attached to the modal card (`DIALOG_SURFACE` or `SHEET_SURFACE`).
    pub class: &'static str,
    /// Card sizing requirements.
    pub dims: Dimensions,
    /// Interior card padding.
    pub padding: Padding,
    /// Anchor position inside the full-window `ZStack`.
    pub alignment: UnitPoint,
    /// Entrance spatial slide offset.
    pub enter_offset: Vec2,
    /// Entrance animation duration.
    pub duration: MotionDuration,
    /// Whether clicking the backdrop scrim triggers `on_dismiss`.
    pub dismiss_on_scrim: bool,
}

/// ViewState maintained by views delegating to [`modal_shell_build`].
#[expect(
    missing_debug_implementations,
    reason = "Holds type-erased scrim view and view state"
)]
pub struct ModalShellState<State: 'static, Action: 'static, ChildState> {
    scrim_view: Box<AnyWidgetView<State, Action>>,
    scrim_state: <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
    child_state: ChildState,
    config: ModalShellConfig,
}

fn with_zstack_parts<R>(
    element: &mut Mut<'_, Pod<EscapeCatcher>>,
    f: impl FnOnce(Mut<'_, Pod<ZStack>>) -> R,
) -> R {
    let mut zstack_dyn = EscapeCatcher::child_mut(element);
    f(zstack_dyn.downcast::<ZStack>())
}

/// Builds an [`EscapeCatcher`] -> `ZStack(scrim, SlideTransition(SizedBox(content)))` modal shell.
pub fn modal_shell_build<State, Action, V>(
    ctx: &mut ViewCtx,
    app_state: &mut State,
    config: ModalShellConfig,
    content: &V,
    on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
) -> (
    Pod<EscapeCatcher>,
    ModalShellState<State, Action, V::ViewState>,
)
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    let scrim_view = ui_scrim(config.dismiss_on_scrim, on_dismiss).boxed();
    let (scrim_el, scrim_state) =
        ctx.with_id(MODAL_SCRIM_ID, |ctx| scrim_view.build(ctx, app_state));
    let ((content_el, child_state), focus_ring) = collect_modal_focusables(|| {
        ctx.with_id(MODAL_CONTENT_ID, |ctx| content.build(ctx, app_state))
    });

    let mut card = NewWidget::new(widgets::SizedBox::new(content_el.new_widget.erased()));
    card.classes.insert(config.class.to_string());
    card.properties.insert(config.dims);
    card.properties.insert(config.padding);

    let reduced_motion = current_spec_arc(ctx).reduced_motion;
    let (enter_offset, duration_ms) = if reduced_motion {
        (Vec2::ZERO, 0)
    } else {
        (config.enter_offset, config.duration.as_millis())
    };
    let clamp_axes = (
        config.alignment != UnitPoint::BOTTOM,
        config.alignment != UnitPoint::RIGHT,
    );
    let slide = NewWidget::new(SlideTransition::new_with_clamp(
        card,
        enter_offset,
        duration_ms,
        MotionEasing::EmphasizedDecelerate,
        clamp_axes,
    ));

    let zstack = ZStack::new()
        .with(scrim_el.new_widget.erased(), UnitPoint::CENTER)
        .with(slide.erased(), config.alignment);

    let pod = ctx.with_action_widget(|ctx| {
        let mut pod = ctx.create_pod(EscapeCatcher::new_modal_with_focus_ring(
            NewWidget::new(zstack),
            focus_ring,
        ));
        pod.new_widget.properties.insert(Dimensions::STRETCH);
        pod
    });

    (
        pod,
        ModalShellState {
            scrim_view,
            scrim_state,
            child_state,
            config,
        },
    )
}

/// Rebuilds a modal shell created by [`modal_shell_build`].
#[expect(
    clippy::too_many_arguments,
    reason = "Coordinates scrim, alignment, card properties, and child content diffing"
)]
pub fn modal_shell_rebuild<State, Action, V>(
    view_state: &mut ModalShellState<State, Action, V::ViewState>,
    ctx: &mut ViewCtx,
    mut element: Mut<'_, Pod<EscapeCatcher>>,
    app_state: &mut State,
    config: ModalShellConfig,
    prev_content: &V,
    content: &V,
    on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
) where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    let next_scrim = ui_scrim(config.dismiss_on_scrim, on_dismiss).boxed();
    with_zstack_parts(&mut element, |mut zstack| {
        {
            let mut scrim_el = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 0);
            ctx.with_id(MODAL_SCRIM_ID, |ctx| {
                next_scrim.rebuild(
                    &view_state.scrim_view,
                    &mut view_state.scrim_state,
                    ctx,
                    scrim_el.downcast::<Passthrough>(),
                    app_state,
                );
            });
        }
        view_state.scrim_view = next_scrim;

        if view_state.config.alignment != config.alignment {
            ZStack::set_params(&mut zstack, 1, config.alignment);
        }

        let mut slide_dyn = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 1);
        let mut slide = slide_dyn.downcast::<SlideTransition>();
        let mut card_dyn = SlideTransition::child_mut(&mut slide);
        let mut card = card_dyn.downcast::<widgets::SizedBox>();
        if view_state.config.dims != config.dims {
            card.insert_prop(config.dims);
        }
        if view_state.config.padding != config.padding {
            card.insert_prop(config.padding);
        }
        if view_state.config.class != config.class {
            card.ctx.remove_class(view_state.config.class);
            card.ctx.add_class(config.class);
        }
        let mut content_dyn =
            widgets::SizedBox::child_mut(&mut card).expect("modal card always holds content child");
        ctx.with_id(MODAL_CONTENT_ID, |ctx| {
            content.rebuild(
                prev_content,
                &mut view_state.child_state,
                ctx,
                content_dyn.downcast(),
                app_state,
            );
        });
    });
    view_state.config = config;
}

/// Tears down a modal shell created by [`modal_shell_build`].
pub fn modal_shell_teardown<State, Action, V>(
    view_state: &mut ModalShellState<State, Action, V::ViewState>,
    ctx: &mut ViewCtx,
    mut element: Mut<'_, Pod<EscapeCatcher>>,
    content: &V,
) where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    with_zstack_parts(&mut element, |mut zstack| {
        {
            let mut scrim_el = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 0);
            ctx.with_id(MODAL_SCRIM_ID, |ctx| {
                view_state.scrim_view.teardown(
                    &mut view_state.scrim_state,
                    ctx,
                    scrim_el.downcast::<Passthrough>(),
                );
            });
        }
        let mut slide_dyn = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 1);
        let mut slide = slide_dyn.downcast::<SlideTransition>();
        let mut card_dyn = SlideTransition::child_mut(&mut slide);
        let mut card = card_dyn.downcast::<widgets::SizedBox>();
        let mut content_dyn =
            widgets::SizedBox::child_mut(&mut card).expect("modal card always holds content child");
        ctx.with_id(MODAL_CONTENT_ID, |ctx| {
            content.teardown(&mut view_state.child_state, ctx, content_dyn.downcast());
        });
    });
    ctx.teardown_action_source(element);
}

/// Routes Xilem messages inside a modal shell created by [`modal_shell_build`].
pub fn modal_shell_message<State, Action, V>(
    view_state: &mut ModalShellState<State, Action, V::ViewState>,
    message: &mut MessageCtx,
    mut element: Mut<'_, Pod<EscapeCatcher>>,
    app_state: &mut State,
    content: &V,
    on_dismiss: &Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
) -> MessageResult<Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    match message.take_first() {
        Some(MODAL_SCRIM_ID) => with_zstack_parts(&mut element, |mut zstack| {
            let mut scrim_el = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 0);
            view_state.scrim_view.message(
                &mut view_state.scrim_state,
                message,
                scrim_el.downcast::<Passthrough>(),
                app_state,
            )
        }),
        Some(MODAL_CONTENT_ID) => with_zstack_parts(&mut element, |mut zstack| {
            let mut slide_dyn = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 1);
            let mut slide = slide_dyn.downcast::<SlideTransition>();
            let mut card_dyn = SlideTransition::child_mut(&mut slide);
            let mut card = card_dyn.downcast::<widgets::SizedBox>();
            let mut content_dyn = widgets::SizedBox::child_mut(&mut card)
                .expect("modal card always holds content child");
            content.message(
                &mut view_state.child_state,
                message,
                content_dyn.downcast(),
                app_state,
            )
        }),
        None => {
            if message.take_message::<EscapePressed>().is_some() {
                MessageResult::Action(on_dismiss(app_state))
            } else {
                MessageResult::Stale
            }
        }
        Some(_) => MessageResult::Stale,
    }
}
