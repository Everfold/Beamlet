//! WinUI 3 signature components: `info_bar`, `settings_card`, `settings_expander`, `command_bar`, `command_bar_with_overflow`, `navigation_view`, `navigation_view_collapsible`, `number_box`.

use std::marker::PhantomData;
use std::sync::Arc;

use brocado_core::components::divider::{horizontal_divider, vertical_divider};
use brocado_core::components::typography::ui_label;
use brocado_core::components::{MenuEntry, NavDestination, dropdown_menu, icon_button};
use brocado_core::foundation::state_layer::blend_over;
use brocado_core::foundation::{UiClassExt, adaptive_pair, beveled_surface, icons, ui_icon, wrap};
use brocado_core::style_engine::{classes, with_spec};
use brocado_core::tokens::{Brightness, DesignSpec, TypeRole};
use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, ArcStr, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget, PaintCtx,
    PointerEvent, PointerScrollEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Widget,
    WidgetMut, WidgetPod,
};
use masonry::dpi::PhysicalPosition;
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, Size};
use masonry::layout::{AsUnit, LenReq, Length};
use masonry::parley::style::FontWeight;
use masonry::peniko::Color;
use masonry::properties::{Dimensions, Padding};
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker as _};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, button, flex_col, flex_row, label, sized_box,
};

/// WinUI 3 `InfoBar` semantic severity presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InfoBarSeverity {
    /// Neutral informational banner.
    #[default]
    Informational,
    /// Positive completion/status banner.
    Success,
    /// Non-blocking caution banner.
    Warning,
    /// Critical/error banner.
    Error,
}

impl InfoBarSeverity {
    fn visual(self, spec: &DesignSpec) -> (&'static str, Color, Color) {
        let p = &spec.palette;
        let is_dark = spec.brightness == Brightness::Dark;
        match self {
            Self::Informational => (icons::INFO, p.accent, p.surface),
            Self::Success => {
                let bg = blend_over(p.surface, p.success, if is_dark { 0.14 } else { 0.10 });
                (icons::CHECK_CIRCLE, p.success, bg)
            }
            Self::Warning => {
                let bg = blend_over(p.surface, p.warning, if is_dark { 0.14 } else { 0.10 });
                (icons::WARNING, p.warning, bg)
            }
            Self::Error => {
                let bg = blend_over(p.surface, p.error, if is_dark { 0.16 } else { 0.10 });
                (icons::ERROR_CIRCLE, p.error, bg)
            }
        }
    }

    const fn badge_colors(self, spec: &DesignSpec) -> (Color, Color) {
        let p = &spec.palette;
        match self {
            Self::Informational => (p.accent, p.on_accent),
            Self::Success => (p.success, p.on_success),
            Self::Warning => (p.warning, p.on_warning),
            Self::Error => (p.error, p.on_error),
        }
    }
}

/// Close button callback for [`info_bar`].
pub type InfoBarCloseCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// WinUI 3 inline status banner (`InfoBar`) with severity badge, title, message, and optional dismiss button.
pub fn info_bar<State: 'static, Action: 'static>(
    severity: InfoBarSeverity,
    title: impl Into<ArcStr>,
    message: impl Into<ArcStr>,
    on_close: Option<InfoBarCloseCallback<State, Action>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let title: ArcStr = title.into();
    let message: ArcStr = message.into();
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let (icon_path, badge_color, bg) = severity.visual(spec);

        let title_lbl = ui_label(TypeRole::LabelLarge, title.clone()).color(p.text_primary);
        let msg_lbl = ui_label(TypeRole::BodyMedium, message.clone()).color(p.text_secondary);

        let close_btn = on_close.as_ref().map(|cb| {
            let cb = cb.clone();
            button(ui_icon(icons::CLOSE, 14.0, p.text_secondary), move |s| {
                cb(s)
            })
            .ui_class(classes::ICON_BTN_STD)
        });

        let icon_and_text = flex_row((
            ui_icon(icon_path, 18.0, badge_color),
            flex_col((title_lbl, msg_lbl))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px());

        let inner: Box<AnyWidgetView<State, Action>> = if let Some(close_btn) = close_btn {
            adaptive_pair(icon_and_text, close_btn)
                .gap(10.px())
                .run_gap(6.px())
                .min_leading(56.0)
                .padding(Padding::from_vh(10.px(), 12.px()))
                .boxed()
        } else {
            icon_and_text
                .padding(Padding::from_vh(10.px(), 12.px()))
                .boxed()
        };

        sized_box(inner)
            .background_color(bg)
            .border(p.border, 1.px())
            .corner_radius(spec.metrics.card_radius.px())
    })
}

