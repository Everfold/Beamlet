//! Standalone macOS Aqua Inspector example using `xilem-hig` directly without `xilem-ui`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};
use xilem::{AppState, EventLoop, WidgetView, Xilem};
use xilem_hig::{
    Brightness, HigAccent, HigTheme, basic_dialog, body_medium, body_small, disclosure_group,
    form_group, form_row, headline_small, help_button, hig_window, icon_button, icons,
    inspector_panel, label_medium, macos_toolbar_with_center, path_control, primary_button,
    search_bar, segmented_button, source_list_sidebar, stepper, switch, text_field,
};

struct InspectorApp {
    window_id: xilem::WindowId,
    running: bool,
    brightness: Brightness,
    accent: HigAccent,
    sidebar_sel: (usize, usize),
    search: String,
    title_input: String,
    grid_snap: bool,
    live_preview: bool,
    columns: i32,
    inspector_visible: bool,
    metadata_expanded: bool,
    path_idx: usize,
    show_help_dialog: bool,
}

impl AppState for InspectorApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for InspectorApp {
    fn default() -> Self {
        Self {
            window_id: xilem::WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            accent: HigAccent::Blue,
            sidebar_sel: (0, 0),
            search: String::new(),
            title_input: "Aqua 工作区文档".into(),
            grid_snap: true,
            live_preview: true,
            columns: 4,
            inspector_visible: true,
            metadata_expanded: true,
            path_idx: 2,
            show_help_dialog: false,
        }
    }
}

impl InspectorApp {
    fn current_theme(&self) -> HigTheme {
        HigTheme {
            brightness: self.brightness,
            accent: self.accent,
            ..HigTheme::light()
        }
    }
}

