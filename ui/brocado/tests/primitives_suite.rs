//! Headless integration tests for the 6 standard UI library primitives:
//! `wrap`, `split_view`, `text_area`, `rich_prose`, `context_menu`, and `popover`.

use brocado::prelude::*;
use brocado_preview::diff::{ImageDiffTolerance, compare_images};
use brocado_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use brocado_preview::inspect::{LayoutNode, dump_layout_tree, lint_layout};
use masonry::core::keyboard::{Code, Key, KeyState, Modifiers, NamedKey};
use masonry::core::pointer::{PointerId, PointerInfo, PointerType};
use masonry::core::{
    KeyboardEvent, PointerButton, PointerEvent, PointerScrollEvent, PointerState, ScrollDelta,
    TextEvent,
};
use masonry::dpi::LogicalPosition;
use masonry::kurbo::{Axis, Point, Vec2};
use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::{Background, CornerRadius, Dimensions, Padding};
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, ZStackExt, flex_col, flex_row, label, sized_box, zstack,
};

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
                    && find_first_node(ff, "ConstrainedBox").is_some(),
                "text_area FocusFrame should host class-styled Flex box + ConstrainedBox"
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

fn find_node_by_text<'a>(node: &'a LayoutNode, text: &str) -> Option<&'a LayoutNode> {
    if node.text.as_deref() == Some(text) {
        return Some(node);
    }
    for ch in &node.children {
        if let Some(found) = find_node_by_text(ch, text) {
            return Some(found);
        }
    }
    None
}

fn assert_all_descendants_within(node: &LayoutNode, max_right: f64, label_ctx: &str) {
    assert!(
        node.rect.x + node.rect.width <= max_right + 0.5,
        "{label_ctx} descendant {} overflowed x + w > {max_right}: {:?}",
        node.type_name,
        node.rect
    );
    for ch in &node.children {
        assert_all_descendants_within(ch, max_right, label_ctx);
    }
}

#[test]
fn scaffold_fab_and_toast_avoid_bottom_bar() {
    #[derive(Default)]
    struct State;

    let mut harness = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_| {
            scaffold(label("Body"))
                .top_bar(top_app_bar(TopAppBarVariant::Small, "App"))
                .bottom_bar(bottom_app_bar(
                    vec![icon_button(icons::HOME, |_| {}).boxed()],
                    None,
                ))
                .floating_action(fab(icons::ADD, FabColor::Primary, |_| {}))
                .toast(toast(
                    &ToastData {
                        id: 1,
                        message: "Saved".into(),
                        action_label: None,
                        duration: ToastDuration::Short,
                        with_close: false,
                    },
                    None,
                    None,
                ))
                .build()
                .boxed()
        },
        PreviewConfig {
            width: 480,
            height: 640,
            ..PreviewConfig::default()
        },
    );
    harness.settle_animations();
    let tree = dump_layout_tree(&mut harness);
    // Root scaffold ZStack -> child 0 is `base_col` (`Flex`) containing [top_bar, body_area, bottom_bar].
    let root_flex = find_first_node(&tree, "Flex").expect("scaffold Flex root missing");
    assert_eq!(
        root_flex.children.len(),
        3,
        "expected top_bar, body_area, bottom_bar in scaffold Flex"
    );
    let body_area = &root_flex.children[1];
    let bottom_bar = &root_flex.children[2];
    let bottom_bar_top = bottom_bar.rect.y;

    let fab_node = find_first_node(body_area, "Button").expect("FAB button missing from body_area");
    let toast_label = find_node_by_text(body_area, "Saved").expect("Toast label missing");
    assert!(fab_node.rect.width > 1.0 && fab_node.rect.height > 1.0);
    assert!(toast_label.rect.width > 1.0 && toast_label.rect.height > 1.0);

    assert!(
        fab_node.rect.y + fab_node.rect.height <= bottom_bar_top + 0.5,
        "FAB overlaps bottom_bar: fab={:?}, bottom_bar={:?}",
        fab_node.rect,
        bottom_bar.rect
    );
    assert!(
        toast_label.rect.y + toast_label.rect.height <= bottom_bar_top + 0.5,
        "Toast overlaps bottom_bar: toast={:?}, bottom_bar={:?}",
        toast_label.rect,
        bottom_bar.rect
    );
    assert!(
        fab_node.rect.x >= 0.0
            && fab_node.rect.y >= 0.0
            && fab_node.rect.x + fab_node.rect.width <= 480.5,
        "FAB outside viewport: {:?}",
        fab_node.rect
    );
}

