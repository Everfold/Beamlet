//! Toast / Snackbar queue state, timer-backed host, and spec-styled banner view.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;

use xilem::Color;
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem::kurbo::{Axis, Point, Size};
use xilem::masonry::accesskit::{Node, Role};
use xilem::masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetPod,
};
use xilem::masonry::imaging::Painter;
use xilem::masonry::layout::{AsUnit, LenReq, Length};
use xilem::masonry::properties::{CornerRadius, Dimensions, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, button, flex_row, label, sized_box};
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::foundation::overlay::empty_slot;
use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, ElevationLevel, TypeRole};

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
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let msg = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(data.message.clone()))
            .color(p.inverse_on_surface);

        let act_btn = data
            .action_label
            .as_ref()
            .zip(on_action.as_ref())
            .map(|(lbl, cb)| {
                let cb = cb.clone();
                let action_lbl = spec
                    .typography
                    .style_label(TypeRole::LabelLarge, label(lbl.clone()))
                    .color(p.accent_container);
                button(action_lbl, move |s: &mut State| cb(s)).ui_class(classes::BTN_SUBTLE)
            });
        let close_btn = if data.with_close
            && let Some(cb) = &on_dismiss
        {
            let cb = cb.clone();
            Some(
                button(
                    ui_icon(icons::CLOSE, 16.0, p.inverse_on_surface),
                    move |s| cb(s),
                )
                .ui_class(classes::ICON_BTN_STD),
            )
        } else {
            None
        };

        sized_box(
            flex_row((msg, act_btn, close_btn))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px())
                .padding(Padding::from_vh(10.px(), 16.px())),
        )
        .dims(Dimensions::MAX)
        .background_color(p.inverse_surface)
        .corner_radius(spec.metrics.button_radius.max(8.0).px())
        .box_shadow(spec.box_shadow(ElevationLevel::Level3))
    })
}

/// Internal action emitted by [`ToastTimerHost`] when a toast's display duration expires.
#[derive(Debug)]
pub struct ToastExpired;

const TOAST_CHILD_ID: ViewId = ViewId::new(0x5549_3450);

/// Internal Masonry widget timing auto-dismissal for [`toast_host`].
#[expect(missing_debug_implementations, reason = "Holds erased child WidgetPod")]
pub struct ToastTimerHost {
    child: WidgetPod<dyn Widget>,
    active_id: Option<u64>,
    duration_ns: Option<u64>,
    elapsed_ns: u64,
    fired: bool,
}

impl ToastTimerHost {
    fn new(
        child: NewWidget<impl Widget + ?Sized>,
        active_id: Option<u64>,
        duration_ns: Option<u64>,
    ) -> Self {
        Self {
            child: child.erased().to_pod(),
            active_id,
            duration_ns,
            elapsed_ns: 0,
            fired: false,
        }
    }
}

impl Widget for ToastTimerHost {
    type Action = ToastExpired;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded)
            && self.active_id.is_some()
            && self.duration_ns.is_some()
        {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.active_id.is_some()
            && let Some(limit) = self.duration_ns
            && !self.fired
        {
            self.elapsed_ns += interval;
            if self.elapsed_ns >= limit {
                self.fired = true;
                ctx.submit_action::<ToastExpired>(ToastExpired);
            } else {
                ctx.request_anim_frame();
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

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
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

/// View created by [`toast_host`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiToastHost<State: 'static, Action: 'static> {
    current: Option<ToastData>,
    on_action: Option<ToastCallback<State, Action>>,
    on_dismiss: ToastCallback<State, Action>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State: 'static, Action: 'static> UiToastHost<State, Action> {
    fn child_view(&self) -> Box<AnyWidgetView<State, Action>> {
        if let Some(data) = &self.current {
            toast(data, self.on_action.clone(), Some(self.on_dismiss.clone())).boxed()
        } else {
            empty_slot().boxed()
        }
    }
}

impl<State: 'static, Action: 'static> ViewMarker for UiToastHost<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiToastHost<State, Action> {
    type Element = Pod<ToastTimerHost>;
    type ViewState = (
        Box<AnyWidgetView<State, Action>>,
        <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let view = self.child_view();
        let (child_pod, child_state) =
            ctx.with_id(TOAST_CHILD_ID, |ctx| view.build(ctx, app_state));
        let active_id = self.current.as_ref().map(|d| d.id);
        let duration_ns = self.current.as_ref().and_then(|d| d.duration.as_nanos());
        let pod = ctx.with_action_widget(|ctx| {
            ctx.create_pod(ToastTimerHost::new(
                child_pod.new_widget,
                active_id,
                duration_ns,
            ))
        });
        (pod, (view, child_state))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_view, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let next_id = self.current.as_ref().map(|d| d.id);
        let next_dur = self.current.as_ref().and_then(|d| d.duration.as_nanos());
        if element.widget.active_id != next_id || element.widget.duration_ns != next_dur {
            element.widget.active_id = next_id;
            element.widget.duration_ns = next_dur;
            element.widget.elapsed_ns = 0;
            element.widget.fired = false;
            if next_id.is_some() && next_dur.is_some() {
                element.ctx.request_anim_frame();
            }
        }
        let next_view = self.child_view();
        let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
        ctx.with_id(TOAST_CHILD_ID, |ctx| {
            next_view.rebuild(prev_view, child_state, ctx, child_dyn.downcast(), app_state);
        });
        *prev_view = next_view;
    }

    fn teardown(
        &self,
        (view, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
            ctx.with_id(TOAST_CHILD_ID, |ctx| {
                view.teardown(child_state, ctx, child_dyn.downcast());
            });
        }
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (view, child_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(TOAST_CHILD_ID) => {
                let mut child_dyn = element.ctx.get_mut(&mut element.widget.child);
                view.message(child_state, message, child_dyn.downcast(), app_state)
            }
            None => match message.take_message::<ToastExpired>() {
                Some(_) => MessageResult::Action((self.on_dismiss)(app_state)),
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
}
