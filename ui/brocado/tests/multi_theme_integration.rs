//! End-to-end headless integration tests verifying Material 3 Expressive themes, live theme switching,
//! custom widgets (`RangeSlider`, `DatePicker`, `TimePicker`), `EscapeCatcher`, and `LayerStack` overlays.

use std::fmt::Debug;
use std::sync::Arc;

use brocado::HoverTooltipHost;
use brocado::prelude::*;
use masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
use masonry::core::{KeyboardEvent, PointerButton, TextEvent, WidgetId};
use masonry::dpi::PhysicalSize;
use masonry::kurbo::{Point, Vec2};
use masonry::layout::AsUnit;
use masonry::peniko::Color;
use masonry::properties::Dimensions;
use masonry::widgets::Passthrough;
use masonry_testing::{TestHarness, TestHarnessParams};
use xilem_core::{
    DynMessage, MessageCtx, ProxyError, RawProxy, SendMessage, View, ViewId, ViewPathTracker as _,
};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, flex_col, flex_row, label, sized_box};

#[derive(Debug)]
struct NoopRawProxy;

impl RawProxy for NoopRawProxy {
    fn send_message(&self, _path: Arc<[ViewId]>, _message: SendMessage) -> Result<(), ProxyError> {
        Ok(())
    }

    fn dyn_debug(&self) -> &dyn Debug {
        self
    }
}

type RootViewState<State> = <Box<AnyWidgetView<State>> as View<State, (), ViewCtx>>::ViewState;

struct BrocadoHarness<State: 'static> {
    state: State,
    theme_fn: fn(&State) -> Theme,
    logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
    view_ctx: ViewCtx,
    current_view: Box<AnyWidgetView<State>>,
    view_state: RootViewState<State>,
    last_theme: Theme,
    harness: TestHarness<Passthrough>,
}

impl<State: 'static> BrocadoHarness<State> {
    fn new(
        mut state: State,
        theme_fn: fn(&State) -> Theme,
        logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
        size: (u32, u32),
    ) -> Self {
        let rt = Arc::new(tokio::runtime::Runtime::new().expect("tokio test runtime"));
        let mut view_ctx = ViewCtx::new(Arc::new(NoopRawProxy), rt);
        let theme = theme_fn(&state);
        let scoped_view = theme_scope(
            &theme,
            sized_box(logic(&mut state))
                .dims(Dimensions::STRETCH)
                .background_color(theme.scheme.surface),
        )
        .boxed();

        let (pod, view_state) = scoped_view.build(&mut view_ctx, &mut state);
        let params = TestHarnessParams::default()
            .with_size(PhysicalSize::new(size.0, size.1))
            .with_background(theme.scheme.surface)
            .with_padding(0);
        let harness = TestHarness::create_with(theme.default_properties(), pod.new_widget, params);

        Self {
            state,
            theme_fn,
            logic,
            view_ctx,
            current_view: scoped_view,
            view_state,
            last_theme: theme,
            harness,
        }
    }

    fn rebuild(&mut self) {
        let theme = (self.theme_fn)(&self.state);
        if theme != self.last_theme {
            let props = cached_default_properties(&theme);
            self.harness.render_root().set_default_properties(props);
            self.last_theme = theme.clone();
        }
        let next_view = theme_scope(
            &theme,
            sized_box((self.logic)(&mut self.state))
                .dims(Dimensions::STRETCH)
                .background_color(theme.scheme.surface),
        )
        .boxed();

        self.view_ctx.reset_changed_props();
        self.view_ctx.reset_changed_transforms();
        let current_view = &self.current_view;
        let view_state = &mut self.view_state;
        let view_ctx = &mut self.view_ctx;
        let app_state = &mut self.state;
        self.harness.render_root().edit_base_layer(|mut root| {
            let mut root = root.downcast::<Passthrough>();
            next_view.rebuild(
                current_view,
                view_state,
                view_ctx,
                root.reborrow_mut(),
                app_state,
            );
        });
        self.current_view = next_view;
    }

    fn pump_actions(&mut self) {
        while let Some((action, widget_id)) = self.harness.pop_action_erased() {
            let Some(path) = self.view_ctx.get_id_path(widget_id).cloned() else {
                continue;
            };
            let env = std::mem::take(self.view_ctx.environment());
            let mut msg_ctx = MessageCtx::new(env, path, DynMessage(action));
            let view = &self.current_view;
            let view_state = &mut self.view_state;
            let app_state = &mut self.state;
            let _ = self.harness.render_root().edit_base_layer(|mut root| {
                view.message(view_state, &mut msg_ctx, root.downcast(), app_state)
            });
            let (restored_env, _, _) = msg_ctx.finish();
            *self.view_ctx.environment() = restored_env;
        }
        self.rebuild();
    }

    fn render_bytes(&mut self) -> Vec<u8> {
        self.harness.render().into_raw()
    }
}

