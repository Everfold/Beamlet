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
use xilem_ui_core::foundation::{UiClassExt, ui_icon};
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
