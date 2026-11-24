//! GNOME Libadwaita signature components: `header_bar`, `header_bar_with_title`, `preferences_group`, `action_row`, `clickable_action_row`, `switch_row`, `status_page`, `banner`.

use std::marker::PhantomData;
use std::sync::Arc;

use brocado_core::components::button::{
    destructive_button, primary_button, secondary_button, subtle_button,
};
use brocado_core::components::divider::horizontal_divider;
use brocado_core::components::menu::select;
use brocado_core::components::selection::switch;
use brocado_core::components::typography::ui_label;
use brocado_core::foundation::{
    EscapeCatcher, FocusFrame, ModalAxisSizing, ModalShellConfig, ModalShellState, UiClassExt,
    adaptive_pair, clip_box, constrained_box, focus_frame, icons, modal_shell_build,
    modal_shell_message, modal_shell_rebuild, modal_shell_teardown, ui_icon, wrap,
};
use brocado_core::style_engine::{classes, current_spec_arc, with_spec};
use brocado_core::tokens::{DesignSpec, MotionDuration, TypeRole};
use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetId, WidgetMut,
    WidgetPod,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size, Vec2};
use masonry::layout::{AsUnit, LayoutSize, LenDef, LenReq, Length, UnitPoint};
use masonry::properties::{LineBreaking, Padding};
use masonry::widgets;
use tracing::{Span, trace_span};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, sized_box,
    text_input,
};