#[test]
fn adaptive_dialogs_locale_and_font_scale() {
    struct State {
        scale: f32,
        date: CalendarDate,
        time: TimeOfDay,
    }

    let make_date_harness = |scale: f32| {
        build_ui_harness(
            State {
                scale,
                date: CalendarDate::new(2024, 1, 15),
                time: TimeOfDay::new(9, 30),
            },
            |s| {
                UiTheme::new(DesignLanguage::Md3, Brightness::Light)
                    .with_locale(UiLocale::zh_cn())
                    .with_font_scale(s.scale)
            },
            |s| {
                let d = s.date;
                date_picker_dialog(d, |st: &mut State, new_d| st.date = new_d, |_| {}, |_| {})
                    .boxed()
            },
            PreviewConfig {
                width: 640,
                height: 640,
                ..PreviewConfig::default()
            },
        )
    };

    let mut h_100 = make_date_harness(1.0);
    h_100.settle_animations();
    let tree_100 = dump_layout_tree(&mut h_100);
    let modal_box_100 = &find_first_node(&tree_100, "SlideTransition")
        .expect("dialog SlideTransition missing at 1.0")
        .children[0];

    let mut h_150 = make_date_harness(1.5);
    h_150.settle_animations();
    let tree_150 = dump_layout_tree(&mut h_150);
    let modal_box_150 = &find_first_node(&tree_150, "SlideTransition")
        .expect("dialog SlideTransition missing at 1.5")
        .children[0];

    assert!(
        modal_box_100.rect.x + modal_box_100.rect.width <= 640.5
            && modal_box_100.rect.y + modal_box_100.rect.height <= 640.5,
        "1.0x date_picker_dialog overflowed 640x640: {:?}",
        modal_box_100.rect
    );
    assert!(
        modal_box_150.rect.x + modal_box_150.rect.width <= 640.5
            && modal_box_150.rect.y + modal_box_150.rect.height <= 640.5,
        "1.5x date_picker_dialog overflowed 640x640: {:?}",
        modal_box_150.rect
    );
    assert!(
        modal_box_150.rect.height > modal_box_100.rect.height + 8.0,
        "expected font_scale(1.5) to enlarge date dialog height: 1.0x={:?}, 1.5x={:?}",
        modal_box_100.rect,
        modal_box_150.rect
    );

    assert!(
        find_node_by_text(&tree_100, "确定").is_some(),
        "expected zh_cn ok_label '确定' in date_picker_dialog"
    );
    assert!(
        find_node_by_text(&tree_100, "取消").is_some(),
        "expected zh_cn cancel_label '取消' in date_picker_dialog"
    );
    assert!(
        find_node_by_text(&tree_100, "一").is_some(),
        "expected zh_cn weekday header '一' in date_picker_dialog"
    );
    assert!(
        find_node_by_text(&tree_100, "1月 2024").is_some(),
        "expected zh_cn month header '1月 2024' in date_picker_dialog"
    );

    let mut h_time = build_ui_harness(
        State {
            scale: 1.5,
            date: CalendarDate::new(2024, 1, 15),
            time: TimeOfDay::new(9, 30),
        },
        |s| {
            UiTheme::new(DesignLanguage::Md3, Brightness::Light)
                .with_locale(UiLocale::zh_cn())
                .with_font_scale(s.scale)
        },
        |s| {
            let t = s.time;
            time_picker_dialog(t, |st: &mut State, new_t| st.time = new_t, |_| {}, |_| {}).boxed()
        },
        PreviewConfig {
            width: 640,
            height: 640,
            ..PreviewConfig::default()
        },
    );
    h_time.settle_animations();
    let tree_time = dump_layout_tree(&mut h_time);
    let modal_time = &find_first_node(&tree_time, "SlideTransition")
        .expect("time dialog SlideTransition missing")
        .children[0];
    assert!(
        modal_time.rect.x + modal_time.rect.width <= 640.5
            && modal_time.rect.y + modal_time.rect.height <= 640.5,
        "1.5x time_picker_dialog overflowed 640x640: {:?}",
        modal_time.rect
    );
    assert!(
        find_node_by_text(&tree_time, "确定").is_some(),
        "expected zh_cn ok_label '确定' in time_picker_dialog"
    );
}

#[test]
fn adw_alert_dialog_switches_by_content_width() {
    struct State {
        cancel_label: &'static str,
        confirm_label: &'static str,
    }

    let run_alert = |cancel_label: &'static str, confirm_label: &'static str| {
        let mut h = build_ui_harness(
            State {
                cancel_label,
                confirm_label,
            },
            |_| UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
            |s| {
                alert_dialog(
                    "Delete?",
                    "Confirm action.",
                    s.cancel_label,
                    s.confirm_label,
                    |_| {},
                    |_| {},
                )
                .boxed()
            },
            PreviewConfig {
                width: 340,
                height: 480,
                ..PreviewConfig::default()
            },
        );
        h.settle_animations();
        dump_layout_tree(&mut h)
    };

    let short_tree = run_alert("No", "Yes");
    let short_row = find_first_node(&short_tree, "AdwResponseRowWidget")
        .expect("short AdwResponseRowWidget missing");
    assert_eq!(short_row.children.len(), 2);
    let sb0 = short_row.children[0].rect;
    let sb1 = short_row.children[1].rect;
    assert!(
        (sb0.y - sb1.y).abs() <= 1.0 && sb1.x >= sb0.x + sb0.width - 0.5,
        "expected horizontal responses for short labels at 340px: b0={sb0:?}, b1={sb1:?}"
    );

    let long_tree = run_alert("Keep Existing Document", "Permanently Delete Everything");
    let long_row = find_first_node(&long_tree, "AdwResponseRowWidget")
        .expect("long AdwResponseRowWidget missing");
    assert_eq!(long_row.children.len(), 2);
    let lb0 = long_row.children[0].rect;
    let lb1 = long_row.children[1].rect;
    assert!(
        lb1.y >= lb0.y + lb0.height - 0.5 || lb0.y >= lb1.y + lb1.height - 0.5,
        "expected stacked vertical responses for long labels at 340px: b0={lb0:?}, b1={lb1:?}"
    );
    assert!(
        lb0.x + lb0.width <= 340.5 && lb1.x + lb1.width <= 340.5 && lb1.y + lb1.height <= 480.5,
        "stacked alert buttons overflowed 340x480: b0={lb0:?}, b1={lb1:?}"
    );
}

