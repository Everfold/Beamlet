//! Standalone GNOME Libadwaita Preferences & Status example using `xilem-adwaita` directly without `xilem-ui`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, portal, sized_box, zstack};
use xilem::{AppState, EventLoop, WidgetView, Xilem};
use xilem_adwaita::{
    AdwaitaAccent, AdwaitaTheme, Brightness, action_row, adwaita_window, banner, basic_dialog,
    clamp, clickable_action_row, combo_row, destructive_button, entry_row, expander_row,
    header_bar_with_title, icon_button, icons, preferences_group, status_page, switch_row,
    view_switcher,
};

struct PrefsApp {
    window_id: xilem::WindowId,
    running: bool,
    brightness: Brightness,
    accent: AdwaitaAccent,
    active_view: usize,
    banner_open: bool,
    flat_overlays: bool,
    telemetry_opt_in: bool,
    expander_open: bool,
    combo_open: bool,
    combo_idx: usize,
    app_title: String,
    show_wipe_dialog: bool,
}

impl AppState for PrefsApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for PrefsApp {
    fn default() -> Self {
        Self {
            window_id: xilem::WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            accent: AdwaitaAccent::Blue,
            active_view: 0,
            banner_open: true,
            flat_overlays: true,
            telemetry_opt_in: false,
            expander_open: true,
            combo_open: false,
            combo_idx: 0,
            app_title: "Beamlet Adwaita Studio".into(),
            show_wipe_dialog: false,
        }
    }
}

impl PrefsApp {
    fn current_theme(&self) -> AdwaitaTheme {
        AdwaitaTheme {
            brightness: self.brightness,
            accent: self.accent,
            ..AdwaitaTheme::light()
        }
    }
}

