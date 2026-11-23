//! Incremental rebuild (`memoize_with_spec`), 10k-row `virtual_list`, and `DesignSpec` inspector.

use std::sync::atomic::Ordering;

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, label, sized_box};
use xilem::{AnyWidgetView, Color, WidgetView};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

fn swatch<State: 'static, Action: 'static>(
    spec: &DesignSpec,
    name: &'static str,
    fill: Color,
    ink: Color,
) -> Box<AnyWidgetView<State, Action>> {
    sized_box(
        spec.typography
            .style_label(TypeRole::LabelSmall, label(name))
            .color(ink),
    )
    .padding(Padding::from_vh(8.px(), 10.px()))
    .background_color(fill)
    .border(spec.palette.border, 1.px())
    .corner_radius(6.px())
    .dims(Dimensions::fixed(150.px(), 36.px()))
    .boxed()
}

pub fn lab_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let ticks = state.perf.unrelated_ticks;
    let memo_tag = state.perf.memo_tag.clone();
    let unmemo_counter = state.perf.unmemoized_builds.clone();
    let memo_counter = state.perf.memoized_builds.clone();

    let unmemoized_panel = with_spec(move |spec: &DesignSpec, _| {
        let count = unmemo_counter.fetch_add(1, Ordering::Relaxed) + 1;
        outlined_card(
            flex_col((
                title_small("with_spec"),
                body_medium(format!("重建：{count} ({})", spec.name)),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
    });

    let memoized_panel = memoize_with_spec(memo_tag, move |spec: &DesignSpec, tag| {
        let count = memo_counter.fetch_add(1, Ordering::Relaxed) + 1;
        elevated_card(
            flex_col((
                title_small("memoize_with_spec"),
                body_medium(format!("重建：{count} · {tag} ({})", spec.name)),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
    });

    let incremental_lab = elevated_card(
        flex_col((
            title_medium("memoize_with_spec"),
            flex_row((
                primary_button(
                    format!("无关状态 +1 ({ticks})"),
                    |s: &mut GalleryState| {
                        s.perf.unrelated_ticks += 1;
                    },
                ),
                secondary_button("切换 Key", |s: &mut GalleryState| {
                    s.perf.memo_tag = if s.perf.memo_tag == "Alpha" {
                        "Beta".to_owned()
                    } else {
                        "Alpha".to_owned()
                    };
                }),
                outlined_button("清零", |s: &mut GalleryState| {
                    s.perf.unmemoized_builds.store(0, Ordering::Relaxed);
                    s.perf.memoized_builds.store(0, Ordering::Relaxed);
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((unmemoized_panel.flex(1.0), memoized_panel.flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let selected_row_text = state
        .perf
        .selected_virtual_row
        .map_or("未选中".to_owned(), |r| format!("#{r}"));

    let virtual_lab = outlined_card(
        flex_col((
            flex_row((
                title_medium("virtual_list (10,000)").flex(1.0),
                body_small(selected_row_text),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center),
            flex_row((
                secondary_button("#0", |s: &mut GalleryState| {
                    s.perf.virtual_jump_idx = Some(0);
                }),
                secondary_button("#5000", |s: &mut GalleryState| {
                    s.perf.virtual_jump_idx = Some(5000);
                }),
                secondary_button("#9990", |s: &mut GalleryState| {
                    s.perf.virtual_jump_idx = Some(9990);
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
            sized_box(
                virtual_list(10_000, |s: &mut GalleryState, idx| {
                    let is_selected = s.perf.selected_virtual_row == Some(idx);
                    sized_box(clickable_list_item_view(
                        flex_row((
                            body_medium(format!("Row #{idx:04}")).flex(1.0),
                            label_small(format!("B{}", idx / 100)),
                        ))
                        .cross_axis_alignment(CrossAxisAlignment::Center),
                        is_selected,
                        move |st: &mut GalleryState| {
                            st.perf.selected_virtual_row = Some(idx);
                        },
                    ))
                    .dims(Dimensions::height(38.px()))
                })
                .jump_to(state.perf.virtual_jump_idx),
            )
            .dims(Dimensions::height(280.px())),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let spec_card = filled_card(with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let m = &spec.metrics;
        let row1 = flex_row((
            swatch(spec, "accent", p.accent, p.on_accent),
            swatch(
                spec,
                "secondary",
                p.secondary_container,
                p.on_secondary_container,
            ),
            swatch(spec, "success", p.success, p.on_success),
            swatch(spec, "warning", p.warning, p.on_warning),
            swatch(spec, "error", p.error, p.on_error),
        ))
        .gap(8.px());

        let row2 = flex_row((
            swatch(spec, "window_bg", p.window_bg, p.text_primary),
            swatch(spec, "surface_low", p.surface_low, p.text_primary),
            swatch(spec, "surface", p.surface, p.text_primary),
            swatch(spec, "surface_high", p.surface_high, p.text_primary),
            swatch(spec, "header_bar", p.header_bar_bg, p.text_primary),
        ))
        .gap(8.px());

        flex_col((
            title_medium("DesignSpec"),
            body_small(format!(
                "{} · {:?} · {:?} · btn={}px/r={}px · 快捷键={}",
                spec.name,
                spec.brightness,
                spec.backdrop,
                m.button_height,
                m.button_radius,
                spec.shortcut_convention.format_primary("S"),
            )),
            row1,
            row2,
            flex_row((
                display_small("Display"),
                headline_small("Headline"),
                title_medium("Title"),
                body_medium("Body"),
                label_small("Label"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px())
    }));

    flex_col((incremental_lab, virtual_lab, spec_card))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