#[test]
fn sidebars_and_inspector_clamp_in_narrow_containers() {
    #[derive(Default)]
    struct State;

    let mut inspector_h = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Hig, Brightness::Light),
        |_| inspector_panel("Inspector", vec![label("Item").boxed()]).boxed(),
        PreviewConfig {
            width: 160,
            height: 480,
            ..PreviewConfig::default()
        },
    );
    inspector_h.settle_animations();
    let inspector_tree = dump_layout_tree(&mut inspector_h);
    let inspector_box = find_first_node(&inspector_tree, "ConstrainedBox")
        .expect("inspector ConstrainedBox missing");
    assert!(
        inspector_box.rect.width <= 160.5,
        "inspector_panel overflowed 160px container: {:?}",
        inspector_box.rect
    );
    assert_all_descendants_within(inspector_box, 160.0, "inspector_panel@160");

    let mut drawer_narrow = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_| {
            navigation_drawer(
                Some("Nav"),
                &[NavDestination::new("Home", icons::HOME)],
                0,
                |_, _| {},
            )
            .boxed()
        },
        PreviewConfig {
            width: 160,
            height: 480,
            ..PreviewConfig::default()
        },
    );
    drawer_narrow.settle_animations();
    let drawer_narrow_tree = dump_layout_tree(&mut drawer_narrow);
    let drawer_narrow_box = find_first_node(&drawer_narrow_tree, "ConstrainedBox")
        .expect("drawer ConstrainedBox missing at 160px");
    assert!(
        drawer_narrow_box.rect.width <= 160.5,
        "navigation_drawer overflowed 160px container: {:?}",
        drawer_narrow_box.rect
    );
    assert_all_descendants_within(drawer_narrow_box, 160.0, "navigation_drawer@160");

    let mut drawer_desktop = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_| {
            let drawer = navigation_drawer(
                Some("Nav"),
                &[NavDestination::new("Home", icons::HOME)],
                0,
                |_, _| {},
            );
            flex_row((drawer, label("Main").flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .boxed()
        },
        PreviewConfig {
            width: 800,
            height: 480,
            ..PreviewConfig::default()
        },
    );
    drawer_desktop.settle_animations();
    let drawer_desktop_tree = dump_layout_tree(&mut drawer_desktop);
    let drawer_desktop_box = find_first_node(&drawer_desktop_tree, "ConstrainedBox")
        .expect("drawer ConstrainedBox missing at 800px");
    assert!(
        drawer_desktop_box.rect.width >= 240.0,
        "navigation_drawer failed to expand to desktop width at 800px: {:?}",
        drawer_desktop_box.rect
    );
}

#[test]
fn time_picker_dialog_has_no_phantom_bottom_whitespace() {
    fn find_button_containing_text<'a>(node: &'a LayoutNode, text: &str) -> Option<&'a LayoutNode> {
        if node.type_name == "Button" && find_node_by_text(node, text).is_some() {
            return Some(node);
        }
        for ch in &node.children {
            if let Some(found) = find_button_containing_text(ch, text) {
                return Some(found);
            }
        }
        None
    }

    struct State {
        time: TimeOfDay,
    }

    let mut h = build_ui_harness(
        State {
            time: TimeOfDay::new(16, 0),
        },
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |s| {
            let t = s.time;
            time_picker_dialog(t, |st: &mut State, new_t| st.time = new_t, |_| {}, |_| {}).boxed()
        },
        PreviewConfig {
            width: 1200,
            height: 800,
            ..PreviewConfig::default()
        },
    );
    h.settle_animations();
    let tree = dump_layout_tree(&mut h);
    let card = &find_first_node(&tree, "SlideTransition")
        .expect("SlideTransition missing")
        .children[0];
    let ok_btn = find_button_containing_text(card, "OK").expect("'OK' button missing");
    let bottom_gap = (card.rect.y + card.rect.height) - (ok_btn.rect.y + ok_btn.rect.height);
    assert!(
        bottom_gap <= 24.0 && card.rect.height <= 230.0,
        "time_picker_dialog has phantom bottom whitespace (gap={bottom_gap:.2}): card={:?}, ok={:?}",
        card.rect,
        ok_btn.rect
    );
}

