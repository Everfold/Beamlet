//! Standalone Windows 11 / WinUI 3 Settings example using `brocado-fluent` directly without `brocado`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use brocado_fluent::{
    Brightness, FluentAccent, FluentTheme, InfoBarSeverity, NavDestination, basic_dialog,
    body_small, breadcrumb_bar, command_bar_with_overflow, expander, headline_small,
    horizontal_divider, icons, info_badge, info_bar, menu_item, navigation_view_collapsible,
    number_box, primary_button, settings_card, settings_expander, subtle_button, switch,
};
use brocado_winit::{AppRunner, AppState, WindowId, desktop_window};
use masonry::dpi::LogicalSize;
use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::Padding;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};

struct SettingsApp {
    window_id: WindowId,
    running: bool,
    brightness: Brightness,
    accent: FluentAccent,
    nav_idx: usize,
    nav_collapsed: bool,
    overflow_open: bool,
    mica_enabled: bool,
    auto_updates: bool,
    expander_open: bool,
    custom_expander_open: bool,
    worker_threads: f64,
    show_banner: bool,
    show_reset_dialog: bool,
}

impl AppState for SettingsApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for SettingsApp {
    fn default() -> Self {
        Self {
            window_id: WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            accent: FluentAccent::WindowsBlue,
            nav_idx: 0,
            nav_collapsed: false,
            overflow_open: false,
            mica_enabled: true,
            auto_updates: true,
            expander_open: true,
            custom_expander_open: false,
            worker_threads: 12.0,
            show_banner: true,
            show_reset_dialog: false,
        }
    }
}

impl SettingsApp {
    fn current_theme(&self) -> FluentTheme {
        FluentTheme {
            brightness: self.brightness,
            accent: self.accent,
            ..FluentTheme::light()
        }
    }
}

