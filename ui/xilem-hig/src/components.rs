//! macOS signature components: `macos_toolbar`, `macos_toolbar_with_center`, `source_list_sidebar`, `form_group`, `form_row`, `help_button`.

use std::sync::Arc;

use xilem::Color;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, label, sized_box,
};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui_core::components::divider::{horizontal_divider, vertical_divider};
use xilem_ui_core::foundation::{UiClassExt, icons, ui_icon};
use xilem_ui_core::style_engine::{classes, with_spec};
use xilem_ui_core::tokens::{DesignSpec, TypeRole};

/// Unified macOS compact `38px` window toolbar hosting an arbitrary custom center view (e.g. search bar or segmented switcher).
pub fn macos_toolbar_with_center<State: 'static, Action: 'static, C>(
    leading_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    center: C,
    trailing_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    C: WidgetView<State, Action>,
{
    flex_col((
        sized_box(
            flex_row((
                flex_row(leading_actions)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                center.flex(1.0),
                flex_row(trailing_actions)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px())
            .padding(Padding::from_vh(4.px(), 12.px())),
        )
        .dims(Dimensions::height(38.px()))
        .ui_class(classes::HEADER_BAR),
        horizontal_divider(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// Unified macOS compact `38px` window toolbar with leading actions, title, and trailing actions.
pub fn macos_toolbar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    leading_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    trailing_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> {
    let title: ArcStr = title.into();
    let title_view = with_spec(move |spec: &DesignSpec, _| {
        spec.typography
            .style_label(TypeRole::TitleSmall, label(title.clone()))
            .color(spec.palette.text_primary)
    });
    macos_toolbar_with_center(leading_actions, title_view, trailing_actions)
}

/// Frosted macOS Source List sidebar grouping labeled sections with rounded row selection.
pub fn source_list_sidebar<State: 'static, Action: 'static, F>(
    sections: &[(&str, Vec<(&str, &'static str)>)],
    selected: (usize, usize),
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action, F>
where
    F: Fn(&mut State, (usize, usize)) -> Action + Send + Sync + 'static,
{
    let owned_sections: Vec<(ArcStr, Vec<(ArcStr, &'static str)>)> = sections
        .iter()
        .map(|(title, items)| {
            (
                ArcStr::from(*title),
                items
                    .iter()
                    .map(|(name, icon)| (ArcStr::from(*name), *icon))
                    .collect(),
            )
        })
        .collect();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let mut nodes: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
        for (sec_idx, (sec_title, items)) in owned_sections.iter().enumerate() {
            nodes.push(
                spec.typography
                    .style_label(TypeRole::LabelSmall, label(sec_title.clone()))
                    .color(p.text_secondary)
                    .padding(Padding::from_vh(4.px(), 10.px()))
                    .boxed(),
            );
            for (item_idx, (item_name, icon)) in items.iter().enumerate() {
                let active = selected == (sec_idx, item_idx);
                let cb = cb.clone();
                let fg = if active {
                    p.text_primary
                } else {
                    p.text_secondary
                };
                let icon_c = if active { p.accent } else { fg };
                let lbl = spec
                    .typography
                    .style_label(TypeRole::BodyMedium, label(item_name.clone()))
                    .color(fg);
                let cls = if active {
                    classes::NAV_PILL_ACTIVE
                } else {
                    classes::NAV_PILL
                };
                nodes.push(
                    button(
                        flex_row((ui_icon(icon, 16.0, icon_c), lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(8.px()),
                        move |s: &mut State| cb(s, (sec_idx, item_idx)),
                    )
                    .ui_class(cls)
                    .boxed(),
                );
            }
        }

        flex_row((
            sized_box(
                flex_col(nodes)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(4.px())
                    .padding(Padding::all(10.px())),
            )
            .dims(Dimensions::STRETCH.with_width(spec.metrics.sidebar_width.px()))
            .background_color(p.sidebar_bg),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
    })
}

/// macOS grouped Settings `Form` container separating child [`form_row`] items with inset dividers.
pub fn form_group<State: 'static, Action: 'static>(
    header: Option<&str>,
    rows: Vec<Box<AnyWidgetView<State, Action>>>,
    footer: Option<&str>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action> {
    let header_owned = header.map(ArcStr::from);
    let footer_owned = footer.map(ArcStr::from);

    let head_view = header_owned.map(|h| {
        with_spec(move |spec: &DesignSpec, _| {
            spec.typography
                .style_label(TypeRole::LabelMedium, label(h.clone()))
                .color(spec.palette.text_secondary)
                .padding(Padding::from_vh(2.px(), 6.px()))
        })
    });

    let mut card_children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
    for (idx, row) in rows.into_iter().enumerate() {
        if idx > 0 {
            card_children.push(horizontal_divider().boxed());
        }
        card_children.push(row);
    }

    let card = sized_box(flex_col(card_children).cross_axis_alignment(CrossAxisAlignment::Stretch))
        .ui_class(classes::GROUPED_CARD);

    let foot_view = footer_owned.map(|f| {
        with_spec(move |spec: &DesignSpec, _| {
            spec.typography
                .style_label(TypeRole::BodySmall, label(f.clone()))
                .color(spec.palette.text_secondary)
                .padding(Padding::from_vh(2.px(), 6.px()))
        })
    });

    flex_col((head_view, card, foot_view))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(6.px())
}

/// macOS `Form` row aligning label on the leading edge and `control` on the trailing edge.
pub fn form_row<State: 'static, Action: 'static, C>(
    label_text: impl Into<ArcStr>,
    control: C,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    C: WidgetView<State, Action>,
{
    let text: ArcStr = label_text.into();
    let lbl = with_spec(move |spec: &DesignSpec, _| {
        spec.typography
            .style_label(TypeRole::BodyMedium, label(text.clone()))
            .color(spec.palette.text_primary)
    });

    flex_row((lbl.flex(1.0), control))
        .main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(16.px())
        .padding(Padding::from_vh(10.px(), 14.px()))
}

/// Classic circular macOS `?` help button (`24×24`), returning `Pod<widgets::Button>` directly.
pub fn help_button<State: 'static, Action: 'static, F>(
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let cb = cb.clone();
        button(
            spec.typography
                .style_label(TypeRole::LabelMedium, label("?"))
                .color(p.text_primary),
            move |s: &mut State| cb(s),
        )
        .ui_class(classes::BTN_OUTLINED)
        .corner_radius(999.px())
        .background_color(Color::TRANSPARENT)
        .dims(Dimensions::fixed(24.px(), 24.px()))
        .padding(Padding::ZERO)
    })
}

/// macOS SwiftUI `DisclosureGroup` revealing indented custom `content` under a chevron header.
pub fn disclosure_group<State: 'static, Action: 'static, C, F>(
    title: impl Into<ArcStr>,
    expanded: bool,
    on_toggle: F,
    content: C,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    C: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let title: ArcStr = title.into();
    let cb = Arc::new(on_toggle);
    let header_btn = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let cb = cb.clone();
        let chevron = if expanded {
            icons::ARROW_DROP_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        let lbl = spec
            .typography
            .style_label(TypeRole::TitleSmall, label(title.clone()))
            .color(p.text_primary);
        button(
            flex_row((ui_icon(chevron, 16.0, p.text_secondary), lbl.flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px()),
            move |s: &mut State| cb(s),
        )
        .ui_class(classes::BTN_SUBTLE)
    });

    let body = expanded.then(|| sized_box(content).padding(Padding::left(20.px())));
    flex_col((header_btn, body))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(6.px())
}

/// macOS `NSStepper` / SwiftUI `Stepper` pairing a leading label, numeric readout, and compact `−` / `+` buttons.
pub fn stepper<State: 'static, Action: 'static, F>(
    label_text: impl Into<ArcStr>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: F,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    F: Fn(&mut State, i32) -> Action + Send + Sync + 'static,
{
    let label_text: ArcStr = label_text.into();
    let step = step.abs().max(1);
    let clamped = value.clamp(min, max);
    let can_dec = clamped > min;
    let can_inc = clamped < max;
    let cb = Arc::new(on_change);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let dec_cb = cb.clone();
        let inc_cb = cb.clone();
        let next_dec = clamped.saturating_sub(step).clamp(min, max);
        let next_inc = clamped.saturating_add(step).clamp(min, max);

        let lbl = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(label_text.clone()))
            .color(p.text_primary);
        let readout = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(format!("{clamped}")))
            .color(p.text_secondary);

        let dec_btn = button(
            ui_icon(
                icons::REMOVE,
                14.0,
                spec.content_color(p.text_primary, !can_dec),
            ),
            move |s: &mut State| dec_cb(s, next_dec),
        )
        .disabled(!can_dec)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(24.px(), 24.px()));

        let inc_btn = button(
            ui_icon(
                icons::ADD,
                14.0,
                spec.content_color(p.text_primary, !can_inc),
            ),
            move |s: &mut State| inc_cb(s, next_inc),
        )
        .disabled(!can_inc)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(24.px(), 24.px()));

        let pill = sized_box(
            flex_row((
                dec_btn,
                vertical_divider().dims(Dimensions::height(16.px())),
                inc_btn,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(0.px()),
        )
        .background_color(p.surface)
        .border(p.border_strong, 1.px())
        .corner_radius(spec.metrics.button_radius.px());

        flex_row((lbl.flex(1.0), readout, pill))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px())
    })
}

/// Trailing macOS Inspector pane (`260px`) with leading hairline divider and compact grouped section stack.
pub fn inspector_panel<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    sections: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> {
    let title: ArcStr = title.into();
    let hdr = with_spec(move |spec: &DesignSpec, _| {
        spec.typography
            .style_label(TypeRole::TitleSmall, label(title.clone()))
            .color(spec.palette.text_primary)
    });

    flex_row((
        vertical_divider(),
        sized_box(
            flex_col((
                hdr,
                horizontal_divider(),
                flex_col(sections)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(12.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(10.px()),
        )
        .ui_class(classes::CARD_FILLED)
        .corner_radius(0.px())
        .padding(Padding::all(12.px()))
        .dims(Dimensions::STRETCH.with_width(260.px())),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// macOS Finder-style bottom `NSPathControl` bar with `14px` segment icons and chevron separators.
pub fn path_control<State: 'static, Action: 'static, F>(
    segments: &[(&str, &'static str)],
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let items: Vec<(ArcStr, &'static str)> = segments
        .iter()
        .map(|(name, ic)| (ArcStr::from(*name), *ic))
        .collect();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let last_idx = items.len().saturating_sub(1);
        let mut children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
        for (idx, (name, icon_path)) in items.iter().enumerate() {
            if idx > 0 {
                children.push(ui_icon(icons::CHEVRON_RIGHT, 12.0, p.text_secondary).boxed());
            }
            let cb = cb.clone();
            let fg = if idx == last_idx {
                p.text_primary
            } else {
                p.text_secondary
            };
            let lbl = spec
                .typography
                .style_label(TypeRole::BodySmall, label(name.clone()))
                .color(fg);
            children.push(
                button(
                    flex_row((ui_icon(icon_path, 14.0, fg), lbl))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px()),
                    move |s: &mut State| cb(s, idx),
                )
                .ui_class(classes::BTN_SUBTLE)
                .dims(Dimensions::height(22.px()))
                .boxed(),
            );
        }

        sized_box(
            flex_col((
                horizontal_divider(),
                flex_row(children)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(4.px())
                    .padding(Padding::from_vh(3.px(), 10.px())),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .background_color(p.header_bar_bg)
    })
}
