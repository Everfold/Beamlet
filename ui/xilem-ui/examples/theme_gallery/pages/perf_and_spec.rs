//! Performance, incremental rebuild (`memoize_with_spec`), virtualization (`virtual_list`),
//! and `DesignSpec` token inspector page.

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
    .dims(Dimensions::fixed(150.px(), 38.px()))
    .boxed()
}

pub fn perf_and_spec_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let ticks = state.perf.unrelated_ticks;
    let memo_tag = state.perf.memo_tag.clone();
    let unmemo_counter = state.perf.unmemoized_builds.clone();
    let memo_counter = state.perf.memoized_builds.clone();

    let unmemoized_panel = with_spec(move |spec: &DesignSpec, _| {
        let count = unmemo_counter.fetch_add(1, Ordering::Relaxed) + 1;
        outlined_card(
            flex_col((
                title_small("普通 `with_spec` 子树"),
                body_medium(format!("构建/重建次数：{count}")),
                body_small(format!(
                    "每次页面状态变更都会重跑闭包 · 主题：{}",
                    spec.name
                )),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
    });

    let memoized_panel = memoize_with_spec(memo_tag, move |spec: &DesignSpec, tag| {
        let count = memo_counter.fetch_add(1, Ordering::Relaxed) + 1;
        elevated_card(
            flex_col((
                title_small("`memoize_with_spec` 缓存子树"),
                body_medium(format!("构建/重建次数：{count} · Key：{tag}")),
                body_small(format!(
                    "无关状态变更时跳过闭包与子视图 rebuild；仅在 Key 或 DesignSpec ({}) 变更时刷新",
                    spec.name
                )),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px()),
        )
    });

    let incremental_lab = elevated_card(
        flex_col((
            title_medium("1. `memoize_with_spec` 增量重建隔离演示"),
            flex_row((
                primary_button(
                    format!("触发无关状态变更 (Ticks: {ticks})"),
                    |s: &mut GalleryState| {
                        s.perf.unrelated_ticks += 1;
                    },
                ),
                secondary_button(
                    "切换 Memo Key (Alpha <-> Beta)",
                    |s: &mut GalleryState| {
                        s.perf.memo_tag = if s.perf.memo_tag == "Alpha" {
                            "Beta".to_owned()
                        } else {
                            "Alpha".to_owned()
                        };
                    },
                ),
                outlined_button("清零计数器", |s: &mut GalleryState| {
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
        .map_or("未选中行".to_owned(), |r| {
            format!("已选中 VirtualRow #{r}")
        });

    let virtual_lab = outlined_card(
        flex_col((
            flex_row((
                title_medium("2. 万级虚拟滚动 (`virtual_list`, 10,000 项)").flex(1.0),
                body_small(selected_row_text),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center),
            body_small(
                "底层仅挂载当前 300px 视口可见行及少量缓冲区，保持构建、布局与绘制复杂度为 O(viewport)。",
            ),
            flex_row((
                secondary_button("跳转至 #0", |s: &mut GalleryState| {
                    s.perf.virtual_jump_idx = Some(0);
                }),
                secondary_button("跳转至 #5000", |s: &mut GalleryState| {
                    s.perf.virtual_jump_idx = Some(5000);
                }),
                secondary_button("跳转至 #9990", |s: &mut GalleryState| {
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
                            ui_icon(
                                if is_selected {
                                    icons::STAR
                                } else {
                                    icons::DESCRIPTION
                                },
                                16.0,
                                if is_selected {
                                    Color::from_rgb8(0x2F, 0x6F, 0xED)
                                } else {
                                    Color::from_rgb8(0x7A, 0x82, 0x8E)
                                },
                            ),
                            body_medium(format!("虚拟列表行 #{idx:04}"))
                                .flex(1.0),
                            label_small(format!("批次 {}", idx / 100)),
                        ))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(10.px()),
                        is_selected,
                        move |st: &mut GalleryState| {
                            st.perf.selected_virtual_row = Some(idx);
                        },
                    ))
                    .dims(Dimensions::height(40.px()))
                })
                .jump_to(state.perf.virtual_jump_idx),
            )
            .dims(Dimensions::height(300.px())),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

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
        let body_spec = spec.typography.get(TypeRole::BodyMedium);
        let lines = [
            format!(
                "设计语言：{} ({theme_name}) · 明暗模式：{:?} · 合成器材质：{:?}",
                spec.name, spec.brightness, spec.backdrop
            ),
            format!(
                "控件几何尺寸：按钮={}px (圆角 {}px)，标签={}px，顶栏={}px，侧栏={}px",
                m.button_height, m.button_radius, m.chip_height, m.top_bar_height, m.sidebar_width
            ),
            format!(
                "交互与边框：聚焦={:?}，选项卡={:?}，导航={:?}，边框={:?}",
                spec.input_focus, spec.tab_indicator, spec.nav_indicator, spec.control_border
            ),
            format!(
                "平台惯例：弹窗按钮序={:?}，保存={}，正文={}px/{}px (字距 {:+.2})，减弱动效={}",
                spec.dialog_action_order,
                spec.shortcut_convention.format_primary("S"),
                body_spec.font_size,
                body_spec.line_height,
                body_spec.letter_spacing,
                spec.reduced_motion
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
            title_medium("15 级语义排版字阶 (Typographic Ramp)"),
            display_small("Display Small — 核心大字指标"),
            headline_large("Headline Large — 窗口级标题"),
            headline_medium("Headline Medium — 分区标题"),
            headline_small("Headline Small — 弹窗与页面标题"),
            title_large("Title Large — 顶栏标题"),
            title_medium("Title Medium — 卡片标题"),
            title_small("Title Small — 侧栏分组与列表项"),
            body_large("Body Large — 主要正文与表单主标签。"),
            body_medium("Body Medium — 紧凑桌面控件与设置行正文。"),
            body_small("Body Small — 辅助说明、校验与次要描述。"),
            label_large("Label Large — 按钮与选项卡文案"),
            label_medium("Label Medium — 徽章与分组眉题"),
            label_small("Label Small — 微型快捷键提示"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.px()),
    );

    flex_col((
        headline_small("性能工程、增量重建与 DesignSpec 令牌"),
        incremental_lab,
        virtual_lab,
        elevated_card(
            flex_col((
                title_medium("当前几何度量、材质与交互规则"),
                metrics_summary,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(10.px()),
        ),
        filled_card(
            flex_col((title_medium("语义调色板与状态色板"), swatches))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
        ),
        typography_ramp,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
