//! Standalone Material Design 3 Notes & Tasks example using `xilem-md3` directly without `xilem-ui`.

#![warn(clippy::print_stdout, clippy::print_stderr)]

use xilem::dpi::LogicalSize;
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, portal, sized_box, zstack,
};
use xilem::{AppState, Color, EventLoop, WidgetView, Xilem};
use xilem_md3::{
    Brightness, ContrastLevel, FabColor, Md3Theme, NavDestination, ToastDuration, ToastHostState,
    TopAppBarVariant, basic_dialog, body_medium, body_small, destructive_button, elevated_card,
    extended_fab, filter_chip, floating_toolbar, headline_small, horizontal_divider, icon_button,
    icons, large_fab, list_item_view, md3_window, navigation_rail_with_fab, outlined_card,
    primary_button, search_bar, small_fab, subtle_button, title_medium, toast_host,
    top_app_bar_slots,
};

struct NoteItem {
    title: String,
    pinned: bool,
    tag: &'static str,
}

struct NotesApp {
    window_id: xilem::WindowId,
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
            window_id: xilem::WindowId::next(),
            running: true,
            brightness: Brightness::Light,
            high_contrast: false,
            seed: Color::from_rgb8(0x67, 0x50, 0xA4),
            section: 0,
            filter_pinned_only: false,
            query: String::new(),
            notes: vec![
                NoteItem {
                    title: "OKLCH Tonal Palette Audit".into(),
                    pinned: true,
                    tag: "Design",
                },
                NoteItem {
                    title: "Refactor FocusFrame State Machine".into(),
                    pinned: true,
                    tag: "Architecture",
                },
                NoteItem {
                    title: "Expressive Floating Toolbar Polish".into(),
                    pinned: false,
                    tag: "UI",
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
        NavDestination::new("Notes", icons::DESCRIPTION),
        NavDestination::new("Pinned", icons::STAR),
        NavDestination::new("Archive", icons::ARCHIVE),
    ];

    let rail = navigation_rail_with_fab(
        Some(small_fab(
            icons::ADD,
            FabColor::Tertiary,
            |s: &mut NotesApp| {
                let n = s.notes.len() + 1;
                s.notes.push(NoteItem {
                    title: format!("Quick Idea #{n}"),
                    pinned: s.section == 1,
                    tag: "Inbox",
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
        "Material 3 Notes",
        None,
        vec![
            filter_chip(
                "Pinned Only",
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

    let q = state.query.trim().to_ascii_lowercase();
    let note_cards: Vec<_> = state
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            (!state.filter_pinned_only || n.pinned)
                && (q.is_empty() || n.title.to_ascii_lowercase().contains(&q))
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
                        body_small(format!("Tag: {}", note.tag)),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(2.px())
                    .flex(1.0),
                    destructive_button("Delete", move |s: &mut NotesApp| {
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
                headline_small("Expressive M3 Components"),
                body_medium("Built standalone with xilem-md3::md3_window, navigation_rail_with_fab, and floating_toolbar."),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(4.px())
            .flex(1.0),
            large_fab(icons::EDIT, FabColor::Primary, |s: &mut NotesApp| {
                s.toasts.show("Compose opened", None::<&str>, ToastDuration::Short, true);
            }),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(16.px()),
    );

    let body = portal(
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
                subtle_button("High Contrast", |s: &mut NotesApp| {
                    s.high_contrast = !s.high_contrast;
                })
                .boxed(),
                primary_button("Add Note", |s: &mut NotesApp| {
                    let n = s.notes.len() + 1;
                    s.notes.push(NoteItem {
                        title: format!("Material Task #{n}"),
                        pinned: false,
                        tag: "Tasks",
                    });
                })
                .boxed(),
            ]),
            extended_fab(
                icons::ADD,
                "New Note",
                FabColor::Secondary,
                |s: &mut NotesApp| {
                    let n = s.notes.len() + 1;
                    s.notes.push(NoteItem {
                        title: format!("Extended FAB Note #{n}"),
                        pinned: false,
                        tag: "FAB",
                    });
                },
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(12.px())
        .dims(Dimensions::MAX),
    )
    .padding(Padding::all(20.px()))
    .alignment(UnitPoint::BOTTOM_RIGHT);

    let delete_modal = state.pending_delete.map(|idx| {
        basic_dialog(
            "Delete note permanently?",
            "Destructive confirmation removes this note from your list.",
            "Delete",
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
        .dismiss_text("Cancel")
    });

    let toast_layer = sized_box(toast_host(&state.toasts, |s: &mut NotesApp| {
        let _ = s.toasts.dismiss_current();
    }))
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
    let props = xilem_md3::build_default_properties(&initial.current_theme().to_spec());
    let app = Xilem::new(initial, |state: &mut NotesApp| {
        let theme = state.current_theme();
        let wid = state.window_id;
        std::iter::once(
            md3_window(wid, "Material Design 3 Notes", &theme, app_view(state)).with_options(
                |opts| {
                    opts.on_close(|s: &mut NotesApp| s.running = false)
                        .with_initial_inner_size(LogicalSize::new(1040.0, 740.0))
                },
            ),
        )
    })
    .with_default_properties(props);
    app.run_in(EventLoop::with_user_event())
        .expect("md3_notes event loop exited cleanly");
}