#[test]
fn scroll_view_wheel_and_scrollbar_drag_update_viewport() {
    struct State {
        visibility: ScrollBarVisibility,
    }

    let make_harness = |visibility: ScrollBarVisibility| {
        let mut h = build_ui_harness(
            State { visibility },
            |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
            |s| {
                scroll_view(
                    flex_col((
                        label("ScrollTopMarker"),
                        sized_box(label("Spacer")).dims(Dimensions::height(800.px())),
                        label("ScrollBottomMarker"),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start),
                )
                .constrain_horizontal(true)
                .scrollbar_visibility(s.visibility)
                .boxed()
            },
            PreviewConfig {
                width: 320,
                height: 200,
                ..PreviewConfig::default()
            },
        );
        h.settle_animations();
        h
    };

    let mut h = make_harness(ScrollBarVisibility::OverlayAlways);
    let tree0 = dump_layout_tree(&mut h);
    let top0 = find_node_by_text(&tree0, "ScrollTopMarker")
        .expect("ScrollTopMarker missing")
        .rect;
    let bar0 = find_first_node(&tree0, "UiScrollBar").expect("OverlayAlways UiScrollBar missing");
    assert!(
        bar0.rect.width > 1.0 && bar0.rect.height > 100.0,
        "unexpected UiScrollBar rect: {:?}",
        bar0.rect
    );

    // Wheel scroll down
    {
        let inner = h.inner_harness_mut();
        inner.mouse_move(Point::new(100.0, 100.0));
        inner.mouse_wheel(Vec2::new(0.0, -120.0));
    }
    h.pump_actions();
    h.settle_animations();

    let tree_wheel = dump_layout_tree(&mut h);
    let top_wheel = find_node_by_text(&tree_wheel, "ScrollTopMarker")
        .expect("ScrollTopMarker missing after wheel")
        .rect;
    assert!(
        top_wheel.y < top0.y - 80.0,
        "expected wheel down to shift content upward: before={}, after={}",
        top0.y,
        top_wheel.y
    );

    // Drag vertical scrollbar thumb downward
    let bar_rect = find_first_node(&tree_wheel, "UiScrollBar")
        .expect("UiScrollBar missing before drag")
        .rect;
    {
        let inner = h.inner_harness_mut();
        let cx = bar_rect.x + bar_rect.width * 0.5;
        inner.mouse_move(Point::new(cx, bar_rect.y + 8.0));
        inner.mouse_button_press(Some(PointerButton::Primary));
        inner.mouse_move(Point::new(cx, bar_rect.y + bar_rect.height - 8.0));
        inner.mouse_button_release(Some(PointerButton::Primary));
    }
    h.pump_actions();
    h.settle_animations();

    let tree_drag = dump_layout_tree(&mut h);
    let top_drag = find_node_by_text(&tree_drag, "ScrollTopMarker")
        .expect("ScrollTopMarker missing after drag")
        .rect;
    assert!(
        top_drag.y < top_wheel.y - 20.0,
        "expected scrollbar drag to scroll further down: wheel={}, drag={}",
        top_wheel.y,
        top_drag.y
    );

    // `Never` visibility hides scrollbars
    let mut h_never = make_harness(ScrollBarVisibility::Never);
    let tree_never = dump_layout_tree(&mut h_never);
    assert!(
        find_first_node(&tree_never, "UiScrollBar").is_none(),
        "expected ScrollBarVisibility::Never to stash UiScrollBar widgets"
    );
}

#[test]
fn scroll_view_reserved_mode_reserves_track_width_and_applies_custom_style() {
    struct State {
        style: ScrollBarStyle,
    }

    let make_harness = |style: ScrollBarStyle| {
        let mut h = build_ui_harness(
            State { style },
            |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
            |s| {
                scroll_view(
                    flex_col((
                        label("RowA"),
                        sized_box(label("")).dims(Dimensions::height(500.px())),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                )
                .constrain_horizontal(true)
                .scrollbar_style(s.style)
                .boxed()
            },
            PreviewConfig {
                width: 300,
                height: 160,
                ..PreviewConfig::default()
            },
        );
        h.settle_animations();
        h
    };

    let overlay_style =
        ScrollBarStyle::default().with_visibility(ScrollBarVisibility::OverlayAlways);
    let mut h_overlay = make_harness(overlay_style);
    let tree_overlay = dump_layout_tree(&mut h_overlay);
    let sv_overlay = find_first_node(&tree_overlay, "ScrollViewWidget").unwrap();
    let child_overlay = &sv_overlay.children[0];

    let reserved_style = ScrollBarStyle::default()
        .with_visibility(ScrollBarVisibility::Reserved)
        .with_thickness(8.0, 14.0)
        .with_padding(3.0);
    assert!((reserved_style.reserved_size() - 20.0).abs() < 1e-6);

    let mut h_reserved = make_harness(reserved_style);
    let tree_reserved = dump_layout_tree(&mut h_reserved);
    let sv_reserved = find_first_node(&tree_reserved, "ScrollViewWidget").unwrap();
    let child_reserved = &sv_reserved.children[0];
    let bar_reserved = find_first_node(&tree_reserved, "UiScrollBar").unwrap();

    assert!(
        (bar_reserved.rect.width - 20.0).abs() <= 0.5,
        "expected custom reserved UiScrollBar width ~20.0, got {:?}",
        bar_reserved.rect
    );
    assert!(
        (child_overlay.rect.width - child_reserved.rect.width - 20.0).abs() <= 0.5,
        "expected Reserved mode to narrow constrained child by 20px: overlay={:?}, reserved={:?}",
        child_overlay.rect,
        child_reserved.rect
    );
}

#[test]
fn scroll_view_shift_wheel_and_focus_pan_to_child() {
    #[derive(Default)]
    struct State;

    // 1. Shift+Wheel scrolls horizontally when vertically constrained
    let mut h_horiz = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |_| {
            scroll_view(sized_box(label("WideStart")).dims(Dimensions::fixed(900.px(), 60.px())))
                .constrain_vertical(true)
                .boxed()
        },
        PreviewConfig {
            width: 240,
            height: 120,
            ..PreviewConfig::default()
        },
    );
    h_horiz.settle_animations();
    let tree_h0 = dump_layout_tree(&mut h_horiz);
    let wide0 = find_node_by_text(&tree_h0, "WideStart").unwrap().rect;
    {
        let inner = h_horiz.inner_harness_mut();
        let scale = inner.render_root().scale_factor();
        let ptr_pos = LogicalPosition::new(100.0, 60.0).to_physical(scale);
        inner.mouse_move(Point::new(100.0, 60.0));
        inner.process_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            delta: ScrollDelta::PixelDelta(LogicalPosition::new(0.0, -120.0).to_physical(scale)),
            state: PointerState {
                modifiers: Modifiers::SHIFT,
                position: ptr_pos,
                ..PointerState::default()
            },
        }));
    }
    h_horiz.pump_actions();
    h_horiz.settle_animations();
    let tree_h1 = dump_layout_tree(&mut h_horiz);
    let wide1 = find_node_by_text(&tree_h1, "WideStart").unwrap().rect;
    assert!(
        wide1.x < wide0.x - 80.0,
        "expected Shift+Wheel to scroll horizontally: before={}, after={}",
        wide0.x,
        wide1.x
    );

    // 2. Tab focus onto an offscreen button triggers RequestPanToChild into viewport
    let mut h_pan = build_ui_harness(
        State,
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_| {
            scroll_view(
                flex_col(vec![
                    primary_button("Item0", |_| {}).boxed(),
                    primary_button("Item1", |_| {}).boxed(),
                    primary_button("Item2", |_| {}).boxed(),
                    primary_button("Item3", |_| {}).boxed(),
                    primary_button("Item4", |_| {}).boxed(),
                    primary_button("Item5", |_| {}).boxed(),
                    primary_button("Item6", |_| {}).boxed(),
                    primary_button("Item7", |_| {}).boxed(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px()),
            )
            .constrain_horizontal(true)
            .boxed()
        },
        PreviewConfig {
            width: 240,
            height: 120,
            ..PreviewConfig::default()
        },
    );
    h_pan.settle_animations();
    let tree_pan0 = dump_layout_tree(&mut h_pan);
    let item7_before = find_node_by_text(&tree_pan0, "Item7").unwrap().rect;
    assert!(
        item7_before.y > 120.0,
        "Item7 must start outside 120px viewport: {item7_before:?}"
    );

    let btn_ids = h_pan.find_widget_ids("Button");
    let item7_id = *btn_ids.last().expect("Item7 button id missing");
    h_pan.inner_harness_mut().scroll_into_view(item7_id);
    h_pan.pump_actions();
    h_pan.settle_animations();

    let tree_pan1 = dump_layout_tree(&mut h_pan);
    let item7_after = find_node_by_text(&tree_pan1, "Item7").unwrap().rect;
    assert!(
        item7_after.y >= -0.5 && item7_after.y + item7_after.height <= 120.5,
        "expected RequestPanToChild to bring Item7 inside 0..120 viewport: before={item7_before:?}, after={item7_after:?}"
    );
}

#[test]
fn compile_time_bounded_scaffold_zero_overflow_on_narrow_and_short_viewports() {
    #[derive(Default)]
    struct BoundedState;

    fn build_bounded_scene(_: &mut BoundedState) -> impl WidgetView<BoundedState> + Bounded {
        let dynamic_rows: Vec<BoundedXAnyView<BoundedState>> = vec![
            ui_label(
                TypeRole::BodyMedium,
                "Compile-time bounded paragraph that wraps across ultra-narrow widths without pushing horizontal overflow.",
            )
            .boxed_bounded_x(),
            adaptive_pair(
                ui_label(TypeRole::LabelLarge, "Trailing Action"),
                primary_button("Apply", |_| {}),
            )
            .boxed_bounded_x(),
        ];

        let chip_strip = scroll_horizontal(
            row((
                primary_button("Alpha", |_| {}),
                secondary_button("Beta", |_| {}),
                outlined_button("Gamma", |_| {}),
            ))
            .gap(8.px()),
        )
        .assert_bounded();

        let wrapped = wrap((
            primary_button("One", |_| {}),
            secondary_button("Two", |_| {}),
            outlined_button("Three", |_| {}),
        ));

        let card_view = bounded_card(
            CardVariant::Elevated,
            column((wrapped, chip_strip, column(dynamic_rows).gap(8.px())))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
        );

        let body = scroll_vertical(
            container(constrained_box(card_view).max_width(420.0)).padding(Padding::all(12.px())),
        );

        bounded_scaffold(body)
            .top_bar(clip_box(ui_label(
                TypeRole::TitleMedium,
                "Bounded Scaffold Title",
            )))
            .dialog(Some(bounded_dialog(
                ui_label(
                    TypeRole::BodyMedium,
                    "Modal content guaranteed horizontally bounded",
                ),
                |_| {},
            )))
            .build()
    }

    for &(w, h) in &[(360_u32, 480_u32), (200_u32, 240_u32), (140_u32, 160_u32)] {
        let mut harness = build_ui_harness(
            BoundedState,
            |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
            |st| build_bounded_scene(st).boxed(),
            PreviewConfig {
                width: w,
                height: h,
                ..PreviewConfig::default()
            },
        );
        harness.settle_animations();
        let tree = dump_layout_tree(&mut harness);
        let issues = lint_layout(&tree, (f64::from(w), f64::from(h)));
        let overflow_issues: Vec<_> = issues
            .iter()
            .filter(|issue| issue.kind == "parent_overflow" || issue.kind == "offscreen")
            .collect();
        assert!(
            overflow_issues.is_empty(),
            "expected zero overflow/offscreen issues at viewport ({w}x{h}), got: {overflow_issues:?}"
        );
    }
}

#[test]
fn scroll_view_and_constrained_box_type_state_runtime_parity() {
    #[derive(Default)]
    struct ParityState;

    // 1. scroll_horizontal permits horizontal wheel scroll on a Bounded row while preserving Bounded
    let mut h_horiz = build_ui_harness(
        ParityState,
        |_| UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
        |_| {
            bounded(scroll_horizontal(
                row((
                    primary_button("FirstWideActionButton", |_| {}),
                    primary_button("SecondWideActionButton", |_| {}),
                    primary_button("ThirdWideActionButton", |_| {}),
                ))
                .gap(16.px()),
            ))
            .boxed()
        },
        PreviewConfig {
            width: 180,
            height: 80,
            ..PreviewConfig::default()
        },
    );
    h_horiz.settle_animations();
    let before = find_node_by_text(&dump_layout_tree(&mut h_horiz), "FirstWideActionButton")
        .unwrap()
        .rect
        .x;
    {
        let inner = h_horiz.inner_harness_mut();
        let scale = inner.render_root().scale_factor();
        let ptr_pos = LogicalPosition::new(90.0, 40.0).to_physical(scale);
        inner.mouse_move(Point::new(90.0, 40.0));
        inner.process_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            delta: ScrollDelta::PixelDelta(LogicalPosition::new(0.0, -100.0).to_physical(scale)),
            state: PointerState {
                modifiers: Modifiers::SHIFT,
                position: ptr_pos,
                ..PointerState::default()
            },
        }));
    }
    h_horiz.pump_actions();
    h_horiz.settle_animations();
    let after = find_node_by_text(&dump_layout_tree(&mut h_horiz), "FirstWideActionButton")
        .unwrap()
        .rect
        .x;
    assert!(
        after < before - 40.0,
        "expected scroll_horizontal row to pan horizontally: before={before}, after={after}"
    );

    // 2. constrained_box max_width preserves BoundedX and caps card width cleanly
    let mut h_cbox = build_ui_harness(
        ParityState,
        |_| UiTheme::new(DesignLanguage::Hig, Brightness::Dark),
        |_| {
            scroll_vertical(
                constrained_box(bounded_card(
                    CardVariant::Outlined,
                    ui_label(
                        TypeRole::BodyMedium,
                        "Long label inside max-width constrained card",
                    ),
                ))
                .max_width(220.0),
            )
            .assert_bounded()
            .boxed()
        },
        PreviewConfig {
            width: 480,
            height: 240,
            ..PreviewConfig::default()
        },
    );
    h_cbox.settle_animations();
    let tree = dump_layout_tree(&mut h_cbox);
    let cbox_node = find_first_node(&tree, "ConstrainedBox").expect("ConstrainedBox missing");
    assert!(
        cbox_node.rect.width <= 220.5,
        "ConstrainedBox must respect 220px max_width: {:?}",
        cbox_node.rect
    );
}

