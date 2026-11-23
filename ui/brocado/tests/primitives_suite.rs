//! Headless integration tests for the 6 standard UI library primitives:
//! `wrap`, `split_view`, `text_area`, `rich_prose`, `context_menu`, and `popover`.

use brocado::prelude::*;
use brocado_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use brocado_preview::inspect::{LayoutNode, dump_layout_tree};
use masonry::core::keyboard::{Code, Key, KeyState, Modifiers, NamedKey};
use masonry::core::pointer::{PointerId, PointerInfo, PointerType};
use masonry::core::{KeyboardEvent, PointerButton, PointerEvent, TextEvent};
use masonry::kurbo::{Axis, Point};
use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::{Dimensions, Padding};
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, ZStackExt, flex_col, label, sized_box, zstack};

#[allow(dead_code)]
mod preview_support;

use preview_support::build_ui_harness;

fn find_first_node<'a>(node: &'a LayoutNode, type_name: &str) -> Option<&'a LayoutNode> {
    if node.type_name == type_name {
        return Some(node);
    }
    for c in &node.children {
        if let Some(found) = find_first_node(c, type_name) {
            return Some(found);
        }
    }
    None
}

fn find_first_node_containing<'a>(node: &'a LayoutNode, needle: &str) -> Option<&'a LayoutNode> {
    if node.type_name.contains(needle) {
        return Some(node);
    }
    for c in &node.children {
        if let Some(found) = find_first_node_containing(c, needle) {
            return Some(found);
        }
    }
    None
}

fn send_key(harness: &mut PreviewHarness<impl Sized>, key: Key, code: Code, modifiers: Modifiers) {
    let inner = harness.inner_harness_mut();
    inner.process_text_event(TextEvent::Keyboard(KeyboardEvent {
        state: KeyState::Down,
        key: key.clone(),
        code,
        location: masonry::core::keyboard::Location::Standard,
        modifiers,
        repeat: false,
        is_composing: false,
    }));
    inner.process_text_event(TextEvent::Keyboard(KeyboardEvent {
        state: KeyState::Up,
        key,
        code,
        location: masonry::core::keyboard::Location::Standard,
        modifiers,
        repeat: false,
        is_composing: false,
    }));
    harness.pump_actions();
}

#[test]
fn wrap_reflows_multiple_rows_and_fits_320px() {
    #[derive(Default)]
    struct State {
        taps: usize,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_s| {
            let items: Vec<_> = [
                "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta",
            ]
            .into_iter()
            .enumerate()
            .map(|(i, name)| filter_chip(name, i % 2 == 0, |s: &mut State| s.taps += 1))
            .collect();
            sized_box(wrap(items).gap(8.px()).run_gap(8.px()))
                .dims(Dimensions::width(320.px()))
                .padding(Padding::all(12.px()))
                .boxed()
        },
        PreviewConfig {
            width: 360,
            height: 480,
            ..PreviewConfig::default()
        },
    );

    harness.settle_animations();
    let tree = dump_layout_tree(&mut harness);
    let wrap_node = find_first_node(&tree, "WrapWidget").expect("WrapWidget should exist");
    assert_eq!(
        wrap_node.children.len(),
        8,
        "expected 8 wrapped children, got {}",
        wrap_node.children.len()
    );

    let mut distinct_rows: Vec<i32> = Vec::new();
    for ch in &wrap_node.children {
        assert!(
            ch.rect.x >= wrap_node.rect.x - 0.5
                && ch.rect.x + ch.rect.width <= wrap_node.rect.x + wrap_node.rect.width + 0.5,
            "child clipped outside WrapWidget: child={:?}, wrap={:?}",
            ch.rect,
            wrap_node.rect
        );
        let ry = ch.rect.y.round() as i32;
        if !distinct_rows.iter().any(|&y| (y - ry).abs() <= 2) {
            distinct_rows.push(ry);
        }
    }
    assert!(
        distinct_rows.len() >= 2,
        "expected WrapWidget to wrap onto at least 2 rows at 320px, got {distinct_rows:?}"
    );
}

