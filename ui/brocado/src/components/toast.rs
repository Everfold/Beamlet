//! Toast / Snackbar queue state, timer-backed host, and spec-styled banner view.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, ComposeCtx, EventCtx, LayoutCtx, MeasureCtx, NewWidget,
    PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget,
    WidgetPod,
};
use masonry::imaging::{Composite, GroupRef, Painter};
use masonry::kurbo::{Axis, Point, Size, Vec2};
use masonry::layout::{AsUnit, Dim, LenReq, Length};
use masonry::peniko::{self, Color};
use masonry::properties::{CornerRadius, Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, button, flex_row, sized_box};

use crate::components::typography::ui_label;
use crate::foundation::overlay::empty_slot;
use crate::foundation::{UiClassExt, adaptive_pair, clip_box, focus_frame, icons, ui_icon};
use crate::style_engine::{classes, current_theme_arc, with_theme};
use crate::theme::metrics::{BUTTON_RADIUS, ICON_BTN_RADIUS};
use crate::theme::{ElevationLevel, Theme};
use crate::tokens::{MotionDuration, MotionEasing, TypeRole};

/// Auto-dismiss duration presets for [`ToastData`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToastDuration {
    /// `4.0s` standard display duration.
    #[default]
    Short,
    /// `10.0s` extended display duration.
    Long,
    /// Requires explicit user dismissal.
    Indefinite,
}

impl ToastDuration {
    /// Returns duration in nanoseconds, or `None` when [`Self::Indefinite`].
    #[must_use]
    pub const fn as_nanos(self) -> Option<u64> {
        match self {
            Self::Short => Some(4_000_000_000),
            Self::Long => Some(10_000_000_000),
            Self::Indefinite => None,
        }
    }
}

/// Visual payload for a single queued notification banner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastData {
    /// Unique identifier incremented automatically by [`ToastHostState`].
    pub id: u64,
    /// Notification message text.
    pub message: ArcStr,
    /// Optional trailing action button caption (e.g. `"Undo"`).
    pub action_label: Option<ArcStr>,
    /// Auto-dismiss duration policy.
    pub duration: ToastDuration,
    /// Whether to display an inline close icon button.
    pub with_close: bool,
}

/// FIFO queue managing active and pending [`ToastData`] notifications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToastHostState {
    next_id: u64,
    queue: VecDeque<ToastData>,
}

impl ToastHostState {
    /// Creates an empty [`ToastHostState`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a notification to the queue and returns its assigned `id`.
    pub fn show(
        &mut self,
        message: impl Into<ArcStr>,
        action_label: Option<impl Into<ArcStr>>,
        duration: ToastDuration,
        with_close: bool,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.queue.push_back(ToastData {
            id,
            message: message.into(),
            action_label: action_label.map(Into::into),
            duration,
            with_close,
        });
        id
    }

    /// Returns the currently visible front toast, if any.
    #[must_use]
    pub fn current(&self) -> Option<&ToastData> {
        self.queue.front()
    }

    /// Pops the currently active toast so the next queued toast (if any) becomes visible.
    pub fn dismiss_current(&mut self) -> Option<ToastData> {
        self.queue.pop_front()
    }