/// GNOME `AdwHeaderBar` (`46px` bar) hosting a custom center `title_view` alongside start and end action clusters.
pub fn header_bar_with_title<State: 'static, Action: 'static, T>(
    title_view: T,
    start_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    end_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    T: WidgetView<State, Action>,
{
    flex_col((
        sized_box(
            constrained_box(
                flex_row((
                    (!start_actions.is_empty()).then(|| {
                        flex_row(start_actions)
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(4.px())
                    }),
                    clip_box(title_view).fill_available(true).flex(1.0),
                    (!end_actions.is_empty()).then(|| {
                        flex_row(end_actions)
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(4.px())
                    }),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .padding(Padding::from_vh(6.px(), 6.px())),
            )
            .min_height_from_role(TypeRole::TitleMedium, 18.0, 46.0),
        )
        .ui_class(classes::HEADER_BAR),
        horizontal_divider(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// GNOME `AdwHeaderBar` (`46px` bar with centered title/subtitle and start/end action clusters).
pub fn header_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    start_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    end_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> {
    let title: ArcStr = title.into();
    let subtitle: Option<ArcStr> = subtitle.map(Into::into);
    let center_title = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let t = clip_box(
            ui_label(TypeRole::TitleSmall, title.clone())
                .color(p.text_primary)
                .line_breaking(LineBreaking::Clip),
        );
        let sub = subtitle.as_ref().map(|s| {
            clip_box(
                ui_label(TypeRole::BodySmall, s.clone())
                    .color(p.text_secondary)
                    .line_breaking(LineBreaking::Clip),
            )
        });
        clip_box(flex_col((t, sub)).cross_axis_alignment(CrossAxisAlignment::Center))
            .fill_available(true)
    });

    header_bar_with_title(center_title, start_actions, end_actions)
}

/// GNOME `AdwPreferencesGroup` displaying a heading, optional description, and `boxed-list` card of rows.
pub fn preferences_group<State: 'static, Action: 'static>(
    title: Option<&str>,
    description: Option<&str>,
    rows: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action> {
    let title_owned = title.map(ArcStr::from);
    let desc_owned = description.map(ArcStr::from);

    let header = if title_owned.is_some() || desc_owned.is_some() {
        Some(with_spec(move |spec: &DesignSpec, _| {
            let p = &spec.palette;
            let t = title_owned
                .as_ref()
                .map(|s| ui_label(TypeRole::TitleMedium, s.clone()).color(p.text_primary));
            let d = desc_owned
                .as_ref()
                .map(|s| ui_label(TypeRole::BodySmall, s.clone()).color(p.text_secondary));
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(2.px())
        }))
    } else {
        None
    };

    let mut children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
    for (idx, r) in rows.into_iter().enumerate() {
        if idx > 0 {
            children.push(horizontal_divider().boxed());
        }
        children.push(r);
    }

    let boxed_list =
        sized_box(flex_col(children).cross_axis_alignment(CrossAxisAlignment::Stretch))
            .padding(Padding::ZERO)
            .ui_class(classes::GROUPED_CARD);

    flex_col((header, boxed_list))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(8.px())
}

/// Action callback for clickable [`action_row`] and [`clickable_action_row`].
pub type ActionRowCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

fn action_row_inner<State: 'static, Action: 'static, S>(
    title: ArcStr,
    subtitle: Option<ArcStr>,
    prefix_icon: Option<&'static str>,
    suffix: Option<S>,
) -> Box<AnyWidgetView<State, Action>>
where
    S: WidgetView<State, Action>,
{
    let leading_and_text = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let ic = prefix_icon.map(|path| ui_icon(path, 18.0, p.text_secondary));
        let t = ui_label(TypeRole::BodyLarge, title.clone()).color(p.text_primary);
        let sub = subtitle
            .as_ref()
            .map(|s| ui_label(TypeRole::BodySmall, s.clone()).color(p.text_secondary));
        flex_row((
            ic,
            flex_col((t, sub))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
    });

    if let Some(suffix) = suffix {
        adaptive_pair(leading_and_text, suffix)
            .gap(12.px())
            .run_gap(8.px())
            .min_leading(56.0)
            .boxed()
    } else {
        leading_and_text.boxed()
    }
}

/// GNOME `AdwActionRow` displaying leading icon, title/subtitle, and trailing suffix widget.
pub fn action_row<State: 'static, Action: 'static, S>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    prefix_icon: Option<&'static str>,
    suffix: Option<S>,
    on_activate: Option<ActionRowCallback<State, Action>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    S: WidgetView<State, Action>,
{
    let row_content = action_row_inner(title.into(), subtitle.map(Into::into), prefix_icon, suffix);

    let inner = if let Some(cb) = on_activate {
        button(row_content, move |s: &mut State| cb(s))
            .ui_class(classes::LIST_ITEM)
            .corner_radius(0.px())
            .boxed()
    } else {
        row_content
            .padding(Padding::from_vh(10.px(), 14.px()))
            .boxed()
    };
    sized_box(inner)
}

/// Clickable GNOME `AdwActionRow` returning `Pod<widgets::Button>` directly.
pub fn clickable_action_row<State: 'static, Action: 'static, S, F>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    prefix_icon: Option<&'static str>,
    suffix: Option<S>,
    on_activate: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    S: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let row_content = action_row_inner(title.into(), subtitle.map(Into::into), prefix_icon, suffix);
    button(row_content, on_activate)
        .ui_class(classes::LIST_ITEM)
        .corner_radius(0.px())
}

/// GNOME `AdwSwitchRow` pairing a title/subtitle directly with an interactive [`switch`].
pub fn switch_row<State: 'static, Action: 'static, F>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    value: bool,
    on_toggle: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    F: Fn(&mut State, bool) -> Action + Send + Sync + 'static,
{
    action_row(title, subtitle, None, Some(switch(value, on_toggle)), None)
}

/// Action callback for [`status_page`] and [`banner`].
pub type AdwaitaActionCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// GNOME `AdwStatusPage` centered empty-state / welcome hero with icon, title, description, and CTA button.
pub fn status_page<State: 'static, Action: 'static, T1: Into<ArcStr>, T2: Into<ArcStr>>(
    icon_svg: &'static str,
    title: T1,
    description: T2,
    primary_action: Option<(&str, AdwaitaActionCallback<State, Action>)>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, T1, T2> {
    let title: ArcStr = title.into();
    let description: ArcStr = description.into();

    let hero_text = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        flex_col((
            ui_icon(icon_svg, 56.0, p.text_secondary),
            ui_label(TypeRole::HeadlineMedium, title.clone()).color(p.text_primary),
            ui_label(TypeRole::BodyMedium, description.clone()).color(p.text_secondary),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px())
    });

    let cta = primary_action.map(|(lbl, cb)| {
        let lbl = lbl.to_string();
        primary_button(lbl, move |s: &mut State| cb(s))
    });

    flex_col((hero_text, cta))
        .main_axis_alignment(MainAxisAlignment::Center)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(16.px())
        .padding(Padding::all(16.px()))
}

/// GNOME `AdwBanner` full-width inline accent alert strip with optional button and close icon.
pub fn banner<State: 'static, Action: 'static, T: Into<ArcStr>>(
    title: T,
    action_label: Option<&str>,
    on_action: Option<AdwaitaActionCallback<State, Action>>,
    on_dismiss: Option<AdwaitaActionCallback<State, Action>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, T> {
    let title: ArcStr = title.into();
    let action_label = action_label.map(ArcStr::from);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let lbl = ui_label(TypeRole::LabelLarge, title.clone()).color(p.on_accent_container);
        let act = action_label
            .as_ref()
            .zip(on_action.as_ref())
            .map(|(txt, cb)| {
                let cb = cb.clone();
                subtle_button(txt.clone(), move |s: &mut State| cb(s))
            });
        let close = on_dismiss.as_ref().map(|cb| {
            let cb = cb.clone();
            button(
                ui_icon(icons::CLOSE, 14.0, p.on_accent_container),
                move |s| cb(s),
            )
            .ui_class(classes::ICON_BTN_STD)
        });

        let body: Box<AnyWidgetView<State, Action>> = if act.is_some() || close.is_some() {
            adaptive_pair(
                lbl,
                flex_row((act, close))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(4.px()),
            )
            .gap(8.px())
            .run_gap(6.px())
            .min_leading(56.0)
            .padding(Padding::from_vh(8.px(), 12.px()))
            .boxed()
        } else {
            lbl.padding(Padding::from_vh(8.px(), 12.px())).boxed()
        };

        sized_box(body).background_color(p.accent_container)
    })
}

/// GNOME `AdwViewSwitcher` header-bar pill switcher displaying horizontal icon + title views.
pub fn view_switcher<State: 'static, Action: 'static, F>(
    items: &[(&str, &'static str)],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = brocado_core::foundation::WrapWidget> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let owned: Vec<(ArcStr, &'static str)> = items
        .iter()
        .map(|(name, ic)| (ArcStr::from(*name), *ic))
        .collect();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let views: Vec<Box<AnyWidgetView<State, Action>>> = owned
            .iter()
            .enumerate()
            .map(|(idx, (name, icon_path))| {
                let active = idx == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.text_primary
                } else {
                    p.text_secondary
                };
                let cls = if active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                let lbl = clip_box(
                    ui_label(TypeRole::LabelMedium, name.clone())
                        .color(fg)
                        .line_breaking(LineBreaking::Clip),
                );
                button(
                    clip_box(
                        flex_row((ui_icon(icon_path, 16.0, fg), lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(6.px()),
                    ),
                    move |s: &mut State| cb(s, idx),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();

        wrap(views)
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(4.px())
    })
}

/// GNOME `AdwExpanderRow` revealing indented nested rows inside a `PreferencesGroup` boxed list.
pub fn expander_row<State: 'static, Action: 'static, F>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    prefix_icon: Option<&'static str>,
    expanded: bool,
    on_toggle: F,
    nested_rows: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_toggle);
    let chevron_btn = with_spec(move |spec: &DesignSpec, _| {
        let cb = cb.clone();
        let chevron = if expanded {
            icons::ARROW_DROP_UP
        } else {
            icons::ARROW_DROP_DOWN
        };
        button(
            ui_icon(chevron, 18.0, spec.palette.text_secondary),
            move |s: &mut State| cb(s),
        )
        .ui_class(classes::ICON_BTN_STD)
    });

    let header = sized_box(
        action_row_inner(
            title.into(),
            subtitle.map(Into::into),
            prefix_icon,
            Some(chevron_btn),
        )
        .padding(Padding::from_vh(10.px(), 14.px())),
    );

    let child_block = expanded.then(|| {
        let mut items: Vec<Box<AnyWidgetView<State, Action>>> =
            Vec::with_capacity(nested_rows.len() * 2);
        for row in nested_rows {
            items.push(horizontal_divider().boxed());
            items.push(sized_box(row).padding(Padding::left(24.px())).boxed());
        }
        flex_col(items).cross_axis_alignment(CrossAxisAlignment::Stretch)
    });

    flex_col((header, child_block)).cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// GNOME `AdwComboRow` pairing an action row label with an anchored dropdown selector.
#[expect(
    clippy::too_many_arguments,
    reason = "Matches AdwComboRow title/subtitle plus anchored select state and callbacks"
)]
pub fn combo_row<State, Action, T1, T2, OnToggle, OnSelect, OnDismiss>(
    title: T1,
    subtitle: Option<T2>,
    options: &[&str],
    selected: usize,
    open: bool,
    on_toggle: OnToggle,
    on_select: OnSelect,
    on_dismiss: OnDismiss,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
+ use<State, Action, T1, T2, OnToggle, OnSelect, OnDismiss>
where
    State: 'static,
    Action: 'static,
    T1: Into<ArcStr>,
    T2: Into<ArcStr>,
    OnToggle: Fn(&mut State) -> Action + Send + Sync + 'static,
    OnSelect: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let selector = select(options, selected, open, on_toggle, on_select, on_dismiss);
    action_row(title, subtitle, None, Some(selector), None)
}

/// GNOME `AdwEntryRow` providing an inline labeled editable input inside a reactive [`FocusFrame`].
pub fn entry_row<State: 'static, Action: 'static, F>(
    title: impl Into<ArcStr>,
    value: String,
    on_change: F,
) -> impl WidgetView<State, Action, Widget = FocusFrame>
where
    F: Fn(&mut State, String) -> Action + Send + Sync + 'static,
{
    let title: ArcStr = title.into();
    let cb = Arc::new(on_change);
    let inner = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let cb = cb.clone();
        let top = ui_label(TypeRole::LabelSmall, title.clone()).color(p.text_secondary);
        let style = spec.typography.get(TypeRole::BodyLarge);
        let ti = text_input(value.clone(), move |s: &mut State, txt| cb(s, txt))
            .text_size(style.font_size)
            .weight(style.weight)
            .text_color(p.text_primary)
            .font(spec.typography.resolved_font_family())
            .ui_class(classes::TEXT_FIELD_FLUSH);

        sized_box(
            flex_col((top, ti))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(2.px())
                .padding(Padding::from_vh(8.px(), 14.px())),
        )
    });

    focus_frame(inner)
}

/// GNOME `AdwViewSwitcherBar` compact bottom navigation bar displayed by responsive narrow shells.
pub fn view_switcher_bar<State: 'static, Action: 'static, F>(
    items: &[(&str, &'static str)],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let owned: Vec<(ArcStr, &'static str)> = items
        .iter()
        .map(|(name, ic)| (ArcStr::from(*name), *ic))
        .collect();
    let cb = Arc::new(on_select);

    let bar_items = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let views: Vec<_> = owned
            .iter()
            .enumerate()
            .map(|(idx, (name, icon_path))| {
                let active = idx == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.text_primary
                } else {
                    p.text_secondary
                };
                let cls = if active {
                    classes::SEG_BTN_SELECTED
                } else {
                    classes::SEG_BTN
                };
                let lbl = clip_box(
                    ui_label(TypeRole::LabelSmall, name.clone())
                        .color(fg)
                        .line_breaking(LineBreaking::Clip),
                );
                button(
                    clip_box(
                        flex_col((ui_icon(icon_path, 18.0, fg), lbl))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(2.px()),
                    ),
                    move |s: &mut State| cb(s, idx),
                )
                .ui_class(cls)
                .padding(Padding::from_vh(4.px(), 2.px()))
                .boxed()
                .flex(1.0)
            })
            .collect();

        let min_h = 56.0_f64.max(f64::from(spec.typography.label_small.line_height) + 28.0);
        sized_box(
            constrained_box(
                flex_row(views)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(2.px())
                    .padding(Padding::from_vh(4.px(), 4.px())),
            )
            .min_height(min_h),
        )
        .ui_class(classes::HEADER_BAR)
    });

    flex_col((horizontal_divider(), bar_items)).cross_axis_alignment(CrossAxisAlignment::Stretch)
}