#[test]
fn fluent_collapsed_navigation_view_aligns_toggle_and_items() {
    #[derive(Default)]
    struct NavState;

    fn collect_nodes<'a>(node: &'a LayoutNode, type_name: &str, out: &mut Vec<&'a LayoutNode>) {
        if node.type_name == type_name {
            out.push(node);
        }
        for c in &node.children {
            collect_nodes(c, type_name, out);
        }
    }

    let mut harness = build_ui_harness(
        NavState,
        |_| UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        |_| {
            let primary = [
                NavDestination::new("Home", icons::HOME),
                NavDestination::new("Explore", icons::EXPLORE),
            ];
            let footer = [NavDestination::new("Settings", icons::SETTINGS)];
            navigation_view_collapsible(
                Some("Fluent"),
                &primary,
                &footer,
                0,
                true,
                |_| {},
                |_, _| {},
            )
            .boxed()
        },
        PreviewConfig {
            width: 320,
            height: 360,
            ..PreviewConfig::default()
        },
    );
    harness.settle_animations();
    let tree = dump_layout_tree(&mut harness);

    let mut buttons = Vec::new();
    collect_nodes(&tree, "Button", &mut buttons);
    assert_eq!(
        buttons.len(),
        4,
        "expected toggle + 2 primary + 1 footer nav buttons"
    );

    let toggle_btn = buttons[0];
    for (idx, btn) in buttons.iter().enumerate() {
        assert!(
            (btn.rect.x - toggle_btn.rect.x).abs() < 0.5
                && (btn.rect.width - toggle_btn.rect.width).abs() < 0.5,
            "button[{idx}] outer rect must match toggle button {:?}: got {:?}",
            toggle_btn.rect,
            btn.rect
        );
    }

    let mut toggle_svgs = Vec::new();
    collect_nodes(toggle_btn, "Svg", &mut toggle_svgs);
    assert_eq!(toggle_svgs.len(), 1);
    let toggle_icon = &toggle_svgs[0].rect;
    let toggle_center = toggle_icon.x + toggle_icon.width * 0.5;

    for (idx, btn) in buttons.iter().enumerate().skip(1) {
        let mut svgs = Vec::new();
        collect_nodes(btn, "Svg", &mut svgs);
        assert_eq!(svgs.len(), 1);
        let icon_rect = &svgs[0].rect;
        let icon_center = icon_rect.x + icon_rect.width * 0.5;
        assert!(
            (icon_center - toggle_center).abs() < 0.5,
            "nav item[{idx}] icon center ({icon_center}) must align with toggle icon center ({toggle_center}): item={icon_rect:?}, toggle={toggle_icon:?}"
        );

        let mut bars = Vec::new();
        collect_nodes(btn, "SizedBox", &mut bars);
        let bar = bars
            .into_iter()
            .find(|n| (n.rect.width - 3.0).abs() < 0.5 && (n.rect.height - 16.0).abs() < 0.5)
            .expect("compact nav item must render 3x16 active indicator slot");
        assert!(
            bar.rect.x + bar.rect.width <= icon_rect.x - 1.0,
            "active bar {:?} must stay clear of icon {:?}",
            bar.rect,
            icon_rect
        );
    }
}

