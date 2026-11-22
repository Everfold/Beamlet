//! WinUI 3 signature components: `info_bar`, `settings_card`, `settings_expander`, `command_bar`, `navigation_view`.

use std::sync::Arc;

use xilem::Color;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, button, flex_col, flex_row, label, sized_box};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui_core::components::NavDestination;
use xilem_ui_core::components::divider::{horizontal_divider, vertical_divider};
use xilem_ui_core::foundation::state_layer::blend_over;
use xilem_ui_core::foundation::{UiClassExt, icons, ui_icon};
use xilem_ui_core::style_engine::{classes, with_spec};
use xilem_ui_core::tokens::{Brightness, DesignSpec, TypeRole};

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
                let badge = if is_dark {
                    Color::from_rgb8(0x6C, 0xCB, 0x5F)
                } else {
                    Color::from_rgb8(0x0F, 0x7B, 0x0F)
                };
                let bg = blend_over(p.surface, badge, if is_dark { 0.14 } else { 0.10 });
                (icons::CHECK_CIRCLE, badge, bg)
            }
            Self::Warning => {
                let badge = if is_dark {
                    Color::from_rgb8(0xFC, 0xE1, 0x00)
                } else {
                    Color::from_rgb8(0x9D, 0x5D, 0x00)
                };
                let bg = blend_over(p.surface, badge, if is_dark { 0.14 } else { 0.10 });
                (icons::WARNING, badge, bg)
            }
            Self::Error => {
                let bg = blend_over(p.surface, p.error, if is_dark { 0.16 } else { 0.10 });
                (icons::ERROR_CIRCLE, p.error, bg)
            }
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

        let title_lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, label(title.clone()))
            .color(p.text_primary);
        let msg_lbl = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(message.clone()))
            .color(p.text_secondary);

        let close_btn = on_close.as_ref().map(|cb| {
            let cb = cb.clone();
            button(ui_icon(icons::CLOSE, 14.0, p.text_secondary), move |s| {
                cb(s)
            })
            .ui_class(classes::ICON_BTN_STD)
        });

        sized_box(
            flex_row((
                ui_icon(icon_path, 18.0, badge_color),
                flex_col((title_lbl, msg_lbl))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(2.px())
                    .flex(1.0),
                close_btn,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px())
            .padding(Padding::from_vh(10.px(), 14.px())),
        )
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
        let t = spec
            .typography
            .style_label(TypeRole::BodyLarge, label(title.clone()))
            .color(p.text_primary);
        let d = description.as_ref().map(|sub| {
            spec.typography
                .style_label(TypeRole::BodySmall, label(sub.clone()))
                .color(p.text_secondary)
        });
        flex_row((
            ic,
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(14.px())
    });

    sized_box(
        flex_row((title_view.flex(1.0), trailing_control))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px())
            .padding(Padding::from_vh(14.px(), 16.px())),
    )
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
        let t = spec
            .typography
            .style_label(TypeRole::BodyLarge, label(title.clone()))
            .color(p.text_primary);
        let d = description.as_ref().map(|sub| {
            spec.typography
                .style_label(TypeRole::BodySmall, label(sub.clone()))
                .color(p.text_secondary)
        });
        flex_row((
            ic,
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(2.px())
                .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(14.px())
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

    let header_row = flex_row((header_left.flex(1.0), header_control, toggle_btn))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
        .padding(Padding::from_vh(14.px(), 16.px()));

    let expanded_section = expanded.then(|| {
        let mut children: Vec<Box<AnyWidgetView<State, Action>>> =
            Vec::with_capacity(items.len() * 2);
        for item in items {
            children.push(horizontal_divider().boxed());
            children.push(
                sized_box(item)
                    .padding(Padding::from_vh(10.px(), 20.px()))
                    .boxed(),
            );
        }
        flex_col(children).cross_axis_alignment(CrossAxisAlignment::Stretch)
    });

    sized_box(
        flex_col((header_row, expanded_section)).cross_axis_alignment(CrossAxisAlignment::Stretch),
    )
    .ui_class(classes::GROUPED_CARD)
}

/// WinUI 3 horizontal `CommandBar` grouping primary and trailing command actions.
pub fn command_bar<State: 'static, Action: 'static>(
    primary_commands: Vec<Box<AnyWidgetView<State, Action>>>,
    secondary_commands: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(
        flex_row((
            flex_row(primary_commands)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .flex(1.0),
            flex_row(secondary_commands)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .padding(Padding::from_vh(4.px(), 8.px())),
    )
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