    /// Returns the number of queued + active toasts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns `true` when no toasts are active or queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Action callback used by [`toast`] and [`toast_host`].
pub type ToastCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Renders an inverse-surface toast / snackbar banner view.
pub fn toast<State: 'static, Action: 'static>(
    data: &ToastData,
    on_action: Option<ToastCallback<State, Action>>,
    on_dismiss: Option<ToastCallback<State, Action>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let data = data.clone();
    with_theme(move |theme: &Theme, _| {
        let s = &theme.scheme;
        let msg = ui_label(TypeRole::BodyMedium, data.message.clone()).color(s.inverse_on_surface);

        let act_btn = data
            .action_label
            .as_ref()
            .zip(on_action.as_ref())
            .map(|(lbl, cb)| {
                let cb = cb.clone();
                let action_lbl = clip_box(
                    ui_label(TypeRole::LabelLarge, lbl.clone())
                        .color(s.primary_container)
                        .line_breaking(LineBreaking::Clip),
                );
                focus_frame(
                    button(action_lbl, move |st: &mut State| cb(st))
                        .ui_class(classes::TOAST_ACTION_BTN),
                )
                .accent(s.primary_container)
                .radius(BUTTON_RADIUS)
            });
        let close_btn = if data.with_close
            && let Some(cb) = &on_dismiss
        {
            let cb = cb.clone();
            Some(
                focus_frame(
                    button(
                        ui_icon(icons::CLOSE, 16.0, s.inverse_on_surface),
                        move |st| cb(st),
                    )
                    .ui_class(classes::TOAST_CLOSE_BTN),
                )
                .accent(s.inverse_on_surface)
                .radius(ICON_BTN_RADIUS),
            )
        } else {
            None
        };

        let inner: Box<AnyWidgetView<State, Action>> = if act_btn.is_some() || close_btn.is_some() {
            adaptive_pair(
                msg,
                flex_row((act_btn, close_btn))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(4.px()),
            )
            .gap(8.px())
            .run_gap(6.px())
            .min_leading(48.0)
            .fill_available(false)
            .padding(Padding::from_vh(8.px(), 12.px()))
            .boxed()
        } else {
            msg.padding(Padding::from_vh(8.px(), 12.px())).boxed()
        };

        sized_box(inner)
            .dims(Dimensions::new(Dim::FitContent, Dim::MaxContent))
            .background_color(s.inverse_surface)
            .corner_radius(BUTTON_RADIUS.max(8.0).px())
            .box_shadow(theme.box_shadow(ElevationLevel::Level3))
    })
}

/// Internal action emitted by [`ToastTimerHost`] when a toast's display duration expires.
#[derive(Debug)]
pub struct ToastExpired;

#[derive(Debug)]
pub enum ToastHostAction {
    Expired,
    ExitFinished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToastPhase {
    Idle,
    Entering,
    Visible,
    Exiting { auto_dismiss: bool },
}

const TOAST_CHILD_ID: ViewId = ViewId::new(0x5549_3450);
const TOAST_ENTER_OFFSET_Y: f64 = 12.0;
const TOAST_EXIT_OFFSET_Y: f64 = 8.0;

fn toast_motion_durations_ns(theme: &Theme) -> (u64, u64) {
    if theme.reduced_motion {
        (0, 0)
    } else {
        (
            MotionDuration::Short3.as_millis().saturating_mul(1_000_000),
            MotionDuration::Short2.as_millis().saturating_mul(1_000_000),
        )
    }
}

/// Internal Masonry widget managing toast fade-in, auto-dismiss timing, and fade-out for [`toast_host`].
#[expect(missing_debug_implementations, reason = "Holds erased child WidgetPod")]
pub struct ToastTimerHost {
    child: WidgetPod<dyn Widget>,
    active_id: Option<u64>,
    duration_ns: Option<u64>,
    enter_duration_ns: u64,
    exit_duration_ns: u64,
    phase: ToastPhase,
    phase_elapsed_ns: u64,
    stay_elapsed_ns: u64,
}

impl ToastTimerHost {
    fn new(
        child: NewWidget<impl Widget + ?Sized>,
        active_id: Option<u64>,
        duration_ns: Option<u64>,
        enter_duration_ns: u64,
        exit_duration_ns: u64,
    ) -> Self {
        let phase = match active_id {
            Some(_) if enter_duration_ns > 0 => ToastPhase::Entering,
            Some(_) => ToastPhase::Visible,
            None => ToastPhase::Idle,
        };
        Self {
            child: child.erased().to_pod(),
            active_id,
            duration_ns,
            enter_duration_ns,
            exit_duration_ns,
            phase,
            phase_elapsed_ns: 0,
            stay_elapsed_ns: 0,
        }
    }

    fn current_alpha(&self) -> f32 {
        match self.phase {
            ToastPhase::Idle | ToastPhase::Visible => 1.0,
            ToastPhase::Entering => {
                if self.enter_duration_ns == 0 {
                    1.0
                } else {
                    let raw = (self.phase_elapsed_ns as f64 / self.enter_duration_ns as f64)
                        .clamp(0.0, 1.0);
                    (MotionEasing::EmphasizedDecelerate.sample(raw) as f32).clamp(0.0, 1.0)
                }
            }
            ToastPhase::Exiting { .. } => {
                if self.exit_duration_ns == 0 {
                    0.0
                } else {
                    let raw = (self.phase_elapsed_ns as f64 / self.exit_duration_ns as f64)
                        .clamp(0.0, 1.0);
                    (1.0 - MotionEasing::EmphasizedAccelerate.sample(raw) as f32).clamp(0.0, 1.0)
                }
            }
        }
    }

