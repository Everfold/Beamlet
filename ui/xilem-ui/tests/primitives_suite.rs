//! Headless integration tests for the 6 standard UI library primitives:
//! `wrap`, `split_view`, `text_area`, `rich_prose`, `context_menu`, and `popover`.

use xilem::WidgetView;
use xilem::kurbo::Point;
use xilem::masonry::core::keyboard::{Code, Key, KeyState, Modifiers, NamedKey};
use xilem::masonry::core::{KeyboardEvent, PointerButton, TextEvent};
use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::{Dimensions, Padding};
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, ZStackExt, flex_col, label, sized_box, zstack};
use xilem_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use xilem_preview::inspect::{LayoutNode, dump_layout_tree};
use xilem_ui::prelude::*;

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
        location: xilem::masonry::core::keyboard::Location::Standard,
        modifiers,
        repeat: false,
        is_composing: false,
    }));
    inner.process_text_event(TextEvent::Keyboard(KeyboardEvent {
        state: KeyState::Up,
        key,
        code,
        location: xilem::masonry::core::keyboard::Location::Standard,
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