#[test]
fn split_view_lays_out_panes_and_resizes_on_drag() {
    #[derive(Default)]
    struct State;

    let mut harness = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Hig, Brightness::Light),
        |_s| {
            sized_box(
                split_view(
                    filled_card(label("First Pane")),
                    outlined_card(label("Second Pane")),
                )
                .split_point(0.5)
                .min_lengths(60.px(), 60.px()),
            )
            .dims(Dimensions::fixed(600.px(), 200.px()))
            .boxed()
        },
        PreviewConfig {
            width: 720,
            height: 360,
            ..PreviewConfig::default()
        },
    );

    harness.settle_animations();
    let tree = dump_layout_tree(&mut harness);
    let split_node = find_first_node_containing(&tree, "Split").expect("Split widget should exist");

    assert_eq!(split_node.children.len(), 2);
    let before_left_w = split_node.children[0].rect.width;
    let bar_x = split_node.children[0].rect.x + split_node.children[0].rect.width + 1.0;
    let bar_y = split_node.rect.y + split_node.rect.height * 0.5;

    let inner = harness.inner_harness_mut();
    inner.mouse_move(Point::new(bar_x, bar_y));
    inner.mouse_button_press(Some(PointerButton::Primary));
    inner.mouse_move(Point::new(bar_x - 100.0, bar_y));
    inner.mouse_button_release(Some(PointerButton::Primary));
    harness.pump_actions();
    harness.settle_animations();

    let tree_after = dump_layout_tree(&mut harness);
    let split_after =
        find_first_node_containing(&tree_after, "Split").expect("Split should still exist");
    let after_left_w = split_after.children[0].rect.width;
    let after_right_w = split_after.children[1].rect.width;

    assert!(
        after_left_w < before_left_w - 40.0,
        "expected left pane to shrink from {before_left_w} to < {}, got {after_left_w}",
        before_left_w - 40.0
    );
    assert!(
        after_right_w > before_left_w + 40.0,
        "expected right pane to grow beyond {}, got {after_right_w}",
        before_left_w + 40.0
    );
}

#[test]
fn text_area_inserts_newline_on_enter_and_submits_with_on_shift_enter() {
    #[derive(Default)]
    struct State {
        enter_text: String,
        shift_text: String,
        submitted: Option<String>,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |s| {
            flex_col((
                text_area(s.enter_text.clone(), |st: &mut State, v| {
                    st.enter_text = v;
                })
                .label("Notes")
                .min_height(72.px())
                .max_height(160.px()),
                filled_text_area(s.shift_text.clone(), |st: &mut State, v| {
                    st.shift_text = v;
                })
                .label("Composer")
                .insert_newline(InsertNewline::OnShiftEnter)
                .on_submit(|st: &mut State, v| {
                    st.submitted = Some(v);
                }),
            ))
            .gap(12.px())
            .boxed()
        },
        PreviewConfig {
            width: 640,
            height: 480,
            ..PreviewConfig::default()
        },
    );

    // 1. Default InsertNewline::OnEnter
    harness
        .run_step(&PreviewStep::ClickType {
            type_name: "TextArea".into(),
            index: 0,
        })
        .unwrap();
    harness
        .run_step(&PreviewStep::TypeText("Line1".into()))
        .unwrap();
    send_key(
        &mut harness,
        Key::Named(NamedKey::Enter),
        Code::Enter,
        Modifiers::default(),
    );
    harness
        .run_step(&PreviewStep::TypeText("Line2".into()))
        .unwrap();
    assert_eq!(harness.state().enter_text, "Line1\nLine2");

    // 2. InsertNewline::OnShiftEnter + on_submit
    harness
        .run_step(&PreviewStep::ClickType {
            type_name: "TextArea".into(),
            index: 1,
        })
        .unwrap();
    harness
        .run_step(&PreviewStep::TypeText("Alpha".into()))
        .unwrap();
    send_key(
        &mut harness,
        Key::Named(NamedKey::Enter),
        Code::Enter,
        Modifiers::SHIFT,
    );
    harness
        .run_step(&PreviewStep::TypeText("Beta".into()))
        .unwrap();
    assert_eq!(harness.state().shift_text, "Alpha\nBeta");
    assert_eq!(harness.state().submitted, None);

    send_key(
        &mut harness,
        Key::Named(NamedKey::Enter),
        Code::Enter,
        Modifiers::default(),
    );
    assert_eq!(harness.state().submitted.as_deref(), Some("Alpha\nBeta"));
}

