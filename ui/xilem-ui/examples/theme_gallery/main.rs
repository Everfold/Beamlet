//! Multi-theme showcase gallery demonstrating shared state alongside distinct per-platform native layouts.

#![warn(clippy::print_stdout, clippy::print_stderr)]

mod pages;
mod state;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};
use xilem::{AnyWidgetView, AppState, EventLoop, WidgetView, Xilem};
use xilem_ui::prelude::*;

use state::{GalleryState, GalleryTab};

impl AppState for GalleryState {
    fn keep_running(&self) -> bool {
        self.running
    }
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
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
        .padding(Padding::from_vh(8.px(), 16.px())),
    )
}

fn active_page_body(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let page: Box<AnyWidgetView<GalleryState>> = match state.active_tab {
        GalleryTab::Overview => pages::overview_page(state).boxed(),
        GalleryTab::Inputs => pages::inputs_page(state).boxed(),
        GalleryTab::Overlays => pages::overlays_page(state).boxed(),
        GalleryTab::Tokens => pages::tokens_page(state).boxed(),
        GalleryTab::PlatformNative => pages::platform_native_page(state).boxed(),
        GalleryTab::Workspace => pages::workspace_demo_page(state).boxed(),
    };
    portal(sized_box(page).padding(Padding::all(20.px()))).constrain_horizontal(true)
}

