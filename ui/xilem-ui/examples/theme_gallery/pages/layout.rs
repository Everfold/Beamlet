//! Layout system showcase: spacing rhythm, Flex weights, AdwClamp, BreakpointBin, and Elevation/ZStack.

use xilem::WidgetView;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, sized_box, zstack};
use xilem_ui::core::tokens::{SPACE_4, SPACE_8, SPACE_12, SPACE_16, SPACE_24};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn layout_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    const SPACINGS: [xilem::masonry::layout::Length; 5] =
        [SPACE_4, SPACE_8, SPACE_12, SPACE_16, SPACE_24];
    const CLAMP_WIDTHS: [f64; 3] = [480.0, 680.0, 880.0];

    let gap_len = SPACINGS[state.layout.spacing_idx.min(SPACINGS.len() - 1)];
    let cross_align = match state.layout.align_idx {
        0 => CrossAxisAlignment::Start,
        1 => CrossAxisAlignment::Center,
        _ => CrossAxisAlignment::Stretch,
    };
    let clamp_w = CLAMP_WIDTHS[state.layout.clamp_idx.min(CLAMP_WIDTHS.len() - 1)];

    let seg_gap = || {
        segmented_button(
            &["4", "8", "12", "16", "24"],
            state.layout.spacing_idx,
            |s: &mut GalleryState, idx| s.layout.spacing_idx = idx,
        )
    };
    let seg_align = || {
        segmented_button(
            &["Start", "Center", "Stretch"],
            state.layout.align_idx,
            |s: &mut GalleryState, idx| s.layout.align_idx = idx,
        )
    };

    let flex_lab = elevated_card(
        flex_col((
            title_medium("Flex 间距与比例"),
            breakpoint_bin(
                540.0,
                flex_row((seg_gap(), seg_align()))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(12.px()),
                flex_col((seg_gap(), seg_align()))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(8.px()),
            ),
            with_spec(move |spec: &DesignSpec, _| {
                let p = &spec.palette;
                let cell = |title: &'static str, bg: xilem::Color| {
                    sized_box(title_small(title))
                        .padding(Padding::all(12.px()))
                        .background_color(bg)
                        .border(p.border, 1.px())
                        .corner_radius(spec.metrics.card_radius.px())
                };
                flex_row((
                    cell("1x", p.surface_container).flex(1.0),
                    cell("2x", p.accent_container).flex(2.0),
                    cell("1x", p.secondary_container).flex(1.0),
                ))
                .cross_axis_alignment(cross_align)
                .gap(gap_len)
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let clamp_lab = outlined_card(
        flex_col((
            title_medium("AdwClamp 限宽"),
            segmented_button(
                &["480px", "680px", "880px"],
                state.layout.clamp_idx,
                |s: &mut GalleryState, idx| s.layout.clamp_idx = idx,
            ),
            clamp(
                clamp_w,
                filled_card(title_small(format!("max-width: {clamp_w:.0}px"))),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let breakpoint_wide = flex_row((
        filled_card(title_small("宽屏：双栏 A")).flex(1.0),
        filled_card(title_small("宽屏：双栏 B")).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(12.px());

    let breakpoint_narrow = flex_col((
        filled_card(title_small("窄屏：堆叠 A")),
        filled_card(title_small("窄屏：堆叠 B")),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(8.px());

    let adaptive_container: Box<xilem::AnyWidgetView<GalleryState>> =
        if state.layout.simulate_compact {
            clamp(
                420.0,
                breakpoint_bin(520.0, breakpoint_wide, breakpoint_narrow),
            )
            .boxed()
        } else {
            breakpoint_bin(520.0, breakpoint_wide, breakpoint_narrow).boxed()
        };

    let breakpoint_lab = outlined_card(
        flex_col((
            flex_row((
                title_medium("BreakpointBin 断点 (520px)").flex(1.0),
                filter_chip(
                    "模拟窄屏 420px",
                    state.layout.simulate_compact,
                    |s: &mut GalleryState| {
                        s.layout.simulate_compact = !s.layout.simulate_compact;
                    },
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            adaptive_container,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let bevel_pair = || {
        (
            beveled_surface(sized_box(body_small("BeveledBorder")).padding(Padding::all(14.px()))),
            sized_box(zstack((
                surface_box(ElevationLevel::Level1, body_small("ZStack"))
                    .alignment(UnitPoint::TOP_LEFT),
                sized_box(info_badge(InfoBarSeverity::Success, Some("TOP_RIGHT")))
                    .padding(Padding::all(6.px()))
                    .alignment(UnitPoint::TOP_RIGHT),
                sized_box(info_badge(InfoBarSeverity::Warning, Some("BOTTOM_RIGHT")))
                    .padding(Padding::all(6.px()))
                    .alignment(UnitPoint::BOTTOM_RIGHT),
            )))
            .dims(Dimensions::height(88.px())),
        )
    };
    let (b_w, z_w) = bevel_pair();
    let (b_n, z_n) = bevel_pair();

    let elevation_and_zstack = filled_card(
        flex_col((
            title_medium("层级与 ZStack"),
            breakpoint_bin(
                440.0,
                flex_row((
                    surface_box(ElevationLevel::Level0, body_small("Level 0")).flex(1.0),
                    surface_box(ElevationLevel::Level1, body_small("Level 1")).flex(1.0),
                    surface_box(ElevationLevel::Level2, body_small("Level 2")).flex(1.0),
                    surface_box(ElevationLevel::Level3, body_small("Level 3")).flex(1.0),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px()),
                flex_col((
                    flex_row((
                        surface_box(ElevationLevel::Level0, body_small("Level 0")).flex(1.0),
                        surface_box(ElevationLevel::Level1, body_small("Level 1")).flex(1.0),
                    ))
                    .gap(8.px()),
                    flex_row((
                        surface_box(ElevationLevel::Level2, body_small("Level 2")).flex(1.0),
                        surface_box(ElevationLevel::Level3, body_small("Level 3")).flex(1.0),
                    ))
                    .gap(8.px()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px()),
            ),
            breakpoint_bin(
                440.0,
                flex_row((b_w.flex(1.0), z_w.flex(1.0)))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(12.px()),
                flex_col((b_n, z_n))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(8.px()),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((flex_lab, clamp_lab, breakpoint_lab, elevation_and_zstack))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