#[test]
fn rich_prose_clicks_link_and_drags_selection() {
    #[derive(Default)]
    struct State {
        clicked_link: Option<String>,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
        |_s| {
            sized_box(
                rich_prose(vec![
                    TextSpan::new("Plain "),
                    TextSpan::bold("bold "),
                    TextSpan::code("fn() "),
                    TextSpan::link("doc-target", "Open Documentation Link Here"),
                ])
                .on_link_click(|s: &mut State, link_id| {
                    s.clicked_link = Some(link_id.to_string());
                }),
            )
            .padding(Padding::all(16.px()))
            .boxed()
        },
        PreviewConfig {
            width: 560,
            height: 240,
            ..PreviewConfig::default()
        },
    );

    harness.settle_animations();
    let rich_id = *harness
        .find_widget_ids("RichTextWidget")
        .first()
        .expect("RichTextWidget should mount");
    let (rx, ry, rw, _rh) = harness
        .widget_rect(rich_id)
        .expect("RichTextWidget should have bounds");

    for frac in [0.25, 0.35, 0.45, 0.55, 0.65, 0.78] {
        if harness.state().clicked_link.is_some() {
            break;
        }
        harness.click_point(rx + rw * frac, ry + 10.0);
    }
    assert_eq!(
        harness.state().clicked_link.as_deref(),
        Some("doc-target"),
        "clicking link span should emit LinkClicked(\"doc-target\")"
    );

    let inner = harness.inner_harness_mut();
    inner.mouse_move(Point::new(rx + 6.0, ry + 10.0));
    inner.mouse_button_press(Some(PointerButton::Primary));
    inner.mouse_move(Point::new(rx + rw * 0.25, ry + 10.0));
    inner.mouse_button_release(Some(PointerButton::Primary));
    harness.pump_actions();
    send_key(
        &mut harness,
        Key::Character("c".into()),
        Code::KeyC,
        Modifiers::CONTROL,
    );
    let _img = harness.render_image();
}

#[test]
fn context_menu_opens_on_secondary_click_selects_and_dismisses() {
    #[derive(Default)]
    struct State {
        selected: Option<usize>,
        dismissed: bool,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_s| {
            context_menu(
                sized_box(label("Right-click surface")).padding(Padding::all(28.px())),
                vec![
                    menu_item("Copy Item"),
                    menu_item("Rename Item"),
                    menu_divider(),
                    menu_item("Delete Item"),
                ],
                |s: &mut State, idx| s.selected = Some(idx),
            )
            .on_dismiss(|s: &mut State| s.dismissed = true)
            .boxed()
        },
        PreviewConfig {
            width: 520,
            height: 360,
            ..PreviewConfig::default()
        },
    );

    harness.settle_animations();
    let host_id = *harness
        .find_widget_ids("ContextMenuHost")
        .first()
        .expect("ContextMenuHost should mount");
    let (hx, hy, hw, hh) = harness
        .widget_rect(host_id)
        .expect("ContextMenuHost should have bounds");

    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(hx + hw * 0.5, hy + hh * 0.5));
        inner.mouse_button_press(Some(PointerButton::Secondary));
        inner.mouse_button_release(Some(PointerButton::Secondary));
    }
    harness.pump_actions();
    harness.settle_animations();

    let layers = harness.find_widget_ids("PopupLayer");
    assert_eq!(layers.len(), 1, "expected 1 PopupLayer after right-click");
    let (lx, ly, lw, lh) = harness
        .widget_rect(layers[0])
        .expect("PopupLayer should have bounds");
    let (vw, vh) = harness.viewport();
    assert!(
        lx >= 0.0 && ly >= 0.0 && lx + lw <= vw && ly + lh <= vh,
        "PopupLayer clipped outside viewport: ({lx}, {ly}, {lw}, {lh}) in ({vw}, {vh})"
    );

    harness
        .run_step(&PreviewStep::ClickLabel("Rename Item".into()))
        .unwrap();
    harness.settle_animations();
    assert_eq!(harness.state().selected, Some(1));
    assert!(
        harness.find_widget_ids("PopupLayer").is_empty(),
        "PopupLayer should close after item selection"
    );

    // Re-open via secondary click and dismiss by clicking outside.
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(hx + hw * 0.5, hy + hh * 0.5));
        inner.mouse_button_press(Some(PointerButton::Secondary));
        inner.mouse_button_release(Some(PointerButton::Secondary));
    }
    harness.pump_actions();
    harness.settle_animations();
    assert_eq!(harness.find_widget_ids("PopupLayer").len(), 1);

    harness.click_point(vw - 12.0, vh - 12.0);
    harness.settle_animations();
    assert!(
        harness.find_widget_ids("PopupLayer").is_empty(),
        "Clicking outside ContextMenu should close PopupLayer"
    );
    assert!(harness.state().dismissed);
}

