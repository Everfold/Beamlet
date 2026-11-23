//! Multi-pane desktop workbench combining filterable list, toggleable inspector detail pane,
//! destructive confirmation dialog, and per-platform native settings containers.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn workbench_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let query = state.workbench.search_query.trim().to_lowercase();
    let accent_color = state.theme.to_spec().palette.accent;
    let total = state.workbench.items.len();
    let selected = state.workbench.selected_idx.min(total.saturating_sub(1));

    let rows: Vec<_> = state
        .workbench
        .items
        .iter()
        .enumerate()
        .filter(|(_, (title, desc, _))| {
            query.is_empty()
                || title.to_lowercase().contains(&query)
                || desc.to_lowercase().contains(&query)
        })
        .map(|(idx, (title, desc, status))| {
            let active = idx == selected;
            let row_content = flex_row((
                ui_icon(
                    if active { icons::STAR } else { icons::FOLDER },
                    18.0,
                    if active {
                        accent_color
                    } else {
                        xilem::Color::from_rgb8(0x78, 0x7E, 0x88)
                    },
                ),
                flex_col((title_small(*title), body_small(*desc)))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(2.px())
                    .flex(1.0),
                info_badge(
                    if active {
                        InfoBarSeverity::Success
                    } else {
                        InfoBarSeverity::Informational
                    },
                    Some(*status),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px());

            clickable_list_item_view(row_content, active, move |s: &mut GalleryState| {
                s.workbench.selected_idx = idx;
            })
            .boxed()
        })
        .collect();

    let filter_bar = flex_row((
        search_bar(
            state.workbench.search_query.clone(),
            |s: &mut GalleryState, val| {
                s.workbench.search_query = val;
            },
        )
        .on_clear(|s: &mut GalleryState| s.workbench.search_query.clear())
        .flex(1.0),
        filter_chip(
            "内嵌详情栏",
            state.workbench.show_detail_pane,
            |s: &mut GalleryState| {
                s.workbench.show_detail_pane = !s.workbench.show_detail_pane;
            },
        ),
        secondary_button("检查器抽屉", |s: &mut GalleryState| {
            s.overlays.show_side_sheet = true;
        })
        .leading_icon(icons::DOCK_LEFT),
        destructive_button("移除选中项", |s: &mut GalleryState| {
            s.overlays.show_destructive_dialog = true;
        })
        .leading_icon(icons::DELETE)
        .disabled(state.workbench.items.is_empty()),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(10.px());

    let list_section = if rows.is_empty() {
        status_page(
            icons::SEARCH,
            "没有匹配的项目",
            "请清空搜索条件，或添加新的工作区条目。",
            Some((
                "清空搜索",
                std::sync::Arc::new(|s: &mut GalleryState| s.workbench.search_query.clear()),
            )),
        )
        .boxed()
    } else {
        grouped_card(
            flex_col(rows)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(2.px()),
        )
        .boxed()
    };

    let detail_pane = state.workbench.show_detail_pane.then(|| {
        let item = state.workbench.items.get(selected).copied();
        let (title, desc, status) = item.unwrap_or(("未选中目标", "列表为空", "空闲"));
        elevated_card(
            flex_col((
                title_medium("详情检视器"),
                title_small(title),
                body_medium(desc),
                info_badge(InfoBarSeverity::Informational, Some(status)),
                primary_button("触发平台原生弹窗", |s: &mut GalleryState| {
                    s.overlays.show_platform_alert = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(10.px()),
        )
    });

    let master_detail = flex_row((list_section.flex(2.0), detail_pane.map(|d| d.flex(1.0))))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px());

    let platform_settings = match state.theme.language {
        DesignLanguage::Md3 => filled_card(
            flex_col((
                title_medium("Material 3 工作区操作"),
                body_small("结合 M3 悬浮工具栏与 Small / Large FAB 人机工学。"),
                floating_toolbar(vec![
                    icon_button(icons::REFRESH, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .boxed(),
                    icon_button(icons::COPY, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .boxed(),
                    icon_button(icons::SHARE, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .boxed(),
                    primary_button("部署构建", |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .leading_icon(icons::SEND)
                    .boxed(),
                ]),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(10.px()),
        )
        .boxed(),
        DesignLanguage::Fluent => settings_card(
            Some(icons::TERMINAL),
            "并发构建线程数",
            Some("通过 SettingsCard 内嵌 WinUI 3 NumberBox 进行微调"),
            number_box(
                state.native.fluent_number_val,
                1.0,
                64.0,
                2.0,
                |s: &mut GalleryState, next| s.native.fluent_number_val = next,
            ),
        )
        .boxed(),
        DesignLanguage::Hig => form_group(
            Some("MACOS 构建设置"),
            vec![
                form_row(
                    "构建工作线程",
                    stepper(
                        "线程",
                        state.native.hig_stepper_val,
                        1,
                        32,
                        1,
                        |s: &mut GalleryState, next| s.native.hig_stepper_val = next,
                    ),
                )
                .boxed(),
                form_row(
                    "显示右侧检查器栏",
                    switch(
                        state.native.show_hig_inspector,
                        |s: &mut GalleryState, v| {
                            s.native.show_hig_inspector = v;
                        },
                    ),
                )
                .boxed(),
            ],
            Some("Aqua 分组卡片内的 macOS Form 表单行。"),
        )
        .boxed(),
        DesignLanguage::Adwaita => preferences_group(
            Some("Libadwaita 项目设置"),
            Some("使用 AdwComboRow 与 AdwEntryRow"),
            vec![
                entry_row(
                    "工作区标识 (Slug)",
                    state.native.adw_entry_val.clone(),
                    |s: &mut GalleryState, txt| s.native.adw_entry_val = txt,
                )
                .boxed(),
                combo_row(
                    "发布配置档案",
                    Some("选择编译优化目标"),
                    &["调试模式 (Debug)", "发布模式 (Release)", "带调试信息发布"],
                    state.native.adw_combo_idx,
                    state.native.adw_combo_open,
                    |s: &mut GalleryState| s.native.adw_combo_open = !s.native.adw_combo_open,
                    |s: &mut GalleryState, idx| {
                        s.native.adw_combo_idx = idx;
                        s.native.adw_combo_open = false;
                    },
                    |s: &mut GalleryState| s.native.adw_combo_open = false,
                )
                .boxed(),
            ],
        )
        .boxed(),
    };

    flex_col((
        headline_small("多栏桌面工作台样板"),
        body_small("演示主从列表 + 内嵌详情检视器 + 平台原生设置区与浮层抽屉的真实组合。"),
        filter_bar,
        master_detail,
        platform_settings,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(14.px())
}
