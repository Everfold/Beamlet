//! Cross-theme buttons, tabs, chips, text inputs, and selection controls.

use masonry::layout::AsUnit;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn components_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let c = &state.components;
    let clicks = c.click_count;
    let prog = c.slider_val;
    let range = c.range_val;
    let choice = c.radio_choice;

    let btn_rows_wide = flex_col((
        flex_row((
            primary_button("主要", |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .leading_icon(icons::ADD),
            secondary_button("次要", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            elevated_button("浮起", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            outlined_button("描边", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            subtle_button("文本", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px()),
        flex_row((
            ripple_layer(primary_button("涟漪", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }))
            .corner_radius(20.px()),
            destructive_button("删除", |s: &mut GalleryState| {
                s.overlays.show_destructive_dialog = true;
            })
            .leading_icon(icons::DELETE),
            primary_button("禁用", |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .disabled(true),
            icon_button(icons::FAVORITE, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            filled_icon_button(icons::BOOKMARK, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            tonal_icon_button(icons::SETTINGS, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            outlined_icon_button(icons::SEARCH, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            with_badge(
                icon_button(icons::NOTIFICATIONS, |s: &mut GalleryState| {
                    s.components.click_count += 1;
                }),
                Some(large_badge(format!("{clicks}"))),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(12.px());

    let btn_rows_narrow = flex_col((
        flex_row((
            primary_button("主要", |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .leading_icon(icons::ADD),
            secondary_button("次要", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            elevated_button("浮起", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        flex_row((
            outlined_button("描边", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            subtle_button("文本", |s: &mut GalleryState| {
                s.components.click_count += 1
            }),
            ripple_layer(primary_button("涟漪", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }))
            .corner_radius(20.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        flex_row((
            destructive_button("删除", |s: &mut GalleryState| {
                s.overlays.show_destructive_dialog = true;
            })
            .leading_icon(icons::DELETE),
            primary_button("禁用", |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .disabled(true),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        flex_row((
            icon_button(icons::FAVORITE, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            filled_icon_button(icons::BOOKMARK, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            tonal_icon_button(icons::SETTINGS, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            outlined_icon_button(icons::SEARCH, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            with_badge(
                icon_button(icons::NOTIFICATIONS, |s: &mut GalleryState| {
                    s.components.click_count += 1;
                }),
                Some(large_badge(format!("{clicks}"))),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(8.px());

    let buttons_card = elevated_card(
        flex_col((
            title_medium("按钮"),
            breakpoint_bin(480.0, btn_rows_wide, btn_rows_narrow),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let make_tag_chips = || {
        c.input_tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                input_chip(tag.clone(), |s: &mut GalleryState| {
                    s.components.click_count += 1;
                })
                .on_remove(move |s: &mut GalleryState| {
                    if i < s.components.input_tags.len() {
                        s.components.input_tags.remove(i);
                    }
                })
                .boxed()
            })
            .collect::<Vec<_>>()
    };
    let tag_chips_wide = make_tag_chips();
    let mut tag_chips_narrow = make_tag_chips();
    let tag_row_2 = if tag_chips_narrow.len() > 2 {
        tag_chips_narrow.split_off(2)
    } else {
        Vec::new()
    };

    let make_action_chips = || {
        (
            assist_chip("操作", |s: &mut GalleryState| {
                s.components.click_count += 1;
            })
            .leading_icon(icons::PALETTE),
            filter_chip("筛选 A", c.chip_flags[0], |s: &mut GalleryState| {
                s.components.chip_flags[0] = !s.components.chip_flags[0];
            }),
            filter_chip("筛选 B", c.chip_flags[1], |s: &mut GalleryState| {
                s.components.chip_flags[1] = !s.components.chip_flags[1];
            }),
            suggestion_chip("Fluent", |s: &mut GalleryState| {
                s.theme.language = DesignLanguage::Fluent;
            }),
        )
    };
    let (ac_w, fa_w, fb_w, sc_w) = make_action_chips();
    let (ac_n, fa_n, fb_n, sc_n) = make_action_chips();

    let tabs_and_chips_card = filled_card(
        flex_col((
            title_medium("选项卡与标签"),
            tabs(
                &["常规", "外观", "快捷键"],
                c.selected_tab,
                |s: &mut GalleryState, idx| s.components.selected_tab = idx,
            ),
            secondary_tabs(
                &["概览", "规范", "日志"],
                c.secondary_tab,
                |s: &mut GalleryState, idx| s.components.secondary_tab = idx,
            ),
            multi_choice_segmented_button(
                &[
                    ("粗体", c.multi_seg_mask[0]),
                    ("斜体", c.multi_seg_mask[1]),
                    ("代码", c.multi_seg_mask[2]),
                ],
                |s: &mut GalleryState, idx| {
                    if let Some(flag) = s.components.multi_seg_mask.get_mut(idx) {
                        *flag = !*flag;
                    }
                },
            ),
            breakpoint_bin(
                440.0,
                flex_row((ac_w, fa_w, fb_w, sc_w))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                flex_col((
                    flex_row((ac_n, fa_n))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(8.px()),
                    flex_row((fb_n, sc_n))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(8.px()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(6.px()),
            ),
            breakpoint_bin(
                440.0,
                flex_row(tag_chips_wide)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                flex_col((
                    flex_row(tag_chips_narrow)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(8.px()),
                    (!tag_row_2.is_empty()).then(|| {
                        flex_row(tag_row_2)
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(8.px())
                    }),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(6.px()),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let link_status =
        c.last_link_clicked
            .as_deref()
            .map_or("点击链接测试跳转事件", |id| match id {
                "spec-docs" => "已点击：设计规范文档",
                _ => id,
            });

    let text_card = outlined_card(
        flex_col((
            title_medium("输入与富文本"),
            search_bar(c.search_query.clone(), |s: &mut GalleryState, val| {
                s.components.search_query = val;
            })
            .on_clear(|s: &mut GalleryState| s.components.search_query.clear()),
            text_field(c.text_value.clone(), |s: &mut GalleryState, val| {
                s.components.text_value = val;
            })
            .label("名称")
            .prefix_text("https://")
            .max_chars(48),
            filled_text_field(c.email_value.clone(), |s: &mut GalleryState, val| {
                s.components.email_value = val;
            })
            .label("邮箱")
            .error_text("格式无效"),
            text_area(c.notes_value.clone(), |s: &mut GalleryState, val| {
                s.components.notes_value = val;
            })
            .label("多行备注")
            .placeholder("输入多行内容，超出高度自动滚动...")
            .min_height(84.px())
            .max_height(160.px())
            .max_chars(240),
            rich_prose(vec![
                TextSpan::new("支持 "),
                TextSpan::bold("粗体"),
                TextSpan::new("、"),
                TextSpan::italic("斜体"),
                TextSpan::new("、行内代码 "),
                TextSpan::code("rich_prose()"),
                TextSpan::new("、文本划选复制，以及可点击 "),
                TextSpan::link("spec-docs", "设计规范链接"),
                TextSpan::new("。"),
            ])
            .on_link_click(|s: &mut GalleryState, id| {
                s.components.last_link_clicked = Some(id.to_string());
                s.components.click_count += 1;
            }),
            body_small(link_status),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let selection_and_progress_card = elevated_card(
        flex_col((
            title_medium("选择与进度"),
            flex_row((
                checkbox("同意协议", c.agree_terms, |s: &mut GalleryState, v| {
                    s.components.agree_terms = v;
                }),
                tri_state_checkbox("半选", c.tri_state, |s: &mut GalleryState, next| {
                    s.components.tri_state = next;
                }),
                switch(c.notifications_on, |s: &mut GalleryState, v| {
                    s.components.notifications_on = v;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(16.px()),
            radio_group(vec![
                radio("标准", choice == 0, |s: &mut GalleryState| {
                    s.components.radio_choice = 0;
                }),
                radio("紧凑", choice == 1, |s: &mut GalleryState| {
                    s.components.radio_choice = 1;
                }),
            ]),
            slider(0.0, 1.0, prog, None, |s: &mut GalleryState, v| {
                s.components.slider_val = v;
            }),
            linear_progress(Some(prog)),
            flex_row((
                determinate_circular_progress(prog),
                circular_progress(),
                small_circular_progress(),
                body_small(format!(
                    "{:.0}% · {:.0}..{:.0}",
                    prog * 100.0,
                    range.0,
                    range.1
                )),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(14.px()),
            range_slider(0.0, 100.0, range, |s: &mut GalleryState, next| {
                s.components.range_val = next;
            }),
            select(
                &["自动", "浅色", "深色"],
                c.select_index,
                c.select_open,
                |s: &mut GalleryState| s.components.select_open = !s.components.select_open,
                |s: &mut GalleryState, i| {
                    s.components.select_index = i;
                    s.components.select_open = false;
                },
                |s: &mut GalleryState| s.components.select_open = false,
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((
        buttons_card,
        tabs_and_chips_card,
        text_card,
        selection_and_progress_card,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
