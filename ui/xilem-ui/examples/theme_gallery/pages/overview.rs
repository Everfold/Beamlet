//! Overview gallery page: typography, buttons, icon buttons, cards, chips, progress.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn overview_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let clicks = state.click_count;
    let prog = state.slider_val;

    let buttons_card = elevated_card(
        flex_col((
            title_medium("按钮与图标操作"),
            flex_row((
                primary_button("主要按钮", |s: &mut GalleryState| s.click_count += 1)
                    .leading_icon(icons::ADD),
                secondary_button("次要按钮", |s: &mut GalleryState| s.click_count += 1),
                elevated_button("浮起按钮", |s: &mut GalleryState| s.click_count += 1),
                outlined_button("描边按钮", |s: &mut GalleryState| s.click_count += 1),
                subtle_button("文本按钮", |s: &mut GalleryState| s.click_count += 1),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((
                destructive_button("删除目标", |s: &mut GalleryState| {
                    s.show_destructive_dialog = true;
                })
                .leading_icon(icons::DELETE),
                primary_button("已禁用", |s: &mut GalleryState| s.click_count += 1).disabled(true),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((
                icon_button(icons::FAVORITE, |s: &mut GalleryState| s.click_count += 1),
                filled_icon_button(icons::BOOKMARK, |s: &mut GalleryState| s.click_count += 1),
                tonal_icon_button(icons::SETTINGS, |s: &mut GalleryState| s.click_count += 1),
                outlined_icon_button(icons::SEARCH, |s: &mut GalleryState| s.click_count += 1),
                with_badge(
                    icon_button(icons::NOTIFICATIONS, |s: &mut GalleryState| {
                        s.click_count += 1
                    }),
                    Some(large_badge(format!("{clicks}"))),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let progress_card = outlined_card(
        flex_col((
            title_medium("进度指示与层级表面"),
            body_small(format!(
                "操作计数：{clicks} · 确定性进度：{:.0}%",
                prog * 100.0
            )),
            linear_progress(Some(prog)),
            linear_progress(None),
            flex_row((
                determinate_circular_progress(prog),
                circular_progress(),
                small_circular_progress(),
                surface_box(ElevationLevel::Level2, body_medium("二级海拔高度表面")),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let tag_chips: Vec<_> = state
        .input_tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            input_chip(tag.clone(), |s: &mut GalleryState| s.click_count += 1)
                .on_remove(move |s: &mut GalleryState| {
                    if i < s.input_tags.len() {
                        s.input_tags.remove(i);
                    }
                })
                .boxed()
        })
        .collect();

    let chips_card = filled_card(
        flex_col((
            title_medium("选项卡、分段按钮与标签 (Chip)"),
            tabs(
                &["常规", "外观", "快捷键"],
                state.selected_tab,
                |s: &mut GalleryState, idx| s.selected_tab = idx,
            ),
            secondary_tabs(
                &["概览", "规范", "更新日志"],
                state.secondary_tab,
                |s: &mut GalleryState, idx| s.secondary_tab = idx,
            ),
            multi_choice_segmented_button(
                &[
                    ("粗体", state.multi_seg_mask[0]),
                    ("斜体", state.multi_seg_mask[1]),
                    ("代码", state.multi_seg_mask[2]),
                ],
                |s: &mut GalleryState, idx| {
                    if let Some(flag) = s.multi_seg_mask.get_mut(idx) {
                        *flag = !*flag;
                    }
                },
            ),
            flex_row((
                assist_chip("辅助操作", |s: &mut GalleryState| s.click_count += 1)
                    .leading_icon(icons::PALETTE),
                filter_chip("筛选 A", state.chip_flags[0], |s: &mut GalleryState| {
                    s.chip_flags[0] = !s.chip_flags[0];
                }),
                filter_chip("筛选 B", state.chip_flags[1], |s: &mut GalleryState| {
                    s.chip_flags[1] = !s.chip_flags[1];
                }),
                suggestion_chip("切换至 Fluent", |s: &mut GalleryState| {
                    s.theme.language = DesignLanguage::Fluent;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
            flex_row(tag_chips)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((
        headline_small("跨主题通用组件矩阵"),
        buttons_card,
        progress_card,
        chips_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