    fn current_offset_y(&self) -> f64 {
        match self.phase {
            ToastPhase::Idle | ToastPhase::Visible => 0.0,
            ToastPhase::Entering => {
                if self.enter_duration_ns == 0 {
                    0.0
                } else {
                    let raw = (self.phase_elapsed_ns as f64 / self.enter_duration_ns as f64)
                        .clamp(0.0, 1.0);
                    TOAST_ENTER_OFFSET_Y * (1.0 - MotionEasing::EmphasizedDecelerate.sample(raw))
                }
            }
            ToastPhase::Exiting { .. } => {
                if self.exit_duration_ns == 0 {
                    TOAST_EXIT_OFFSET_Y
                } else {
                    let raw = (self.phase_elapsed_ns as f64 / self.exit_duration_ns as f64)
                        .clamp(0.0, 1.0);
                    TOAST_EXIT_OFFSET_Y * MotionEasing::EmphasizedAccelerate.sample(raw)
                }
            }
        }
    }

    fn needs_anim(&self) -> bool {
        matches!(
            self.phase,
            ToastPhase::Entering | ToastPhase::Exiting { .. }
        ) || (self.phase == ToastPhase::Visible && self.duration_ns.is_some())
    }
}

impl Widget for ToastTimerHost {
    type Action = ToastHostAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) && self.needs_anim() {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        match self.phase {
            ToastPhase::Idle => {}
            ToastPhase::Entering => {
                self.phase_elapsed_ns = self.phase_elapsed_ns.saturating_add(interval);
                ctx.request_compose();
                ctx.request_paint_only();
                ctx.request_post_paint();
                if self.phase_elapsed_ns >= self.enter_duration_ns {
                    self.phase = ToastPhase::Visible;
                    self.phase_elapsed_ns = 0;
                    if self.duration_ns.is_some() {
                        ctx.request_anim_frame();
                    }
                } else {
                    ctx.request_anim_frame();
                }
            }
            ToastPhase::Visible => {
                if let Some(limit) = self.duration_ns {
                    self.stay_elapsed_ns = self.stay_elapsed_ns.saturating_add(interval);
                    if self.stay_elapsed_ns >= limit {
                        if self.exit_duration_ns > 0 {
                            self.phase = ToastPhase::Exiting { auto_dismiss: true };
                            self.phase_elapsed_ns = 0;
                            ctx.request_compose();
                            ctx.request_paint_only();
                            ctx.request_post_paint();
                            ctx.request_anim_frame();
                        } else {
                            self.phase = ToastPhase::Idle;
                            ctx.submit_action::<ToastHostAction>(ToastHostAction::Expired);
                        }
                    } else {
                        ctx.request_anim_frame();
                    }
                }
            }
            ToastPhase::Exiting { auto_dismiss } => {
                self.phase_elapsed_ns = self.phase_elapsed_ns.saturating_add(interval);
                ctx.request_compose();
                ctx.request_paint_only();
                ctx.request_post_paint();
                if self.phase_elapsed_ns >= self.exit_duration_ns {
                    self.phase = ToastPhase::Idle;
                    self.phase_elapsed_ns = 0;
                    ctx.submit_action::<ToastHostAction>(if auto_dismiss {
                        ToastHostAction::Expired
                    } else {
                        ToastHostAction::ExitFinished
                    });
                } else {
                    ctx.request_anim_frame();
                }
            }
        }
    }

    fn on_pointer_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &PointerEvent,
    ) {
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        _len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        ctx.redirect_measurement(&mut self.child, axis, cross_length)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.child, size);
        ctx.place_child(&mut self.child, Point::ORIGIN);
    }

    fn compose(&mut self, ctx: &mut ComposeCtx<'_>) {
        let dy = self.current_offset_y();
        ctx.set_animated_child_scroll_translation(&mut self.child, Vec2::new(0.0, dy));
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let alpha = self.current_alpha();
        if alpha < 0.999 {
            painter.push_group(
                GroupRef::new().with_composite(Composite::new(peniko::BlendMode::default(), alpha)),
            );
        }
    }

    fn post_paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        if self.current_alpha() < 0.999 {
            painter.pop_group();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Alert
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }
}

#[doc(hidden)]
#[expect(
    missing_debug_implementations,
    reason = "Holds boxed child View and ViewState"
)]
pub struct UiToastHostViewState<State: 'static, Action: 'static> {
    exiting_toast: Option<ToastData>,
    child_view: Box<AnyWidgetView<State, Action>>,
    child_state: <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
}