#[test]
fn popover_anchor_opens_interactive_surface_and_dismisses() {
    #[derive(Default)]
    struct State {
        open: bool,
        anchor: Option<PopoverAnchorRect>,
        checked: bool,
        applied: usize,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |s| {
            let main = sized_box(popover_anchor(
                primary_button("Toggle Popover", |st: &mut State| {
                    st.open = !st.open;
                }),
                |st: &mut State, rect| {
                    if let Some(r) = rect {
                        st.anchor = Some(r);
                    } else {
                        st.open = false;
                        st.anchor = None;
                    }
                },
            ))
            .padding(Padding::all(20.px()))
            .alignment(UnitPoint::TOP_LEFT);

            let pop = s.open.then_some(s.anchor).flatten().map(|rect| {
                popover_surface(
                    rect,
                    flex_col((
                        checkbox("Compact", s.checked, |st: &mut State, v| st.checked = v),
                        primary_button("Apply Popover", |st: &mut State| {
                            st.applied += 1;
                            st.open = false;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(8.px()),
                    |st: &mut State| st.open = false,
                )
            });

            zstack((main, pop.map(|p| p.alignment(UnitPoint::TOP_LEFT)))).boxed()
        },
        PreviewConfig {
            width: 560,
            height: 400,
            ..PreviewConfig::default()
        },
    );

    harness
        .run_step(&PreviewStep::ClickLabel("Toggle Popover".into()))
        .unwrap();
    harness.settle_animations();
    assert!(harness.state().open, "popover should open on anchor click");
    assert!(
        !harness.find_widget_ids("PopoverPlacementWidget").is_empty(),
        "PopoverPlacementWidget should be mounted"
    );

    harness
        .run_step(&PreviewStep::ClickType {
            type_name: "UiCheckbox".into(),
            index: 0,
        })
        .unwrap();
    assert!(harness.state().checked);
    assert!(
        harness.state().open,
        "interacting inside popover must not dismiss it"
    );

    harness
        .run_step(&PreviewStep::ClickLabel("Apply Popover".into()))
        .unwrap();
    assert_eq!(harness.state().applied, 1);
    assert!(!harness.state().open);
    assert!(harness.find_widget_ids("PopoverPlacementWidget").is_empty());

    harness
        .run_step(&PreviewStep::ClickLabel("Toggle Popover".into()))
        .unwrap();
    harness.settle_animations();
    assert!(harness.state().open);
    let (vw, vh) = harness.viewport();
    harness.click_point(vw - 12.0, vh - 12.0);
    assert!(
        !harness.state().open,
        "outside click on transparent scrim should dismiss popover"
    );

    harness
        .run_steps(&[
            PreviewStep::ClickLabel("Toggle Popover".into()),
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".into(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
    assert!(!harness.state().open, "Escape key should dismiss popover");
}

#[test]
fn split_view_extreme_drag_cancel_asymmetric_mins_and_vertical() {
    #[derive(Default)]
    struct State {
        dark: bool,
    }

    // 1. Horizontal out-of-bounds drag, secondary button ignore, and PointerCancel recovery.
    let mut harness = build_ui_harness(
        State { dark: true },
        |s| {
            UiTheme::new(
                DesignLanguage::Md3,
                if s.dark {
                    Brightness::Dark
                } else {
                    Brightness::Light
                },
            )
        },
        |_s| {
            sized_box(
                split_view(
                    filled_card(label("Pane One Very Long Content")),
                    outlined_card(label("Pane Two Very Long Content")),
                )
                .split_point(0.5)
                .min_lengths(60.px(), 80.px()),
            )
            .dims(Dimensions::fixed(400.px(), 160.px()))
            .boxed()
        },
        PreviewConfig {
            width: 480,
            height: 240,
            ..PreviewConfig::default()
        },
    );
    harness.settle_animations();

    let tree = dump_layout_tree(&mut harness);
    let split = find_first_node_containing(&tree, "UiSplitWidget").unwrap();
    let mid_bar_x = split.children[0].rect.x + split.children[0].rect.width + 2.0;
    let mid_bar_y = split.rect.y + split.rect.height * 0.5;
    let orig_w = split.children[0].rect.width;

    // Secondary (right) mouse drag must NOT move the splitter bar.
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(mid_bar_x, mid_bar_y));
        inner.mouse_button_press(Some(PointerButton::Secondary));
        inner.mouse_move(Point::new(mid_bar_x - 80.0, mid_bar_y));
        inner.mouse_button_release(Some(PointerButton::Secondary));
    }
    harness.pump_actions();
    harness.settle_animations();
    let tree_sec = dump_layout_tree(&mut harness);
    let split_sec = find_first_node_containing(&tree_sec, "UiSplitWidget").unwrap();
    assert!(
        (split_sec.children[0].rect.width - orig_w).abs() < 1.0,
        "secondary click must ignore splitter drag"
    );

    // Extreme drag way past left edge (-500) -> clamped to min_first (60px).
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(mid_bar_x, mid_bar_y));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(-500.0, mid_bar_y));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    harness.pump_actions();
    harness.settle_animations();
    let tree_min = dump_layout_tree(&mut harness);
    let split_min = find_first_node_containing(&tree_min, "UiSplitWidget").unwrap();
    let w1_min = split_min.children[0].rect.width;
    assert!(
        (w1_min - 60.0).abs() <= 2.5,
        "expected left pane clamped near 60px, got {w1_min}"
    );

    // Drag to extreme right (+2000) -> clamped so right pane stays >= 80px.
    let bar_after_min = split_min.children[0].rect.x + w1_min + 2.0;
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(bar_after_min, mid_bar_y));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(2000.0, mid_bar_y));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    harness.pump_actions();
    harness.settle_animations();
    let tree_max = dump_layout_tree(&mut harness);
    let split_max = find_first_node_containing(&tree_max, "UiSplitWidget").unwrap();
    let w2_max = split_max.children[1].rect.width;
    assert!(
        (w2_max - 80.0).abs() <= 2.5,
        "expected right pane clamped near 80px, got {w2_max}"
    );
    assert!(
        split_max.children[0].rect.x + split_max.children[0].rect.width
            <= split_max.children[1].rect.x + 0.5,
        "clipped left pane must never overlap right pane"
    );

    // Start drag then send PointerEvent::Cancel -> moving mouse afterwards must NOT drag bar.
    let bar_max_x = split_max.children[0].rect.x + split_max.children[0].rect.width + 2.0;
    let saved_w = split_max.children[0].rect.width;
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(bar_max_x, mid_bar_y));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.process_pointer_event(PointerEvent::Cancel(PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }));
        inner.mouse_move(Point::new(bar_max_x - 120.0, mid_bar_y));
    }
    harness.pump_actions();
    harness.settle_animations();
    let tree_cancel = dump_layout_tree(&mut harness);
    let split_cancel = find_first_node_containing(&tree_cancel, "UiSplitWidget").unwrap();
    assert!(
        (split_cancel.children[0].rect.width - saved_w).abs() < 1.0,
        "PointerEvent::Cancel must abort active splitter drag"
    );

    // 2. Asymmetric min_lengths(200, 60) inside narrow 120px width + Vertical collapse-to-zero recovery.
    #[derive(Default)]
    struct NarrowSplitState;
    let mut tight_harness = build_ui_harness(
        NarrowSplitState,
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |_| {
            flex_col((
                sized_box(
                    split_view(label("A"), label("B"))
                        .min_lengths(200.px(), 60.px())
                        .split_point(0.5),
                )
                .dims(Dimensions::fixed(120.px(), 60.px())),
                sized_box(
                    split_view(label("Top"), label("Bottom"))
                        .axis(Axis::Vertical)
                        .min_lengths(0.px(), 0.px())
                        .split_point(0.5),
                )
                .dims(Dimensions::fixed(180.px(), 160.px())),
            ))
            .gap(8.px())
            .boxed()
        },
        PreviewConfig {
            width: 240,
            height: 360,
            ..PreviewConfig::default()
        },
    );
    tight_harness.settle_animations();
    let tight_tree = dump_layout_tree(&mut tight_harness);
    let mut splits = Vec::new();
    fn collect_splits<'a>(n: &'a LayoutNode, out: &mut Vec<&'a LayoutNode>) {
        if n.type_name.contains("UiSplitWidget") {
            out.push(n);
        }
        for c in &n.children {
            collect_splits(c, out);
        }
    }
    collect_splits(&tight_tree, &mut splits);
    assert_eq!(splits.len(), 2);
    let narrow_split = splits[0];
    assert!(
        narrow_split.children[0].rect.width + narrow_split.children[1].rect.width <= 120.5,
        "asymmetric min_lengths inside 120px must scale proportionally without overflow: {:?}",
        narrow_split.children
    );
    assert!(
        narrow_split.children[0].rect.width > narrow_split.children[1].rect.width,
        "200:60 asymmetric min ratio should favor first pane proportionally"
    );

    // Vertical split with min_lengths(0, 0): drag to collapse Top pane to 0, then drag back to 80px.
    let v_split = splits[1];
    let v_bar_x = v_split.rect.x + v_split.rect.width * 0.5;
    let v_bar_y = v_split.children[0].rect.y + v_split.children[0].rect.height + 2.0;
    {
        let inner = tight_harness.inner_harness_mut();
        inner.mouse_move(Point::new(v_bar_x, v_bar_y));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(v_bar_x, -200.0));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    tight_harness.pump_actions();
    tight_harness.settle_animations();
    let v_tree1 = dump_layout_tree(&mut tight_harness);
    let mut splits1 = Vec::new();
    collect_splits(&v_tree1, &mut splits1);
    assert!(
        splits1[1].children[0].rect.height <= 1.0,
        "min_lengths(0,0) should allow collapsing top pane to 0"
    );
    let top_y = splits1[1].rect.y;
    {
        let inner = tight_harness.inner_harness_mut();
        inner.mouse_move(Point::new(v_bar_x, top_y + 2.0));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(v_bar_x, top_y + 80.0));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    tight_harness.pump_actions();
    tight_harness.settle_animations();
    let v_tree2 = dump_layout_tree(&mut tight_harness);
    let mut splits2 = Vec::new();
    collect_splits(&v_tree2, &mut splits2);
    assert!(
        splits2[1].children[0].rect.height > 40.0,
        "collapsed splitter with min_lengths(0,0) must still be draggable open cleanly"
    );
}