#[test]
fn toast_action_and_close_buttons_render_focus_and_hover_feedback() {
    #[derive(Clone)]
    struct ToastFxState {
        toasts: ToastHostState,
    }

    let mut initial_toasts = ToastHostState::new();
    initial_toasts.show("Saved draft", Some("Undo"), ToastDuration::Indefinite, true);

    let mut h = build_ui_harness(
        ToastFxState {
            toasts: initial_toasts,
        },
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |s| {
            let tst = toast_host(&s.toasts, |st: &mut ToastFxState| {
                let _ = st.toasts.dismiss_current();
            })
            .on_action(|st: &mut ToastFxState| {
                let _ = st.toasts.dismiss_current();
            });
            zstack((tst.alignment(UnitPoint::BOTTOM),)).boxed()
        },
        PreviewConfig {
            width: 420,
            height: 160,
            ..PreviewConfig::default()
        },
    );
    h.settle_animations();

    let focus_frames = h.find_widget_ids("FocusFrame");
    assert_eq!(
        focus_frames.len(),
        2,
        "toast action and close buttons must each be wrapped in FocusFrame"
    );
    let btn_ids = h.find_widget_ids("Button");
    assert_eq!(btn_ids.len(), 2);

    let rest_img = h.render_image();

    h.inner_harness_mut().focus_on(Some(btn_ids[0]));
    let focus_action_img = h.render_image();
    assert_ne!(
        rest_img.as_raw(),
        focus_action_img.as_raw(),
        "focusing toast action button must render visible focus ring"
    );

    h.inner_harness_mut().focus_on(Some(btn_ids[1]));
    let focus_close_img = h.render_image();
    assert_ne!(
        rest_img.as_raw(),
        focus_close_img.as_raw(),
        "focusing toast close button must render visible focus ring"
    );
    assert_ne!(
        focus_action_img.as_raw(),
        focus_close_img.as_raw(),
        "focus ring should move from action button to close button"
    );

    h.inner_harness_mut().focus_on(None);
    let tree = dump_layout_tree(&mut h);
    fn collect_buttons<'a>(node: &'a LayoutNode, out: &mut Vec<&'a LayoutNode>) {
        if node.type_name == "Button" {
            out.push(node);
        }
        for c in &node.children {
            collect_buttons(c, out);
        }
    }
    let mut tree_buttons = Vec::new();
    collect_buttons(&tree, &mut tree_buttons);
    let close_rect = tree_buttons[1].rect;
    h.inner_harness_mut().mouse_move(Point::new(
        close_rect.x + close_rect.width * 0.5,
        close_rect.y + close_rect.height * 0.5,
    ));
    let hover_close_img = h.render_image();
    assert_ne!(
        rest_img.as_raw(),
        hover_close_img.as_raw(),
        "hovering toast close button on inverse_surface must produce visible hover tint"
    );
}

