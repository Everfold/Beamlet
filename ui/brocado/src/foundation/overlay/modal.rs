//! Shared modal overlay shell composing `EscapeCatcher`, scrim backdrop, `ZStack`, and `SlideTransition`.

use std::sync::Arc;

use masonry::core::{CollectionWidget, NewWidget};
use masonry::kurbo::Vec2;
use masonry::layout::{Dim, UnitPoint};
use masonry::properties::{Dimensions, Padding};
use masonry::widgets::{self, Passthrough, ZStack};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;

use crate::foundation::ScrollViewWidget;

use crate::foundation::overlay::focus_bridge::collect_modal_focusables;
use crate::foundation::overlay::{EscapeCatcher, EscapePressed, ui_scrim};
use crate::foundation::transition::{ModalAxisSizing, SlideTransition};
use crate::style_engine::current_theme_arc;
use crate::tokens::{MotionDuration, MotionEasing};

const MODAL_SCRIM_ID: ViewId = ViewId::new(0x5549_3430);
const MODAL_CONTENT_ID: ViewId = ViewId::new(0x5549_3431);

/// Configuration describing a modal surface hosted over the window root.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalShellConfig {
    /// Masonry surface class attached to the modal card (`DIALOG_SURFACE` or `SHEET_SURFACE`).
    pub class: &'static str,
    /// Horizontal card sizing policy (`FitContent { min, max }` or `FillUpTo { max }`).
    pub width: ModalAxisSizing,
    /// Vertical card sizing policy (`FitContent { min, max }` or `FillUpTo { max }`).
    pub height: ModalAxisSizing,
    /// Optional explicit safe viewport margin (defaults to adaptive `12px`/`24px`).
    pub viewport_margin: Option<f64>,
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

struct ModalLimits {
    card_dims: Dimensions,
    clamp_axes: (bool, bool),
    axis_sizing: (ModalAxisSizing, ModalAxisSizing),
}

fn resolve_modal_limits(
    width: ModalAxisSizing,
    height: ModalAxisSizing,
    alignment: UnitPoint,
) -> ModalLimits {
    let clamp_axes = (
        alignment != UnitPoint::BOTTOM,
        alignment != UnitPoint::RIGHT && alignment != UnitPoint::LEFT,
    );
    let card_w = match width {
        ModalAxisSizing::FitContent { .. } => Dim::Auto,
        ModalAxisSizing::FillUpTo { .. } => Dim::Stretch,
    };
    let card_h = match height {
        ModalAxisSizing::FitContent { .. } => Dim::MaxContent,
        ModalAxisSizing::FillUpTo { .. } => Dim::Stretch,
    };
    ModalLimits {
        card_dims: Dimensions::new(card_w, card_h),
        clamp_axes,
        axis_sizing: (width, height),
    }
}

/// Builds an [`EscapeCatcher`] -> `ZStack(scrim, SlideTransition(SizedBox(Portal(content))))` modal shell.
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

    let limits = resolve_modal_limits(config.width, config.height, config.alignment);
    let scroll_view = NewWidget::new(
        ScrollViewWidget::new(content_el.new_widget.erased()).constrain_horizontal(true),
    );
    let mut card = NewWidget::new(widgets::SizedBox::new(scroll_view.erased()));
    card.classes.insert(config.class.to_string());
    card.properties.insert(limits.card_dims);
    card.properties.insert(config.padding);

    let reduced_motion = current_theme_arc(ctx).reduced_motion;
    let (enter_offset, duration_ms) = if reduced_motion {
        (Vec2::ZERO, 0)
    } else {
        (config.enter_offset, config.duration.as_millis())
    };
    let slide = NewWidget::new(SlideTransition::new_with_limits(
        card,
        enter_offset,
        duration_ms,
        MotionEasing::EmphasizedDecelerate,
        limits.clamp_axes,
        limits.axis_sizing,
        config.viewport_margin,
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

        let prev_limits = resolve_modal_limits(
            view_state.config.width,
            view_state.config.height,
            view_state.config.alignment,
        );
        let next_limits = resolve_modal_limits(config.width, config.height, config.alignment);

        let mut slide_dyn = <ZStack as CollectionWidget<_>>::get_mut(&mut zstack, 1);
        let mut slide = slide_dyn.downcast::<SlideTransition>();
        SlideTransition::set_limits(
            &mut slide,
            next_limits.clamp_axes,
            next_limits.axis_sizing,
            config.viewport_margin,
        );
        let mut card_dyn = SlideTransition::child_mut(&mut slide);
        let mut card = card_dyn.downcast::<widgets::SizedBox>();
        if prev_limits.card_dims != next_limits.card_dims {
            card.insert_prop(next_limits.card_dims);
        }
        if view_state.config.padding != config.padding {
            card.insert_prop(config.padding);
        }
        if view_state.config.class != config.class {
            card.ctx.remove_class(view_state.config.class);
            card.ctx.add_class(config.class);
        }
        let mut scroll_dyn = widgets::SizedBox::child_mut(&mut card)
            .expect("modal card always holds scroll view child");
        let mut scroll = scroll_dyn.downcast::<ScrollViewWidget>();
        let mut content_dyn = ScrollViewWidget::child_mut(&mut scroll);
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
        let mut scroll_dyn = widgets::SizedBox::child_mut(&mut card)
            .expect("modal card always holds scroll view child");
        let mut scroll = scroll_dyn.downcast::<ScrollViewWidget>();
        let mut content_dyn = ScrollViewWidget::child_mut(&mut scroll);
        ctx.with_id(MODAL_CONTENT_ID, |ctx| {
            content.teardown(&mut view_state.child_state, ctx, content_dyn.downcast());
        });
    });
    ctx.teardown_action_source(element);
}

/// Routes Brocado messages inside a modal shell created by [`modal_shell_build`].
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
            let mut scroll_dyn = widgets::SizedBox::child_mut(&mut card)
                .expect("modal card always holds scroll view child");
            let mut scroll = scroll_dyn.downcast::<ScrollViewWidget>();
            let mut content_dyn = ScrollViewWidget::child_mut(&mut scroll);
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
