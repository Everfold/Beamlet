//! Inputs & selection controls gallery page.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn inputs_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let range = state.range_val;
    let choice = state.radio_choice;

    let text_card = outlined_card(
        flex_col((
            title_medium("文本输入（主题自适应聚焦框）"),
            search_bar(state.search_query.clone(), |s: &mut GalleryState, val| {
                s.search_query = val;
            })
            .on_clear(|s: &mut GalleryState| s.search_query.clear()),
            text_field(state.text_value.clone(), |s: &mut GalleryState, val| {
                s.text_value = val;
            })
            .label("工作区名称")
            .prefix_text("https://")
            .max_chars(48)
            .supporting_text("对比 MD3 外框、Fluent 底部强调线与 HIG 外发光环"),
            filled_text_field(state.email_value.clone(), |s: &mut GalleryState, val| {
                s.email_value = val;
            })
            .label("校验示例")
            .error_text("请输入有效的电子邮箱地址"),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let selection_card = elevated_card(
        flex_col((
            title_medium("选择控件、滑块与下拉列表"),
            flex_row((
                checkbox(
                    "同意许可协议",
                    state.agree_terms,
                    |s: &mut GalleryState, v| {
                        s.agree_terms = v;
                    },
                ),
                tri_state_checkbox(
                    "分组同步",
                    state.tri_state,
                    |s: &mut GalleryState, next| {
                        s.tri_state = next;
                    },
                ),
                switch(state.notifications_on, |s: &mut GalleryState, v| {
                    s.notifications_on = v;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px()),
            radio_group(vec![
                radio("标准密度", choice == 0, |s: &mut GalleryState| {
                    s.radio_choice = 0;
                }),
                radio("紧凑桌面", choice == 1, |s: &mut GalleryState| {
                    s.radio_choice = 1;
                }),
            ]),
            body_small(format!("单滑块数值：{:.2}", state.slider_val)),
            slider(
                0.0,
                1.0,
                state.slider_val,
                None,
                |s: &mut GalleryState, v| {
                    s.slider_val = v;
                },
            ),
            body_small(format!("双滑块区间：{:.0} .. {:.0}", range.0, range.1)),
            range_slider(0.0, 100.0, range, |s: &mut GalleryState, next| {
                s.range_val = next;
            }),
            select(
                &["自动检测", "强制浅色", "强制深色"],
                state.select_index,
                state.select_open,
                |s: &mut GalleryState| s.select_open = !s.select_open,
                |s: &mut GalleryState, i| {
                    s.select_index = i;
                    s.select_open = false;
                },
                |s: &mut GalleryState| s.select_open = false,
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((
        headline_small("文本输入与选择控件"),
        text_card,
        selection_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