#[test]
fn toast_host_fades_in_and_out_and_respects_reduced_motion() {
    #[derive(Clone)]
    struct ToastMotionState {
        theme: UiTheme,
        toasts: ToastHostState,
    }

    fn run_dismiss_sequence(reduced_motion: bool) {
        let mut toasts = ToastHostState::new();
        toasts.show(
            "ToastMotion",
            Option::<&str>::None,
            ToastDuration::Indefinite,
            false,
        );
        let state = ToastMotionState {
            theme: UiTheme::new(DesignLanguage::Fluent, Brightness::Light)
                .with_reduced_motion(reduced_motion),
            toasts,
        };
        let mut h = build_ui_harness(
            state,
            |s| s.theme.clone(),
            |s| {
                let dismiss_btn = primary_button("DismissNow", |st: &mut ToastMotionState| {
                    let _ = st.toasts.dismiss_current();
                });
                let tst = toast_host(&s.toasts, |st: &mut ToastMotionState| {
                    let _ = st.toasts.dismiss_current();
                });
                zstack((
                    dismiss_btn.alignment(UnitPoint::TOP),
                    tst.alignment(UnitPoint::BOTTOM),
                ))
                .boxed()
            },
            PreviewConfig {
                width: 360,
                height: 200,
                ..PreviewConfig::default()
            },
        );
        h.settle_animations();
        let before_tree = dump_layout_tree(&mut h);
        let settled_rect = find_node_by_text(&before_tree, "ToastMotion")
            .expect("toast must be visible before dismissal")
            .rect;

        h.run_step(&PreviewStep::ClickLabel("DismissNow".to_owned()))
            .unwrap();

        if reduced_motion {
            let immediate_tree = dump_layout_tree(&mut h);
            assert!(
                find_node_by_text(&immediate_tree, "ToastMotion").is_none(),
                "reduced_motion toast must unmount immediately on dismissal"
            );
        } else {
            h.run_step(&PreviewStep::AnimateMs(40)).unwrap();
            let mid_tree = dump_layout_tree(&mut h);
            let exiting_rect = find_node_by_text(&mid_tree, "ToastMotion")
                .expect("toast should still be fading out 40ms into exit animation")
                .rect;
            assert!(
                exiting_rect.y > settled_rect.y + 0.5,
                "toast exit transition should slide downward slightly while fading: settled={}, mid={}",
                settled_rect.y,
                exiting_rect.y
            );

            h.settle_animations();
            let done_tree = dump_layout_tree(&mut h);
            assert!(
                find_node_by_text(&done_tree, "ToastMotion").is_none(),
                "toast should unmount after exit fade completes"
            );
        }
    }

    run_dismiss_sequence(false);
    run_dismiss_sequence(true);
}

