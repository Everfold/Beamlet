//! Platform-authentic workspace shells (Material 3, Fluent 2, macOS HIG, GNOME Adwaita).

use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, portal, sized_box};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui::prelude::*;

use crate::overlays::wrap_with_overlays;
use crate::pages;
use crate::state::{GalleryState, GalleryTab};

fn destinations() -> [NavDestination; 7] {
    GalleryTab::ALL.map(|tab| NavDestination::new(tab.title(), tab.icon()))
}

fn switcher_bar(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let lang_idx = DesignLanguage::ALL
        .iter()
        .position(|l| *l == state.theme.language)
        .unwrap_or(0);
    let accent_idx = UiAccent::PRESETS
        .iter()
        .position(|a| *a == state.theme.accent)
        .unwrap_or(0);
    let lang_titles = ["Material 3", "Windows Fluent", "macOS HIG", "GNOME Adwaita"];
    let accent_labels = [
        "系统默认",
        "蓝色",
        "青绿",
        "翠绿",
        "紫色",
        "粉色",
        "橙色",
        "红色",
    ];
    let is_dark = state.theme.brightness == Brightness::Dark;
    let is_hc = state.theme.high_contrast;
    let is_rm = state.theme.reduced_motion;

    sized_box(
        flex_row((
            segmented_button(&lang_titles, lang_idx, |s: &mut GalleryState, idx| {
                s.theme.language = DesignLanguage::ALL[idx];
            }),
            select(
                &accent_labels,
                accent_idx,
                state.accent_menu_open,
                |s: &mut GalleryState| s.accent_menu_open = !s.accent_menu_open,
                |s: &mut GalleryState, idx| {
                    s.theme.accent = UiAccent::PRESETS[idx];
                    s.accent_menu_open = false;
                },
                |s: &mut GalleryState| s.accent_menu_open = false,
            ),
            filter_chip("深色模式", is_dark, |s: &mut GalleryState| {
                s.theme.toggle_brightness();
            }),
            filter_chip("高对比度", is_hc, |s: &mut GalleryState| {
                s.theme.high_contrast = !s.theme.high_contrast;
            }),
            filter_chip("减少动效", is_rm, |s: &mut GalleryState| {
                s.theme.reduced_motion = !s.theme.reduced_motion;
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(10.px())
        .padding(Padding::from_vh(8.px(), 16.px())),
    )
}

fn active_page_body(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let page: Box<AnyWidgetView<GalleryState>> = match state.active_tab {
        GalleryTab::Home => pages::home_page(state).boxed(),
        GalleryTab::Layout => pages::layout_page(state).boxed(),
        GalleryTab::Components => pages::components_page(state).boxed(),
        GalleryTab::Overlays => pages::overlays_page(state).boxed(),
        GalleryTab::PlatformNative => pages::platform_native_page(state).boxed(),
        GalleryTab::PerfAndSpec => pages::perf_and_spec_page(state).boxed(),
        GalleryTab::Workbench => pages::workbench_page(state).boxed(),
    };
    portal(sized_box(page).padding(Padding::all(20.px()))).constrain_horizontal(true)
}

pub fn md3_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let rail = navigation_rail_with_fab(
        Some(small_fab(
            icons::ADD,
            FabColor::Tertiary,
            |s: &mut GalleryState| {
                s.components.click_count += 1;
            },
        )),
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| {
            s.active_tab = GalleryTab::from_index(i);
        },
    );
    let bar = top_app_bar_slots(
        TopAppBarVariant::Small,
        "Beamlet Material 3 展示台",
        None,
        vec![
            icon_button(icons::PALETTE, |s: &mut GalleryState| {
                s.theme.toggle_brightness();
            })
            .boxed(),
        ],
    );
    let main = flex_col((
        bar,
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((rail, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    let fab_view = flex_row((
        floating_toolbar(vec![
            icon_button(icons::DOCK_LEFT, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Layout;
            })
            .boxed(),
            icon_button(icons::TUNE, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::PerfAndSpec;
            })
            .boxed(),
        ]),
        extended_fab(
            icons::ADD,
            "新建操作",
            FabColor::Primary,
            |s: &mut GalleryState| {
                s.components.click_count += 1;
                s.overlays.toasts.show(
                    "已触发 Material 3 扩展悬浮按钮 (Extended FAB)",
                    Some("关闭"),
                    ToastDuration::Short,
                    true,
                );
            },
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(12.px())
    .dims(Dimensions::MAX)
    .boxed();
    wrap_with_overlays(state, workspace, Some(fab_view))
}

pub fn fluent_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let nav = navigation_view_collapsible(
        Some("WinUI 3 外壳"),
        &dests[..5],
        &dests[5..],
        state.active_tab.index(),
        state.native.nav_collapsed,
        |s: &mut GalleryState| s.native.nav_collapsed = !s.native.nav_collapsed,
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    let crumbs = ["Beamlet", "多主题展示台", state.active_tab.title()];
    let bc = breadcrumb_bar(&crumbs, |s: &mut GalleryState, idx| {
        if idx == 0 {
            s.active_tab = GalleryTab::Home;
        }
    });
    let cmd = command_bar_with_overflow(
        vec![
            primary_button("新建", |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .boxed(),
            subtle_button("计数清零", |s: &mut GalleryState| {
                s.components.click_count = 0;
            })
            .boxed(),
            outlined_button("打开弹窗", |s: &mut GalleryState| {
                s.overlays.show_basic_dialog = true;
            })
            .boxed(),
        ],
        vec![
            menu_item("前往布局系统"),
            menu_item("前往性能与令牌"),
            menu_item("切换明暗主题"),
        ],
        state.native.cmd_overflow_open,
        |s: &mut GalleryState| s.native.cmd_overflow_open = !s.native.cmd_overflow_open,
        |s: &mut GalleryState, idx| {
            match idx {
                0 => s.active_tab = GalleryTab::Layout,
                1 => s.active_tab = GalleryTab::PerfAndSpec,
                _ => s.theme.toggle_brightness(),
            }
            s.native.cmd_overflow_open = false;
        },
        |s: &mut GalleryState| s.native.cmd_overflow_open = false,
    );
    let notice = state.native.info_bar_visible.then(|| {
        info_bar(
            InfoBarSeverity::Informational,
            "Windows Fluent 可折叠 NavigationView + 面包屑导航 + 带溢出菜单的命令栏",
            "点击左上角汉堡按钮可折叠侧栏，底部固定页脚导航直达性能与工作台。",
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.native.info_bar_visible = false;
            })),
        )
    });
    let main = flex_col((
        switcher_bar(state),
        horizontal_divider(),
        sized_box(flex_col((bc, cmd, notice)).gap(8.px()))
            .padding(Padding::from_vh(8.px(), 16.px())),
        active_page_body(state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((nav, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

pub fn hig_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let sections = [
        (
            "核心与设计",
            vec![
                ("首页", icons::HOME),
                ("布局系统", icons::DOCK_LEFT),
                ("组件与输入", icons::EDIT),
                ("浮层与反馈", icons::MORE_HORIZ),
            ],
        ),
        (
            "平台与工程",
            vec![
                ("平台原生", icons::PALETTE),
                ("性能与令牌", icons::TUNE),
                ("工作台", icons::FOLDER),
            ],
        ),
    ];
    let idx = state.active_tab.index();
    let selected = if idx >= 4 { (1, idx - 4) } else { (0, idx) };
    let sidebar = source_list_sidebar(&sections, selected, |s: &mut GalleryState, (sec, row)| {
        let next = if sec == 1 { 4 + row } else { row };
        s.active_tab = GalleryTab::from_index(next);
    });
    let toolbar = macos_toolbar_with_center(
        vec![
            icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Home;
            })
            .boxed(),
        ],
        search_bar(
            state.components.search_query.clone(),
            |s: &mut GalleryState, val| {
                s.components.search_query = val;
            },
        )
        .on_clear(|s: &mut GalleryState| s.components.search_query.clear())
        .width(260.px()),
        vec![
            icon_button(icons::DOCK_LEFT, |s: &mut GalleryState| {
                s.native.show_hig_inspector = !s.native.show_hig_inspector;
            })
            .boxed(),
            help_button(|s: &mut GalleryState| s.overlays.show_platform_alert = true).boxed(),
        ],
    );
    let hig_path_items = [
        ("Macintosh HD", icons::FOLDER),
        ("Beamlet", icons::CODE),
        (state.active_tab.title(), icons::DESCRIPTION),
    ];
    let inspector = state.native.show_hig_inspector.then(|| {
        inspector_panel(
            "属性检查器",
            vec![
                form_group(
                    Some("当前状态"),
                    vec![
                        form_row(
                            "点击次数",
                            label_medium(format!("{}", state.components.click_count)),
                        )
                        .boxed(),
                        form_row(
                            "深色 Aqua",
                            switch(
                                state.theme.brightness == Brightness::Dark,
                                |s: &mut GalleryState, _| {
                                    s.theme.toggle_brightness();
                                },
                            ),
                        )
                        .boxed(),
                    ],
                    None,
                )
                .boxed(),
                disclosure_group(
                    "快捷键说明",
                    state.native.hig_disclosure_open,
                    |s: &mut GalleryState| {
                        s.native.hig_disclosure_open = !s.native.hig_disclosure_open;
                    },
                    body_small("保存：⌘S · 重做：⇧⌘Z"),
                )
                .boxed(),
            ],
        )
    });
    let main = flex_col((
        toolbar,
        switcher_bar(state),
        horizontal_divider(),
        active_page_body(state).flex(1.0),
        path_control(&hig_path_items, |s: &mut GalleryState, path_idx| {
            if path_idx == 0 {
                s.active_tab = GalleryTab::Home;
            }
        }),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace = flex_row((sidebar, main.flex(1.0), inspector))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

pub fn adwaita_shell(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let sidebar = navigation_drawer(
        Some("Beamlet"),
        &dests,
        state.active_tab.index(),
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    let hdr = header_bar_with_title(
        view_switcher(
            &[
                ("首页", icons::HOME),
                ("布局", icons::DOCK_LEFT),
                ("性能", icons::TUNE),
                ("工作台", icons::FOLDER),
            ],
            match state.active_tab {
                GalleryTab::Home => 0,
                GalleryTab::Layout => 1,
                GalleryTab::PerfAndSpec => 2,
                GalleryTab::Workbench => 3,
                _ => 0,
            },
            |s: &mut GalleryState, idx| {
                s.active_tab = match idx {
                    0 => GalleryTab::Home,
                    1 => GalleryTab::Layout,
                    2 => GalleryTab::PerfAndSpec,
                    _ => GalleryTab::Workbench,
                };
            },
        ),
        vec![
            icon_button(icons::MENU, |s: &mut GalleryState| {
                s.components.click_count += 1
            })
            .boxed(),
        ],
        vec![
            primary_button("原生警告框", |s: &mut GalleryState| {
                s.overlays.show_platform_alert = true;
            })
            .boxed(),
        ],
    );
    let top_banner = state.native.banner_visible.then(|| {
        banner(
            "GNOME Adwaita HeaderBar + ViewSwitcher + AdwClamp(960px) 限宽布局",
            Some("隐藏"),
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.native.banner_visible = false;
            })),
            None,
        )
    });
    let main = flex_col((
        hdr,
        top_banner,
        switcher_bar(state),
        horizontal_divider(),
        clamp(960.0, active_page_body(state)).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((sidebar, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}
