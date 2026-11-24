//! Material Design 3 Expressive Notes & Tasks example using `brocado`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use brocado::prelude::*;
use masonry::dpi::LogicalSize;
use masonry::layout::{AsUnit, UnitPoint};
use masonry::peniko::Color;
use masonry::properties::{Dimensions, Padding};
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, sized_box, zstack,
};

struct NoteItem {
    title: String,
    pinned: bool,
    tag: &'static str,
}

struct NotesApp {
    window_id: WindowId,
    running: bool,
    brightness: Brightness,
    high_contrast: bool,
    seed: Color,
    section: usize,
    filter_pinned_only: bool,
    query: String,
    notes: Vec<NoteItem>,
    pending_delete: Option<usize>,
    toasts: ToastHostState,
}

impl AppState for NotesApp {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl Default for NotesApp {
    fn default() -> Self {
        Self {
            window_id: WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            high_contrast: false,
            seed: Color::from_rgb8(0x67, 0x50, 0xA4),
            section: 0,
            filter_pinned_only: false,
            query: String::new(),
            notes: vec![
                NoteItem {
                    title: "OKLCH 色阶调色板核查".into(),
                    pinned: true,
                    tag: "设计",
                },
                NoteItem {
                    title: "重构 FocusFrame 状态机".into(),
                    pinned: true,
                    tag: "架构",
                },
                NoteItem {
                    title: "悬浮工具栏表现力打磨".into(),
                    pinned: false,
                    tag: "界面",
                },
            ],
            pending_delete: None,
            toasts: ToastHostState::new(),
        }
    }
}

impl NotesApp {
    fn current_theme(&self) -> Md3Theme {
        let contrast = if self.high_contrast {
            ContrastLevel::High
        } else {
            ContrastLevel::Standard
        };
        Md3Theme::from_seed(self.seed, self.brightness).with_contrast(self.seed, contrast)
    }
}

fn app_view(state: &mut NotesApp) -> impl WidgetView<NotesApp> + use<> {
    let dests = [
        NavDestination::new("全部笔记", icons::DESCRIPTION),
        NavDestination::new("已置顶", icons::STAR),
        NavDestination::new("归档箱", icons::ARCHIVE),
    ];

    let rail = navigation_rail_with_fab(
        Some(small_fab(
            icons::ADD,
            FabColor::Tertiary,
            |s: &mut NotesApp| {
                let n = s.notes.len() + 1;
                s.notes.push(NoteItem {
                    title: format!("快速灵感 #{n}"),
                    pinned: s.section == 1,
                    tag: "收集箱",
                });
            },
        )),
        &dests,
        state.section,
        |s: &mut NotesApp, idx| {
            s.section = idx;
            s.filter_pinned_only = idx == 1;
        },
    );

    let bar = top_app_bar_slots(
        TopAppBarVariant::Small,
        "Material 3 笔记",
        None,
        vec![
            filter_chip(
                "仅看置顶",
                state.filter_pinned_only,
                |s: &mut NotesApp| {
                    s.filter_pinned_only = !s.filter_pinned_only;
                },
            )
            .boxed(),
            icon_button(icons::PALETTE, |s: &mut NotesApp| {
                s.seed = if s.seed == Color::from_rgb8(0x67, 0x50, 0xA4) {
                    Color::from_rgb8(0x00, 0x6A, 0x60)
                } else {
                    Color::from_rgb8(0x67, 0x50, 0xA4)
                };
            })
            .boxed(),
            icon_button(icons::DARK_MODE, |s: &mut NotesApp| {
                s.brightness = s.brightness.toggled();
            })
            .boxed(),
        ],
    );

    let q = state.query.trim().to_lowercase();
    let note_cards: Vec<_> = state
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            (!state.filter_pinned_only || n.pinned)
                && (q.is_empty() || n.title.to_lowercase().contains(&q))
        })
        .map(|(idx, note)| {
            let pinned = note.pinned;
            let row = list_item_view(
                flex_row((
                    icon_button(icons::STAR, move |s: &mut NotesApp| {
                        if let Some(item) = s.notes.get_mut(idx) {
                            item.pinned = !item.pinned;
                        }
                    })
                    .selected(pinned),
                    flex_col((
                        title_medium(note.title.clone()),
                        body_small(format!("标签：{}", note.tag)),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(2.px())
                    .flex(1.0),
                    destructive_button("删除", move |s: &mut NotesApp| {
                        s.pending_delete = Some(idx);
                    }),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px()),
            );
            outlined_card(row).boxed()
        })
        .collect();

    let hero = elevated_card(
        flex_row((
            flex_col((
                headline_small("Expressive M3 专属组件体系"),
                body_medium("直接使用 brocado::desktop_window、navigation_rail_with_fab 与 floating_toolbar 构建 Material 3 Expressive 笔记应用。"),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px())
            .flex(1.0),
            ripple_layer(large_fab(
                icons::EDIT,
                FabColor::Primary,
                |s: &mut NotesApp| {
                    s.toasts
                        .show("已打开新建笔记编辑器", Some("知道了"), ToastDuration::Short, true);
                },
            )),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(16.px()),
    );

    let body = scroll_view(
        sized_box(
            flex_col((
                search_bar(state.query.clone(), |s: &mut NotesApp, v| s.query = v)
                    .on_clear(|s: &mut NotesApp| s.query.clear()),
                hero,
                flex_col(note_cards)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(8.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(14.px()),
        )
        .padding(Padding::all(20.px())),
    )
    .constrain_horizontal(true);

    let main_col = flex_col((bar, horizontal_divider(), body.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Stretch);

    let shell =
        flex_row((rail, main_col.flex(1.0))).cross_axis_alignment(CrossAxisAlignment::Stretch);

    let dock = sized_box(
        flex_row((
            floating_toolbar(vec![
                subtle_button("高对比度", |s: &mut NotesApp| {
                    s.high_contrast = !s.high_contrast;
                })
                .boxed(),
                primary_button("添加待办", |s: &mut NotesApp| {
                    let n = s.notes.len() + 1;
                    s.notes.push(NoteItem {
                        title: format!("Material 待办任务 #{n}"),
                        pinned: false,
                        tag: "任务",
                    });
                })
                .boxed(),
            ]),
            ripple_layer(extended_fab(
                icons::ADD,
                "新建笔记",
                FabColor::Secondary,
                |s: &mut NotesApp| {
                    let n = s.notes.len() + 1;
                    s.notes.push(NoteItem {
                        title: format!("Extended FAB 笔记 #{n}"),
                        pinned: false,
                        tag: "随记",
                    });
                },
            )),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
        .dims(Dimensions::MAX),
    )
    .padding(Padding::all(20.px()))
    .alignment(UnitPoint::BOTTOM_RIGHT);

    let delete_modal = state.pending_delete.map(|idx| {
        basic_dialog(
            "永久删除此笔记？",
            "该操作属于高风险确认，将从列表中永久移除这条笔记。",
            "确认删除",
            move |s: &mut NotesApp| {
                if idx < s.notes.len() {
                    s.notes.remove(idx);
                }
                s.pending_delete = None;
            },
            |s: &mut NotesApp| s.pending_delete = None,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("取消")
    });

    let toast_layer = sized_box(
        toast_host(&state.toasts, |s: &mut NotesApp| {
            let _ = s.toasts.dismiss_current();
        })
        .on_action(|s: &mut NotesApp| {
            let _ = s.toasts.dismiss_current();
        }),
    )
    .padding(Padding::all(16.px()))
    .alignment(UnitPoint::BOTTOM);

    zstack((
        shell.alignment(UnitPoint::CENTER),
        dock,
        toast_layer,
        delete_modal.map(|d| d.alignment(UnitPoint::CENTER)),
    ))
}

fn main() {
    let initial = NotesApp::default();
    let props = brocado::build_default_properties(&initial.current_theme().to_spec());
    AppRunner::new(initial, |state: &mut NotesApp| {
        let spec = state.current_theme().to_spec();
        let wid = state.window_id;
        std::iter::once(
            desktop_window(wid, "Material Design 3 笔记", &spec, app_view(state)).with_config(
                |cfg| {
                    cfg.on_close(|s: &mut NotesApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(1040.0, 740.0))
                },
            ),
        )
    })
    .with_default_properties(props)
    .run()
    .expect("md3_notes event loop exited cleanly");
}