fn app_view(state: &mut InspectorApp) -> impl WidgetView<InspectorApp> + use<> {
    let sections = [
        (
            "个人收藏",
            vec![
                ("画布", icons::HOME),
                ("图层", icons::DESCRIPTION),
                ("资源库", icons::FOLDER),
            ],
        ),
        ("偏好设置", vec![("外观", icons::PALETTE)]),
    ];

    let sidebar = source_list_sidebar(&sections, state.sidebar_sel, |s: &mut InspectorApp, pos| {
        s.sidebar_sel = pos;
    });

    let toolbar = macos_toolbar_with_center(
        vec![
            icon_button(icons::ARROW_BACK, |s: &mut InspectorApp| {
                s.sidebar_sel = (0, 0);
            })
            .boxed(),
            icon_button(icons::DARK_MODE, |s: &mut InspectorApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
        ],
        search_bar(state.search.clone(), |s: &mut InspectorApp, v| s.search = v)
            .on_clear(|s: &mut InspectorApp| s.search.clear())
            .width(280.px()),
        vec![
            icon_button(icons::DOCK_LEFT, |s: &mut InspectorApp| {
                s.inspector_visible = !s.inspector_visible;
            })
            .boxed(),
            help_button(|s: &mut InspectorApp| s.show_help_dialog = true).boxed(),
        ],
    );

    let accent_names = ["蓝色", "紫色", "粉色", "绿色"];
    let accent_idx = match state.accent {
        HigAccent::Blue => 0,
        HigAccent::Purple => 1,
        HigAccent::Pink => 2,
        HigAccent::Green => 3,
        _ => 0,
    };

    let main_body = portal(
        sized_box(
            flex_col((
                headline_small("macOS 文档与检查器"),
                body_medium(
                    "体现 26px 紧凑 Aqua 控件高度、⌘ 快捷键组合以及 3px 响应式外发光聚焦环。",
                ),
                form_group(
                    Some("文档元数据"),
                    vec![
                        text_field(state.title_input.clone(), |s: &mut InspectorApp, v| {
                            s.title_input = v;
                        })
                        .label("文档标题")
                        .supporting_text("点击聚焦输入框可观察 macOS OuterHalo3px 外发光环")
                        .boxed(),
                        form_row(
                            "系统强调色",
                            segmented_button(
                                &accent_names,
                                accent_idx,
                                |s: &mut InspectorApp, i| {
                                    s.accent = match i {
                                        0 => HigAccent::Blue,
                                        1 => HigAccent::Purple,
                                        2 => HigAccent::Pink,
                                        _ => HigAccent::Green,
                                    };
                                },
                            ),
                        )
                        .boxed(),
                    ],
                    Some("HIG 分组表单页脚说明文字。"),
                ),
                disclosure_group(
                    "快捷键与导出说明",
                    state.metadata_expanded,
                    |s: &mut InspectorApp| s.metadata_expanded = !s.metadata_expanded,
                    body_small("按 ⌘S 快速导出，或点击工具栏右上角“?”查看 macOS 帮助弹窗。"),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(16.px()),
        )
        .padding(Padding::all(20.px())),
    )
    .constrain_horizontal(true);

    let path_segments = [
        ("Macintosh HD", icons::FOLDER),
        ("用户", icons::PERSON),
        ("项目", icons::FOLDER),
        ("Beamlet.xcodeproj", icons::CODE),
    ];

    let inspector = state.inspector_visible.then(|| {
        inspector_panel(
            "检查器",
            vec![
                form_group(
                    Some("布局参数"),
                    vec![
                        form_row(
                            "网格栏数",
                            stepper(
                                "栏数",
                                state.columns,
                                1,
                                12,
                                1,
                                |s: &mut InspectorApp, next| s.columns = next,
                            ),
                        )
                        .boxed(),
                        form_row(
                            "吸附网格",
                            switch(state.grid_snap, |s: &mut InspectorApp, v| {
                                s.grid_snap = v;
                            }),
                        )
                        .boxed(),
                        form_row(
                            "实时渲染",
                            switch(state.live_preview, |s: &mut InspectorApp, v| {
                                s.live_preview = v;
                            }),
                        )
                        .boxed(),
                    ],
                    None,
                )
                .boxed(),
                form_group(
                    Some("摘要信息"),
                    vec![
                        form_row("当前选中", label_medium(format!("{:?}", state.sidebar_sel)))
                            .boxed(),
                        form_row(
                            "导出项目",
                            primary_button("保存 (⌘S)", |s: &mut InspectorApp| {
                                s.show_help_dialog = true
                            }),
                        )
                        .boxed(),
                    ],
                    None,
                )
                .boxed(),
            ],
        )
    });

    let center_col = flex_col((
        toolbar,
        main_body.flex(1.0),
        path_control(
            &path_segments[..=(state.path_idx.min(3))],
            |s: &mut InspectorApp, idx| {
                s.path_idx = idx;
            },
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let shell = flex_row((sidebar, center_col.flex(1.0), inspector))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let help_modal = state.show_help_dialog.then(|| {
        basic_dialog(
            "macOS 检查器快捷键指南",
            "遵循 Apple HIG 规范：“关闭/取消”靠右偏左，“完成/确认”按钮置于最右侧 (PrimaryTrailing)。",
            "完成",
            |s: &mut InspectorApp| s.show_help_dialog = false,
            |s: &mut InspectorApp| s.show_help_dialog = false,
        )
        .icon(icons::HELP)
        .dismiss_text("关闭")
    });

    zstack((
        shell.alignment(UnitPoint::CENTER),
        help_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = InspectorApp::default();
    let props = xilem_hig::build_default_properties(&initial.current_theme().to_spec());
    let app = Xilem::new(initial, |state: &mut InspectorApp| {
        let theme = state.current_theme();
        let wid = state.window_id;
        std::iter::once(
            hig_window(wid, "macOS Apple HIG 检查器", &theme, app_view(state)).with_options(
                |opts| {
                    opts.on_close(|s: &mut InspectorApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(1120.0, 740.0))
                },
            ),
        )
    })
    .with_default_properties(props);
    app.run_in(EventLoop::with_user_event())
        .expect("hig_inspector event loop exited cleanly");
}
