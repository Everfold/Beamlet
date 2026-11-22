//! End-to-end headless integration tests verifying all 4 design languages, live theme switching,
//! custom widgets (`RangeSlider`, `DatePicker`, `TimePicker`), `EscapeCatcher`, and `LayerStack` overlays.

use std::fmt::Debug;
use std::sync::Arc;

use masonry_testing::{TestHarness, TestHarnessParams};
use xilem::core::{
    DynMessage, MessageCtx, ProxyError, RawProxy, SendMessage, View, ViewId, ViewPathTracker as _,
};
use xilem::kurbo::{Point, Vec2};
use xilem::masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
use xilem::masonry::core::{KeyboardEvent, PointerButton, TextEvent, WidgetId};
use xilem::masonry::dpi::PhysicalSize;
use xilem::masonry::layout::AsUnit;
use xilem::masonry::properties::Dimensions;
use xilem::masonry::widgets::Passthrough;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row, label, sized_box};
use xilem::{AnyWidgetView, ViewCtx, WidgetView};
use xilem_ui::core::foundation::overlay::HoverTooltipHost;
use xilem_ui::prelude::*;

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

struct XilemHarness<State: 'static> {
    state: State,
    theme_fn: fn(&State) -> UiTheme,
    logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
    view_ctx: ViewCtx,
    current_view: Box<AnyWidgetView<State>>,
    view_state: RootViewState<State>,
    last_spec: DesignSpec,
    harness: TestHarness<Passthrough>,
}