const ADW_CANCEL_ID: ViewId = ViewId::new(0x5549_3830);
const ADW_CONFIRM_ID: ViewId = ViewId::new(0x5549_3831);

struct AdwResponseRowWidget {
    cancel: WidgetPod<dyn Widget>,
    confirm: WidgetPod<dyn Widget>,
}

impl AdwResponseRowWidget {
    const H_GAP: f64 = 10.0;
    const V_GAP: f64 = 8.0;

    fn new(
        cancel: NewWidget<impl Widget + ?Sized>,
        confirm: NewWidget<impl Widget + ?Sized>,
    ) -> Self {
        Self {
            cancel: cancel.erased().to_pod(),
            confirm: confirm.erased().to_pod(),
        }
    }

    fn cancel_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.cancel)
    }

    fn confirm_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.confirm)
    }

    fn side_by_side(wc: f64, w_ok: f64, avail_w: f64) -> bool {
        !avail_w.is_finite() || (wc.max(w_ok) * 2.0 + Self::H_GAP) <= avail_w + 0.5
    }
}

impl Widget for AdwResponseRowWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.cancel);
        ctx.register_child(&mut self.confirm);
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
        let wc = ctx
            .compute_length(
                &mut self.cancel,
                LenDef::MaxContent,
                LayoutSize::NONE,
                Axis::Horizontal,
                None,
            )
            .get();
        let w_ok = ctx
            .compute_length(
                &mut self.confirm,
                LenDef::MaxContent,
                LayoutSize::NONE,
                Axis::Horizontal,
                None,
            )
            .get();
        let max_btn = wc.max(w_ok);
        let horiz_need = max_btn * 2.0 + Self::H_GAP;

        match axis {
            Axis::Horizontal => match len_req {
                LenReq::MinContent => {
                    let mc = ctx
                        .compute_length(
                            &mut self.cancel,
                            LenDef::MinContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    let mok = ctx
                        .compute_length(
                            &mut self.confirm,
                            LenDef::MinContent,
                            LayoutSize::NONE,
                            Axis::Horizontal,
                            None,
                        )
                        .get();
                    Length::px(mc.max(mok))
                }
                LenReq::MaxContent => Length::px(horiz_need),
                LenReq::FitContent(space) => {
                    let av = space.get().max(0.0);
                    if Self::side_by_side(wc, w_ok, av) {
                        Length::px(horiz_need.min(av))
                    } else {
                        Length::px(max_btn.min(av))
                    }
                }
            },
            Axis::Vertical => {
                let avail_w = cross_length
                    .or_else(|| ctx.context_size().length(Axis::Horizontal))
                    .map(Length::get);
                if avail_w.is_none_or(|aw| Self::side_by_side(wc, w_ok, aw)) {
                    let col_w = avail_w.map(|aw| Length::px(((aw - Self::H_GAP) * 0.5).max(0.0)));
                    let hc = ctx
                        .compute_length(
                            &mut self.cancel,
                            LenDef::MaxContent,
                            LayoutSize::maybe(Axis::Horizontal, col_w),
                            Axis::Vertical,
                            col_w,
                        )
                        .get();
                    let h_ok = ctx
                        .compute_length(
                            &mut self.confirm,
                            LenDef::MaxContent,
                            LayoutSize::maybe(Axis::Horizontal, col_w),
                            Axis::Vertical,
                            col_w,
                        )
                        .get();
                    Length::px(hc.max(h_ok))
                } else {
                    let full_w = avail_w.map(|aw| Length::px(aw.max(0.0)));
                    let h_ok = ctx
                        .compute_length(
                            &mut self.confirm,
                            LenDef::MaxContent,
                            LayoutSize::maybe(Axis::Horizontal, full_w),
                            Axis::Vertical,
                            full_w,
                        )
                        .get();
                    let hc = ctx
                        .compute_length(
                            &mut self.cancel,
                            LenDef::MaxContent,
                            LayoutSize::maybe(Axis::Horizontal, full_w),
                            Axis::Vertical,
                            full_w,
                        )
                        .get();
                    Length::px(h_ok + Self::V_GAP + hc)
                }
            }
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let wc = ctx
            .compute_length(
                &mut self.cancel,
                LenDef::MaxContent,
                LayoutSize::NONE,
                Axis::Horizontal,
                None,
            )
            .get();
        let w_ok = ctx
            .compute_length(
                &mut self.confirm,
                LenDef::MaxContent,
                LayoutSize::NONE,
                Axis::Horizontal,
                None,
            )
            .get();

        if Self::side_by_side(wc, w_ok, size.width) {
            let col_w = ((size.width - Self::H_GAP) * 0.5).max(0.0);
            let col_len = Some(Length::px(col_w));
            let hc = ctx
                .compute_length(
                    &mut self.cancel,
                    LenDef::MaxContent,
                    LayoutSize::one(Axis::Horizontal, Length::px(col_w)),
                    Axis::Vertical,
                    col_len,
                )
                .get();
            let h_ok = ctx
                .compute_length(
                    &mut self.confirm,
                    LenDef::MaxContent,
                    LayoutSize::one(Axis::Horizontal, Length::px(col_w)),
                    Axis::Vertical,
                    col_len,
                )
                .get();
            let row_h = hc
                .max(h_ok)
                .max(size.height)
                .min(size.height.max(hc.max(h_ok)));
            ctx.run_layout(&mut self.cancel, Size::new(col_w, row_h));
            ctx.place_child(&mut self.cancel, Point::ORIGIN);
            ctx.run_layout(&mut self.confirm, Size::new(col_w, row_h));
            ctx.place_child(&mut self.confirm, Point::new(col_w + Self::H_GAP, 0.0));
        } else {
            let full_w = size.width.max(0.0);
            let full_len = Some(Length::px(full_w));
            let h_ok = ctx
                .compute_length(
                    &mut self.confirm,
                    LenDef::MaxContent,
                    LayoutSize::one(Axis::Horizontal, Length::px(full_w)),
                    Axis::Vertical,
                    full_len,
                )
                .get();
            let hc = ctx
                .compute_length(
                    &mut self.cancel,
                    LenDef::MaxContent,
                    LayoutSize::one(Axis::Horizontal, Length::px(full_w)),
                    Axis::Vertical,
                    full_len,
                )
                .get();
            ctx.run_layout(&mut self.confirm, Size::new(full_w, h_ok));
            ctx.place_child(&mut self.confirm, Point::ORIGIN);
            ctx.run_layout(&mut self.cancel, Size::new(full_w, hc));
            ctx.place_child(&mut self.cancel, Point::new(0.0, h_ok + Self::V_GAP));
        }
        ctx.derive_baselines(&self.confirm);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.cancel.id(), self.confirm.id()])
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("AdwResponseRowWidget", id = id.trace())
    }
}

