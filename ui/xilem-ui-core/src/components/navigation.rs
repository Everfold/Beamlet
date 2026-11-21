//! Cross-theme navigation bars, rails, permanent/modal drawers honoring [`NavIndicatorStyle`], and adaptive scaffold.

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::Color;
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::kurbo::Vec2;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{
    AnyFlexChild, CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row,
    label, sized_box,
};
use xilem::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::components::divider::{horizontal_divider, vertical_divider};
use crate::foundation::overlay::{
    EscapeCatcher, ModalShellConfig, ModalShellState, modal_shell_build, modal_shell_message,
    modal_shell_rebuild, modal_shell_teardown,
};
use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, MotionDuration, NavIndicatorStyle, TypeRole, WindowSizeClass};

/// Navigation item metadata shared across bottom bar, rail, drawer, and adaptive scaffold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavDestination {
    /// Destination title.
    pub label: ArcStr,
    /// SVG icon path (`<path d="..."/>`).
    pub icon: &'static str,
    /// Optional numeric/badge caption.
    pub badge_text: Option<ArcStr>,
}

impl NavDestination {
    /// Creates a navigation destination without a badge.
    #[must_use]
    pub fn new(label: impl Into<ArcStr>, icon: &'static str) -> Self {
        Self {
            label: label.into(),
            icon,
            badge_text: None,
        }
    }

    /// Attaches a badge label to this destination.
    #[must_use]
    pub fn with_badge(mut self, badge: impl Into<ArcStr>) -> Self {
        self.badge_text = Some(badge.into());
        self
    }
}

struct TopBarWrapper<V, State, Action> {
    inner: V,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State: 'static, Action: 'static> ViewMarker for TopBarWrapper<V, State, Action> {}

impl<V, State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for TopBarWrapper<V, State, Action>
where
    V: WidgetView<State, Action>,
{
    type Element = Pod<xilem::masonry::widgets::SizedBox>;
    type ViewState = (f64, V::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = crate::style_engine::current_spec(ctx);
        let h = spec.metrics.top_bar_height;
        let (child_pod, child_state) = self.inner.build(ctx, app_state);
        let mut pod = ctx.create_pod(xilem::masonry::widgets::SizedBox::new(
            child_pod.new_widget.erased(),
        ));
        pod.new_widget
            .classes
            .insert(classes::HEADER_BAR.to_string());
        pod.new_widget.properties.insert(Dimensions::height(h.px()));
        (pod, (h, child_state))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (prev_h, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = crate::style_engine::current_spec(ctx);
        let next_h = spec.metrics.top_bar_height;
        if (*prev_h - next_h).abs() > 1e-6 {
            element.insert_prop(Dimensions::height(next_h.px()));
            *prev_h = next_h;
        }
        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("top bar SizedBox always has child");
        self.inner.rebuild(
            &prev.inner,
            child_state,
            ctx,
            child_dyn.downcast(),
            app_state,
        );
    }

    fn teardown(
        &self,
        (_, child_state): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("top bar SizedBox always has child");
        self.inner.teardown(child_state, ctx, child_dyn.downcast());
    }

    fn message(
        &self,
        (_, child_state): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        let mut child_dyn = xilem::masonry::widgets::SizedBox::child_mut(&mut element)
            .expect("top bar SizedBox always has child");
        self.inner
            .message(child_state, message, child_dyn.downcast(), app_state)
    }
}

/// Top navigation/header chrome bar (neutral base; platform themes provide signature variants too).
pub fn top_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    leading: Option<Box<AnyWidgetView<State, Action>>>,
    trailing: Option<Box<AnyWidgetView<State, Action>>>,
) -> Box<AnyWidgetView<State, Action>> {
    let title: ArcStr = title.into();
    let title_view = with_spec(move |spec: &DesignSpec, _| {
        spec.typography
            .style_label(TypeRole::TitleLarge, label(title.clone()))
            .color(spec.palette.text_primary)
    });
    let row = flex_row((leading, title_view.flex(1.0), trailing))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px())
        .padding(Padding::from_vh(8.px(), 16.px()));
    TopBarWrapper {
        inner: row,
        phantom: PhantomData,
    }
    .boxed()
}

fn nav_badge_view<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    badge: &Option<ArcStr>,
) -> Option<Box<AnyWidgetView<State, Action>>> {
    badge.as_ref().map(|b| {
        spec.typography
            .style_label(TypeRole::LabelSmall, label(b.clone()))
            .color(spec.palette.on_error)
            .background_color(spec.palette.error)
            .padding(Padding::from_vh(2.px(), 6.px()))
            .corner_radius(999.px())
            .boxed()
    })
}