fn app_view(state: &mut SettingsApp) -> impl WidgetView<SettingsApp> + use<> {
    let primary_nav = [
        NavDestination::new("系统", icons::SETTINGS),
        NavDestination::new("个性化", icons::PALETTE),
        NavDestination::new("开发者", icons::CODE),
    ];
    let footer_nav = [NavDestination::new("Windows 更新", icons::REFRESH).with_badge("1")];

    let nav = navigation_view_collapsible(
        Some("Windows 设置"),
        &primary_nav,
        &footer_nav,
        state.nav_idx,
        state.nav_collapsed,
        |s: &mut SettingsApp| s.nav_collapsed = !s.nav_collapsed,
        |s: &mut SettingsApp, i| s.nav_idx = i,
    );

    let page_title = match state.nav_idx {
        0 => "系统",
        1 => "个性化",
        2 => "开发者",
        _ => "Windows 更新",
    };

    let crumbs = ["设置", page_title, "显示与性能"];
    let bc = breadcrumb_bar(&crumbs, |s: &mut SettingsApp, idx| {
        if idx == 0 {
            s.nav_idx = 0;
        }
    });

    let cmd = command_bar_with_overflow(
        vec![
            primary_button("切换深色 Mica", |s: &mut SettingsApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
            subtle_button("切换强调色", |s: &mut SettingsApp| {
                s.accent = match s.accent {
                    FluentAccent::WindowsBlue => FluentAccent::Teal,
                    FluentAccent::Teal => FluentAccent::Purple,
                    FluentAccent::Purple => FluentAccent::Green,
                    _ => FluentAccent::WindowsBlue,
                };
            })
            .boxed(),
        ],
        vec![
            menu_item("折叠/展开导航栏").leading_icon(icons::MENU),
            menu_item("恢复默认设置").leading_icon(icons::REFRESH),
        ],
        state.overflow_open,
        |s: &mut SettingsApp| s.overflow_open = !s.overflow_open,
        |s: &mut SettingsApp, idx| {
            s.overflow_open = false;
            match idx {
                0 => s.nav_collapsed = !s.nav_collapsed,
                1 => s.show_reset_dialog = true,
                _ => {}
            }
        },
        |s: &mut SettingsApp| s.overflow_open = false,
    );

    let banner_opt = state.show_banner.then(|| {
        info_bar(
            InfoBarSeverity::Informational,
            "Fluent 2 系统外壳已就绪",
            "完整展示 NavigationView 底部固定项、BreadcrumbBar 面包屑、InfoBadge 徽章以及 PrimaryLeading 弹窗按钮序。",
            Some(Arc::new(|s: &mut SettingsApp| s.show_banner = false)),
        )
    });

    let body = portal(
        sized_box(
            flex_col((
                headline_small(page_title),
                cmd,
                banner_opt,
                settings_card(
                    Some(icons::PALETTE),
                    "Mica 云母背景材质效果",
                    Some("根据浅色 #F3F3F3 与深色 #202020 自动适配桌面分层底色"),
                    flex_row((
                        info_badge(InfoBarSeverity::Success, Some("Mica")),
                        switch(state.mica_enabled, |s: &mut SettingsApp, v| {
                            s.mica_enabled = v;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                ),
                settings_card(
                    Some(icons::TUNE),
                    "后台编译工作线程",
                    Some("受范围约束的 WinUI 3 NumberBox 微调控件"),
                    number_box(
                        state.worker_threads,
                        1.0,
                        64.0,
                        1.0,
                        |s: &mut SettingsApp, next| s.worker_threads = next,
                    ),
                ),
                settings_expander(
                    Some(icons::SETTINGS),
                    "Windows 更新与诊断",
                    Some("自动下载重要安全更新"),
                    state.expander_open,
                    |s: &mut SettingsApp| s.expander_open = !s.expander_open,
                    Some(switch(state.auto_updates, |s: &mut SettingsApp, v| {
                        s.auto_updates = v;
                    })),
                    vec![
                        body_small("WinUI 3 SettingsExpander 可动态展开或收起嵌套子设置行。")
                            .boxed(),
                    ],
                ),
                expander(
                    "实验性构建选项",
                    Some("独立 WinUI 3 Expander 容器，支持包裹任意自定义视图"),
                    state.custom_expander_open,
                    |s: &mut SettingsApp| s.custom_expander_open = !s.custom_expander_open,
                    flex_row((
                        info_badge(InfoBarSeverity::Warning, Some("预览版")),
                        body_small("自定义嵌套诊断信息负载内容。"),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(12.px()),
        )
        .padding(Padding::all(20.px())),
    )
    .constrain_horizontal(true);

    let main = flex_col((
        sized_box(bc).padding(Padding::from_vh(8.px(), 16.px())),
        horizontal_divider(),
        body.flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let shell = flex_row((nav, main.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);

    let reset_modal = state.show_reset_dialog.then(|| {
        basic_dialog(
            "重置 Fluent 设置？",
            "Windows Fluent 规范将主确认按钮放在取消按钮左侧 (`DialogActionOrder::PrimaryLeading`)。",
            "立即重置",
            |s: &mut SettingsApp| {
                s.brightness = Brightness::Light;
                s.accent = FluentAccent::WindowsBlue;
                s.worker_threads = 12.0;
                s.show_reset_dialog = false;
            },
            |s: &mut SettingsApp| s.show_reset_dialog = false,
        )
        .destructive(true)
        .dismiss_text("取消")
    });

    zstack((
        shell.alignment(UnitPoint::CENTER),
        reset_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = SettingsApp::default();
    let props = brocado_fluent::build_default_properties(&initial.current_theme().to_spec());
    AppRunner::new(initial, |state: &mut SettingsApp| {
        let spec = state.current_theme().to_spec();
        let wid = state.window_id;
        std::iter::once(
            desktop_window(wid, "WinUI 3 Fluent 设置", &spec, app_view(state)).with_config(|cfg| {
                cfg.on_close(|s: &mut SettingsApp| s.running = false)
                    .with_initial_inner_size(LogicalSize::new(1080.0, 740.0))
            }),
        )
    })
    .with_default_properties(props)
    .run()
    .expect("fluent_settings event loop exited cleanly");
}