#[test]
fn preview_harness_probes_resolved_props_cropped_widget_and_pixel_diff() {
    #[derive(Default)]
    struct ProbeState;

    let theme = UiTheme::new(DesignLanguage::Md3, Brightness::Light);
    let spec = theme.to_spec();
    let expected_states = brocado::core_ui::foundation::resolve_interactive_colors(
        spec.palette.accent,
        spec.palette.on_accent,
        spec.disabled_container(),
        spec.brightness,
        spec.feedback,
    );

    let mut harness = build_ui_harness(
        ProbeState,
        |_| UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        |_| {
            outlined_card(
                column((primary_button("Save", |_| {}),))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .padding(Padding::all(12.px())),
            )
            .boxed()
        },
        PreviewConfig {
            width: 320,
            height: 160,
            ..PreviewConfig::default()
        },
    );
    harness.settle_animations();

    let btn_ids = harness.find_widget_ids("Button");
    assert_eq!(btn_ids.len(), 1);
    let btn_id = btn_ids[0];

    let rest_bg = harness
        .widget_prop::<Background>(btn_id)
        .expect("Button must resolve Background property");
    assert_eq!(rest_bg, Background::Color(expected_states.rest));

    let radius = harness
        .widget_prop::<CornerRadius>(btn_id)
        .expect("Button must resolve CornerRadius property");
    assert!((radius.radius.get() - spec.metrics.button_radius).abs() < 1e-6);

    let btn_rect = harness.widget_rect(btn_id).expect("Button rect missing");
    let rest_crop = harness
        .render_widget_image(btn_id)
        .expect("cropped button image should render");
    assert!(
        (f64::from(rest_crop.width()) - btn_rect.2).abs() <= 2.0
            && (f64::from(rest_crop.height()) - btn_rect.3).abs() <= 2.0,
        "crop {:?} should tightly match rect {:?}",
        rest_crop.dimensions(),
        btn_rect
    );

    let center_px = harness
        .sample_widget_pixel(btn_id, 0.5, 0.5)
        .expect("center pixel should be sampleable");
    assert_eq!(center_px[3], 255, "button center pixel should be opaque");

    harness
        .run_step(&PreviewStep::HoverLabel("Save".into()))
        .unwrap();
    let hover_bg = harness
        .widget_prop::<Background>(btn_id)
        .expect("Hovered Button must resolve Background property");
    assert_eq!(
        hover_bg,
        Background::Color(expected_states.hover),
        "hovered Button should resolve spec hover background color"
    );

    let hover_crop = harness
        .render_widget_image(btn_id)
        .expect("hovered crop should render");
    assert!(compare_images(&rest_crop, &rest_crop, ImageDiffTolerance::exact()).matched);
    let diff = compare_images(&rest_crop, &hover_crop, ImageDiffTolerance::exact());
    assert!(
        !diff.matched && diff.diff_pixels > 0,
        "hovered button crop must visibly differ from resting crop"
    );
}