/// Compact bottom navigation bar for narrow screens or compact panes.
pub fn navigation_bar<State: 'static, Action: 'static, F>(
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
) -> Box<AnyWidgetView<State, Action>>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let dests = destinations.to_vec();
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let items: Vec<AnyFlexChild<State, Action>> = dests
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.on_secondary_container
                } else {
                    p.text_secondary
                };
                let pill_bg = if active {
                    p.secondary_container
                } else {
                    Color::TRANSPARENT
                };
                let icon_pill = sized_box(ui_icon(d.icon, 18.0, fg))
                    .padding(Padding::from_vh(4.px(), 14.px()))
                    .background_color(pill_bg)
                    .corner_radius(999.px());
                let txt = spec
                    .typography
                    .style_label(TypeRole::LabelMedium, label(d.label.clone()))
                    .color(if active {
                        p.text_primary
                    } else {
                        p.text_secondary
                    });
                let col = flex_col((icon_pill, txt))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(4.px());
                AnyFlexChild::Item(
                    button(col, move |s: &mut State| cb(s, i))
                        .ui_class(classes::TAB_ITEM)
                        .boxed()
                        .flex(1.0),
                )
            })
            .collect();
        flex_col((
            horizontal_divider(),
            sized_box(
                flex_row(items)
                    .main_axis_alignment(MainAxisAlignment::SpaceAround)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .padding(Padding::from_vh(6.px(), 8.px())),
            )
            .ui_class(classes::BOTTOM_BAR),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .boxed()
    })
    .boxed()
}

/// Medium-width vertical icon navigation rail.
pub fn navigation_rail<State: 'static, Action: 'static, F>(
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
) -> Box<AnyWidgetView<State, Action>>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let dests = destinations.to_vec();
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let items: Vec<Box<AnyWidgetView<State, Action>>> = dests
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.on_secondary_container
                } else {
                    p.text_secondary
                };
                let pill_bg = if active {
                    p.secondary_container
                } else {
                    Color::TRANSPARENT
                };
                let pill = sized_box(ui_icon(d.icon, 18.0, fg))
                    .padding(Padding::from_vh(5.px(), 14.px()))
                    .background_color(pill_bg)
                    .corner_radius(spec.metrics.nav_pill_radius.px());
                let txt = spec
                    .typography
                    .style_label(TypeRole::LabelSmall, label(d.label.clone()))
                    .color(if active {
                        p.text_primary
                    } else {
                        p.text_secondary
                    });
                button(
                    flex_col((pill, txt))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(classes::TAB_ITEM)
                .boxed()
            })
            .collect();

        flex_row((
            sized_box(
                flex_col(items)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px())
                    .padding(Padding::from_vh(12.px(), 6.px())),
            )
            .dims(Dimensions::STRETCH.with_width(80.px()))
            .background_color(p.surface_low),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .boxed()
    })
    .boxed()
}