struct AdwResponseRowView<C, K, State, Action> {
    cancel: C,
    confirm: K,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<C, K, State, Action> ViewMarker for AdwResponseRowView<C, K, State, Action> {}

impl<C, K, State, Action> View<State, Action, ViewCtx> for AdwResponseRowView<C, K, State, Action>
where
    State: 'static,
    Action: 'static,
    C: WidgetView<State, Action>,
    K: WidgetView<State, Action>,
{
    type Element = Pod<AdwResponseRowWidget>;
    type ViewState = (C::ViewState, K::ViewState);

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (c_pod, c_st) = ctx.with_id(ADW_CANCEL_ID, |ctx| self.cancel.build(ctx, app_state));
        let (k_pod, k_st) = ctx.with_id(ADW_CONFIRM_ID, |ctx| self.confirm.build(ctx, app_state));
        let pod = ctx.create_pod(AdwResponseRowWidget::new(
            c_pod.new_widget,
            k_pod.new_widget,
        ));
        (pod, (c_st, k_st))
    }

    fn rebuild(
        &self,
        prev: &Self,
        (c_st, k_st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        {
            let mut c = AdwResponseRowWidget::cancel_mut(&mut element);
            ctx.with_id(ADW_CANCEL_ID, |ctx| {
                self.cancel
                    .rebuild(&prev.cancel, c_st, ctx, c.downcast(), app_state);
            });
        }
        {
            let mut k = AdwResponseRowWidget::confirm_mut(&mut element);
            ctx.with_id(ADW_CONFIRM_ID, |ctx| {
                self.confirm
                    .rebuild(&prev.confirm, k_st, ctx, k.downcast(), app_state);
            });
        }
    }

    fn teardown(
        &self,
        (c_st, k_st): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut c = AdwResponseRowWidget::cancel_mut(&mut element);
            ctx.with_id(ADW_CANCEL_ID, |ctx| {
                self.cancel.teardown(c_st, ctx, c.downcast());
            });
        }
        {
            let mut k = AdwResponseRowWidget::confirm_mut(&mut element);
            ctx.with_id(ADW_CONFIRM_ID, |ctx| {
                self.confirm.teardown(k_st, ctx, k.downcast());
            });
        }
    }