fn wrap_with_overlays<V: WidgetView<GalleryState>>(
    state: &mut GalleryState,
    workspace: V,
    floating_action: Option<Box<AnyWidgetView<GalleryState>>>,
) -> impl WidgetView<GalleryState, Widget = widgets::ZStack> + use<V> {
    let dialog_overlay = state.show_basic_dialog.then(|| {
        basic_dialog(
            "重置主题偏好设置？",
            "将恢复宿主系统默认的设计语言与强调色预设。",
            "恢复默认",
            |s: &mut GalleryState| {
                s.theme = UiTheme::native_light();
                s.show_basic_dialog = false;
            },
            |s: &mut GalleryState| s.show_basic_dialog = false,
        )
        .icon(icons::PALETTE)
        .dismiss_text("取消")
    });

    let destructive_overlay = state.show_destructive_dialog.then(|| {
        basic_dialog(
            "移除选中的工作区目标？",
            "将从展示列表中移除当前选中的工作区目标条目。",
            "确认移除",
            |s: &mut GalleryState| {
                if !s.workspace_items.is_empty() {
                    let idx = s
                        .selected_workspace_idx
                        .min(s.workspace_items.len().saturating_sub(1));
                    s.workspace_items.remove(idx);
                    s.selected_workspace_idx = 0;
                }
                s.show_destructive_dialog = false;
            },
            |s: &mut GalleryState| s.show_destructive_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("取消")
    });

    let date_overlay = state.show_date_dialog.then(|| {
        date_picker_dialog(
            state.selected_date,
            |s: &mut GalleryState, d| s.selected_date = d,
            |s: &mut GalleryState| s.show_date_dialog = false,
            |s: &mut GalleryState| s.show_date_dialog = false,
        )
    });

    let time_overlay = state.show_time_dialog.then(|| {
        time_picker_dialog(
            state.selected_time,
            |s: &mut GalleryState, t| s.selected_time = t,
            |s: &mut GalleryState| s.show_time_dialog = false,
            |s: &mut GalleryState| s.show_time_dialog = false,
        )
    });

    let sheet_overlay = state.show_bottom_sheet.then(|| {
        bottom_sheet(
            flex_col((
                headline_small("底部抽屉面板"),
                body_medium("通过 SlideTransition + EscapeCatcher 实现的共享模态底部抽屉。"),
                primary_button("关闭抽屉", |s: &mut GalleryState| {
                    s.show_bottom_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.show_bottom_sheet = false,
        )
    });

    let side_sheet_overlay = state.show_side_sheet.then(|| {
        let selected_title = state
            .workspace_items
            .get(state.selected_workspace_idx)
            .map_or("未选中任何项目", |(t, _, _)| *t);
        side_sheet(
            flex_col((
                headline_small("目标详情侧边抽屉"),
                title_medium(selected_title),
                body_medium("由 modal_shell + EscapeCatcher 驱动的右侧检查器抽屉。"),
                primary_button("关闭侧边抽屉", |s: &mut GalleryState| {
                    s.show_side_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.show_side_sheet = false,
        )
        .extent(360.0)
    });

    let fab_layer = floating_action.map(|f| {
        sized_box(f)
            .padding(Padding::all(20.px()))
            .alignment(UnitPoint::BOTTOM_RIGHT)
    });
    let toast_layer = sized_box(
        toast_host(&state.toasts, |s: &mut GalleryState| {
            let _ = s.toasts.dismiss_current();
        })
        .on_action(|s: &mut GalleryState| {
            let _ = s.toasts.dismiss_current();
        }),
    )
    .padding(Padding::all(16.px()))
    .alignment(UnitPoint::BOTTOM);

    zstack((
        workspace.alignment(UnitPoint::TOP_LEFT),
        fab_layer,
        toast_layer,
        sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        side_sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        date_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        time_overlay.map(|t| t.alignment(UnitPoint::CENTER)),
        dialog_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        destructive_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn destinations() -> [NavDestination; 6] {
    [
        NavDestination::new("概览", icons::HOME),
        NavDestination::new("输入控件", icons::EDIT),
        NavDestination::new("浮层组件", icons::MORE_HORIZ),
        NavDestination::new("设计令牌", icons::TUNE),
        NavDestination::new("原生套件", icons::PALETTE),
        NavDestination::new("工作台", icons::FOLDER),
    ]
}

fn md3_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let rail = navigation_rail_with_fab(
        Some(small_fab(
            icons::ADD,
            FabColor::Tertiary,
            |s: &mut GalleryState| {
                s.click_count += 1;
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
                s.theme.toggle_brightness()
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
            icon_button(icons::EDIT, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Inputs;
            })
            .boxed(),
            icon_button(icons::TUNE, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Tokens;
            })
            .boxed(),
        ]),
        extended_fab(
            icons::ADD,
            "新建操作",
            FabColor::Primary,
            |s: &mut GalleryState| {
                s.click_count += 1;
                s.toasts.show(
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

fn fluent_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let dests = destinations();
    let nav = navigation_view(
        Some("WinUI 3 外壳"),
        &dests[..4],
        &dests[4..],
        state.active_tab.index(),
        |s: &mut GalleryState, i| s.active_tab = GalleryTab::from_index(i),
    );
    let tab_title = match state.active_tab {
        GalleryTab::Overview => "概览",
        GalleryTab::Inputs => "输入控件",
        GalleryTab::Overlays => "浮层组件",
        GalleryTab::Tokens => "设计令牌",
        GalleryTab::PlatformNative => "原生套件",
        GalleryTab::Workspace => "工作台",
    };
    let crumbs = ["Beamlet", "多主题展示台", tab_title];
    let bc = breadcrumb_bar(&crumbs, |s: &mut GalleryState, idx| {
        if idx == 0 {
            s.active_tab = GalleryTab::Overview;
        }
    });
    let cmd = command_bar(
        vec![
            primary_button("新建", |s: &mut GalleryState| s.click_count += 1).boxed(),
            subtle_button("计数清零", |s: &mut GalleryState| s.click_count = 0).boxed(),
        ],
        vec![
            outlined_button("打开弹窗", |s: &mut GalleryState| {
                s.show_basic_dialog = true;
            })
            .boxed(),
        ],
    );
    let notice = state.info_bar_visible.then(|| {
        info_bar(
            InfoBarSeverity::Informational,
            "Windows Fluent 导航视图 + 面包屑导航 + 命令栏",
            "注意底部的页脚固定导航项，以及 Windows 惯例下“主确认按钮居左 (PrimaryLeading)”的弹窗排布。",
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.info_bar_visible = false;
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

fn hig_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let sections = [
        (
            "基础组件",
            vec![
                ("概览", icons::HOME),
                ("输入控件", icons::EDIT),
                ("浮层组件", icons::MORE_HORIZ),
                ("设计令牌", icons::TUNE),
            ],
        ),
        (
            "平台能力",
            vec![("原生套件", icons::PALETTE), ("工作台", icons::FOLDER)],
        ),
    ];
    let selected = match state.active_tab {
        GalleryTab::PlatformNative => (1, 0),
        GalleryTab::Workspace => (1, 1),
        other => (0, other.index()),
    };
    let sidebar = source_list_sidebar(&sections, selected, |s: &mut GalleryState, (sec, row)| {
        let idx = if sec == 1 { 4 + row } else { row };
        s.active_tab = GalleryTab::from_index(idx);
    });
    let toolbar = macos_toolbar_with_center(
        vec![
            icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                s.active_tab = GalleryTab::Overview;
            })
            .boxed(),
        ],
        search_bar(state.search_query.clone(), |s: &mut GalleryState, val| {
            s.search_query = val;
        })
        .on_clear(|s: &mut GalleryState| s.search_query.clear())
        .width(260.px()),
        vec![
            icon_button(icons::DOCK_LEFT, |s: &mut GalleryState| {
                s.show_hig_inspector = !s.show_hig_inspector;
            })
            .boxed(),
            help_button(|s: &mut GalleryState| s.show_basic_dialog = true).boxed(),
        ],
    );
    let hig_path_items = [
        ("Macintosh HD", icons::FOLDER),
        ("Beamlet", icons::CODE),
        ("组件展示台", icons::DESCRIPTION),
    ];
    let inspector = state.show_hig_inspector.then(|| {
        inspector_panel(
            "属性检查器",
            vec![
                form_group(
                    Some("当前状态"),
                    vec![
                        form_row("点击次数", label_medium(format!("{}", state.click_count)))
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
                    state.hig_disclosure_open,
                    |s: &mut GalleryState| s.hig_disclosure_open = !s.hig_disclosure_open,
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
        path_control(&hig_path_items, |s: &mut GalleryState, idx| {
            if idx == 0 {
                s.active_tab = GalleryTab::Overview;
            }
        }),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace = flex_row((sidebar, main.flex(1.0), inspector))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

fn adwaita_workspace(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
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
                ("概览", icons::HOME),
                ("令牌", icons::TUNE),
                ("原生", icons::PALETTE),
                ("工作台", icons::FOLDER),
            ],
            match state.active_tab {
                GalleryTab::Overview => 0,
                GalleryTab::Tokens => 1,
                GalleryTab::PlatformNative => 2,
                GalleryTab::Workspace => 3,
                _ => 0,
            },
            |s: &mut GalleryState, idx| {
                s.active_tab = match idx {
                    0 => GalleryTab::Overview,
                    1 => GalleryTab::Tokens,
                    2 => GalleryTab::PlatformNative,
                    _ => GalleryTab::Workspace,
                };
            },
        ),
        vec![icon_button(icons::MENU, |s: &mut GalleryState| s.click_count += 1).boxed()],
        vec![
            primary_button("弹窗", |s: &mut GalleryState| {
                s.show_basic_dialog = true;
            })
            .boxed(),
        ],
    );
    let top_banner = state.banner_visible.then(|| {
        banner(
            "GNOME Adwaita SplitView 分栏 + AdwViewSwitcher 切换器 + AdwClamp(880px) 限宽布局",
            Some("隐藏"),
            Some(std::sync::Arc::new(|s: &mut GalleryState| {
                s.banner_visible = false;
            })),
            None,
        )
    });
    let main = flex_col((
        hdr,
        top_banner,
        switcher_bar(state),
        horizontal_divider(),
        clamp(880.0, active_page_body(state)).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let workspace =
        flex_row((sidebar, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrap_with_overlays(state, workspace, None)
}

fn app_view(state: &mut GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    // Explicit developer-owned dispatch: shared state/pages, platform-authentic workspaces.
    match state.theme.language {
        DesignLanguage::Md3 => md3_workspace(state).boxed(),
        DesignLanguage::Fluent => fluent_workspace(state).boxed(),
        DesignLanguage::Hig => hig_workspace(state).boxed(),
        DesignLanguage::Adwaita => adwaita_workspace(state).boxed(),
    }
}

fn main() {
    let initial = GalleryState::default();
    let initial_props = initial.theme.default_properties();
    let app = Xilem::new(initial, |state: &mut GalleryState| {
        let wid = state.window_id;
        let theme = state.theme.clone();
        let title = format!("Beamlet 多主题 UI 展示台 ({})", theme.language.label());
        let view = app_view(state);
        std::iter::once(ui_window(wid, title, &theme, view).with_options(|opts| {
            opts.on_close(|s: &mut GalleryState| s.running = false)
                .with_initial_inner_size(LogicalSize::new(1180.0, 800.0))
        }))
    })
    .with_default_properties(initial_props);
    app.run_in(EventLoop::with_user_event())
        .expect("theme_gallery event loop exited cleanly");
}
