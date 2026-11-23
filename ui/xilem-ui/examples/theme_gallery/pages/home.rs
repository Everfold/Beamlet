//! Home overview showcase page: architecture pillars, active DesignSpec summary, and quick page jumps.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::{GalleryState, GalleryTab};

fn quick_jump_card(
    title: &'static str,
    subtitle: &'static str,
    cta: &'static str,
    icon: &'static str,
    target: GalleryTab,
) -> impl WidgetView<GalleryState> + use<> {
    outlined_card(
        flex_col((
            flex_row((
                title_medium(title).flex(1.0),
                info_badge(InfoBarSeverity::Informational, Some("展示")),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center),
            body_small(subtitle),
            secondary_button(cta, move |s: &mut GalleryState| {
                s.active_tab = target;
            })
            .leading_icon(icon),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px()),
    )
}

pub fn home_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let lang = state.theme.language.label();
    let brightness = if state.theme.brightness == Brightness::Dark {
        "深色"
    } else {
        "浅色"
    };
    let clicks = state.components.click_count;

    let hero = elevated_card(
        flex_col((
            headline_medium("Beamlet UI — Xilem 多主题桌面框架展示台"),
            body_large(
                "基于 Xilem + Masonry 构建：单一业务状态同时驱动 Material 3、Windows Fluent 2、macOS HIG 与 GNOME Adwaita 四套平台设计语言，提供三层样式治理、增量重建边界 (`memoize_with_spec`)、万级虚拟列表 (`virtual_list`) 与原生 AI-First 质检能力。",
            ),
            with_spec(move |spec: &DesignSpec, _| {
                flex_row((
                    surface_box(
                        ElevationLevel::Level1,
                        body_small(format!("当前设计语言：{lang} ({brightness})")),
                    ),
                    surface_box(
                        ElevationLevel::Level1,
                        body_small(format!("窗口材质：{:?}", spec.backdrop)),
                    ),
                    surface_box(
                        ElevationLevel::Level1,
                        body_small(format!(
                            "减弱动效：{} · 累计操作：{clicks}",
                            spec.reduced_motion
                        )),
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px())
            }),
            flex_row((
                primary_button("探索布局系统", |s: &mut GalleryState| {
                    s.active_tab = GalleryTab::Layout;
                })
                .leading_icon(icons::DOCK_LEFT),
                tonal_icon_button(icons::TUNE, |s: &mut GalleryState| {
                    s.active_tab = GalleryTab::PerfAndSpec;
                }),
                outlined_button("打开原生警告框", |s: &mut GalleryState| {
                    s.overlays.show_platform_alert = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let row_top = flex_row((
        quick_jump_card(
            "布局与响应式系统",
            "交互式调试 SPACE_* 间距、Flex 弹性分栏、AdwClamp 限宽、BreakpointBin 断点与 ZStack 海拔叠加。",
            "进入布局系统",
            icons::DOCK_LEFT,
            GalleryTab::Layout,
        )
        .flex(1.0),
        quick_jump_card(
            "基础与输入控件矩阵",
            "按钮变体、RippleLayer 涟漪、徽章、选项卡、分段按钮、浮动标签输入与双滑块选择器。",
            "浏览组件矩阵",
            icons::EDIT,
            GalleryTab::Components,
        )
        .flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(12.px());

    let row_bottom = flex_row((
        quick_jump_card(
            "增量重建与万级虚拟化",
            "现场对比 memoize_with_spec 与普通 with_spec 重建计数，交互体验 10,000 行 virtual_list 与令牌检查器。",
            "打开性能实验室",
            icons::TUNE,
            GalleryTab::PerfAndSpec,
        )
        .flex(1.0),
        quick_jump_card(
            "四平台原生组件与工作台",
            "同屏体验 MD3 FAB/工具栏、Fluent 可折叠侧栏与溢出菜单、HIG 检查器与弹出按钮、Adwaita 首选项组。",
            "查看平台原生",
            icons::PALETTE,
            GalleryTab::PlatformNative,
        )
        .flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(12.px());

    flex_col((hero, row_top, row_bottom))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