    fn message(
        &self,
        (c_st, k_st): &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        match message.take_first() {
            Some(ADW_CANCEL_ID) => {
                let mut c = AdwResponseRowWidget::cancel_mut(&mut element);
                self.cancel.message(c_st, message, c.downcast(), app_state)
            }
            Some(ADW_CONFIRM_ID) => {
                let mut k = AdwResponseRowWidget::confirm_mut(&mut element);
                self.confirm.message(k_st, message, k.downcast(), app_state)
            }
            _ => MessageResult::Stale,
        }
    }
}

/// Callback invoked by [`AdwAlertDialog`].
pub type AlertDialogCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// GNOME `AdwAlertDialog` modal featuring centered copy and adaptive split/stacked footer responses.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct AdwAlertDialog<State, Action> {
    heading: ArcStr,
    body: ArcStr,
    confirm_text: ArcStr,
    cancel_text: ArcStr,
    destructive: bool,
    width: ModalAxisSizing,
    on_confirm: AlertDialogCallback<State, Action>,
    on_dismiss: AlertDialogCallback<State, Action>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<State, Action> AdwAlertDialog<State, Action> {
    /// Colors the primary response with Adwaita `.destructive-action` styling when `true`.
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Forces a preferred modal dialog card width up to `width` (still clamped on narrow viewports).
    pub fn width(mut self, width: f64) -> Self {
        self.width = ModalAxisSizing::FillUpTo { max: Some(width) };
        self
    }

    /// Sets the minimum content-adaptive modal dialog width (defaults to `260px`).
    pub fn min_width(mut self, min_width: f64) -> Self {
        let max = match self.width {
            ModalAxisSizing::FitContent { max, .. } | ModalAxisSizing::FillUpTo { max } => max,
        };
        self.width = ModalAxisSizing::FitContent {
            min: Some(min_width),
            max,
        };
        self
    }

    /// Sets the maximum content-adaptive modal dialog width (defaults to `420px`).
    pub fn max_width(mut self, max_width: f64) -> Self {
        let min = match self.width {
            ModalAxisSizing::FitContent { min, .. } => min,
            ModalAxisSizing::FillUpTo { .. } => Some(260.0),
        };
        self.width = ModalAxisSizing::FitContent {
            min,
            max: Some(max_width),
        };
        self
    }

    fn build_content(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let header = flex_col((
            ui_label(TypeRole::HeadlineSmall, self.heading.clone()).color(p.text_primary),
            ui_label(TypeRole::BodyMedium, self.body.clone()).color(p.text_secondary),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px())
        .padding(Padding {
            top: 18.px(),
            right: 16.px(),
            bottom: 14.px(),
            left: 16.px(),
        });

        let cancel_cb = self.on_dismiss.clone();
        let cancel_btn =
            secondary_button(self.cancel_text.clone(), move |s: &mut State| cancel_cb(s));
        let confirm_cb = self.on_confirm.clone();
        let confirm_btn: Box<AnyWidgetView<State, Action>> = if self.destructive {
            destructive_button(self.confirm_text.clone(), move |s: &mut State| {
                confirm_cb(s)
            })
            .boxed()
        } else {
            primary_button(self.confirm_text.clone(), move |s: &mut State| {
                confirm_cb(s)
            })
            .boxed()
        };

        let responses = AdwResponseRowView {
            cancel: cancel_btn,
            confirm: confirm_btn,
            phantom: PhantomData,
        }
        .padding(Padding {
            top: 0.px(),
            right: 16.px(),
            bottom: 16.px(),
            left: 16.px(),
        });

        flex_col((header, responses))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .boxed()
    }
}

impl<State: 'static, Action: 'static> ViewMarker for AdwAlertDialog<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for AdwAlertDialog<State, Action>
{
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
        let spec = current_spec_arc(ctx);
        let content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            width: self.width,
            height: ModalAxisSizing::FitContent {
                min: None,
                max: None,
            },
            viewport_margin: None,
            padding: Padding::ZERO,
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 16.0),
            duration: MotionDuration::Medium2,
            dismiss_on_scrim: true,
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
        let spec = current_spec_arc(ctx);
        let next_content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            width: self.width,
            height: ModalAxisSizing::FitContent {
                min: None,
                max: None,
            },
            viewport_margin: None,
            padding: Padding::ZERO,
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 16.0),
            duration: MotionDuration::Medium2,
            dismiss_on_scrim: true,
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