#[test]
fn rich_prose_out_of_bounds_drag_and_narrow_wrap() {
    #[derive(Default)]
    struct State {
        clicked: Option<String>,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Adwaita, Brightness::Dark),
        |_| {
            flex_col((sized_box(
                rich_prose(vec![
                    TextSpan::new("Prefix text with "),
                    TextSpan::code("very_long_inline_code_identifier()"),
                    TextSpan::new(" followed by "),
                    TextSpan::link("wrapped-link", "Clickable Wrapped Link"),
                ])
                .on_link_click(|s: &mut State, id| s.clicked = Some(id.to_string())),
            )
            .dims(Dimensions::width(160.px()))
            .padding(Padding::all(10.px())),))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .boxed()
        },
        PreviewConfig {
            width: 240,
            height: 360,
            ..PreviewConfig::default()
        },
    );

    harness.settle_animations();
    let rich_id = harness.find_widget_ids("RichTextWidget")[0];
    let (rx, ry, rw, rh) = harness.widget_rect(rich_id).unwrap();
    assert!(
        rw <= 160.5 && rh > 36.0,
        "rich_prose should wrap vertically inside 160px without horizontal bleed: ({rw}, {rh})"
    );

    // Drag backwards past left edge into negative coordinates, release outside bounds.
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(rx + rw * 0.8, ry + 12.0));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(rx - 40.0, ry + rh + 30.0));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    harness.pump_actions();
    let _ = harness.render_image();

    // Probe wrapped link lines across lower rows to verify click still activates link.
    'outer: for y_frac in [0.45, 0.65, 0.82] {
        for x_frac in [0.25, 0.55, 0.8] {
            harness.click_point(rx + rw * x_frac, ry + rh * y_frac);
            if harness.state().clicked.is_some() {
                break 'outer;
            }
        }
    }
    assert_eq!(
        harness.state().clicked.as_deref(),
        Some("wrapped-link"),
        "wrapped multi-line link span should activate cleanly"
    );
}