/// Persistent sidebar navigation drawer honoring [`NavIndicatorStyle`]:
/// - MD3 `PillBackground`
/// - Windows Fluent `LeadingAccentBar` (`3px × 16px` left pill accent bar)
/// - macOS HIG / GNOME Adwaita `RoundedRow`
pub fn navigation_drawer<State: 'static, Action: 'static, F>(
    header_title: Option<&str>,
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
) -> Box<AnyWidgetView<State, Action>>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let header_title = header_title.map(ArcStr::from);
    let dests = destinations.to_vec();
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let head = header_title.as_ref().map(|h| {
            spec.typography
                .style_label(TypeRole::TitleSmall, label(h.clone()))
                .color(p.text_secondary)
                .padding(Padding::from_vh(8.px(), 12.px()))
        });

        let items: Vec<Box<AnyWidgetView<State, Action>>> = dests
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = match (spec.nav_indicator, active) {
                    (NavIndicatorStyle::PillBackground, true) => p.on_secondary_container,
                    (NavIndicatorStyle::LeadingAccentBar | NavIndicatorStyle::RoundedRow, true) => {
                        p.text_primary
                    }
                    (_, false) => p.text_secondary,
                };
                let leading_bar = matches!(spec.nav_indicator, NavIndicatorStyle::LeadingAccentBar)
                    .then(|| {
                        sized_box(label(""))
                            .dims(Dimensions::fixed(3.px(), 16.px()))
                            .background_color(if active { p.accent } else { Color::TRANSPARENT })
                            .corner_radius(2.px())
                    });
                let icon_c = if active { p.accent } else { fg };
                let lbl = spec
                    .typography
                    .style_label(TypeRole::LabelLarge, label(d.label.clone()))
                    .color(fg);
                let badge = nav_badge_view(spec, &d.badge_text);
                let cls = if active {
                    classes::NAV_PILL_ACTIVE
                } else {
                    classes::NAV_PILL
                };
                button(
                    flex_row((
                        leading_bar,
                        ui_icon(d.icon, 18.0, icon_c),
                        lbl.flex(1.0),
                        badge,
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();

        flex_row((
            sized_box(
                flex_col((
                    head,
                    flex_col(items)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(6.px())
                .padding(Padding::all(12.px())),
            )
            .dims(Dimensions::STRETCH.with_width(240.px()))
            .background_color(p.surface_low),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .boxed()
    })
    .boxed()
}

/// Modal drawer view created by [`modal_navigation_drawer`].
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct UiModalDrawer<V, State, Action> {
    drawer: V,
    width: f64,
    on_dismiss: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<V, State, Action> UiModalDrawer<V, State, Action> {
    fn shell_config(&self) -> ModalShellConfig {
        ModalShellConfig {
            class: classes::SHEET_SURFACE,
            dims: Dimensions::STRETCH.with_width(self.width.px()),
            padding: Padding::all(0.px()),
            alignment: UnitPoint::LEFT,
            enter_offset: Vec2::new(-48.0, 0.0),
            duration: MotionDuration::Medium3,
            dismiss_on_scrim: true,
        }
    }
}

impl<V, State, Action> ViewMarker for UiModalDrawer<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UiModalDrawer<V, State, Action>
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
            &self.drawer,
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
            &prev.drawer,
            &self.drawer,
            self.on_dismiss.clone(),
        );
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        modal_shell_teardown(view_state, ctx, element, &self.drawer);
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
            &self.drawer,
            &self.on_dismiss,
        )
    }
}

/// Hosts an animated left-anchored modal navigation drawer over scrim + Escape key handling.
pub fn modal_navigation_drawer<State: 'static, Action: 'static, F, S>(
    header_title: Option<&str>,
    destinations: &[NavDestination],
    selected: usize,
    on_select: S,
    on_dismiss: F,
) -> UiModalDrawer<Box<AnyWidgetView<State, Action>>, State, Action>
where
    S: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let drawer = navigation_drawer(header_title, destinations, selected, on_select);
    UiModalDrawer {
        drawer,
        width: 260.0,
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

/// Adaptive navigation layout mode derived from [`WindowSizeClass`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationSuiteType {
    /// Bottom navigation bar (`Compact`).
    NavigationBar,
    /// Vertical icon rail (`Medium`).
    NavigationRail,
    /// Permanent sidebar drawer (`Expanded`).
    NavigationDrawer,
}

impl NavigationSuiteType {
    /// Chooses the canonical navigation chrome for `size_class`.
    #[must_use]
    pub const fn from_window_size(size_class: WindowSizeClass) -> Self {
        match size_class {
            WindowSizeClass::Compact => Self::NavigationBar,
            WindowSizeClass::Medium => Self::NavigationRail,
            WindowSizeClass::Expanded => Self::NavigationDrawer,
        }
    }
}

/// Responsive scaffold switching automatically between [`navigation_bar`], [`navigation_rail`],
/// and [`navigation_drawer`] according to `size_class`.
pub fn adaptive_scaffold<State: 'static, Action: 'static, V, F>(
    size_class: WindowSizeClass,
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
    body: V,
) -> Box<AnyWidgetView<State, Action>>
where
    V: WidgetView<State, Action>,
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let _ = icons::MENU;
    match NavigationSuiteType::from_window_size(size_class) {
        NavigationSuiteType::NavigationBar => {
            let nav = navigation_bar(destinations, selected, on_select);
            flex_col((body.flex(1.0), nav))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .boxed()
        }
        NavigationSuiteType::NavigationRail => {
            let nav = navigation_rail(destinations, selected, on_select);
            flex_row((nav, body.flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .boxed()
        }
        NavigationSuiteType::NavigationDrawer => {
            let nav = navigation_drawer(Some("Navigation"), destinations, selected, on_select);
            flex_row((nav, body.flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .boxed()
        }
    }
}
