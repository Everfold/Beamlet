//! Visual `DesignSpec` inspector page displaying typography ramp, semantic status/surface swatches, and metrics.

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row, label, sized_box};
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
    .dims(Dimensions::fixed(155.px(), 38.px()))
    .boxed()
}

pub fn tokens_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let theme_name = state.theme.language.label();

    let swatches = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let row1 = flex_row((
            swatch(spec, "accent", p.accent, p.on_accent),
            swatch(
                spec,
                "accent_container",
                p.accent_container,
                p.on_accent_container,
            ),
            swatch(spec, "secondary", p.secondary, p.on_secondary),
            swatch(
                spec,
                "secondary_container",
                p.secondary_container,
                p.on_secondary_container,
            ),
            swatch(
                spec,
                "tertiary_container",
                p.tertiary_container,
                p.on_tertiary_container,
            ),
        ))
        .gap(8.px());

        let row2 = flex_row((
            swatch(spec, "success", p.success, p.on_success),
            swatch(spec, "warning", p.warning, p.on_warning),
            swatch(spec, "error", p.error, p.on_error),
            swatch(
                spec,
                "inverse_surface",
                p.inverse_surface,
                p.inverse_on_surface,
            ),
            swatch(spec, "window_bg", p.window_bg, p.text_primary),
        ))
        .gap(8.px());

        let row3 = flex_row((
            swatch(spec, "surface_low", p.surface_low, p.text_primary),
            swatch(spec, "surface", p.surface, p.text_primary),
            swatch(spec, "surface_high", p.surface_high, p.text_primary),
            swatch(spec, "surface_highest", p.surface_highest, p.text_primary),
            swatch(spec, "header_bar_bg", p.header_bar_bg, p.text_primary),
        ))
        .gap(8.px());

        flex_col((row1, row2, row3))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(8.px())
    });

    let metrics_summary = with_spec(move |spec: &DesignSpec, _| {
        let m = &spec.metrics;
        let lines = [
            format!(
                "Design Language: {} ({theme_name}) · Brightness: {:?}",
                spec.name, spec.brightness
            ),
            format!(
                "Control Geometry: button={}px (r={}px), chip={}px, top_bar={}px, sidebar={}px",
                m.button_height, m.button_radius, m.chip_height, m.top_bar_height, m.sidebar_width
            ),
            format!(
                "Interaction Grammar: input_focus={:?}, tab={:?}, nav={:?}",
                spec.input_focus, spec.tab_indicator, spec.nav_indicator
            ),
            format!(
                "Platform Conventions: dialog_order={:?}, shortcut_save={}",
                spec.dialog_action_order,
                spec.shortcut_convention.format_primary("S")
            ),
        ];
        flex_col(
            lines
                .into_iter()
                .map(|line| body_medium(line).boxed())
                .collect::<Vec<_>>(),
        )
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.px())
    });

    let typography_ramp = outlined_card(
        flex_col((
            title_medium("15-Role Typographic Ramp"),
            display_small("Display Small — Hero Metric"),
            headline_large("Headline Large — Window Title"),
            headline_medium("Headline Medium — Section Header"),
            headline_small("Headline Small — Dialog Heading"),
            title_large("Title Large — App Bar"),
            title_medium("Title Medium — Card Heading"),
            title_small("Title Small — Sidebar Group"),
            body_large("Body Large — Primary reading copy and form labels."),
            body_medium("Body Medium — Dense desktop controls and settings rows."),
            body_small("Body Small — Captions, hints, and secondary descriptions."),
            label_large("Label Large — Button & Tab Label"),
            label_medium("Label Medium — Badge & Form Header"),
            label_small("Label Small — Micro Shortcut Caption"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.px()),
    );

    flex_col((
        headline_small("DesignSpec Tokens, Metrics & Conventions"),
        elevated_card(
            flex_col((title_medium("Active Geometry & Behavior"), metrics_summary))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
        ),
        filled_card(
            flex_col((title_medium("Semantic Palette & Status Swatches"), swatches))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
        ),
        typography_ramp,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