#[test]
fn text_area_has_visible_chrome_and_fits_tight_viewports() {
    struct State {
        val: String,
        brightness: Brightness,
    }

    for (w, h, brightness) in [(320, 480, Brightness::Light), (640, 240, Brightness::Dark)] {
        let mut harness = build_ui_harness(
            State {
                val: "Multi-line body content\nSecond line".to_owned(),
                brightness,
            },
            |s| UiTheme::new(DesignLanguage::Md3, s.brightness),
            |s| {
                sized_box(
                    flex_col((
                        text_area(s.val.clone(), |st: &mut State, v| st.val = v)
                            .label("Notes")
                            .min_height(72.px())
                            .max_height(140.px()),
                        filled_text_area(s.val.clone(), |st: &mut State, v| st.val = v)
                            .label("Error Note")
                            .error_text("Required")
                            .min_height(56.px())
                            .max_height(100.px()),
                    ))
                    .gap(8.px()),
                )
                .padding(Padding::all(12.px()))
                .boxed()
            },
            PreviewConfig {
                width: w,
                height: h,
                ..PreviewConfig::default()
            },
        );
        harness.settle_animations();
        let tree = dump_layout_tree(&mut harness);
        let focus_frames: Vec<_> = {
            let mut out = Vec::new();
            fn walk<'a>(n: &'a LayoutNode, out: &mut Vec<&'a LayoutNode>) {
                if n.type_name == "FocusFrame" {
                    out.push(n);
                }
                for c in &n.children {
                    walk(c, out);
                }
            }
            walk(&tree, &mut out);
            out
        };
        assert_eq!(focus_frames.len(), 2);
        for ff in &focus_frames {
            assert!(
                ff.rect.x >= 0.0 && ff.rect.x + ff.rect.width <= f64::from(w) + 0.5,
                "text_area FocusFrame overflowed horizontally at {w}x{h}: {:?}",
                ff.rect
            );
            assert!(
                find_first_node(ff, "Flex").is_some()
                    && find_first_node(ff, "HeightRangeWidget").is_some(),
                "text_area FocusFrame should host class-styled Flex box + HeightRangeWidget"
            );
        }
    }
}

