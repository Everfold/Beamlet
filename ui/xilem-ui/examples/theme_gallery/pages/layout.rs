//! Layout system showcase page: spacing rhythm, Flex weights, AdwClamp, BreakpointBin, and Elevation/ZStack.

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

    let flex_lab = elevated_card(
        flex_col((
            title_medium("1. Flex 弹性比例与 `SPACE_*` 间距节奏实验室"),
            body_small("通过主轴弹性权重 (.flex(1.0) / .flex(2.0))、交叉轴对齐与标准间距 Token 构建规整桌面布局。"),
            flex_row((
                segmented_button(
                    &["4px", "8px", "12px", "16px", "24px"],
                    state.layout.spacing_idx,
                    |s: &mut GalleryState, idx| s.layout.spacing_idx = idx,
                ),
                segmented_button(
                    &["顶部对齐 (Start)", "居中 (Center)", "拉伸 (Stretch)"],
                    state.layout.align_idx,
                    |s: &mut GalleryState, idx| s.layout.align_idx = idx,
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
            with_spec(move |spec: &DesignSpec, _| {
                let p = &spec.palette;
                let cell = |title: &'static str, detail: &'static str, bg: xilem::Color| {
                    sized_box(
                        flex_col((title_small(title), body_small(detail)))
                            .cross_axis_alignment(CrossAxisAlignment::Start)
                            .gap(2.px()),
                    )
                    .padding(Padding::all(12.px()))
                    .background_color(bg)
                    .border(p.border, 1.px())
                    .corner_radius(spec.metrics.card_radius.px())
                };
                flex_row((
                    cell("1x 导航列", "flex(1.0)", p.surface_container).flex(1.0),
                    cell(
                        "2x 主内容区",
                        "flex(2.0) · 随窗口等比伸缩",
                        p.accent_container,
                    )
                    .flex(2.0),
                    cell("1x 检视列", "flex(1.0)", p.secondary_container).flex(1.0),
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
            title_medium("2. AdwClamp 居中限宽排版"),
            body_small("宽屏桌面下限制文本与表单最大可读宽度，避免内容在超宽窗口中失控拉伸。"),
            segmented_button(
                &["紧凑 480px", "标准 680px", "宽松 880px"],
                state.layout.clamp_idx,
                |s: &mut GalleryState, idx| s.layout.clamp_idx = idx,
            ),
            clamp(
                clamp_w,
                filled_card(
                    flex_col((
                        title_small(format!("当前 AdwClamp 最大宽度：{clamp_w:.0} px")),
                        body_small("内部视图自动水平居中并保持最大宽度上限，当可用宽度小于上限时平滑收缩至 100%。"),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(6.px()),
                ),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let breakpoint_wide = flex_row((
        filled_card(
            flex_col((
                title_small("宽屏模式 — 左右双栏"),
                body_small("当 BreakpointBin 宽度 >= 阈值时自动采用并排网格结构。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
        .flex(1.0),
        filled_card(
            flex_col((
                title_small("右侧上下文面板"),
                body_small("次要辅助信息随宽度展开并列呈现。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
        .flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(12.px());

    let breakpoint_narrow = flex_col((
        filled_card(
            flex_col((
                title_small("窄屏模式 — 纵向堆叠"),
                body_small("当容器窄于断点阈值时切换为单列纵向流式布局，保持零溢出。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        ),
        filled_card(
            flex_col((
                title_small("堆叠辅助区"),
                body_small("断点阈值以上下结构重新流排。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(8.px());

    let adaptive_container: Box<xilem::AnyWidgetView<GalleryState>> =
        if state.layout.simulate_compact {
            sized_box(breakpoint_bin(520.0, breakpoint_wide, breakpoint_narrow))
                .dims(Dimensions::width(420.px()))
                .boxed()
        } else {
            breakpoint_bin(520.0, breakpoint_wide, breakpoint_narrow).boxed()
        };

    let breakpoint_lab = outlined_card(
        flex_col((
            flex_row((
                title_medium("3. AdwBreakpointBin 响应式断点容器").flex(1.0),
                filter_chip(
                    "模拟窄容器 (420px)",
                    state.layout.simulate_compact,
                    |s: &mut GalleryState| {
                        s.layout.simulate_compact = !s.layout.simulate_compact;
                    },
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            body_small("既响应真实窗口缩放，也可点击右上角筛选标签将容器锁定至 420px 触发 520px 断点切换。"),
            adaptive_container,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let elevation_and_zstack = filled_card(
        flex_col((
            title_medium("4. 海拔层级 (ElevationLevel)、Fluent 蚀刻边框与 ZStack 锚点叠加"),
            flex_row((
                surface_box(ElevationLevel::Level0, body_small("Level 0 · 平面")).flex(1.0),
                surface_box(ElevationLevel::Level1, body_small("Level 1 · 卡片")).flex(1.0),
                surface_box(ElevationLevel::Level2, body_small("Level 2 · 菜单")).flex(1.0),
                surface_box(ElevationLevel::Level3, body_small("Level 3 · 对话框")).flex(1.0),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((
                beveled_surface(
                    sized_box(body_small("BeveledBorder 1px 顶部高光蚀刻层"))
                        .padding(Padding::all(14.px())),
                )
                .flex(1.0),
                sized_box(zstack((
                    surface_box(ElevationLevel::Level1, body_small("ZStack 底层画布"))
                        .alignment(UnitPoint::TOP_LEFT),
                    sized_box(info_badge(InfoBarSeverity::Success, Some("TOP_RIGHT 角标")))
                        .padding(Padding::all(6.px()))
                        .alignment(UnitPoint::TOP_RIGHT),
                    sized_box(info_badge(
                        InfoBarSeverity::Warning,
                        Some("BOTTOM_RIGHT 锚点"),
                    ))
                    .padding(Padding::all(6.px()))
                    .alignment(UnitPoint::BOTTOM_RIGHT),
                )))
                .dims(Dimensions::height(88.px()))
                .flex(1.0),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((
        headline_small("布局、间距与响应式系统"),
        flex_lab,
        clamp_lab,
        breakpoint_lab,
        elevation_and_zstack,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