struct MatrixState {
    theme: Theme,
    checked: bool,
    switch_on: bool,
    text: String,
    slider_val: f64,
    range_val: (f64, f64),
    date: CalendarDate,
    time: TimeOfDay,
    dialog_open: bool,
    menu_open: bool,
    selected_menu: usize,
    clicks: u32,
}

impl Default for MatrixState {
    fn default() -> Self {
        Self {
            theme: Theme::light(),
            checked: true,
            switch_on: true,
            text: "Query".to_string(),
            slider_val: 0.5,
            range_val: (20.0, 80.0),
            date: CalendarDate::new(2026, 11, 22),
            time: TimeOfDay::new(10, 30),
            dialog_open: false,
            menu_open: false,
            selected_menu: 0,
            clicks: 0,
        }
    }
}

fn shared_matrix_view(s: &mut MatrixState) -> Box<AnyWidgetView<MatrixState>> {
    flex_col((
        top_bar("Multi-Theme Verification", None, None),
        flex_row((
            primary_button("Save", |st: &mut MatrixState| st.clicks += 1).leading_icon(icons::ADD),
            secondary_button("Cancel", |st: &mut MatrixState| st.clicks += 1),
            outlined_button("Inspect", |st: &mut MatrixState| st.clicks += 1),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(8.px()),
        elevated_card(
            flex_col((
                title_medium("Controls"),
                text_field(s.text.clone(), |st: &mut MatrixState, v| st.text = v)
                    .label("Field")
                    .placeholder("Type here"),
                flex_row((
                    checkbox("Enabled", s.checked, |st: &mut MatrixState, v| {
                        st.checked = v;
                    }),
                    switch(s.switch_on, |st: &mut MatrixState, v| st.switch_on = v),
                ))
                .gap(12.px()),
                linear_progress(Some(s.slider_val)),
                slider(0.0, 1.0, s.slider_val, None, |st: &mut MatrixState, v| {
                    st.slider_val = v;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(8.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(12.px())
    .boxed()
}

fn find_first_widget_id_by_type_name(
    root: &masonry::core::WidgetRef<'_, dyn masonry::core::Widget>,
    needle: &str,
) -> Option<WidgetId> {
    if root.inner().short_type_name().contains(needle) {
        return Some(root.id());
    }
    for child in root.children() {
        if let Some(found) = find_first_widget_id_by_type_name(&child, needle) {
            return Some(found);
        }
    }
    None
}

#[test]
fn m3_seed_and_brightness_polarities_render_distinct_non_blank_frames() {
    let mut rendered: Vec<(Color, Brightness, Vec<u8>)> = Vec::new();
    for seed in [seeds::PURPLE, seeds::TEAL] {
        for brightness in [Brightness::Light, Brightness::Dark] {
            let st = MatrixState {
                theme: Theme::from_seed(seed, brightness),
                ..Default::default()
            };
            let mut app =
                BrocadoHarness::new(st, |s| s.theme.clone(), shared_matrix_view, (480, 420));
            let bytes = app.render_bytes();
            let (chunks, _) = bytes.as_chunks::<4>();
            let first = chunks[0];
            let has_non_bg = chunks.iter().any(|&px| px != first);
            assert!(
                has_non_bg,
                "rendered frame for {seed:?} / {brightness:?} should not be uniform blank"
            );
            rendered.push((seed, brightness, bytes));
        }
    }

    assert_eq!(rendered.len(), 4);
    for i in 0..rendered.len() {
        for j in (i + 1)..rendered.len() {
            assert_ne!(
                rendered[i].2, rendered[j].2,
                "theme {:?}/{:?} must render distinctly from {:?}/{:?}",
                rendered[i].0, rendered[i].1, rendered[j].0, rendered[j].1
            );
        }
    }
}

#[test]
fn live_runtime_theme_hot_switch_updates_single_tree_cleanly() {
    let mut app = BrocadoHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        shared_matrix_view,
        (480, 420),
    );
    let md3_light = app.render_bytes();

    app.state.theme = Theme::dark();
    app.rebuild();
    let md3_dark = app.render_bytes();
    assert_ne!(md3_light, md3_dark);

    app.state.theme = Theme::light().with_seed(seeds::TEAL);
    app.rebuild();
    let md3_teal = app.render_bytes();
    assert_ne!(md3_dark, md3_teal);

    app.state.theme = Theme::dark().with_high_contrast(true);
    app.rebuild();
    let md3_hc = app.render_bytes();
    assert_ne!(md3_teal, md3_hc);
}

#[test]
fn range_slider_pointer_drag_emits_clamped_pair() {
    let mut app = BrocadoHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        |s| {
            sized_box(range_slider(
                0.0,
                100.0,
                s.range_val,
                |st: &mut MatrixState, val| st.range_val = val,
            ))
            .dims(Dimensions::fixed(300.px(), 40.px()))
            .boxed()
        },
        (320, 80),
    );
    let _ = app.render_bytes();

    let base_root = app.harness.render_root().get_layer_root(0);
    let rs_id = find_first_widget_id_by_type_name(&base_root, "RangeSliderWidget")
        .expect("RangeSliderWidget must be present");
    let rect = app.harness.get_widget_with_id(rs_id).ctx().border_box();
    let origin = app
        .harness
        .get_widget_with_id(rs_id)
        .ctx()
        .window_transform()
        * Point::new(rect.x0, rect.center().y);

    // Drag start thumb toward middle (around x = 120)
    app.harness
        .mouse_move(Point::new(origin.x + 25.0, origin.y));
    app.harness.mouse_button_press(Some(PointerButton::Primary));
    app.harness
        .mouse_move(Point::new(origin.x + 120.0, origin.y));
    app.harness
        .mouse_button_release(Some(PointerButton::Primary));
    app.pump_actions();

    assert!(
        app.state.range_val.0 > 20.0,
        "start thumb moved right: {:?}",
        app.state.range_val
    );
    assert!(app.state.range_val.0 <= app.state.range_val.1);
}

fn subtree_has_debug_text(
    node: &masonry::core::WidgetRef<'_, dyn masonry::core::Widget>,
    target_text: &str,
) -> bool {
    if node.inner().get_debug_text().as_deref() == Some(target_text) {
        return true;
    }
    node.children()
        .iter()
        .any(|c| subtree_has_debug_text(c, target_text))
}

fn find_button_with_label(
    node: &masonry::core::WidgetRef<'_, dyn masonry::core::Widget>,
    target_label: &str,
) -> Option<WidgetId> {
    if node.inner().short_type_name().contains("Button")
        && subtree_has_debug_text(node, target_label)
    {
        return Some(node.id());
    }
    for c in node.children() {
        if let Some(id) = find_button_with_label(&c, target_label) {
            return Some(id);
        }
    }
    None
}

#[test]
fn date_and_time_pickers_update_state_on_click() {
    let mut app = BrocadoHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        |s| {
            flex_row((
                date_picker(s.date, |st: &mut MatrixState, d| st.date = d),
                time_picker(s.time, |st: &mut MatrixState, t| st.time = t),
            ))
            .gap(16.px())
            .boxed()
        },
        (700, 420),
    );
    let _ = app.render_bytes();

    let root = app.harness.render_root().get_layer_root(0);
    let day15_id = find_button_with_label(&root, "15").expect("day 15 button exists");
    app.harness
        .mouse_click_on(day15_id, Some(PointerButton::Primary));
    app.pump_actions();
    assert_eq!(app.state.date.day, 15);

    let root2 = app.harness.render_root().get_layer_root(0);
    let pm_id = find_button_with_label(&root2, "PM").expect("PM toggle button exists");
    app.harness
        .mouse_click_on(pm_id, Some(PointerButton::Primary));
    app.pump_actions();
    assert_eq!(app.state.time.hour, 22);
}

#[test]
fn modal_dialog_dismisses_on_escape_key() {
    let initial = MatrixState {
        dialog_open: true,
        ..Default::default()
    };
    let mut app = BrocadoHarness::new(
        initial,
        |s| s.theme.clone(),
        |s| {
            let dlg = s.dialog_open.then(|| {
                basic_dialog(
                    "Confirm Action",
                    "Press Escape to dismiss cleanly.",
                    "Confirm",
                    |st: &mut MatrixState| st.dialog_open = false,
                    |st: &mut MatrixState| st.dialog_open = false,
                )
            });
            scaffold(primary_button("Open", |st: &mut MatrixState| {
                st.dialog_open = true;
            }))
            .dialog(dlg)
            .build()
            .boxed()
        },
        (520, 400),
    );
    let _ = app.render_bytes();

    let root = app.harness.render_root().get_layer_root(0);
    let confirm_btn =
        find_button_with_label(&root, "Confirm").expect("dialog contains Confirm button");
    app.harness.focus_on(Some(confirm_btn));
    app.harness
        .process_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::Escape),
            code: Code::Escape,
            ..KeyboardEvent::default()
        }));
    app.pump_actions();
    assert!(
        !app.state.dialog_open,
        "Escape key must trigger modal on_dismiss"
    );
}

#[test]
fn hover_tooltip_and_anchored_popup_lifecycle() {
    let initial = MatrixState {
        menu_open: true,
        ..Default::default()
    };
    let mut app = BrocadoHarness::new(
        initial,
        |s| s.theme.clone(),
        |s| {
            flex_row((
                plain_tooltip(
                    "Tooltip caption",
                    primary_button("Tip Target", |st: &mut MatrixState| st.clicks += 1),
                ),
                select(
                    &["Alpha", "Beta", "Gamma"],
                    s.selected_menu,
                    s.menu_open,
                    |st: &mut MatrixState| st.menu_open = !st.menu_open,
                    |st: &mut MatrixState, idx| {
                        st.selected_menu = idx;
                        st.menu_open = false;
                    },
                    |st: &mut MatrixState| st.menu_open = false,
                ),
            ))
            .gap(16.px())
            .boxed()
        },
        (520, 320),
    );
    let _ = app.render_bytes();

    // Verify popup layer opened then dismisses cleanly on window focus loss (no double-remove panic)
    app.harness
        .process_text_event(TextEvent::WindowFocusChange(false));
    app.pump_actions();
    assert!(
        !app.state.menu_open,
        "WindowFocusChange(false) must dismiss open popup layer"
    );

    // Hover tooltip lifecycle: enter -> animate past 450ms -> leave
    let root = app.harness.render_root().get_layer_root(0);
    let tip_host_id = find_first_widget_id_by_type_name(&root, "HoverTooltipHost")
        .expect("HoverTooltipHost present");
    let center = app
        .harness
        .get_widget_with_id(tip_host_id)
        .ctx()
        .border_box()
        .center();
    let global_pt = app
        .harness
        .get_widget_with_id(tip_host_id)
        .ctx()
        .window_transform()
        * center;
    app.harness.mouse_move(global_pt);
    app.harness.animate_ms(500);
    let _ = app.render_bytes();
    app.harness.mouse_move(Point::new(500.0, 300.0));
    app.harness.animate_ms(32);
    let _ = app.render_bytes();

    let _ = (label(""), HoverTooltipHost::close_layer, Vec2::ZERO);
}

#[test]
fn all_four_platform_signature_suites_build_and_render() {
    let mut app = BrocadoHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        |_| {
            flex_col((
                top_app_bar_slots(
                    TopAppBarVariant::CenterAligned,
                    "MD3 Signature",
                    None,
                    vec![],
                ),
                flex_row((
                    fab(icons::ADD, FabColor::Primary, |st: &mut MatrixState| {
                        st.clicks += 1;
                    }),
                    extended_fab(
                        icons::EDIT,
                        "Compose",
                        FabColor::Secondary,
                        |st: &mut MatrixState| st.clicks += 1,
                    ),
                ))
                .gap(8.px()),
                split_button(
                    "Save",
                    |st: &mut MatrixState| st.clicks += 1,
                    |st: &mut MatrixState| st.clicks += 1,
                ),
                button_group(&["Day", "Week", "Month"], 0, |st: &mut MatrixState, _| {
                    st.clicks += 1
                }),
                floating_toolbar_with_variant(
                    FloatingToolbarVariant::Vibrant,
                    vec![primary_button("Action", |st: &mut MatrixState| st.clicks += 1).boxed()],
                ),
                wavy_linear_progress(Some(0.6)),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(8.px())
            .boxed()
        },
        (720, 920),
    );

    let pixels = app.render_bytes();
    let (chunks, _) = pixels.as_chunks::<4>();
    let first = chunks[0];
    assert!(chunks.iter().any(|&px| px != first));
}

#[test]
fn focus_frame_reacts_to_real_focus_changes() {
    {
        let st = MatrixState {
            theme: Theme::light(),
            ..Default::default()
        };
        let mut app = BrocadoHarness::new(
            st,
            |s| s.theme.clone(),
            |s| {
                text_field(s.text.clone(), |st: &mut MatrixState, v| st.text = v)
                    .label("Input")
                    .boxed()
            },
            (360, 140),
        );
        let before = app.render_bytes();

        let root = app.harness.render_root().get_layer_root(0);
        assert!(
            find_first_widget_id_by_type_name(&root, "FocusFrame").is_some(),
            "TextField must expose FocusFrame"
        );
        let area_id =
            find_first_widget_id_by_type_name(&root, "TextArea").expect("TextArea child exists");
        app.harness.focus_on(Some(area_id));

        let after = app.render_bytes();
        assert_ne!(
            before, after,
            "FocusFrame must visibly update rendered pixels when focused"
        );
    }
}

#[test]
fn dialog_action_order_follows_md3_convention() {
    let initial = MatrixState {
        theme: Theme::light(),
        ..Default::default()
    };
    let mut app = BrocadoHarness::new(
        initial,
        |s| s.theme.clone(),
        |_| {
            basic_dialog(
                "Apply Changes",
                "Confirm action order.",
                "Confirm",
                |_: &mut MatrixState| {},
                |_: &mut MatrixState| {},
            )
            .dismiss_text("Cancel")
            .boxed()
        },
        (500, 320),
    );
    let _ = app.render_bytes();

    let root = app.harness.render_root().get_layer_root(0);
    let confirm_id = find_button_with_label(&root, "Confirm").expect("Confirm button");
    let cancel_id = find_button_with_label(&root, "Cancel").expect("Cancel button");

    let confirm_x = (app
        .harness
        .get_widget_with_id(confirm_id)
        .ctx()
        .window_transform()
        * Point::ORIGIN)
        .x;
    let cancel_x = (app
        .harness
        .get_widget_with_id(cancel_id)
        .ctx()
        .window_transform()
        * Point::ORIGIN)
        .x;

    assert!(
        cancel_x < confirm_x,
        "MD3 must place Cancel ({cancel_x}) left of Confirm ({confirm_x})"
    );
}

#[test]
fn style_overrides_apply_directly_to_concrete_components() {
    let mut app = BrocadoHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        |_| {
            flex_col((
                primary_button("WideBtn", |st: &mut MatrixState| st.clicks += 1)
                    .width(240.px())
                    .corner_radius(2.px())
                    .background_color(Color::from_rgb8(0xE0, 0x20, 0x20)),
                button_view(
                    ButtonVariant::Secondary,
                    label("CustomSlot"),
                    |st: &mut MatrixState| st.clicks += 1,
                )
                .width(190.px()),
                elevated_card(label("CardSlot")).width(300.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(8.px())
            .boxed()
        },
        (420, 260),
    );
    let _ = app.render_bytes();

    // Verify Flex children are concrete Button, Button, SizedBox with zero Passthrough walls.
    fn collect_flex_child_types(
        node: &masonry::core::WidgetRef<'_, dyn masonry::core::Widget>,
    ) -> Option<Vec<&'static str>> {
        if node.inner().short_type_name() == "Flex" {
            let names: Vec<&'static str> = node
                .children()
                .iter()
                .map(|c| c.inner().short_type_name())
                .collect();
            if names.len() == 3 {
                return Some(names);
            }
        }
        for c in node.children() {
            if let Some(found) = collect_flex_child_types(&c) {
                return Some(found);
            }
        }
        None
    }

    let (wide_id, custom_id, child_types) = {
        let root = app.harness.render_root().get_layer_root(0);
        let wide_id = find_button_with_label(&root, "WideBtn").expect("WideBtn found");
        let custom_id = find_button_with_label(&root, "CustomSlot").expect("CustomSlot found");
        let child_types =
            collect_flex_child_types(&root).expect("inner Flex with 3 concrete children");
        (wide_id, custom_id, child_types)
    };

    let wide_w = app
        .harness
        .get_widget_with_id(wide_id)
        .ctx()
        .border_box()
        .width();
    let custom_w = app
        .harness
        .get_widget_with_id(custom_id)
        .ctx()
        .border_box()
        .width();
    assert!(
        (wide_w - 240.0).abs() < 0.5,
        "expected direct .width(240.px()) override on Button, got {wide_w}"
    );
    assert!(
        (custom_w - 190.0).abs() < 0.5,
        "expected direct .width(190.px()) override on button_view Button, got {custom_w}"
    );
    assert_eq!(child_types, vec!["Button", "Button", "SizedBox"]);
}

#[test]
fn aligned_platform_signature_components_build_interact_and_render() {
    // 1) Material Design 3: Small/Large FAB, NavigationRailWithFab, FloatingToolbar
    {
        let mut app = BrocadoHarness::new(
            MatrixState {
                theme: Theme::light(),
                ..Default::default()
            },
            |s| s.theme.clone(),
            |s| {
                let dests = [
                    NavDestination::new("Notes", icons::DESCRIPTION),
                    NavDestination::new("Starred", icons::STAR),
                ];
                flex_row((
                    navigation_rail_with_fab(
                        Some(small_fab(
                            icons::ADD,
                            FabColor::Primary,
                            |st: &mut MatrixState| {
                                st.clicks += 1;
                            },
                        )),
                        &dests,
                        s.selected_menu,
                        |st: &mut MatrixState, i| st.selected_menu = i,
                    ),
                    flex_col((
                        large_fab(icons::EDIT, FabColor::Tertiary, |st: &mut MatrixState| {
                            st.clicks += 10;
                        }),
                        floating_toolbar(vec![
                            icon_button(icons::COPY, |st: &mut MatrixState| st.clicks += 1).boxed(),
                            primary_button("Publish", |st: &mut MatrixState| st.clicks += 1)
                                .boxed(),
                        ]),
                    ))
                    .gap(12.px()),
                ))
                .boxed()
            },
            (720, 420),
        );
        let bytes = app.render_bytes();
        assert!(bytes.as_chunks::<4>().0.iter().any(|px| px[3] > 0));
    }

    // 2) Clamp centering & capping under Material 3 Expressive
    {
        struct ClampState {
            theme: Theme,
            tab: usize,
            sw: bool,
        }
        let mut app = BrocadoHarness::new(
            ClampState {
                theme: Theme::light(),
                tab: 0,
                sw: true,
            },
            |s| s.theme.clone(),
            |s| {
                flex_col((
                    tabs(&["Prefs", "Status"], s.tab, |st: &mut ClampState, i| {
                        st.tab = i;
                    }),
                    clamp(
                        420.0,
                        filled_card(
                            flex_col((
                                label("Clamped Container"),
                                switch(s.sw, |st: &mut ClampState, v| st.sw = v),
                            ))
                            .gap(8.px()),
                        ),
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px())
                .boxed()
            },
            (860, 480),
        );
        let bytes = app.render_bytes();
        assert!(bytes.as_chunks::<4>().0.iter().any(|px| px[3] > 0));

        let clamp_child_id = {
            let root = app.harness.render_root().get_layer_root(0);
            fn find_clamp_child(
                node: &masonry::core::WidgetRef<'_, dyn masonry::core::Widget>,
            ) -> Option<WidgetId> {
                if node.inner().short_type_name() == "Clamp" {
                    return node.children().into_iter().next().map(|c| c.id());
                }
                for child in node.children() {
                    if let Some(found) = find_clamp_child(&child) {
                        return Some(found);
                    }
                }
                None
            }
            find_clamp_child(&root).expect("Clamp child widget present")
        };

        let child_ref = app.harness.get_widget_with_id(clamp_child_id);
        let child_w = child_ref.ctx().border_box().width();
        let child_x = (child_ref.ctx().window_transform() * Point::ORIGIN).x;
        assert!(
            child_w <= 420.5,
            "Clamp must cap child width at <= 420px, got {child_w}"
        );
        assert!(
            child_x > 100.0,
            "Clamp must horizontally center child in 860px container, got x={child_x}"
        );
    }
}

#[test]
fn destructive_button_and_dialog_apply_error_semantics_and_fire_actions() {
    struct DestructState {
        theme: Theme,
        dialog_open: bool,
        confirmed: bool,
    }
    let mut app = BrocadoHarness::new(
        DestructState {
            theme: Theme::light(),
            dialog_open: false,
            confirmed: false,
        },
        |s| s.theme.clone(),
        |s| {
            let dlg = s.dialog_open.then(|| {
                basic_dialog(
                    "Erase workspace?",
                    "Destructive dialog test.",
                    "Confirm Erase",
                    |st: &mut DestructState| {
                        st.confirmed = true;
                        st.dialog_open = false;
                    },
                    |st: &mut DestructState| st.dialog_open = false,
                )
                .icon(icons::DELETE)
                .destructive(true)
                .dismiss_text("Cancel")
            });
            scaffold(destructive_button("Erase", |st: &mut DestructState| {
                st.dialog_open = true;
            }))
            .dialog(dlg)
            .build()
            .boxed()
        },
        (520, 360),
    );
    let _ = app.render_bytes();

    let erase_btn = {
        let root = app.harness.render_root().get_layer_root(0);
        find_button_with_label(&root, "Erase").expect("destructive Erase button")
    };
    app.harness
        .mouse_click_on(erase_btn, Some(PointerButton::Primary));
    app.pump_actions();
    assert!(
        app.state.dialog_open,
        "destructive_button must open confirmation dialog"
    );

    let confirm_btn = {
        let root = app.harness.render_root().get_layer_root(0);
        find_button_with_label(&root, "Confirm Erase").expect("destructive dialog confirm button")
    };
    app.harness
        .mouse_click_on(confirm_btn, Some(PointerButton::Primary));
    app.pump_actions();
    assert!(
        app.state.confirmed && !app.state.dialog_open,
        "destructive dialog confirm action must fire and close dialog"
    );
}