#[test]
fn popover_and_context_menu_fit_extreme_narrow_and_short_viewports() {
    #[derive(Default)]
    struct State {
        open: bool,
        anchor: Option<PopoverAnchorRect>,
        selected: Option<usize>,
    }

    let mut harness = build_ui_harness(
        State::default(),
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |s| {
            let trigger = context_menu(
                popover_anchor(
                    primary_button("Menu Anchor", |st: &mut State| st.open = !st.open),
                    |st: &mut State, rect| {
                        if let Some(r) = rect {
                            st.anchor = Some(r);
                        } else {
                            st.open = false;
                            st.anchor = None;
                        }
                    },
                ),
                vec![
                    menu_item("Very Long Contextual Menu Item Label Action One"),
                    menu_item("Action Two"),
                ],
                |st: &mut State, idx| st.selected = Some(idx),
            );

            let main = sized_box(trigger)
                .padding(Padding::all(12.px()))
                .alignment(UnitPoint::BOTTOM_RIGHT);

            let pop = s.open.then_some(s.anchor).flatten().map(|rect| {
                popover_surface(
                    rect,
                    flex_col((
                        title_small("Wide Configuration Header Title"),
                        body_medium("A long explanatory description that should wrap cleanly inside 240px and 320px viewports without clipping."),
                        primary_button("Done", |st: &mut State| st.open = false),
                    ))
                    .gap(8.px()),
                    |st: &mut State| st.open = false,
                )
            });

            zstack((main, pop.map(|p| p.alignment(UnitPoint::TOP_LEFT)))).boxed()
        },
        PreviewConfig {
            width: 800,
            height: 600,
            ..PreviewConfig::default()
        },
    );

    // Open popover on 800x600 then live-resize to narrow 320x480 and 240x360.
    harness
        .run_step(&PreviewStep::ClickLabel("Menu Anchor".into()))
        .unwrap();
    for (rw, rh) in [(320_u32, 480_u32), (240_u32, 360_u32), (640_u32, 240_u32)] {
        harness.resize(rw, rh);
        harness.settle_animations();
        let tree = dump_layout_tree(&mut harness);
        let placed = find_first_node(&tree, "PopoverPlacementWidget").unwrap();
        let card = &placed.children[0];
        assert!(
            card.rect.x >= 0.0
                && card.rect.y >= 0.0
                && card.rect.x + card.rect.width <= f64::from(rw) + 0.5
                && card.rect.y + card.rect.height <= f64::from(rh) + 0.5,
            "popover card overflowed viewport {rw}x{rh}: {:?}",
            card.rect
        );
    }

    // Dismiss popover via top-left 8px margin outside card, resize to 240x360, trigger context_menu near bottom-right corner and check bounds.
    harness.resize(240, 360);
    harness.settle_animations();
    harness.click_point(2.0, 2.0);
    harness.settle_animations();
    assert!(!harness.state().open);

    let host_id = harness.find_widget_ids("ContextMenuHost")[0];
    let (hx, hy, hw, hh) = harness.widget_rect(host_id).unwrap();
    {
        let inner = harness.inner_harness_mut();
        inner.mouse_move(Point::new(hx + hw * 0.5, hy + hh * 0.5));
        inner.mouse_button_press(Some(PointerButton::Secondary));
        inner.mouse_button_release(Some(PointerButton::Secondary));
    }
    harness.pump_actions();
    harness.settle_animations();

    let layers = harness.find_widget_ids("PopupLayer");
    assert_eq!(layers.len(), 1);
    let (lx, ly, lw, lh) = harness.widget_rect(layers[0]).unwrap();
    assert!(
        lx >= 0.0 && ly >= 0.0 && lx + lw <= 240.5 && ly + lh <= 360.5,
        "PopupLayer overflowed 240x360 viewport: ({lx}, {ly}, {lw}, {lh})"
    );
}