/// WinUI 3 `SettingsCard` row with leading icon, title, subtitle, and trailing interactive control.
pub fn settings_card<State: 'static, Action: 'static, C>(
    icon_svg: Option<&'static str>,
    title: impl Into<ArcStr>,
    description: Option<impl Into<ArcStr>>,
    trailing_control: C,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    C: WidgetView<State, Action>,
{
    let title: ArcStr = title.into();
    let description: Option<ArcStr> = description.map(Into::into);
    let title_view = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let ic = icon_svg.map(|path| ui_icon(path, 20.0, p.text_primary));
        let t = ui_label(TypeRole::BodyLarge, title.clone()).color(p.text_primary);
        let d = description
            .as_ref()
            .map(|sub| ui_label(TypeRole::BodySmall, sub.clone()).color(p.text_secondary));
        flex_row((
            ic,
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
    });

    sized_box(beveled_surface(
        adaptive_pair(title_view, trailing_control)
            .gap(12.px())
            .run_gap(8.px())
            .min_leading(56.0)
            .padding(Padding::from_vh(12.px(), 12.px())),
    ))
    .ui_class(classes::GROUPED_CARD)
}

/// WinUI 3 `SettingsExpander` revealing nested child settings rows when `expanded` is `true`.
pub fn settings_expander<State: 'static, Action: 'static, H, F>(
    icon_svg: Option<&'static str>,
    title: impl Into<ArcStr>,
    description: Option<impl Into<ArcStr>>,
    expanded: bool,
    on_toggle: F,
    header_control: Option<H>,
    items: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    H: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let title: ArcStr = title.into();
    let description: Option<ArcStr> = description.map(Into::into);
    let cb = Arc::new(on_toggle);

    let header_left = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let ic = icon_svg.map(|path| ui_icon(path, 20.0, p.text_primary));
        let t = ui_label(TypeRole::BodyLarge, title.clone()).color(p.text_primary);
        let d = description
            .as_ref()
            .map(|sub| ui_label(TypeRole::BodySmall, sub.clone()).color(p.text_secondary));
        flex_row((
            ic,
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
    });

    let toggle_btn = with_spec(move |spec: &DesignSpec, _| {
        let cb = cb.clone();
        let chevron = if expanded {
            icons::ARROW_DROP_UP
        } else {
            icons::ARROW_DROP_DOWN
        };
        button(
            ui_icon(chevron, 18.0, spec.palette.text_secondary),
            move |s| cb(s),
        )
        .ui_class(classes::ICON_BTN_STD)
    });

    let header_row = adaptive_pair(
        header_left,
        flex_row((header_control, toggle_btn))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(6.px()),
    )
    .gap(12.px())
    .run_gap(8.px())
    .min_leading(56.0)
    .padding(Padding::from_vh(12.px(), 12.px()));

    let expanded_section = expanded.then(|| {
        let mut children: Vec<Box<AnyWidgetView<State, Action>>> =
            Vec::with_capacity(items.len() * 2);
        for item in items {
            children.push(horizontal_divider().boxed());
            children.push(
                sized_box(item)
                    .padding(Padding::from_vh(10.px(), 12.px()))
                    .boxed(),
            );
        }
        flex_col(children).cross_axis_alignment(CrossAxisAlignment::Stretch)
    });

    sized_box(beveled_surface(
        flex_col((header_row, expanded_section)).cross_axis_alignment(CrossAxisAlignment::Stretch),
    ))
    .ui_class(classes::GROUPED_CARD)
}

/// WinUI 3 horizontal `CommandBar` grouping primary and trailing command actions.
pub fn command_bar<State: 'static, Action: 'static>(
    primary_commands: Vec<Box<AnyWidgetView<State, Action>>>,
    secondary_commands: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let primary = wrap(primary_commands)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(4.px())
        .run_gap(4.px());
    let inner: Box<AnyWidgetView<State, Action>> = if secondary_commands.is_empty() {
        primary.padding(Padding::from_vh(4.px(), 8.px())).boxed()
    } else {
        let secondary = wrap(secondary_commands)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(4.px());
        adaptive_pair(primary, secondary)
            .gap(8.px())
            .run_gap(4.px())
            .min_leading(56.0)
            .padding(Padding::from_vh(4.px(), 8.px()))
            .boxed()
    };
    sized_box(beveled_surface(inner)).ui_class(classes::GROUPED_CARD)
}

/// WinUI 3 horizontal `CommandBar` with primary actions and a trailing `...` anchored overflow menu.
pub fn command_bar_with_overflow<State, Action, OnToggle, OnSelect, OnDismiss>(
    primary_commands: Vec<Box<AnyWidgetView<State, Action>>>,
    overflow_entries: Vec<MenuEntry>,
    overflow_open: bool,
    on_toggle_overflow: OnToggle,
    on_select_overflow: OnSelect,
    on_dismiss_overflow: OnDismiss,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    State: 'static,
    Action: 'static,
    OnToggle: Fn(&mut State) -> Action + Send + Sync + 'static,
    OnSelect: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let overflow_btn = (!overflow_entries.is_empty()).then(|| {
        dropdown_menu(
            icon_button(icons::MORE_HORIZ, on_toggle_overflow),
            overflow_open,
            overflow_entries,
            on_select_overflow,
            on_dismiss_overflow,
        )
    });

    sized_box(beveled_surface(
        flex_row((
            wrap(primary_commands)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .run_gap(4.px())
                .flex(1.0),
            overflow_btn,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(4.px())
        .padding(Padding::from_vh(4.px(), 8.px())),
    ))
    .ui_class(classes::GROUPED_CARD)
}

type NavSelectCallback<State, Action> = Arc<dyn Fn(&mut State, usize) -> Action + Send + Sync>;

fn nav_view_item<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    dest: &NavDestination,
    active: bool,
    index: usize,
    cb: NavSelectCallback<State, Action>,
) -> Box<AnyWidgetView<State, Action>> {
    let p = &spec.palette;
    let bar_color = if active { p.accent } else { Color::TRANSPARENT };
    let icon_color = if active { p.accent } else { p.text_secondary };
    let text_color = if active {
        p.text_primary
    } else {
        p.text_secondary
    };
    let bar = sized_box(label(""))
        .dims(Dimensions::fixed(3.px(), 16.px()))
        .background_color(bar_color)
        .corner_radius(2.px());
    let lbl = spec
        .typography
        .style_label(TypeRole::BodyMedium, label(dest.label.clone()))
        .color(text_color);
    let badge = dest.badge_text.as_ref().map(|b| {
        spec.typography
            .style_label(TypeRole::LabelSmall, label(b.clone()))
            .color(p.on_error)
            .background_color(p.error)
            .padding(Padding::from_vh(2.px(), 6.px()))
            .corner_radius(999.px())
    });
    let cls = if active {
        classes::NAV_PILL_ACTIVE
    } else {
        classes::NAV_PILL
    };
    button(
        flex_row((
            bar,
            ui_icon(dest.icon, 16.0, icon_color),
            lbl.flex(1.0),
            badge,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px()),
        move |s: &mut State| cb(s, index),
    )
    .ui_class(cls)
    .boxed()
}

fn nav_view_compact_item<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    dest: &NavDestination,
    active: bool,
    index: usize,
    cb: NavSelectCallback<State, Action>,
) -> Box<AnyWidgetView<State, Action>> {
    let p = &spec.palette;
    let bar_color = if active { p.accent } else { Color::TRANSPARENT };
    let icon_color = if active { p.accent } else { p.text_secondary };
    let bar = sized_box(label(""))
        .dims(Dimensions::fixed(3.px(), 16.px()))
        .background_color(bar_color)
        .corner_radius(2.px());
    let cls = if active {
        classes::NAV_PILL_ACTIVE
    } else {
        classes::NAV_PILL
    };
    button(
        flex_row((bar, ui_icon(dest.icon, 16.0, icon_color)))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(6.px()),
        move |s: &mut State| cb(s, index),
    )
    .ui_class(cls)
    .dims(Dimensions::height(36.px()))
    .padding(Padding::from_vh(6.px(), 4.px()))
    .boxed()
}

/// WinUI 3 `NavigationView` left navigation pane featuring `3px` active accent bars and pinned bottom footer items.
///
/// Primary items use selection indices `0..primary_items.len()`, and footer items use indices
/// `primary_items.len()..(primary_items.len() + footer_items.len())`.
pub fn navigation_view<State: 'static, Action: 'static, F>(
    header: Option<&str>,
    primary_items: &[NavDestination],
    footer_items: &[NavDestination],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let header = header.map(ArcStr::from);
    let primary: Vec<NavDestination> = primary_items.to_vec();
    let footer: Vec<NavDestination> = footer_items.to_vec();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let head = header.as_ref().map(|h| {
            spec.typography
                .style_label(TypeRole::TitleSmall, label(h.clone()))
                .color(p.text_secondary)
                .padding(Padding::from_vh(6.px(), 10.px()))
        });
        let primary_views: Vec<Box<AnyWidgetView<State, Action>>> = primary
            .iter()
            .enumerate()
            .map(|(i, d)| nav_view_item(spec, d, i == selected, i, cb.clone()))
            .collect();

        let footer_section = (!footer.is_empty()).then(|| {
            let offset = primary.len();
            let footer_views: Vec<Box<AnyWidgetView<State, Action>>> = footer
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let idx = offset + i;
                    nav_view_item(spec, d, idx == selected, idx, cb.clone())
                })
                .collect();
            flex_col((
                horizontal_divider(),
                flex_col(footer_views)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(4.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(6.px())
        });

        flex_row((
            sized_box(
                flex_col((
                    head,
                    flex_col(primary_views)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px())
                        .flex(1.0),
                    footer_section,
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px())
                .padding(Padding::all(10.px())),
            )
            .dims(Dimensions::STRETCH.with_width(spec.metrics.sidebar_width.px()))
            .background_color(p.sidebar_bg),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
    })
}

/// Collapsible WinUI 3 `NavigationView` switching between `280px` Expanded and `52px` Compact icon-only rail.
pub fn navigation_view_collapsible<State: 'static, Action: 'static, ToggleF, SelectF>(
    header: Option<&str>,
    primary_items: &[NavDestination],
    footer_items: &[NavDestination],
    selected: usize,
    collapsed: bool,
    on_toggle_collapse: ToggleF,
    on_select: SelectF,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, ToggleF, SelectF>
where
    ToggleF: Fn(&mut State) -> Action + Send + Sync + 'static,
    SelectF: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let header = header.map(ArcStr::from);
    let primary: Vec<NavDestination> = primary_items.to_vec();
    let footer: Vec<NavDestination> = footer_items.to_vec();
    let toggle_cb = Arc::new(on_toggle_collapse);
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let toggle_cb = toggle_cb.clone();
        let hamburger = icon_button(icons::MENU, move |s: &mut State| toggle_cb(s))
            .dims(Dimensions::fixed(32.px(), 32.px()));

        let top_row: Box<AnyWidgetView<State, Action>> = if collapsed {
            hamburger.boxed()
        } else {
            let head_lbl = header.as_ref().map(|h| {
                spec.typography
                    .style_label(TypeRole::TitleSmall, label(h.clone()))
                    .color(p.text_secondary)
            });
            flex_row((hamburger, head_lbl.map(|h| h.flex(1.0))))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
                .boxed()
        };

        let make_item = |d: &NavDestination, idx: usize| {
            if collapsed {
                nav_view_compact_item(spec, d, idx == selected, idx, cb.clone())
            } else {
                nav_view_item(spec, d, idx == selected, idx, cb.clone())
            }
        };

        let primary_views: Vec<Box<AnyWidgetView<State, Action>>> = primary
            .iter()
            .enumerate()
            .map(|(i, d)| make_item(d, i))
            .collect();

        let footer_section = (!footer.is_empty()).then(|| {
            let offset = primary.len();
            let footer_views: Vec<Box<AnyWidgetView<State, Action>>> = footer
                .iter()
                .enumerate()
                .map(|(i, d)| make_item(d, offset + i))
                .collect();
            flex_col((
                horizontal_divider(),
                flex_col(footer_views)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(4.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(6.px())
        });

        let pane_width = if collapsed {
            52.0
        } else {
            spec.metrics.sidebar_width
        };
        let pane_pad = if collapsed {
            Padding::from_vh(8.px(), 6.px())
        } else {
            Padding::all(10.px())
        };

        flex_row((
            sized_box(
                flex_col((
                    top_row,
                    flex_col(primary_views)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px())
                        .flex(1.0),
                    footer_section,
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px())
                .padding(pane_pad),
            )
            .dims(Dimensions::STRETCH.with_width(pane_width.px()))
            .background_color(p.sidebar_bg),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
    })
}

/// WinUI 3 `InfoBadge` semantic notification pill (`Some("3")`) or `10×10` dot (`None`).
pub fn info_badge<State: 'static, Action: 'static>(
    severity: InfoBarSeverity,
    text: Option<impl Into<ArcStr>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let text: Option<ArcStr> = text.map(Into::into);
    with_spec(move |spec: &DesignSpec, _| {
        let (fill, ink) = severity.badge_colors(spec);
        let (inner, dims, pad): (Box<AnyWidgetView<State, Action>>, Dimensions, Padding) =
            match &text {
                Some(txt) => (
                    spec.typography
                        .style_label(TypeRole::LabelSmall, label(txt.clone()))
                        .weight(FontWeight::BOLD)
                        .color(ink)
                        .boxed(),
                    Dimensions::new(
                        masonry::layout::Dim::MaxContent,
                        masonry::layout::Dim::MaxContent,
                    ),
                    Padding::from_vh(2.px(), 7.px()),
                ),
                None => (
                    label("").boxed(),
                    Dimensions::fixed(10.px(), 10.px()),
                    Padding::ZERO,
                ),
            };
        sized_box(inner)
            .dims(dims)
            .padding(pad)
            .background_color(fill)
            .corner_radius(999.px())
    })
}

/// WinUI 3 `BreadcrumbBar` rendering clickable ancestor segments separated by chevrons and a bold current leaf.
pub fn breadcrumb_bar<State: 'static, Action: 'static, F>(
    segments: &[&str],
    on_select: F,
) -> impl WidgetView<State, Action, Widget = brocado_core::foundation::WrapWidget> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let items: Vec<ArcStr> = segments.iter().map(|s| ArcStr::from(*s)).collect();
    let cb = Arc::new(on_select);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let last_idx = items.len().saturating_sub(1);
        let mut children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
        for (idx, seg) in items.iter().enumerate() {
            if idx > 0 {
                children.push(ui_icon(icons::CHEVRON_RIGHT, 14.0, p.text_secondary).boxed());
            }
            if idx == last_idx {
                children.push(
                    ui_label(TypeRole::BodyMedium, seg.clone())
                        .weight(FontWeight::SEMI_BOLD)
                        .color(p.text_primary)
                        .padding(Padding::from_vh(4.px(), 6.px()))
                        .boxed(),
                );
            } else {
                let cb = cb.clone();
                let lbl = ui_label(TypeRole::BodyMedium, seg.clone()).color(p.text_secondary);
                children.push(
                    button(lbl, move |s: &mut State| cb(s, idx))
                        .ui_class(classes::BTN_SUBTLE)
                        .boxed(),
                );
            }
        }
        wrap(children)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(4.px())
    })
}

/// General-purpose WinUI 3 `Expander` revealing arbitrary custom `content` when `expanded` is `true`.
pub fn expander<State: 'static, Action: 'static, C, F>(
    title: impl Into<ArcStr>,
    subtitle: Option<impl Into<ArcStr>>,
    expanded: bool,
    on_toggle: F,
    content: C,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    C: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let title: ArcStr = title.into();
    let subtitle: Option<ArcStr> = subtitle.map(Into::into);
    let cb = Arc::new(on_toggle);

    let header_left = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let t = ui_label(TypeRole::BodyLarge, title.clone()).color(p.text_primary);
        let sub = subtitle
            .as_ref()
            .map(|s| ui_label(TypeRole::BodySmall, s.clone()).color(p.text_secondary));
        flex_col((t, sub))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(2.px())
    });

    let toggle_btn = with_spec(move |spec: &DesignSpec, _| {
        let cb = cb.clone();
        let chevron = if expanded {
            icons::ARROW_DROP_UP
        } else {
            icons::ARROW_DROP_DOWN
        };
        button(
            ui_icon(chevron, 18.0, spec.palette.text_secondary),
            move |s| cb(s),
        )
        .ui_class(classes::ICON_BTN_STD)
    });

    let header_row = adaptive_pair(header_left, toggle_btn)
        .gap(12.px())
        .run_gap(8.px())
        .min_leading(56.0)
        .padding(Padding::from_vh(12.px(), 12.px()));

    let expanded_body = expanded.then(|| {
        flex_col((
            horizontal_divider(),
            sized_box(content).padding(Padding::from_vh(12.px(), 12.px())),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
    });

    sized_box(beveled_surface(
        flex_col((header_row, expanded_body)).cross_axis_alignment(CrossAxisAlignment::Stretch),
    ))
    .ui_class(classes::GROUPED_CARD)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NumberStepDelta(i32);

const NUMBER_STEP_CHILD_ID: ViewId = ViewId::new(0x5549_3630);

struct NumberStepCatcher {
    child: WidgetPod<dyn Widget>,
}

impl NumberStepCatcher {
    fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            child: child.erased().to_pod(),
        }
    }

    fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for NumberStepCatcher {
    type Action = NumberStepDelta;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(_) if !ctx.has_focus_target() => {
                ctx.request_focus();
            }
            PointerEvent::Scroll(PointerScrollEvent { delta, .. }) => {
                let scale = ctx.scale_factor();
                let line_px = PhysicalPosition {
                    x: 40.0 * scale,
                    y: 40.0 * scale,
                };
                let page_px = PhysicalPosition {
                    x: 160.0 * scale,
                    y: 160.0 * scale,
                };
                let px = delta.to_pixel_delta(line_px, page_px);
                if px.y.abs() > 0.1 {
                    let step_dir = if px.y > 0.0 { 1 } else { -1 };
                    ctx.submit_action::<Self::Action>(NumberStepDelta(step_dir));
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(key_ev) = event
            && !key_ev.state.is_up()
        {
            match key_ev.key {
                Key::Named(NamedKey::ArrowUp) => {
                    ctx.submit_action::<Self::Action>(NumberStepDelta(1));
                    ctx.set_handled();
                }
                Key::Named(NamedKey::ArrowDown) => {
                    ctx.submit_action::<Self::Action>(NumberStepDelta(-1));
                    ctx.set_handled();
                }
                _ => {}
            }
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
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
        ctx.derive_baselines(&self.child);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::SpinButton
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

type StepCallback<State, Action> = Arc<dyn Fn(&mut State, i32) -> Action + Send + Sync>;

struct NumberStepCatcherView<V, State, Action> {
    child: V,
    on_step: StepCallback<State, Action>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<V, State, Action> ViewMarker for NumberStepCatcherView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for NumberStepCatcherView<V, State, Action>
where
    V: WidgetView<State, Action>,
    State: 'static,
    Action: 'static,
{
    type Element = Pod<NumberStepCatcher>;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let (child_pod, child_state) =
            ctx.with_id(NUMBER_STEP_CHILD_ID, |ctx| self.child.build(ctx, app_state));
        let pod = ctx
            .with_action_widget(|ctx| ctx.create_pod(NumberStepCatcher::new(child_pod.new_widget)));
        (pod, child_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let mut child_dyn = NumberStepCatcher::child_mut(&mut element);
        ctx.with_id(NUMBER_STEP_CHILD_ID, |ctx| {
            self.child.rebuild(
                &prev.child,
                view_state,
                ctx,
                child_dyn.downcast(),
                app_state,
            );
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child_dyn = NumberStepCatcher::child_mut(&mut element);
            ctx.with_id(NUMBER_STEP_CHILD_ID, |ctx| {
                self.child.teardown(view_state, ctx, child_dyn.downcast());
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
            Some(NUMBER_STEP_CHILD_ID) => {
                let mut child_dyn = NumberStepCatcher::child_mut(&mut element);
                self.child
                    .message(view_state, message, child_dyn.downcast(), app_state)
            }
            None => match message.take_message::<NumberStepDelta>() {
                Some(delta) => MessageResult::Action((self.on_step)(app_state, delta.0)),
                None => MessageResult::Stale,
            },
            Some(_) => MessageResult::Stale,
        }
    }
}

/// WinUI 3 `NumberBox` compact numeric spin control with inline `−` / `+` buttons, `ArrowUp`/`ArrowDown`
/// keyboard stepping, and mouse-wheel stepping clamped to `[min, max]`.
pub fn number_box<State: 'static, Action: 'static, F>(
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    on_change: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox>
where
    F: Fn(&mut State, f64) -> Action + Send + Sync + 'static,
{
    let step = step.abs().max(f64::EPSILON);
    let clamped = value.clamp(min, max);
    let can_dec = clamped > min + f64::EPSILON;
    let can_inc = clamped + f64::EPSILON < max;
    let cb = Arc::new(on_change);

    let formatted = if step.fract() == 0.0 && clamped.fract() == 0.0 {
        format!("{clamped:.0}")
    } else {
        format!("{clamped:.2}")
    };

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let dec_cb = cb.clone();
        let inc_cb = cb.clone();
        let key_cb = cb.clone();
        let next_dec = (clamped - step).clamp(min, max);
        let next_inc = (clamped + step).clamp(min, max);

        let dec_btn = button(
            ui_icon(
                icons::REMOVE,
                16.0,
                spec.content_color(p.text_primary, !can_dec),
            ),
            move |s: &mut State| dec_cb(s, next_dec),
        )
        .disabled(!can_dec)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(28.px(), 28.px()))
        .padding(Padding::all(0.px()));

        let inc_btn = button(
            ui_icon(
                icons::ADD,
                16.0,
                spec.content_color(p.text_primary, !can_inc),
            ),
            move |s: &mut State| inc_cb(s, next_inc),
        )
        .disabled(!can_inc)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(28.px(), 28.px()))
        .padding(Padding::all(0.px()));

        let readout = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(formatted.clone()))
            .color(p.text_primary)
            .padding(Padding::from_vh(0.px(), 8.px()));

        let catcher = NumberStepCatcherView {
            child: flex_row((dec_btn, readout, inc_btn))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(2.px())
                .padding(Padding::from_vh(2.px(), 4.px())),
            on_step: Arc::new(move |s: &mut State, dir| {
                let next = (clamped + f64::from(dir) * step).clamp(min, max);
                key_cb(s, next)
            }),
            phantom: PhantomData,
        };

        sized_box(beveled_surface(catcher).radius(spec.metrics.input_radius))
            .background_color(p.surface)
            .border(p.border_strong, 1.px())
            .corner_radius(spec.metrics.input_radius.px())
    })
}
