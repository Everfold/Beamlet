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
        let body_spec = spec.typography.get(TypeRole::BodyMedium);
        let lines = [
            format!(
                "设计语言：{} ({theme_name}) · 明暗模式：{:?}",
                spec.name, spec.brightness
            ),
            format!(
                "控件几何尺寸：按钮高度={}px（圆角={}px），标签高度={}px，顶栏={}px，侧栏={}px",
                m.button_height, m.button_radius, m.chip_height, m.top_bar_height, m.sidebar_width
            ),
            format!(
                "交互与边框：输入框聚焦={:?}，选项卡指示器={:?}，导航指示器={:?}，边框模式={:?}",
                spec.input_focus, spec.tab_indicator, spec.nav_indicator, spec.control_border
            ),
            format!(
                "平台惯例与排版：弹窗按钮序={:?}，保存快捷键={}，正文排版={}px/{}px (字距 {:+.2})，减弱动效={}",
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
        headline_small("DesignSpec 令牌、度量与平台惯例"),
        elevated_card(
            flex_col((title_medium("当前几何度量与交互规则"), metrics_summary))
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