impl<State: 'static> XilemHarness<State> {
    fn new(
        mut state: State,
        theme_fn: fn(&State) -> UiTheme,
        logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
        size: (u32, u32),
    ) -> Self {
        let rt = Arc::new(tokio::runtime::Runtime::new().expect("tokio test runtime"));
        let mut view_ctx = ViewCtx::new(Arc::new(NoopRawProxy), rt);
        let theme = theme_fn(&state);
        let spec = theme.to_spec();
        let scoped_view = ui_scope(
            &theme,
            sized_box(logic(&mut state))
                .dims(Dimensions::STRETCH)
                .background_color(spec.palette.window_bg),
        )
        .boxed();

        let (pod, view_state) = scoped_view.build(&mut view_ctx, &mut state);
        let params = TestHarnessParams::default()
            .with_size(PhysicalSize::new(size.0, size.1))
            .with_background(spec.palette.window_bg)
            .with_padding(0);
        let harness = TestHarness::create_with(theme.default_properties(), pod.new_widget, params);

        Self {
            state,
            theme_fn,
            logic,
            view_ctx,
            current_view: scoped_view,
            view_state,
            last_spec: spec,
            harness,
        }
    }

    fn rebuild(&mut self) {
        let theme = (self.theme_fn)(&self.state);
        let next_spec = theme.to_spec();
        if next_spec != self.last_spec {
            let props = cached_default_properties(&next_spec);
            self.harness.render_root().set_default_properties(props);
            self.last_spec = next_spec.clone();
        }
        let next_view = ui_scope(
            &theme,
            sized_box((self.logic)(&mut self.state))
                .dims(Dimensions::STRETCH)
                .background_color(next_spec.palette.window_bg),
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
    theme: UiTheme,
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
            theme: UiTheme::new(DesignLanguage::Md3, Brightness::Light),
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
    root: &xilem::masonry::core::WidgetRef<'_, dyn xilem::masonry::core::Widget>,
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
fn all_eight_theme_polarities_render_distinct_non_blank_frames() {
    let mut rendered: Vec<(DesignLanguage, Brightness, Vec<u8>)> = Vec::new();
    for lang in DesignLanguage::ALL {
        for brightness in [Brightness::Light, Brightness::Dark] {
            let st = MatrixState {
                theme: UiTheme::new(lang, brightness),
                ..Default::default()
            };
            let mut app =
                XilemHarness::new(st, |s| s.theme.clone(), shared_matrix_view, (480, 420));
            let bytes = app.render_bytes();
            let (chunks, _) = bytes.as_chunks::<4>();
            let first = chunks[0];
            let has_non_bg = chunks.iter().any(|&px| px != first);
            assert!(
                has_non_bg,
                "rendered frame for {lang:?} / {brightness:?} should not be uniform blank"
            );
            rendered.push((lang, brightness, bytes));
        }
    }

    assert_eq!(rendered.len(), 8);
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
    let mut app = XilemHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        shared_matrix_view,
        (480, 420),
    );
    let md3_light = app.render_bytes();

    app.state.theme = UiTheme::new(DesignLanguage::Fluent, Brightness::Dark);
    app.rebuild();
    let fluent_dark = app.render_bytes();
    assert_ne!(md3_light, fluent_dark);

    app.state.theme = UiTheme::new(DesignLanguage::Hig, Brightness::Light);
    app.rebuild();
    let hig_light = app.render_bytes();
    assert_ne!(fluent_dark, hig_light);

    app.state.theme = UiTheme::new(DesignLanguage::Adwaita, Brightness::Dark);
    app.rebuild();
    let adwaita_dark = app.render_bytes();
    assert_ne!(hig_light, adwaita_dark);

    app.state.theme = UiTheme::new(DesignLanguage::Md3, Brightness::Dark).with_high_contrast(true);
    app.rebuild();
    let md3_hc = app.render_bytes();
    assert_ne!(adwaita_dark, md3_hc);
}

#[test]
fn range_slider_pointer_drag_emits_clamped_pair() {
    let mut app = XilemHarness::new(
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
    node: &xilem::masonry::core::WidgetRef<'_, dyn xilem::masonry::core::Widget>,
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
    node: &xilem::masonry::core::WidgetRef<'_, dyn xilem::masonry::core::Widget>,
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
    let mut app = XilemHarness::new(
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
    let mut app = XilemHarness::new(
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
    let mut app = XilemHarness::new(
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
    let mut app = XilemHarness::new(
        MatrixState::default(),
        |s| s.theme.clone(),
        |s| {
            flex_col((
                xilem_ui::md3::center_aligned_top_app_bar("MD3 Signature"),
                flex_row((
                    xilem_ui::md3::fab(icons::ADD, FabColor::Primary, |st: &mut MatrixState| {
                        st.clicks += 1;
                    }),
                    xilem_ui::md3::extended_fab(
                        icons::EDIT,
                        "Compose",
                        FabColor::Secondary,
                        |st: &mut MatrixState| st.clicks += 1,
                    ),
                ))
                .gap(8.px()),
                xilem_ui::fluent::info_bar(
                    InfoBarSeverity::Informational,
                    "Fluent InfoBar",
                    "Ready",
                    None,
                ),
                xilem_ui::fluent::settings_card(
                    Some(icons::SETTINGS),
                    "Fluent Settings",
                    Some("Subtext"),
                    switch(s.switch_on, |st: &mut MatrixState, v| st.switch_on = v),
                ),
                xilem_ui::hig::macos_toolbar("HIG Toolbar", vec![], vec![]),
                xilem_ui::hig::form_group(
                    Some("HIG FORM"),
                    vec![
                        xilem_ui::hig::form_row(
                            "Help",
                            xilem_ui::hig::help_button(|st: &mut MatrixState| st.clicks += 1),
                        )
                        .boxed(),
                    ],
                    None,
                ),
                xilem_ui::adwaita::header_bar("Adwaita Header", None::<&str>, vec![], vec![]),
                xilem_ui::adwaita::banner("Adwaita Banner", Some("OK"), None, None),
                xilem_ui::adwaita::preferences_group(
                    Some("GNOME Prefs"),
                    None,
                    vec![
                        xilem_ui::adwaita::switch_row(
                            "Option",
                            None::<&str>,
                            s.switch_on,
                            |st: &mut MatrixState, v| st.switch_on = v,
                        )
                        .boxed(),
                    ],
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(8.px())
            .boxed()
        },
        (720, 760),
    );

    let pixels = app.render_bytes();
    let (chunks, _) = pixels.as_chunks::<4>();
    let first = chunks[0];
    assert!(chunks.iter().any(|&px| px != first));
}