#[test]
fn wrap_handles_extreme_narrow_120px_and_60px_without_overflow() {
    struct State {
        max_w: f64,
    }
    for container_w in [120.0_f64, 60.0_f64] {
        let mut harness = build_ui_harness(
            State { max_w: container_w },
            |_| UiTheme::new(DesignLanguage::Hig, Brightness::Light),
            |s| {
                let tags: Vec<_> = ["Alpha", "LongerChipLabel", "B", "Gamma"]
                    .into_iter()
                    .map(|t| assist_chip(t, |_| {}))
                    .collect();
                flex_col((sized_box(wrap(tags).gap(6.px()).run_gap(6.px()))
                    .dims(Dimensions::width(s.max_w.px())),))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .boxed()
            },
            PreviewConfig {
                width: (container_w as u32).max(80),
                height: 360,
                ..PreviewConfig::default()
            },
        );
        harness.settle_animations();
        let tree = dump_layout_tree(&mut harness);
        let wnode = find_first_node(&tree, "WrapWidget").unwrap();
        assert!(
            wnode.rect.width <= container_w + 0.5,
            "WrapWidget exceeded container width {container_w}: {:?}",
            wnode.rect
        );
        for ch in &wnode.children {
            assert!(
                ch.rect.width <= wnode.rect.width + 0.5,
                "wrap child wider than container {container_w}: {:?} > {:?}",
                ch.rect,
                wnode.rect
            );
        }
    }
}
