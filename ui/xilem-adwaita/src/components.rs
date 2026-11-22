//! GNOME Libadwaita signature components: `header_bar`, `header_bar_with_title`, `preferences_group`, `action_row`, `clickable_action_row`, `switch_row`, `status_page`, `banner`.

use std::sync::Arc;

use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, label, sized_box,
    text_input,
};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui_core::components::button::{primary_button, subtle_button};
use xilem_ui_core::components::divider::horizontal_divider;
use xilem_ui_core::components::menu::select;
use xilem_ui_core::components::selection::switch;
use xilem_ui_core::foundation::{FocusFrame, UiClassExt, focus_frame, icons, ui_icon};
use xilem_ui_core::style_engine::{classes, with_spec};
use xilem_ui_core::tokens::{DesignSpec, TypeRole};

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
            flex_row((
                flex_row(start_actions)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                title_view.flex(1.0),
                flex_row(end_actions)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px())
            .padding(Padding::from_vh(6.px(), 12.px())),
        )
        .dims(Dimensions::height(46.px()))
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
        let t = spec
            .typography
            .style_label(TypeRole::TitleSmall, label(title.clone()))
            .color(p.text_primary);
        let sub = subtitle.as_ref().map(|s| {
            spec.typography
                .style_label(TypeRole::BodySmall, label(s.clone()))
                .color(p.text_secondary)
        });
        flex_col((t, sub)).cross_axis_alignment(CrossAxisAlignment::Center)
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
            let t = title_owned.as_ref().map(|s| {
                spec.typography
                    .style_label(TypeRole::TitleMedium, label(s.clone()))
                    .color(p.text_primary)
            });
            let d = desc_owned.as_ref().map(|s| {
                spec.typography
                    .style_label(TypeRole::BodySmall, label(s.clone()))
                    .color(p.text_secondary)
            });
            flex_col((t, d))
                .cross_axis_alignment(CrossAxisAlignment::Start)
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
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    S: WidgetView<State, Action>,
{
    let leading_and_text = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let ic = prefix_icon.map(|path| ui_icon(path, 18.0, p.text_secondary));
        let t = spec
            .typography
            .style_label(TypeRole::BodyLarge, label(title.clone()))
            .color(p.text_primary);
        let sub = subtitle.as_ref().map(|s| {
            spec.typography
                .style_label(TypeRole::BodySmall, label(s.clone()))
                .color(p.text_secondary)
        });
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

    flex_row((leading_and_text.flex(1.0), suffix))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
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
    button(row_content, on_activate).ui_class(classes::LIST_ITEM)
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
            spec.typography
                .style_label(TypeRole::HeadlineMedium, label(title.clone()))
                .color(p.text_primary),
            spec.typography
                .style_label(TypeRole::BodyMedium, label(description.clone()))
                .color(p.text_secondary),
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
        .padding(Padding::all(24.px()))
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
        let lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, label(title.clone()))
            .color(p.on_accent_container)
            .flex(1.0);
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

        sized_box(
            flex_row((lbl, act, close))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px())
                .padding(Padding::from_vh(8.px(), 14.px())),
        )
        .background_color(p.accent_container)
    })
}

/// GNOME `AdwViewSwitcher` header-bar pill switcher displaying horizontal icon + title views.
pub fn view_switcher<State: 'static, Action: 'static, F>(
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
                let lbl = spec
                    .typography
                    .style_label(TypeRole::LabelMedium, label(name.clone()))
                    .color(fg);
                button(
                    flex_row((ui_icon(icon_path, 16.0, fg), lbl))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(6.px()),
                    move |s: &mut State| cb(s, idx),
                )
                .ui_class(cls)
                .boxed()
            })
            .collect();

        flex_row(views)
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
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
        let top = spec
            .typography
            .style_label(TypeRole::LabelSmall, label(title.clone()))
            .color(p.text_secondary);
        let style = spec.typography.get(TypeRole::BodyLarge);
        let mut ti = text_input(value.clone(), move |s: &mut State, txt| cb(s, txt))
            .text_size(style.font_size)
            .weight(style.weight)
            .text_color(p.text_primary);
        if let Some(font) = spec.typography.font_family() {
            ti = ti.font(font.clone());
        }
        let ti = ti.ui_class(classes::TEXT_FIELD_FLUSH);

        sized_box(
            flex_col((top, ti))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(2.px())
                .padding(Padding::from_vh(8.px(), 14.px())),
        )
    });

    focus_frame(inner)
}