fn app_view(state: &mut PrefsApp) -> impl WidgetView<PrefsApp> + use<> {
    let hdr = header_bar_with_title(
        view_switcher(
            &[("首选项", icons::TUNE), ("运行状态", icons::EXPLORE)],
            state.active_view,
            |s: &mut PrefsApp, idx| s.active_view = idx,
        ),
        vec![
            icon_button(icons::PALETTE, |s: &mut PrefsApp| {
                s.accent = match s.accent {
                    AdwaitaAccent::Blue => AdwaitaAccent::Teal,
                    AdwaitaAccent::Teal => AdwaitaAccent::Green,
                    AdwaitaAccent::Green => AdwaitaAccent::Purple,
                    _ => AdwaitaAccent::Blue,
                };
            })
            .boxed(),
        ],
        vec![
            icon_button(icons::DARK_MODE, |s: &mut PrefsApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
        ],
    );

    let top_banner = state.banner_open.then(|| {
        banner(
            "AdwClamp 会自动将 boxed-list 表单可读宽度约束在 640px 以内。",
            Some("切换到状态页"),
            Some(Arc::new(|s: &mut PrefsApp| s.active_view = 1)),
            Some(Arc::new(|s: &mut PrefsApp| s.banner_open = false)),
        )
    });

    let page = if state.active_view == 0 {
        clamp(
            640.0,
            flex_col((
                preferences_group(
                    Some("应用标识与渲染"),
                    Some("在 boxed-list 卡片内组合 AdwEntryRow、AdwComboRow 与 AdwSwitchRow。"),
                    vec![
                        entry_row(
                            "工作区显示名称",
                            state.app_title.clone(),
                            |s: &mut PrefsApp, txt| s.app_title = txt,
                        )
                        .boxed(),
                        combo_row(
                            "GNOME 强调色预设",
                            Some("选择一组 GNOME 47 系统级强调色"),
                            &["经典蓝", "青绿", "翠绿", "优雅紫"],
                            state.combo_idx,
                            state.combo_open,
                            |s: &mut PrefsApp| s.combo_open = !s.combo_open,
                            |s: &mut PrefsApp, idx| {
                                s.combo_idx = idx;
                                s.accent = match idx {
                                    0 => AdwaitaAccent::Blue,
                                    1 => AdwaitaAccent::Teal,
                                    2 => AdwaitaAccent::Green,
                                    _ => AdwaitaAccent::Purple,
                                };
                                s.combo_open = false;
                            },
                            |s: &mut PrefsApp| s.combo_open = false,
                        )
                        .boxed(),
                        switch_row(
                            "Alpha 透明叠加反馈",
                            Some("使用 Adwaita 7% 悬停 / 14% 按下的扁平透明态遮罩"),
                            state.flat_overlays,
                            |s: &mut PrefsApp, v| s.flat_overlays = v,
                        )
                        .boxed(),
                    ],
                ),
                preferences_group(
                    Some("高级选项"),
                    None,
                    vec![
                        expander_row(
                            "诊断数据与日志",
                            Some("支持嵌套 AdwSwitchRow 与 AdwActionRow 的 AdwExpanderRow"),
                            Some(icons::INFO),
                            state.expander_open,
                            |s: &mut PrefsApp| s.expander_open = !s.expander_open,
                            vec![
                                switch_row(
                                    "分享匿名遥测指标",
                                    None::<&str>,
                                    state.telemetry_opt_in,
                                    |s: &mut PrefsApp, v| s.telemetry_opt_in = v,
                                )
                                .boxed(),
                                clickable_action_row(
                                    "导出支持诊断包",
                                    Some("直接返回 Pod<widgets::Button> 的可点击操作行"),
                                    Some(icons::DOWNLOAD),
                                    None::<xilem_adwaita::UiButton<PrefsApp, ()>>,
                                    |s: &mut PrefsApp| s.banner_open = true,
                                )
                                .boxed(),
                            ],
                        )
                        .boxed(),
                        action_row(
                            "危险操作区",
                            Some("演示在 AdwActionRow 内嵌入 destructive_button"),
                            Some(icons::WARNING),
                            Some(destructive_button("重置应用", |s: &mut PrefsApp| {
                                s.show_wipe_dialog = true;
                            })),
                            None,
                        )
                        .boxed(),
                    ],
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(18.px()),
        )
        .boxed()
    } else {
        clamp(
            640.0,
            status_page(
                icons::CHECK_CIRCLE,
                "所有系统运行正常",
                "由 xilem-adwaita 独立驱动的原生风格 GNOME Libadwaita 外壳。",
                Some(("返回首选项", Arc::new(|s: &mut PrefsApp| s.active_view = 0))),
            ),
        )
        .boxed()
    };

    let body = portal(sized_box(page).padding(Padding::all(24.px()))).constrain_horizontal(true);

    let main_col = flex_col((hdr, top_banner, body.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let wipe_modal = state.show_wipe_dialog.then(|| {
        basic_dialog(
            "重置 Adwaita 首选项？",
            "将恢复工作区显示名称及默认的 GNOME 经典蓝强调色。",
            "全部重置",
            |s: &mut PrefsApp| {
                s.app_title = "Beamlet Adwaita Studio".into();
                s.accent = AdwaitaAccent::Blue;
                s.combo_idx = 0;
                s.show_wipe_dialog = false;
            },
            |s: &mut PrefsApp| s.show_wipe_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("取消")
    });

    zstack((
        main_col.alignment(UnitPoint::CENTER),
        wipe_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = PrefsApp::default();
    let props = xilem_adwaita::build_default_properties(&initial.current_theme().to_spec());
    let app = Xilem::new(initial, |state: &mut PrefsApp| {
        let theme = state.current_theme();
        let wid = state.window_id;
        std::iter::once(
            adwaita_window(wid, "GNOME Libadwaita 首选项", &theme, app_view(state)).with_options(
                |opts| {
                    opts.on_close(|s: &mut PrefsApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(980.0, 740.0))
                },
            ),
        )
    })
    .with_default_properties(props);
    app.run_in(EventLoop::with_user_event())
        .expect("adwaita_preferences event loop exited cleanly");
}
