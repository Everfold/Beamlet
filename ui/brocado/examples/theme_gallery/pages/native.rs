//! Active platform-native signature widgets + open custom slot card.

use brocado::components::typography::ui_label;
use brocado::prelude::*;
use masonry::layout::AsUnit;
use masonry::parley::LineHeight;
use masonry::peniko::Color;
use masonry::properties::{LineBreaking, Padding};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, sized_box};

use crate::state::{GalleryState, GalleryTab};

pub fn native_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let clicks = state.components.click_count;

    let make_custom_items = |wide: bool| {
        let save_btn = button_view(
            ButtonVariant::Primary,
            with_spec(|spec, _| {
                let shortcut = spec.shortcut_convention.format_primary("S");
                let text = clip_box(
                    ui_label(TypeRole::LabelLarge, "保存")
                        .color(spec.palette.on_accent)
                        .line_breaking(LineBreaking::Clip),
                );
                clip_box(
                    flex_row((
                        ui_icon(icons::CHECK, 16.0, spec.palette.on_accent),
                        text.flex(1.0),
                        sized_box(
                            ui_label(TypeRole::LabelSmall, shortcut)
                                .line_height(LineHeight::MetricsRelative(1.0))
                                .color(spec.palette.on_accent)
                                .line_breaking(LineBreaking::Clip),
                        )
                        .padding(Padding::from_vh(3.px(), 6.px()))
                        .border(spec.palette.on_accent, 1.px())
                        .corner_radius(4.px()),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                )
            }),
            |s: &mut GalleryState| s.components.click_count += 1,
        )
        .padding(Padding::from_vh(8.px(), 12.px()));
        let wide_btn_base =
            primary_button(format!("宽方角 ({clicks})"), |s: &mut GalleryState| {
                s.components.click_count += 1;
            })
            .corner_radius(3.px());
        let wide_btn: Box<AnyWidgetView<GalleryState>> = if wide {
            wide_btn_base.width(150.px()).boxed()
        } else {
            wide_btn_base.boxed()
        };
        let list_row = list_item_view(
            flex_row((
                ui_icon(icons::INFO, 16.0, Color::from_rgb8(0x4E, 0x8C, 0xE8)),
                body_medium("list_item_view").flex(1.0),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
        );
        (save_btn, wide_btn, list_row)
    };
    let (sb_w, wb_w, lr_w) = make_custom_items(true);
    let (sb_n, wb_n, lr_n) = make_custom_items(false);

    let custom_card = outlined_card(
        flex_col((
            title_medium("自定义插槽与样式覆写"),
            breakpoint_bin(
                520.0,
                flex_row((sb_w, wb_w, lr_w))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                flex_col((sb_n, wb_n, lr_n))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(8.px()),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let suite: Box<AnyWidgetView<GalleryState>> = match state.theme.language {
        DesignLanguage::Md3 => {
            let make_fabs = || {
                (
                    small_fab(icons::EDIT, FabColor::Surface, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    }),
                    fab(icons::ADD, FabColor::Primary, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    }),
                    large_fab(icons::SEND, FabColor::Tertiary, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    }),
                    extended_fab(
                        icons::EDIT,
                        "撰写",
                        FabColor::Secondary,
                        |s: &mut GalleryState| s.components.click_count += 1,
                    ),
                )
            };
            let (sf_w, f_w, lf_w, ef_w) = make_fabs();
            let (sf_n, f_n, lf_n, ef_n) = make_fabs();
            filled_card(
                flex_col((
                    title_medium("Material 3"),
                    top_app_bar_slots(
                        TopAppBarVariant::CenterAligned,
                        "TopAppBar",
                        Some(
                            icon_button(icons::MENU, |s: &mut GalleryState| {
                                s.components.click_count += 1;
                            })
                            .boxed(),
                        ),
                        vec![
                            icon_button(icons::SEARCH, |s: &mut GalleryState| {
                                s.components.click_count += 1;
                            })
                            .boxed(),
                        ],
                    ),
                    breakpoint_bin(
                        340.0,
                        flex_row((sf_w, f_w, lf_w, ef_w))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(12.px()),
                        wrap((sf_n, f_n, lf_n, ef_n))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(8.px())
                            .run_gap(8.px()),
                    ),
                    floating_toolbar(vec![
                        icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                            s.components.click_count += 1;
                        })
                        .boxed(),
                        icon_button(icons::COPY, |s: &mut GalleryState| {
                            s.components.click_count += 1;
                        })
                        .boxed(),
                        primary_button("发布", |s: &mut GalleryState| {
                            s.components.click_count += 1;
                        })
                        .boxed(),
                    ]),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
            )
            .boxed()
        }

        DesignLanguage::Fluent => {
            let crumbs = ["设置", "个性化", "颜色", "强调色"];
            let depth = (state.native.fluent_breadcrumb_depth + 1).clamp(1, crumbs.len());
            let info = state.native.info_bar_visible.then(|| {
                info_bar(
                    InfoBarSeverity::Success,
                    "InfoBar",
                    "Fluent 2 云母与状态提示。",
                    Some(std::sync::Arc::new(|s: &mut GalleryState| {
                        s.native.info_bar_visible = false;
                    })),
                )
            });
            flex_col((
                title_medium("Windows Fluent 2"),
                breadcrumb_bar(&crumbs[..depth], |s: &mut GalleryState, idx| {
                    s.native.fluent_breadcrumb_depth = idx;
                }),
                info,
                command_bar_with_overflow(
                    vec![
                        primary_button("保存", |s: &mut GalleryState| {
                            s.components.click_count += 1;
                        })
                        .boxed(),
                        subtle_button("重置", |s: &mut GalleryState| {
                            s.components.click_count = 0;
                        })
                        .boxed(),
                    ],
                    vec![menu_item("导出"), menu_item("恢复默认")],
                    state.native.cmd_overflow_open,
                    |s: &mut GalleryState| {
                        s.native.cmd_overflow_open = !s.native.cmd_overflow_open;
                    },
                    |s: &mut GalleryState, _| {
                        s.components.click_count += 1;
                        s.native.cmd_overflow_open = false;
                    },
                    |s: &mut GalleryState| s.native.cmd_overflow_open = false,
                ),
                settings_card(
                    Some(icons::TUNE),
                    "NumberBox",
                    Some("支持按钮、键盘、滚轮"),
                    number_box(
                        state.native.fluent_number_val,
                        0.0,
                        100.0,
                        4.0,
                        |s: &mut GalleryState, v| s.native.fluent_number_val = v,
                    ),
                ),
                expander(
                    "Expander",
                    None::<&str>,
                    state.native.fluent_expander_open,
                    |s: &mut GalleryState| {
                        s.native.fluent_expander_open = !s.native.fluent_expander_open;
                    },
                    wrap((
                        info_badge(InfoBarSeverity::Warning, Some("展开内容")),
                        body_small("Fluent Expander 详情"),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px())
                    .run_gap(6.px()),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(10.px())
            .boxed()
        }

        DesignLanguage::Hig => {
            let paths = [
                ("Macintosh HD", icons::FOLDER),
                ("Beamlet", icons::CODE),
                ("UI", icons::DESCRIPTION),
            ];
            let depth = (state.native.hig_path_depth + 1).clamp(1, paths.len());
            flex_col((
                title_medium("macOS HIG"),
                macos_toolbar(
                    "检查器",
                    vec![
                        icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                            s.components.click_count += 1;
                        })
                        .boxed(),
                    ],
                    vec![
                        help_button(|s: &mut GalleryState| {
                            s.overlays.show_platform_alert = true;
                        })
                        .boxed(),
                    ],
                ),
                form_group(
                    Some("外观"),
                    vec![
                        form_row(
                            "图形加速",
                            switch(state.native.native_switch, |s: &mut GalleryState, v| {
                                s.native.native_switch = v;
                            }),
                        )
                        .boxed(),
                        form_row(
                            "弹出菜单",
                            popup_button(
                                &["自动", "Metal", "CPU"],
                                state.native.hig_popup_idx,
                                state.native.hig_popup_open,
                                |s: &mut GalleryState| {
                                    s.native.hig_popup_open = !s.native.hig_popup_open;
                                },
                                |s: &mut GalleryState, idx| {
                                    s.native.hig_popup_idx = idx;
                                    s.native.hig_popup_open = false;
                                },
                                |s: &mut GalleryState| s.native.hig_popup_open = false,
                            ),
                        )
                        .boxed(),
                        form_row(
                            "步进器",
                            stepper(
                                "栏数",
                                state.native.hig_stepper_val,
                                1,
                                16,
                                1,
                                |s: &mut GalleryState, v| s.native.hig_stepper_val = v,
                            ),
                        )
                        .boxed(),
                    ],
                    None,
                ),
                disclosure_group(
                    "详情",
                    state.native.hig_disclosure_open,
                    |s: &mut GalleryState| {
                        s.native.hig_disclosure_open = !s.native.hig_disclosure_open;
                    },
                    body_small("⌘S / ⇧⌘Z"),
                ),
                path_control(&paths[..depth], |s: &mut GalleryState, idx| {
                    s.native.hig_path_depth = idx;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(10.px())
            .boxed()
        }

        DesignLanguage::Adwaita => {
            let adw_banner = state.native.banner_visible.then(|| {
                banner(
                    "AdwBanner",
                    Some("关闭"),
                    Some(std::sync::Arc::new(|s: &mut GalleryState| {
                        s.native.banner_visible = false;
                    })),
                    None,
                )
            });
            let wide_header = header_bar_with_title(
                view_switcher(
                    &[("组件", icons::EDIT), ("平台", icons::PALETTE)],
                    1,
                    |s: &mut GalleryState, idx| {
                        s.active_tab = if idx == 0 {
                            GalleryTab::Components
                        } else {
                            GalleryTab::Native
                        };
                    },
                ),
                vec![],
                vec![
                    primary_button("警告框", |s: &mut GalleryState| {
                        s.overlays.show_platform_alert = true;
                    })
                    .boxed(),
                ],
            );
            let compact_header = header_bar(
                "平台",
                None::<&str>,
                vec![],
                vec![
                    primary_button("警告框", |s: &mut GalleryState| {
                        s.overlays.show_platform_alert = true;
                    })
                    .boxed(),
                ],
            );
            flex_col((
                title_medium("GNOME Adwaita"),
                breakpoint_bin(340.0, wide_header, compact_header),
                adw_banner,
                view_switcher_bar(
                    &[
                        ("动态", icons::NOTIFICATIONS),
                        ("探索", icons::SEARCH),
                        ("设置", icons::SETTINGS),
                    ],
                    state.native.adw_switcher_idx,
                    |s: &mut GalleryState, idx| s.native.adw_switcher_idx = idx,
                ),
                preferences_group(
                    Some("PreferencesGroup"),
                    None::<&str>,
                    vec![
                        entry_row(
                            "名称",
                            state.native.adw_entry_val.clone(),
                            |s: &mut GalleryState, v| s.native.adw_entry_val = v,
                        )
                        .boxed(),
                        combo_row(
                            "后端",
                            None::<&str>,
                            &["WGPU", "CPU", "Software"],
                            state.native.adw_combo_idx,
                            state.native.adw_combo_open,
                            |s: &mut GalleryState| {
                                s.native.adw_combo_open = !s.native.adw_combo_open;
                            },
                            |s: &mut GalleryState, idx| {
                                s.native.adw_combo_idx = idx;
                                s.native.adw_combo_open = false;
                            },
                            |s: &mut GalleryState| s.native.adw_combo_open = false,
                        )
                        .boxed(),
                        switch_row(
                            "半透叠层",
                            None::<&str>,
                            state.native.native_switch,
                            |s: &mut GalleryState, v| s.native.native_switch = v,
                        )
                        .boxed(),
                        expander_row(
                            "更多操作",
                            None::<&str>,
                            Some(icons::CODE),
                            state.native.adw_expander_open,
                            |s: &mut GalleryState| {
                                s.native.adw_expander_open = !s.native.adw_expander_open;
                            },
                            vec![
                                clickable_action_row(
                                    "同步",
                                    None::<&str>,
                                    Some(icons::SETTINGS),
                                    None::<brocado::components::button::UiButton<GalleryState, ()>>,
                                    |s: &mut GalleryState| s.components.click_count += 1,
                                )
                                .boxed(),
                            ],
                        )
                        .boxed(),
                    ],
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(10.px())
            .boxed()
        }
    };

    flex_col((suite, custom_card))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