/// View created by [`toast_host`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiToastHost<State: 'static, Action: 'static> {
    current: Option<ToastData>,
    on_action: Option<ToastCallback<State, Action>>,
    on_dismiss: ToastCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State: 'static, Action: 'static> UiToastHost<State, Action> {
    fn child_view(&self, exiting_toast: Option<&ToastData>) -> Box<AnyWidgetView<State, Action>> {
        if let Some(data) = &self.current {
            toast(data, self.on_action.clone(), Some(self.on_dismiss.clone())).boxed()
        } else if let Some(exiting) = exiting_toast {
            toast(exiting, None, None).boxed()
        } else {
            empty_slot().boxed()
        }
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiToastHost<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiToastHost<State, Action> {
    type Element = Pod<ToastTimerHost>;
    type ViewState = UiToastHostViewState<State, Action>;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let theme = current_theme_arc(ctx);
        let (enter_ns, exit_ns) = toast_motion_durations_ns(&theme);
        let view = self.child_view(None);
        let (child_pod, child_state) =
            ctx.with_id(TOAST_CHILD_ID, |ctx| view.build(ctx, app_state));
        let active_id = self.current.as_ref().map(|d| d.id);
        let duration_ns = self.current.as_ref().and_then(|d| d.duration.as_nanos());
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(ToastTimerHost::new(
                child_pod.new_widget,
                active_id,
                duration_ns,
                enter_ns,
                exit_ns,
            ))
        });
        (
            pod,
            UiToastHostViewState {
                exiting_toast: None,
                child_view: view,
                child_state,
            },
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let theme = current_theme_arc(ctx);
        let (enter_ns, exit_ns) = toast_motion_durations_ns(&theme);
        element.widget.enter_duration_ns = enter_ns;
        element.widget.exit_duration_ns = exit_ns;

        let next_id = self.current.as_ref().map(|d| d.id);
        let next_dur = self.current.as_ref().and_then(|d| d.duration.as_nanos());
        let mut needs_anim = false;
        let mut needs_visual_refresh = false;

        if let Some(id) = next_id {
            view_state.exiting_toast = None;
            if element.widget.active_id != Some(id) || element.widget.duration_ns != next_dur {
                let id_changed = element.widget.active_id != Some(id);
                element.widget.active_id = Some(id);
                element.widget.duration_ns = next_dur;
                element.widget.stay_elapsed_ns = 0;
                if id_changed {
                    element.widget.phase_elapsed_ns = 0;
                    element.widget.phase = if enter_ns > 0 {
                        ToastPhase::Entering
                    } else {
                        ToastPhase::Visible
                    };
                    needs_visual_refresh = true;
                }
                needs_anim = element.widget.needs_anim();
            }
        } else if let Some(prev_data) = &prev.current
            && exit_ns > 0
            && !matches!(
                element.widget.phase,
                ToastPhase::Idle | ToastPhase::Exiting { .. }
            )
        {
            view_state.exiting_toast = Some(prev_data.clone());
            element.widget.active_id = None;
            element.widget.duration_ns = None;
            element.widget.phase = ToastPhase::Exiting {
                auto_dismiss: false,
            };
            element.widget.phase_elapsed_ns = 0;
            element.widget.stay_elapsed_ns = 0;
            needs_visual_refresh = true;
            needs_anim = true;
        } else if view_state.exiting_toast.is_none() {
            element.widget.active_id = None;
            element.widget.duration_ns = None;
            if element.widget.phase != ToastPhase::Idle {
                element.widget.phase = ToastPhase::Idle;
                element.widget.phase_elapsed_ns = 0;
                element.widget.stay_elapsed_ns = 0;
                needs_visual_refresh = true;
            }
        }

        if needs_visual_refresh {
            element.ctx.request_compose();
            element.ctx.request_paint_only();
            element.ctx.request_post_paint();
        }
        if needs_anim {
            element.ctx.request_anim_frame();
        }

        let next_view = self.child_view(view_state.exiting_toast.as_ref());
        let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
        ctx.with_id(TOAST_CHILD_ID, |ctx| {
            next_view.rebuild(
                &view_state.child_view,
                &mut view_state.child_state,
                ctx,
                child_dyn.downcast(),
                app_state,
            );
        });
        view_state.child_view = next_view;
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
            ctx.with_id(TOAST_CHILD_ID, |ctx| {
                view_state.child_view.teardown(
                    &mut view_state.child_state,
                    ctx,
                    child_dyn.downcast(),
                );
            });
        }
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(TOAST_CHILD_ID) => {
                let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
                view_state.child_view.message(
                    &mut view_state.child_state,
                    message,
                    child_dyn.downcast(),
                    app_state,
                )
            }
            None => match message.take_message::<ToastHostAction>() {
                Some(action) => match *action {
                    ToastHostAction::Expired => {
                        view_state.exiting_toast = None;
                        MessageResult::Action((self.on_dismiss)(app_state))
                    }
                    ToastHostAction::ExitFinished => {
                        view_state.exiting_toast = None;
                        MessageResult::RequestRebuild
                    }
                },
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}

/// Hosts queued notifications from [`ToastHostState`] with automatic dismissal timers.
pub fn toast_host<State: 'static, Action: 'static, F>(
    state: &ToastHostState,
    on_dismiss: F,
) -> UiToastHost<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let _ = (Color::TRANSPARENT, CornerRadius { radius: 0.px() });
    UiToastHost {
        current: state.current().cloned(),
        on_action: None,
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

impl<State: 'static, Action: 'static> UiToastHost<State, Action> {
    /// Attaches an action handler for the optional trailing toast action button.
    pub fn on_action<F>(mut self, on_action: F) -> Self
    where
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        self.on_action = Some(Arc::new(on_action));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_queue_is_fifo_and_assigns_monotonic_ids() {
        let mut q = ToastHostState::new();
        assert!(q.is_empty());
        let id1 = q.show("Saved", None::<&str>, ToastDuration::Short, false);
        let id2 = q.show("Synced", Some("Undo"), ToastDuration::Long, true);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(q.len(), 2);
        assert_eq!(q.current().unwrap().message.as_ref(), "Saved");

        let popped = q.dismiss_current().unwrap();
        assert_eq!(popped.id, 1);
        assert_eq!(q.current().unwrap().id, 2);
        assert_eq!(q.current().unwrap().action_label.as_deref(), Some("Undo"));

        q.dismiss_current();
        assert!(q.is_empty());
        assert!(q.current().is_none());
    }

    #[test]
    fn toast_timer_host_progresses_fade_and_respects_reduced_motion() {
        use crate::foundation::overlay::empty_slot;
        use masonry::widgets::SizedBox;

        let mut host = ToastTimerHost::new(
            NewWidget::new(SizedBox::empty()),
            Some(1),
            Some(4_000_000_000),
            150_000_000,
            100_000_000,
        );
        assert_eq!(host.phase, ToastPhase::Entering);
        assert_eq!(host.current_alpha(), 0.0);
        assert!((host.current_offset_y() - TOAST_ENTER_OFFSET_Y).abs() < 1e-6);

        host.phase_elapsed_ns = 75_000_000;
        let mid_alpha = host.current_alpha();
        assert!(mid_alpha > 0.1 && mid_alpha < 0.99);
        assert!(host.current_offset_y() > 0.0 && host.current_offset_y() < TOAST_ENTER_OFFSET_Y);

        host.phase = ToastPhase::Visible;
        host.phase_elapsed_ns = 0;
        assert_eq!(host.current_alpha(), 1.0);
        assert_eq!(host.current_offset_y(), 0.0);

        host.phase = ToastPhase::Exiting { auto_dismiss: true };
        host.phase_elapsed_ns = 50_000_000;
        let exit_alpha = host.current_alpha();
        assert!(exit_alpha > 0.01 && exit_alpha < 0.99);
        assert!(host.current_offset_y() > 0.0 && host.current_offset_y() < TOAST_EXIT_OFFSET_Y);

        host.phase_elapsed_ns = 100_000_000;
        assert_eq!(host.current_alpha(), 0.0);
        assert!((host.current_offset_y() - TOAST_EXIT_OFFSET_Y).abs() < 1e-6);

        let _ = empty_slot::<(), ()>;
        let reduced = ToastTimerHost::new(
            NewWidget::new(SizedBox::empty()),
            Some(2),
            Some(4_000_000_000),
            0,
            0,
        );
        assert_eq!(reduced.phase, ToastPhase::Visible);
        assert_eq!(reduced.current_alpha(), 1.0);
        assert_eq!(reduced.current_offset_y(), 0.0);
    }
}
