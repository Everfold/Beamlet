//! Platform-native signature component showcase and open developer customization demo.

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, label, sized_box};
use xilem::{Color, WidgetView};
use xilem_ui::prelude::*;

use crate::state::{GalleryState, GalleryTab};

pub fn platform_native_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let clicks = state.click_count;

    // Demonstrates open custom slots (`button_view`) and direct Xilem Style overrides
    // hitting concrete underlying Masonry widgets without Passthrough wrappers.
    let dev_control_card = outlined_card(
        flex_col((
            title_medium("开发者控制权与开放式自定义插槽"),
            body_small(
                "无 Passthrough 黑盒阻隔：可直接链式覆写 .width() / .corner_radius() / .background_color()，或传入任意自定义子视图。",
            ),
            flex_row((
                button_view(
                    ButtonVariant::Primary,
                    with_spec(|spec, _| {
                        let shortcut = spec.shortcut_convention.format_primary("S");
                        flex_row((
                            ui_icon(icons::CHECK, 16.0, spec.palette.on_accent),
                            label("保存工作区").color(spec.palette.on_accent),
                            sized_box(
                                label(shortcut)
                                    .text_size(11.0)
                                    .color(spec.palette.on_accent),
                            )
                            .padding(Padding::from_vh(2.px(), 6.px()))
                            .border(spec.palette.on_accent, 1.px())
                            .corner_radius(4.px()),
                        ))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(8.px())
                    }),
                    |s: &mut GalleryState| s.click_count += 1,
                ),
                primary_button(format!("宽方角覆写 ({clicks})"), |s: &mut GalleryState| {
                    s.click_count += 1;
                })
                .width(230.px())
                .corner_radius(3.px()),
                list_item_view(
                    flex_row((
                        ui_icon(icons::INFO, 16.0, Color::from_rgb8(0x4E, 0x8C, 0xE8)),
                        body_medium("自定义 list_item_view 内容插槽").flex(1.0),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(10.px()),
    )
    .corner_radius(18.px());

    let md3_section = filled_card(
        flex_col((
            title_medium("Material Design 3 专属组件"),
            top_app_bar_slots(
                TopAppBarVariant::CenterAligned,
                "居中对齐 MD3 TopAppBar",
                Some(icon_button(icons::MENU, |s: &mut GalleryState| s.click_count += 1).boxed()),
                vec![icon_button(icons::SEARCH, |s: &mut GalleryState| s.click_count += 1).boxed()],
            ),
            flex_row((
                small_fab(icons::EDIT, FabColor::Surface, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                fab(icons::ADD, FabColor::Primary, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                large_fab(icons::SEND, FabColor::Tertiary, |s: &mut GalleryState| {
                    s.click_count += 1;
                }),
                extended_fab(
                    icons::EDIT,
                    "撰写",
                    FabColor::Secondary,
                    |s: &mut GalleryState| s.click_count += 1,
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px()),
            floating_toolbar(vec![
                icon_button(icons::ARROW_BACK, |s: &mut GalleryState| s.click_count += 1).boxed(),
                icon_button(icons::COPY, |s: &mut GalleryState| s.click_count += 1).boxed(),
                icon_button(icons::SHARE, |s: &mut GalleryState| s.click_count += 1).boxed(),
                primary_button("发布", |s: &mut GalleryState| s.click_count += 1).boxed(),
            ]),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let fluent_info = state.info_bar_visible.then(|| {
        info_bar(
            InfoBarSeverity::Success,
            "WinUI 3 InfoBar 信息栏",
            "Fluent Mica 表面与响应式 2px 底部强调色聚焦条已激活。",
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.info_bar_visible = false;
            })),
        )
    });

    let fluent_crumbs = ["设置", "个性化", "颜色与 Mica", "强调色"];
    let fluent_depth = (state.fluent_breadcrumb_depth + 1).clamp(1, fluent_crumbs.len());

    let fluent_section = flex_col((
        title_medium("Windows 11 Fluent / WinUI 3 专属组件"),
        breadcrumb_bar(
            &fluent_crumbs[..fluent_depth],
            |s: &mut GalleryState, idx| {
                s.fluent_breadcrumb_depth = idx;
            },
        ),
        fluent_info,
        settings_card(
            Some(icons::PALETTE),
            "Mica 云母背景材质",
            Some("基础填充随浅色 #F3F3F3 与深色 #202020 自动切换"),
            flex_row((
                info_badge(InfoBarSeverity::Success, Some("已启用")),
                switch(state.native_switch_a, |s: &mut GalleryState, v| {
                    s.native_switch_a = v;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ),
        settings_card(
            Some(icons::TUNE),
            "WinUI 3 NumberBox 数字步进框",
            Some("步进范围限制在 [0, 100] 区间内"),
            number_box(
                state.fluent_number_val,
                0.0,
                100.0,
                4.0,
                |s: &mut GalleryState, v| s.fluent_number_val = v,
            ),
        ),
        expander(
            "WinUI 3 通用 Expander 折叠面板",
            Some("支持包裹任意自定义嵌套视图"),
            state.fluent_expander_open,
            |s: &mut GalleryState| s.fluent_expander_open = !s.fluent_expander_open,
            flex_row((
                info_badge(InfoBarSeverity::Warning, Some("测试版")),
                body_medium("Expander 内嵌实时 InfoBadge 与自定义内容行。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ),
        settings_expander(
            Some(icons::SETTINGS),
            "高级 Fluent 选项",
            Some("CommandBar + 可展开子设置行"),
            state.expander_open,
            |s: &mut GalleryState| s.expander_open = !s.expander_open,
            None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
            vec![
                command_bar(
                    vec![primary_button("保存", |s: &mut GalleryState| s.click_count += 1).boxed()],
                    vec![subtle_button("重置", |s: &mut GalleryState| s.click_count = 0).boxed()],
                )
                .boxed(),
            ],
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let hig_paths = [
        ("Macintosh HD", icons::FOLDER),
        ("项目目录", icons::FOLDER),
        ("Beamlet", icons::CODE),
        ("UI", icons::DESCRIPTION),
    ];
    let hig_depth = (state.hig_path_depth + 1).clamp(1, hig_paths.len());

    let hig_section = flex_col((
        title_medium("macOS Apple HIG 专属组件"),
        macos_toolbar(
            "检查器",
            vec![icon_button(icons::ARROW_BACK, |s: &mut GalleryState| s.click_count += 1).boxed()],
            vec![help_button(|s: &mut GalleryState| s.click_count += 1).boxed()],
        ),
        form_group(
            Some("系统外观设置"),
            vec![
                form_row(
                    "硬件图形加速",
                    switch(state.native_switch_b, |s: &mut GalleryState, v| {
                        s.native_switch_b = v;
                    }),
                )
                .boxed(),
                form_row(
                    "步进密度",
                    stepper(
                        "栏数",
                        state.hig_stepper_val,
                        1,
                        16,
                        1,
                        |s: &mut GalleryState, v| s.hig_stepper_val = v,
                    ),
                )
                .boxed(),
                form_row(
                    "默认操作",
                    primary_button("应用", |s: &mut GalleryState| s.click_count += 1),
                )
                .boxed(),
            ],
            Some("还原 macOS 26px 紧凑型 Aqua 几何尺度与 3px 外发光聚焦环。"),
        ),
        disclosure_group(
            "检查器元数据 (DisclosureGroup)",
            state.hig_disclosure_open,
            |s: &mut GalleryState| s.hig_disclosure_open = !s.hig_disclosure_open,
            body_medium("对齐 NSDisclosureButton / SwiftUI DisclosureGroup 缩进规范。"),
        ),
        path_control(&hig_paths[..hig_depth], |s: &mut GalleryState, idx| {
            s.hig_path_depth = idx;
        }),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    let adwaita_banner = state.banner_visible.then(|| {
        banner(
            "Libadwaita AdwBanner — 已启用扁平化 Alpha 状态叠加层",
            Some("关闭"),
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.banner_visible = false;
            })),
            None,
        )
    });

    let adwaita_section = flex_col((
        title_medium("GNOME Libadwaita 专属组件"),
        header_bar_with_title(
            view_switcher(
                &[("概览", icons::HOME), ("原生组件", icons::PALETTE)],
                0,
                |s: &mut GalleryState, idx| s.active_tab = GalleryTab::from_index(idx),
            ),
            vec![],
            vec![],
        ),
        adwaita_banner,
        clamp(
            620.0,
            preferences_group(
                Some("桌面行为设置"),
                Some("AdwClamp + AdwPreferencesGroup + AdwEntryRow + AdwComboRow + AdwExpanderRow"),
                vec![
                    entry_row(
                        "应用名称",
                        state.adw_entry_val.clone(),
                        |s: &mut GalleryState, v| s.adw_entry_val = v,
                    )
                    .boxed(),
                    combo_row(
                        "渲染后端",
                        Some("AdwComboRow 行内下拉选择器"),
                        &["Vello WGPU", "Vello CPU", "软件光栅化"],
                        state.adw_combo_idx,
                        state.adw_combo_open,
                        |s: &mut GalleryState| s.adw_combo_open = !s.adw_combo_open,
                        |s: &mut GalleryState, idx| {
                            s.adw_combo_idx = idx;
                            s.adw_combo_open = false;
                        },
                        |s: &mut GalleryState| s.adw_combo_open = false,
                    )
                    .boxed(),
                    switch_row(
                        "启用扁平化半透叠层",
                        Some("使用 7% 悬停 / 14% 按下透明遮罩反馈"),
                        state.native_switch_a,
                        |s: &mut GalleryState, v| s.native_switch_a = v,
                    )
                    .boxed(),
                    expander_row(
                        "开发者子设置",
                        Some("AdwExpanderRow 缩进子行列表"),
                        Some(icons::CODE),
                        state.adw_expander_open,
                        |s: &mut GalleryState| s.adw_expander_open = !s.adw_expander_open,
                        vec![
                            clickable_action_row(
                                "同步 GNOME 账号",
                                Some("直接返回 Pod<widgets::Button> 的可点击操作行"),
                                Some(icons::SETTINGS),
                                None::<xilem_ui::components::button::UiButton<GalleryState, ()>>,
                                |s: &mut GalleryState| s.click_count += 1,
                            )
                            .boxed(),
                        ],
                    )
                    .boxed(),
                ],
            ),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(10.px());

    flex_col((
        headline_small("平台原生套件与开放定制"),
        dev_control_card,
        md3_section,
        fluent_section,
        hig_section,
        adwaita_section,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