/// Creates a GNOME `AdwAlertDialog` with centered heading/body and split equal-width response buttons.
pub fn alert_dialog<State: 'static, Action: 'static, F1, F2>(
    heading: impl Into<ArcStr>,
    body: impl Into<ArcStr>,
    cancel_text: impl Into<ArcStr>,
    confirm_text: impl Into<ArcStr>,
    on_confirm: F1,
    on_dismiss: F2,
) -> AdwAlertDialog<State, Action>
where
    F1: Fn(&mut State) -> Action + Send + Sync + 'static,
    F2: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    AdwAlertDialog {
        heading: heading.into(),
        body: body.into(),
        confirm_text: confirm_text.into(),
        cancel_text: cancel_text.into(),
        destructive: false,
        width: ModalAxisSizing::FitContent {
            min: Some(260.0),
            max: Some(420.0),
        },
        on_confirm: Arc::new(on_confirm),
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

impl<C, K, State, Action> brocado_core::foundation::BoundedX
    for AdwResponseRowView<C, K, State, Action>
where
    C: brocado_core::foundation::BoundedX,
    K: brocado_core::foundation::BoundedX,
{
}

impl<C, K, State, Action> brocado_core::foundation::BoundedY
    for AdwResponseRowView<C, K, State, Action>
where
    C: brocado_core::foundation::BoundedY,
    K: brocado_core::foundation::BoundedY,
{
}

impl<State, Action> brocado_core::foundation::BoundedX for AdwAlertDialog<State, Action> {}
impl<State, Action> brocado_core::foundation::BoundedY for AdwAlertDialog<State, Action> {}
