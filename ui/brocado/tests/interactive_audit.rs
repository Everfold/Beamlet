//! Exhaustive headless interactive & layout audit across all Beamlet UI components and themes.

use std::collections::HashSet;

use brocado::prelude::{Brightness, Theme, ToggleableState, seeds};
use brocado_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use brocado_preview::inspect::{IssueSeverity, LayoutNode, dump_layout_tree};

#[allow(dead_code)]
mod preview_support;

use preview_support::scenes::{ALL_SCENES, SceneState, build_scene_harness};
use preview_support::{
    build_ui_harness, lint_theme_contrast, lint_ui_accessibility, lint_ui_layout,
};

fn default_harness(scene_id: &str) -> PreviewHarness<SceneState> {
    build_scene_harness(scene_id, Theme::light(), PreviewConfig::default())
        .expect("scene harness should build")
}

fn assert_healthy(harness: &mut PreviewHarness<SceneState>, ctx_label: &str) {
    harness.settle_animations();
    let tree = dump_layout_tree(harness);
    let issues = lint_ui_layout(&tree, harness.viewport());
    let errors: Vec<_> = issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "layout errors in {ctx_label}: {errors:#?}"
    );
    let a11y_issues = lint_ui_accessibility(harness);
    let a11y_errors: Vec<_> = a11y_issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .collect();
    assert!(
        a11y_errors.is_empty(),
        "accessibility errors in {ctx_label}: {a11y_errors:#?}"
    );
    let contrast = lint_theme_contrast(&harness.state().theme);
    assert!(
        contrast.is_empty(),
        "contrast issues in {ctx_label}: {contrast:#?}"
    );
    let img = harness.render_image();
    assert_eq!(
        img.dimensions(),
        (harness.viewport().0 as u32, harness.viewport().1 as u32)
    );
    let mut distinct = HashSet::new();
    for px in img.pixels().step_by(37) {
        distinct.insert(px.0);
        if distinct.len() >= 2 {
            break;
        }
    }
    assert!(
        distinct.len() >= 2,
        "frame for {ctx_label} collapsed to a single uniform color"
    );
}

fn has_label_descendant(node: &LayoutNode) -> bool {
    if node.type_name == "Label" && node.text.is_some() {
        return true;
    }
    node.children.iter().any(has_label_descendant)
}

fn collect_icon_button_ids(node: &LayoutNode, out: &mut Vec<u64>) {
    if node.type_name.contains("Button") && !has_label_descendant(node) {
        out.push(node.id);
    }
    for child in &node.children {
        collect_icon_button_ids(child, out);
    }
}

fn find_buttons_without_label(harness: &mut PreviewHarness<SceneState>) -> Vec<u64> {
    let tree = dump_layout_tree(harness);
    let mut out = Vec::new();
    collect_icon_button_ids(&tree, &mut out);
    out
}

fn collect_slide_card_heights(node: &LayoutNode, out: &mut Vec<f64>) {
    if node.type_name == "SlideTransition" {
        for child in &node.children {
            out.push(child.rect.height);
        }
    }
    for child in &node.children {
        collect_slide_card_heights(child, out);
    }
}

#[test]
fn audit_overview_all_controls_across_all_themes() {
    {
        let mut h = default_harness("overview");

        let before = h.state().click_count;
        for lbl in ["Primary", "Secondary", "Elevated", "Outlined", "Subtle"] {
            h.run_step(&PreviewStep::ClickLabel(lbl.to_owned()))
                .unwrap_or_else(|e| panic!("[Md3] clicking {lbl}: {e}"));
        }
        assert_eq!(
            h.state().click_count,
            before + 5,
            "[Md3] standard buttons should increment click_count"
        );

        let disabled_before = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("Disabled".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().click_count,
            disabled_before,
            "[Md3] disabled button must not fire callback"
        );

        h.run_step(&PreviewStep::ClickLabel("Delete".to_owned()))
            .unwrap();
        assert!(
            h.state().show_destructive_dialog,
            "[Md3] Delete button should open destructive dialog"
        );
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".to_owned(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(
            !h.state().show_destructive_dialog,
            "[Md3] Escape should close destructive dialog"
        );

        let icon_ids = find_buttons_without_label(&mut h);
        // 5 icon buttons in overview + 4 removable chip close buttons = 9 icon-only buttons;
        // click the first 5 (icon actions row).
        assert!(
            icon_ids.len() >= 9,
            "[Md3] expected >= 9 icon buttons in overview, got {}",
            icon_ids.len()
        );
        let icon_before = h.state().click_count;
        for &raw_id in &icon_ids[..5] {
            let wid = h
                .find_widget_ids("Button")
                .into_iter()
                .find(|w| w.to_raw() == raw_id)
                .expect("icon button WidgetId");
            h.click_widget_id(wid);
        }
        assert_eq!(
            h.state().click_count,
            icon_before + 5,
            "[Md3] all 5 icon buttons should fire"
        );

        // Primary tabs
        h.run_step(&PreviewStep::ClickLabel("Appearance".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 1, "[Md3] primary tab -> 1");
        h.run_step(&PreviewStep::ClickLabel("Shortcuts".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 2, "[Md3] primary tab -> 2");
        h.run_step(&PreviewStep::ClickLabel("General".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 0, "[Md3] primary tab -> 0");

        // Secondary tabs
        h.run_step(&PreviewStep::ClickLabel("Changelog".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 2, "[Md3] secondary tab -> 2");
        h.run_step(&PreviewStep::ClickLabel("Overview".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 0, "[Md3] secondary tab -> 0");
        h.run_step(&PreviewStep::ClickLabel("Spec".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 1, "[Md3] secondary tab -> 1");

        // Multi-choice segmented buttons: initial [true, false, true]
        for lbl in ["Bold", "Italic", "Code"] {
            h.run_step(&PreviewStep::ClickLabel(lbl.to_owned()))
                .unwrap();
        }
        assert_eq!(
            h.state().multi_seg_mask,
            [false, true, false],
            "[Md3] segmented buttons should toggle independently"
        );

        // Chips: Assist, Filter A, Suggestion, Input chip body, and remove button
        let chip_before = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("Assist".to_owned()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("Suggestion".to_owned()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("MD3".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().click_count,
            chip_before + 3,
            "[Md3] assist/suggestion/input chip clicks should fire"
        );

        let flag0 = h.state().chip_flags[0];
        h.run_step(&PreviewStep::ClickLabel("Filter A".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().chip_flags[0],
            !flag0,
            "[Md3] Filter A chip should toggle"
        );

        // Last icon-only button in overview belongs to the last removable input chip ("Spring").
        let tags_before = h.state().tags.len();
        let rem_raw = *find_buttons_without_label(&mut h)
            .last()
            .expect("remove button");
        let rem_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == rem_raw)
            .expect("remove WidgetId");
        h.click_widget_id(rem_wid);
        assert_eq!(
            h.state().tags.len(),
            tags_before - 1,
            "[Md3] chip remove button should remove tag"
        );

        assert_healthy(&mut h, "overview/Md3");
    }
}

#[test]
fn audit_inputs_all_controls_across_all_themes() {
    {
        let mut h = default_harness("inputs");

        // 1. SearchBar + dynamic Clear icon button
        h.run_steps(&[
            PreviewStep::FocusType {
                type_name: "TextArea".to_owned(),
                index: 0,
            },
            PreviewStep::TypeText("beam".to_owned()),
        ])
        .unwrap();
        assert_eq!(
            h.state().search_query,
            "beam",
            "[Md3] search bar should record typed text"
        );
        let clear_btns = find_buttons_without_label(&mut h);
        assert!(
            !clear_btns.is_empty(),
            "[Md3] non-empty search bar should show clear button"
        );
        let clear_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == clear_btns[0])
            .expect("clear button WidgetId");
        h.click_widget_id(clear_wid);
        assert!(
            h.state().search_query.is_empty(),
            "[Md3] clicking search clear button should clear query"
        );

        // 2. Outlined TextField & Validation TextField
        h.run_steps(&[
            PreviewStep::FocusType {
                type_name: "TextArea".to_owned(),
                index: 1,
            },
            PreviewStep::TypeText("!".to_owned()),
        ])
        .unwrap();
        assert!(
            h.state().text_value.contains('!'),
            "[Md3] Workspace Name text_field should update on TypeText"
        );

        h.run_steps(&[
            PreviewStep::FocusType {
                type_name: "TextArea".to_owned(),
                index: 2,
            },
            PreviewStep::TypeText("x".to_owned()),
        ])
        .unwrap();
        assert!(
            h.state().email_value.contains('x'),
            "[Md3] filled_text_field should update on TypeText"
        );

        // 3. Checkbox
        let cb_ids = h.find_widget_ids("Checkbox");
        assert!(!cb_ids.is_empty(), "[Md3] Checkbox should exist");
        let init_agree = h.state().agree_terms;
        h.click_widget_id(cb_ids[0]);
        assert_eq!(
            h.state().agree_terms,
            !init_agree,
            "[Md3] Checkbox should toggle"
        );
        h.click_widget_id(cb_ids[0]);
        assert_eq!(h.state().agree_terms, init_agree);

        // 4. Tri-state Checkbox
        assert_eq!(h.state().tri_state, ToggleableState::Indeterminate);
        h.run_step(&PreviewStep::ClickLabel("Group Sync".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().tri_state,
            ToggleableState::On,
            "[Md3] tri_state_checkbox should transition Indeterminate -> On"
        );
        h.run_step(&PreviewStep::ClickLabel("Group Sync".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().tri_state,
            ToggleableState::Off,
            "[Md3] tri_state_checkbox should transition On -> Off"
        );

        // 5. Switch
        let sw_ids = h.find_widget_ids("Switch");
        assert!(!sw_ids.is_empty(), "[Md3] Switch should exist");
        let init_sw = h.state().switch_on;
        h.click_widget_id(sw_ids[0]);
        assert_eq!(h.state().switch_on, !init_sw, "[Md3] Switch should toggle");

        // 6. Radio buttons
        h.run_step(&PreviewStep::ClickLabel("Compact Desktop".to_owned()))
            .unwrap();
        assert_eq!(h.state().radio_choice, 1, "[Md3] radio -> 1");
        h.run_step(&PreviewStep::ClickLabel("Standard Density".to_owned()))
            .unwrap();
        assert_eq!(h.state().radio_choice, 0, "[Md3] radio -> 0");

        // 7. Slider drag
        h.drag_type("Slider", 0, (0.45, 0.5), (0.85, 0.5)).unwrap();
        assert!(
            h.state().slider_val > 0.65,
            "[Md3] slider drag right should increase slider_val (got {})",
            h.state().slider_val
        );

        // 8. RangeSlider drag left thumb & right thumb
        let before_range = h.state().range_val;
        h.drag_type("RangeSliderWidget", 0, (0.25, 0.5), (0.10, 0.5))
            .unwrap();
        h.drag_type("RangeSliderWidget", 0, (0.75, 0.5), (0.90, 0.5))
            .unwrap();
        assert_ne!(
            h.state().range_val,
            before_range,
            "[Md3] RangeSlider drag should update range_val"
        );

        // 9. Select dropdown open -> choose item -> close
        assert!(!h.state().select_open);
        h.run_step(&PreviewStep::ClickLabel("Force Light".to_owned()))
            .unwrap();
        assert!(h.state().select_open, "[Md3] select should open");
        assert_healthy(&mut h, "inputs/Md3/select-open");
        h.run_step(&PreviewStep::ClickLabel("Force Dark".to_owned()))
            .unwrap();
        assert_eq!(h.state().select_index, 2);
        assert!(!h.state().select_open, "[Md3] select should close");

        assert_healthy(&mut h, "inputs/Md3");
    }
}

#[test]
fn audit_overlays_and_pickers_across_all_themes() {
    {
        let mut h = default_harness("overlays");

        // 1. Basic dialog: Cancel, Reset, Escape, and Scrim click; verify compact height!
        h.run_step(&PreviewStep::ClickLabel("Open Basic Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_basic_dialog);
        h.settle_animations();
        let tree = dump_layout_tree(&mut h);
        let mut heights = Vec::new();
        collect_slide_card_heights(&tree, &mut heights);
        assert!(
            heights.iter().any(|&ht| ht > 120.0 && ht < 420.0),
            "[Md3] basic dialog card height should be compact (<420px), got {heights:?}"
        );
        assert_healthy(&mut h, "overlays/Md3/basic-dialog");
        h.run_step(&PreviewStep::ClickLabel("Cancel".to_owned()))
            .unwrap();
        assert!(!h.state().show_basic_dialog);

        h.run_step(&PreviewStep::ClickLabel("Open Basic Dialog".to_owned()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("Reset".to_owned()))
            .unwrap();
        assert!(!h.state().show_basic_dialog);

        h.run_step(&PreviewStep::ClickLabel("Open Basic Dialog".to_owned()))
            .unwrap();
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".to_owned(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(!h.state().show_basic_dialog);

        h.run_step(&PreviewStep::ClickLabel("Open Basic Dialog".to_owned()))
            .unwrap();
        h.click_point(10.0, 10.0);
        assert!(
            !h.state().show_basic_dialog,
            "[Md3] clicking scrim at (10,10) should dismiss modal"
        );

        // 2. Destructive dialog
        h.run_step(&PreviewStep::ClickLabel("Destructive Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_destructive_dialog);
        assert_healthy(&mut h, "overlays/Md3/destructive-dialog");
        h.run_step(&PreviewStep::ClickLabel("Delete".to_owned()))
            .unwrap();
        assert!(!h.state().show_destructive_dialog);

        // 3. Bottom sheet
        h.run_step(&PreviewStep::ClickLabel("Bottom Sheet".to_owned()))
            .unwrap();
        assert!(h.state().show_bottom_sheet);
        assert_healthy(&mut h, "overlays/Md3/bottom-sheet");
        h.run_step(&PreviewStep::ClickLabel("Close Sheet".to_owned()))
            .unwrap();
        assert!(!h.state().show_bottom_sheet);

        // 4. Side sheet (must have zero overflow warnings!)
        h.run_step(&PreviewStep::ClickLabel("Side Sheet".to_owned()))
            .unwrap();
        assert!(h.state().show_side_sheet);
        h.settle_animations();
        let sheet_tree = dump_layout_tree(&mut h);
        let sheet_issues = lint_ui_layout(&sheet_tree, h.viewport());
        assert!(
            sheet_issues.is_empty(),
            "[Md3] side sheet should have 0 lint errors & warnings, got: {sheet_issues:#?}"
        );
        assert_healthy(&mut h, "overlays/Md3/side-sheet");
        h.run_step(&PreviewStep::ClickLabel("Close Drawer".to_owned()))
            .unwrap();
        assert!(!h.state().show_side_sheet);

        // 5. Toast trigger & dismiss
        h.run_step(&PreviewStep::ClickLabel("Show Toast".to_owned()))
            .unwrap();
        assert!(
            h.state().toasts.current().is_some(),
            "[Md3] Show Toast should enqueue toast"
        );
        assert_healthy(&mut h, "overlays/Md3/toast-visible");
        h.run_step(&PreviewStep::ClickLabel("Dismiss".to_owned()))
            .unwrap();
        assert!(
            h.state().toasts.current().is_none(),
            "[Md3] Toast action button should dismiss toast"
        );

        // 6. Inline DatePicker day select & next month
        h.run_step(&PreviewStep::ClickLabel("15".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_date.day, 15, "[Md3] day -> 15");
        let icon_btns = find_buttons_without_label(&mut h);
        // Header prev/next month are first 2 icon buttons inside DatePicker.
        assert!(icon_btns.len() >= 2);
        let next_month_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == icon_btns[1])
            .expect("next month button");
        h.click_widget_id(next_month_wid);
        assert_eq!(
            h.state().selected_date.month,
            12,
            "[Md3] next month arrow should advance 11 -> 12"
        );

        // 7. Date dialog
        h.run_step(&PreviewStep::ClickLabel("Date Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_date_dialog);
        h.settle_animations();
        let date_tree = dump_layout_tree(&mut h);
        let mut date_heights = Vec::new();
        collect_slide_card_heights(&date_tree, &mut date_heights);
        assert!(
            date_heights.iter().any(|&ht| ht > 200.0 && ht < 540.0),
            "[Md3] date dialog card height should be compact (<540px), got {date_heights:?}"
        );
        assert_healthy(&mut h, "overlays/Md3/date-dialog");
        h.run_step(&PreviewStep::ClickLabel("OK".to_owned()))
            .unwrap();
        assert!(!h.state().show_date_dialog);

        // 8. Inline TimePicker AM / PM + hour/minute steppers
        h.run_step(&PreviewStep::ClickLabel("AM".to_owned()))
            .unwrap();
        assert!(h.state().selected_time.hour < 12, "[Md3] AM toggle");
        h.run_step(&PreviewStep::ClickLabel("PM".to_owned()))
            .unwrap();
        assert!(h.state().selected_time.hour >= 12, "[Md3] PM toggle");
        let hour_before = h.state().selected_time.hour;
        let tp_icons = find_buttons_without_label(&mut h);
        // DatePicker (2) + TimePicker (4) = 6 icon-only buttons -> index 2 is hour +1
        assert!(
            tp_icons.len() >= 6,
            "expected >=6 icon buttons, got {}",
            tp_icons.len()
        );
        let hour_plus_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == tp_icons[2])
            .expect("hour +1 button");
        h.click_widget_id(hour_plus_wid);
        assert_eq!(
            h.state().selected_time.hour,
            (hour_before + 1) % 24,
            "[Md3] TimePicker hour+1 button"
        );

        // 9. Time dialog
        h.run_step(&PreviewStep::ClickLabel("Time Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_time_dialog);
        assert_healthy(&mut h, "overlays/Md3/time-dialog");
        h.run_step(&PreviewStep::ClickLabel("Cancel".to_owned()))
            .unwrap();
        assert!(!h.state().show_time_dialog);
    }
}

#[test]
fn audit_platform_native_all_suites() {
    {
        let mut h = default_harness("platform-native");

        // MD3: Extended FAB + Floating Toolbar Publish + FAB icon buttons
        let clicks_before = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("Compose".to_owned()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("Publish".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().click_count,
            clicks_before + 2,
            "[Md3] MD3 Compose/Publish"
        );
        let icon_btns = find_buttons_without_label(&mut h);
        // First 5 icon-only buttons belong to small_fab, fab, large_fab, and toolbar copy/share
        assert!(icon_btns.len() >= 5);
        for &raw in &icon_btns[..5] {
            let wid = h
                .find_widget_ids("Button")
                .into_iter()
                .find(|w| w.to_raw() == raw)
                .unwrap();
            h.click_widget_id(wid);
        }
        assert_eq!(
            h.state().click_count,
            clicks_before + 7,
            "[Md3] MD3 FAB sizes + toolbar icons"
        );

        assert_healthy(&mut h, "platform-native/Md3");
    }
}

#[test]
fn audit_workspaces_md3() {
    let mut h = default_harness("workspace");
    h.run_step(&PreviewStep::ClickLabel("Tokens".to_owned()))
        .unwrap();
    assert_eq!(h.state().active_tab, 2);
    let clicks = h.state().click_count;
    h.run_step(&PreviewStep::ClickLabel("New Action".to_owned()))
        .unwrap();
    assert_eq!(h.state().click_count, clicks + 1);
    h.run_step(&PreviewStep::ClickLabel("OKLCH Color Scheme".to_owned()))
        .unwrap();
    assert_eq!(h.state().selected_row, 1);
    assert_healthy(&mut h, "workspace/md3");
}

#[test]
fn audit_desktop_all_scenes_themes_brightness() {
    for scene in ALL_SCENES {
        {
            for brightness in [Brightness::Light, Brightness::Dark] {
                let mut h = build_scene_harness(
                    scene.id,
                    Theme::from_seed(seeds::PURPLE, brightness),
                    PreviewConfig::default(),
                )
                .expect("scene builds");
                h.settle_animations();
                let tree = dump_layout_tree(&mut h);
                let issues = lint_ui_layout(&tree, h.viewport());
                assert!(
                    issues.is_empty(),
                    "desktop matrix expected 0 layout issues for {}/Md3/{brightness:?}, got: {issues:#?}",
                    scene.id
                );
                let contrast = lint_theme_contrast(&h.state().theme);
                assert!(
                    contrast.is_empty(),
                    "desktop matrix expected 0 contrast issues for {}/Md3/{brightness:?}, got: {contrast:#?}",
                    scene.id
                );
            }
        }
    }
}

#[test]
fn audit_toast_host_without_outer_max_dims_stays_compact() {
    use brocado::prelude::{ToastDuration, ToastHostState, primary_button, toast_host};
    use masonry::layout::{AsUnit, UnitPoint};
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{ZStackExt, sized_box, zstack};

    #[derive(Clone)]
    struct ToastMountState {
        theme: Theme,
        toasts: ToastHostState,
    }

    let initial = ToastMountState {
        theme: Theme::light(),
        toasts: ToastHostState::new(),
    };

    let mut h = build_ui_harness(
        initial,
        |s| s.theme.clone(),
        |s| {
            // Reproduces exact theme_gallery / md3_notes mount: NO `.dims(Dimensions::MAX)` on outer `sized_box`.
            let toast_layer = sized_box(
                toast_host(&s.toasts, |st: &mut ToastMountState| {
                    let _ = st.toasts.dismiss_current();
                })
                .on_action(|st: &mut ToastMountState| {
                    let _ = st.toasts.dismiss_current();
                }),
            )
            .padding(Padding::all(16.px()))
            .alignment(UnitPoint::BOTTOM);

            let trigger = primary_button("Queue Toast", |st: &mut ToastMountState| {
                st.toasts.show(
                    "Preview queued notice",
                    Some("Undo"),
                    ToastDuration::Short,
                    true,
                );
            });

            zstack((trigger.alignment(UnitPoint::CENTER), toast_layer)).boxed()
        },
        PreviewConfig::default(),
    );

    h.run_step(&PreviewStep::ClickLabel("Queue Toast".to_owned()))
        .unwrap();
    h.settle_animations();

    let tree = dump_layout_tree(&mut h);
    fn find_toast_child_rect(node: &LayoutNode) -> Option<(f64, f64)> {
        if node.type_name == "ToastTimerHost" {
            let child = node.children.first()?;
            return Some((child.rect.width, child.rect.height));
        }
        node.children.iter().find_map(find_toast_child_rect)
    }

    let (w, ht) = find_toast_child_rect(&tree).expect("ToastTimerHost child present");
    assert!(
        w < 600.0 && ht < 120.0,
        "toast must remain a compact pill without outer Dimensions::MAX, got {w}x{ht}"
    );
    let _ = h.render_image();
}

#[test]
fn audit_search_bar_text_vertically_centered_across_all_themes() {
    fn find_first_focus_frame_and_text_area(
        node: &LayoutNode,
    ) -> Option<(
        brocado_preview::inspect::LayoutRect,
        brocado_preview::inspect::LayoutRect,
    )> {
        if node.type_name == "FocusFrame" {
            fn find_text_area(n: &LayoutNode) -> Option<brocado_preview::inspect::LayoutRect> {
                if n.type_name == "TextArea" {
                    return Some(n.rect);
                }
                n.children.iter().find_map(find_text_area)
            }
            if let Some(area_rect) = find_text_area(node) {
                return Some((node.rect, area_rect));
            }
        }
        node.children
            .iter()
            .find_map(find_first_focus_frame_and_text_area)
    }

    {
        let mut h = default_harness("inputs");

        for stage in ["placeholder", "typed"] {
            if stage == "typed" {
                h.run_steps(&[
                    PreviewStep::FocusType {
                        type_name: "TextArea".to_owned(),
                        index: 0,
                    },
                    PreviewStep::TypeText("Search123".to_owned()),
                ])
                .unwrap();
            }

            let tree = dump_layout_tree(&mut h);
            let (frame_rect, area_rect) = find_first_focus_frame_and_text_area(&tree)
                .expect("search bar FocusFrame and TextArea present");
            assert!(
                area_rect.height < frame_rect.height,
                "[Md3 {stage}] TextArea height ({}) must be strictly smaller than FocusFrame height ({})",
                area_rect.height,
                frame_rect.height
            );
            let frame_cy = frame_rect.y + frame_rect.height * 0.5;
            let area_cy = area_rect.y + area_rect.height * 0.5;
            assert!(
                (area_cy - frame_cy).abs() <= 2.0,
                "[Md3 {stage}] search TextArea center y ({area_cy:.2}) must match FocusFrame center y ({frame_cy:.2}) within 2px (frame={frame_rect:?}, area={area_rect:?})"
            );
            assert_healthy(&mut h, &format!("search_bar_center/Md3/{stage}"));
        }
    }
}

#[test]
fn audit_phase2_custom_controls_disabled_bounds_and_drag() {
    use brocado::prelude::{checkbox, radio, radio_group, range_slider, slider, switch};
    use masonry::layout::AsUnit;
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, flex_col};

    #[derive(Clone)]
    struct ControlStressState {
        theme: Theme,
        sw_disabled: bool,
        cb_disabled: bool,
        radio_idx: usize,
        slider_val: f64,
        range_val: (f64, f64),
        disabled_clicks: u32,
    }

    {
        let initial = ControlStressState {
            theme: Theme::light(),
            sw_disabled: true,
            cb_disabled: true,
            radio_idx: 0,
            slider_val: 0.5,
            range_val: (25.0, 75.0),
            disabled_clicks: 0,
        };

        let mut h = build_ui_harness(
            initial,
            |s| s.theme.clone(),
            |s| {
                flex_col((
                    switch(s.sw_disabled, |st: &mut ControlStressState, v| {
                        st.sw_disabled = v;
                        st.disabled_clicks += 1;
                    })
                    .disabled(true),
                    checkbox(
                        "Disabled Box",
                        s.cb_disabled,
                        |st: &mut ControlStressState, v| {
                            st.cb_disabled = v;
                            st.disabled_clicks += 1;
                        },
                    )
                    .disabled(true),
                    radio_group(vec![
                        radio(
                            "Active Option",
                            s.radio_idx == 0,
                            |st: &mut ControlStressState| {
                                st.radio_idx = 0;
                            },
                        ),
                        radio(
                            "Disabled Option",
                            s.radio_idx == 1,
                            |st: &mut ControlStressState| {
                                st.radio_idx = 1;
                                st.disabled_clicks += 1;
                            },
                        )
                        .disabled(true),
                    ]),
                    slider(
                        0.0,
                        1.0,
                        s.slider_val,
                        None,
                        |st: &mut ControlStressState, v| {
                            st.slider_val = v;
                        },
                    ),
                    range_slider(
                        0.0,
                        100.0,
                        s.range_val,
                        |st: &mut ControlStressState, next| st.range_val = next,
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(12.px())
                .boxed()
            },
            PreviewConfig::default(),
        );

        // Disabled switch, checkbox, and radio must reject clicks cleanly.
        let sw = h.find_widget_ids("UiSwitch")[0];
        h.click_widget_id(sw);
        let cb = h.find_widget_ids("UiCheckbox")[0];
        h.click_widget_id(cb);
        h.run_step(&PreviewStep::ClickLabel("Disabled Option".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().disabled_clicks,
            0,
            "[Md3] disabled custom controls must never fire callbacks"
        );
        assert_eq!(h.state().radio_idx, 0);

        // Extreme out-of-bounds slider drags must clamp cleanly to [min, max] without panicking.
        h.drag_type("UiSlider", 0, (0.5, 0.5), (-0.5, 0.5)).unwrap();
        assert!(
            (h.state().slider_val - 0.0).abs() < 1e-6,
            "[Md3] out-of-bounds left drag should clamp to 0.0 (got {})",
            h.state().slider_val
        );
        h.drag_type("UiSlider", 0, (0.05, 0.5), (1.5, 0.5)).unwrap();
        assert!(
            (h.state().slider_val - 1.0).abs() < 1e-6,
            "[Md3] out-of-bounds right drag should clamp to 1.0 (got {})",
            h.state().slider_val
        );

        // RangeSlider extreme crossing drag must clamp and prevent crossover.
        h.drag_type("RangeSliderWidget", 0, (0.25, 0.5), (1.4, 0.5))
            .unwrap();
        assert!(
            h.state().range_val.0 <= h.state().range_val.1
                && h.state().range_val.0 >= 0.0
                && h.state().range_val.1 <= 100.0,
            "[Md3] range slider bounds invariant violated: {:?}",
            h.state().range_val
        );
    }
}

#[test]
fn audit_phase3_breakpoint_bin_and_dialog() {
    use brocado::prelude::{
        basic_dialog, breakpoint_bin, clickable_list_item, filled_card, primary_button, switch,
        tabs,
    };
    use masonry::layout::{AsUnit, UnitPoint};
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, zstack};

    #[derive(Clone)]
    struct ResponsiveState {
        theme: Theme,
        tab: usize,
        toggle: bool,
        show_alert: bool,
        confirmed: u32,
        row_clicks: u32,
    }

    let make_harness = |width: u32| {
        let cfg = PreviewConfig {
            width,
            height: 800,
            ..PreviewConfig::default()
        };
        build_ui_harness(
            ResponsiveState {
                theme: Theme::light(),
                tab: 0,
                toggle: false,
                show_alert: false,
                confirmed: 0,
                row_clicks: 0,
            },
            |s| s.theme.clone(),
            |s| {
                let toggle = s.toggle;
                let make_body = || {
                    let list = filled_card(
                        flex_col((
                            switch(toggle, |st: &mut ResponsiveState, v| {
                                st.toggle = v;
                            }),
                            clickable_list_item("Run Row", |st: &mut ResponsiveState| {
                                st.row_clicks += 1;
                            }),
                        ))
                        .gap(8.px()),
                    );
                    flex_col((
                        list,
                        primary_button("Open Alert", |st: &mut ResponsiveState| {
                            st.show_alert = true;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(12.px())
                };

                let wide = flex_col((
                    tabs(
                        &["WidePrefs", "WideStatus"],
                        s.tab,
                        |st: &mut ResponsiveState, idx| st.tab = idx,
                    ),
                    make_body().flex(1.0),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);

                let narrow = flex_col((
                    make_body().flex(1.0),
                    tabs(
                        &["NarrowPrefs", "NarrowStatus"],
                        s.tab,
                        |st: &mut ResponsiveState, idx| st.tab = idx,
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);

                let bin = breakpoint_bin(600.0, wide, narrow);
                let dlg = s.show_alert.then(|| {
                    basic_dialog(
                        "Confirm Reset?",
                        "This resets preferences.",
                        "Reset",
                        |st: &mut ResponsiveState| {
                            st.confirmed += 1;
                            st.show_alert = false;
                        },
                        |st: &mut ResponsiveState| st.show_alert = false,
                    )
                    .destructive(true)
                    .dismiss_text("Cancel")
                });
                zstack((
                    bin.alignment(UnitPoint::TOP_LEFT),
                    dlg.map(|d| d.alignment(UnitPoint::CENTER)),
                ))
                .boxed()
            },
            cfg,
        )
    };

    // 1. Wide layout (>= 600px): wide tabs visible, narrow tabs stashed.
    let mut wide_h = make_harness(1180);
    wide_h
        .run_step(&PreviewStep::ClickLabel("WideStatus".to_owned()))
        .unwrap();
    assert_eq!(wide_h.state().tab, 1);
    assert!(
        wide_h
            .run_step(&PreviewStep::ClickLabel("NarrowPrefs".to_owned()))
            .is_err(),
        "narrow child must be stashed when width >= 600px"
    );

    // 2. Narrow layout (< 600px): narrow tabs visible & interactive, dialog opens and confirms.
    let mut narrow_h = make_harness(480);
    narrow_h
        .run_step(&PreviewStep::ClickLabel("NarrowStatus".to_owned()))
        .unwrap();
    assert_eq!(narrow_h.state().tab, 1);
    narrow_h
        .run_step(&PreviewStep::ClickLabel("Run Row".to_owned()))
        .unwrap();
    assert_eq!(narrow_h.state().row_clicks, 1);

    narrow_h
        .run_step(&PreviewStep::ClickLabel("Open Alert".to_owned()))
        .unwrap();
    assert!(narrow_h.state().show_alert);
    narrow_h.settle_animations();
    let tree = dump_layout_tree(&mut narrow_h);
    let errors: Vec<_> = lint_ui_layout(&tree, narrow_h.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "narrow dialog errors: {errors:#?}");
    narrow_h
        .run_step(&PreviewStep::ClickLabel("Reset".to_owned()))
        .unwrap();
    assert!(!narrow_h.state().show_alert);
    assert_eq!(narrow_h.state().confirmed, 1);

    let wide_tree = dump_layout_tree(&mut wide_h);
    let wide_errors: Vec<_> = lint_ui_layout(&wide_tree, wide_h.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(
        wide_errors.is_empty(),
        "wide responsive layout errors: {wide_errors:#?}"
    );
}

#[test]
fn audit_phase4_md3_fixed_colors_ripple_and_text_field() {
    use brocado::prelude::{filled_text_field, primary_button, ripple_layer, text_field};
    use masonry::layout::AsUnit;
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, flex_col};

    struct Phase4State {
        theme: Theme,
        clicks: usize,
        outlined: String,
        filled: String,
    }

    let build = |reduced_motion: bool| {
        build_ui_harness(
            Phase4State {
                theme: Theme::light().with_reduced_motion(reduced_motion),
                clicks: 0,
                outlined: String::new(),
                filled: String::new(),
            },
            |s| s.theme.clone(),
            |s| {
                flex_col((
                    ripple_layer(primary_button("Ripple CTA", |st: &mut Phase4State| {
                        st.clicks += 1;
                    })),
                    text_field(s.outlined.clone(), |st: &mut Phase4State, v| {
                        st.outlined = v;
                    })
                    .label("Outlined Floating")
                    .placeholder("Type here"),
                    filled_text_field(s.filled.clone(), |st: &mut Phase4State, v| {
                        st.filled = v;
                    })
                    .label("Filled Floating")
                    .placeholder("Type here"),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(12.px())
                .padding(Padding::all(16.px()))
                .boxed()
            },
            PreviewConfig {
                width: 480,
                height: 360,
                ..PreviewConfig::default()
            },
        )
    };

    // 1. Standard motion: RippleLayer animates on click, terminates cleanly, doesn't block Button click,
    //    and both MD3 floating-label text fields accept typing cleanly with 0 layout errors.
    let mut h = build(false);
    let tree = dump_layout_tree(&mut h);
    let errors: Vec<_> = lint_ui_layout(&tree, h.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "MD3 Phase 4 errors: {errors:#?}");

    h.run_step(&PreviewStep::ClickLabel("Ripple CTA".to_owned()))
        .unwrap();
    assert_eq!(h.state().clicks, 1);
    h.run_step(&PreviewStep::AnimateMs(400)).unwrap();
    assert!(
        !h.inner_harness_mut().render_root().needs_anim(),
        "RippleLayer must stop requesting anim frames after wave completes"
    );

    h.run_step(&PreviewStep::ClickType {
        type_name: "TextInput".to_owned(),
        index: 0,
    })
    .unwrap();
    h.run_step(&PreviewStep::TypeText("m3-out".to_owned()))
        .unwrap();
    h.run_step(&PreviewStep::ClickType {
        type_name: "TextInput".to_owned(),
        index: 1,
    })
    .unwrap();
    h.run_step(&PreviewStep::TypeText("m3-fill".to_owned()))
        .unwrap();
    assert_eq!(h.state().outlined, "m3-out");
    assert_eq!(h.state().filled, "m3-fill");

    // 2. Reduced motion: RippleLayer does not schedule animations at all on press.
    let mut h_rm = build(true);
    h_rm.run_step(&PreviewStep::ClickLabel("Ripple CTA".to_owned()))
        .unwrap();
    assert_eq!(h_rm.state().clicks, 1);
    assert!(
        !h_rm.inner_harness_mut().render_root().needs_anim(),
        "RippleLayer must suppress animations when reduced_motion is true"
    );
}

#[test]
fn audit_phase6_accessibility_keyboard_and_reduced_motion() {
    use brocado::prelude::{
        basic_dialog, determinate_circular_progress, dropdown_menu, linear_progress, menu_item,
        primary_button, range_slider, ripple_layer, slider, switch, ui_transition,
    };
    use masonry::accesskit::Role;
    use masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
    use masonry::core::{KeyboardEvent, TextEvent};
    use masonry::layout::{AsUnit, UnitPoint};
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, ZStackExt, flex_col, zstack};

    // 1. Modal Focus Trap, Instant Escape (without manual FocusType), & Double-Escape safety
    struct ModalTrapState {
        theme: Theme,
        dialog_open: bool,
        menu_open: bool,
        menu_choice: Option<usize>,
        toggled: bool,
        toggle_count: usize,
        slider_val: f64,
        range_val: (f64, f64),
        clicks: usize,
    }

    let build_harness = |reduced_motion: bool| {
        build_ui_harness(
            ModalTrapState {
                theme: Theme::light().with_reduced_motion(reduced_motion),
                dialog_open: false,
                menu_open: false,
                menu_choice: None,
                toggled: false,
                toggle_count: 0,
                slider_val: 0.4,
                range_val: (0.2, 0.8),
                clicks: 0,
            },
            |s| s.theme.clone(),
            |s| {
                let bg_btn = primary_button("BgBtn", |st: &mut ModalTrapState| st.clicks += 1);
                let menu = dropdown_menu(
                    primary_button("OpenMenu", |st: &mut ModalTrapState| {
                        st.menu_open = !st.menu_open;
                    }),
                    s.menu_open,
                    vec![
                        menu_item("MenuZero"),
                        menu_item("MenuOne"),
                        menu_item("MenuTwo"),
                    ],
                    |st: &mut ModalTrapState, idx| {
                        st.menu_choice = Some(idx);
                        st.menu_open = false;
                    },
                    |st: &mut ModalTrapState| st.menu_open = false,
                );
                let main = ui_transition(
                    1,
                    flex_col((
                        bg_btn,
                        primary_button("OpenDlg", |st: &mut ModalTrapState| {
                            st.dialog_open = true;
                        }),
                        menu,
                        ripple_layer(switch(s.toggled, |st: &mut ModalTrapState, v| {
                            st.toggled = v;
                            st.toggle_count += 1;
                        })),
                        slider(
                            0.0,
                            1.0,
                            s.slider_val,
                            None,
                            |st: &mut ModalTrapState, v| {
                                st.slider_val = v;
                            },
                        ),
                        range_slider(0.0, 1.0, s.range_val, |st: &mut ModalTrapState, pair| {
                            st.range_val = pair;
                        }),
                        linear_progress(Some(0.65)),
                        linear_progress(None),
                        determinate_circular_progress(0.45),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .gap(10.px())
                    .padding(Padding::all(16.px())),
                );

                let dlg = s.dialog_open.then(|| {
                    basic_dialog(
                        "Modal Trap Test",
                        "Tab must stay inside this modal.",
                        "Confirm",
                        |st: &mut ModalTrapState| {
                            st.clicks += 10;
                            st.dialog_open = false;
                        },
                        |st: &mut ModalTrapState| st.dialog_open = false,
                    )
                    .dismiss_text("Cancel")
                });

                zstack((
                    main.alignment(UnitPoint::TOP_LEFT),
                    dlg.map(|d| d.alignment(UnitPoint::CENTER)),
                ))
                .boxed()
            },
            PreviewConfig {
                width: 640,
                height: 600,
                ..PreviewConfig::default()
            },
        )
    };

    let mut h = build_harness(false);

    // Check AccessKit on progress indicators and closed AnchoredPopupHost
    let _ = h.render_image();
    let lin_ids = h.find_widget_ids("LinearProgressWidget");
    assert_eq!(lin_ids.len(), 2);
    let circ_ids = h.find_widget_ids("DeterminateCircularWidget");
    assert_eq!(circ_ids.len(), 1);
    let host_ids = h.find_widget_ids("AnchoredPopupHost");
    assert_eq!(host_ids.len(), 1);
    {
        let inner = h.inner_harness_mut();
        let lin_node = inner
            .access_node(lin_ids[0])
            .expect("determinate LinearProgressWidget access node");
        assert_eq!(lin_node.role(), Role::ProgressIndicator);
        assert_eq!(lin_node.numeric_value(), Some(0.65));
        let circ_node = inner
            .access_node(circ_ids[0])
            .expect("DeterminateCircularWidget access node");
        assert_eq!(circ_node.role(), Role::ProgressIndicator);
        assert_eq!(circ_node.numeric_value(), Some(0.45));
        let host_node = inner
            .access_node(host_ids[0])
            .expect("AnchoredPopupHost access node");
        assert_eq!(host_node.data().is_expanded(), Some(false));
    }

    // Identify BgBtn WidgetId before opening modal
    let tree_before = dump_layout_tree(&mut h);
    fn has_text_descendant(node: &LayoutNode, needle: &str) -> bool {
        if node.text.as_deref().is_some_and(|t| t.contains(needle)) {
            return true;
        }
        node.children.iter().any(|c| has_text_descendant(c, needle))
    }
    fn find_btn_id_by_text(node: &LayoutNode, needle: &str) -> Option<u64> {
        if node.type_name == "Button" && has_text_descendant(node, needle) {
            return Some(node.id);
        }
        node.children
            .iter()
            .find_map(|c| find_btn_id_by_text(c, needle))
    }
    let bg_btn_raw_id = find_btn_id_by_text(&tree_before, "BgBtn").unwrap_or_else(|| {
        panic!(
            "BgBtn present in tree:\n{}",
            brocado_preview::inspect::format_tree_text(&tree_before)
        )
    });

    // Open dialog by clicking OpenDlg; move pointer onto EscapeCatcher so desktop hover/click
    // gives modal focus, then verify Role::Dialog + modal=true and Tab trap ring.
    h.run_step(&PreviewStep::ClickLabel("OpenDlg".to_owned()))
        .unwrap();
    assert!(h.state().dialog_open);
    h.run_step(&PreviewStep::HoverType {
        type_name: "EscapeCatcher".to_owned(),
        index: 0,
    })
    .unwrap();
    let _ = h.render_image();
    let trap_ids = h.find_widget_ids("EscapeCatcher");
    assert_eq!(trap_ids.len(), 1);
    {
        let inner = h.inner_harness_mut();
        let dlg_node = inner
            .access_node(trap_ids[0])
            .expect("EscapeCatcher modal access node");
        assert_eq!(dlg_node.role(), Role::Dialog);
        assert!(dlg_node.is_modal());
    }

    // Tab 10 times: focus must cycle ONLY among modal buttons and NEVER reach BgBtn.
    let mut visited_in_modal = HashSet::new();
    for _ in 0..10 {
        h.run_step(&PreviewStep::PressTab { shift: false }).unwrap();
        let focused_raw = h
            .inner_harness_mut()
            .focused_widget_id()
            .map(|id| id.to_raw());
        assert_ne!(
            focused_raw,
            Some(bg_btn_raw_id),
            "Tab must be trapped inside modal EscapeCatcher and never reach BgBtn"
        );
        if let Some(id) = focused_raw {
            visited_in_modal.insert(id);
        }
    }
    assert!(
        visited_in_modal.len() >= 2,
        "Tab trap must cycle across at least 2 buttons inside the modal, saw {visited_in_modal:?}"
    );

    // Press Escape TWICE: first closes modal, second is harmless no-op
    h.run_step(&PreviewStep::PressEscape).unwrap();
    assert!(!h.state().dialog_open);
    h.run_step(&PreviewStep::PressEscape).unwrap();
    assert!(!h.state().dialog_open);

    // 2. PopupLayer keyboard navigation: ArrowDown -> ArrowDown -> Enter selects MenuOne (index 1)
    h.run_step(&PreviewStep::ClickLabel("OpenMenu".to_owned()))
        .unwrap();
    assert!(h.state().menu_open);
    let _ = h.render_image();
    {
        let inner = h.inner_harness_mut();
        let host_node = inner
            .access_node(host_ids[0])
            .expect("expanded AnchoredPopupHost node");
        assert_eq!(host_node.data().is_expanded(), Some(true));
    }
    for _ in 0..2 {
        h.inner_harness_mut()
            .process_text_event(TextEvent::Keyboard(KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::ArrowDown),
                code: Code::ArrowDown,
                ..KeyboardEvent::default()
            }));
        h.pump_actions();
    }
    h.inner_harness_mut()
        .process_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::Enter),
            code: Code::Enter,
            ..KeyboardEvent::default()
        }));
    h.inner_harness_mut()
        .process_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Up,
            key: Key::Named(NamedKey::Enter),
            code: Code::Enter,
            ..KeyboardEvent::default()
        }));
    h.pump_actions();
    assert!(!h.state().menu_open);
    assert_eq!(h.state().menu_choice, Some(1));

    // Reopen and dismiss PopupLayer via Escape
    h.run_step(&PreviewStep::ClickLabel("OpenMenu".to_owned()))
        .unwrap();
    assert!(h.state().menu_open);
    h.run_step(&PreviewStep::PressEscape).unwrap();
    assert!(!h.state().menu_open);

    // 3. Rapid stress test (20x rapid switch clicks + out-of-bounds slider/range drags)
    for _ in 0..20 {
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiSwitch".to_owned(),
            index: 0,
        })
        .unwrap();
    }
    assert_eq!(h.state().toggle_count, 20);
    assert!(!h.state().toggled);

    h.settle_animations();
    h.drag_type("UiSlider", 0, (0.4, 0.5), (1.5, 0.5)).unwrap();
    assert!((h.state().slider_val - 1.0).abs() < 1e-6);
    h.drag_type("UiSlider", 0, (0.9, 0.5), (-0.5, 0.5)).unwrap();
    assert!((h.state().slider_val - 0.0).abs() < 1e-6);
    h.drag_type("RangeSliderWidget", 0, (0.2, 0.5), (1.4, 0.5))
        .unwrap();
    assert!(h.state().range_val.0 <= h.state().range_val.1 && h.state().range_val.1 <= 1.0);

    // 4. reduced_motion = true: UiSwitch, RippleLayer, UiTransition, LinearProgressWidget(None), and Modal
    //    must schedule ZERO animation loops.
    let mut h_rm = build_harness(true);
    assert!(
        !h_rm.inner_harness_mut().render_root().needs_anim(),
        "UiTransition & indeterminate LinearProgressWidget must not animate under reduced_motion"
    );
    h_rm.run_step(&PreviewStep::ClickType {
        type_name: "UiSwitch".to_owned(),
        index: 0,
    })
    .unwrap();
    assert!(h_rm.state().toggled);
    assert!(
        !h_rm.inner_harness_mut().render_root().needs_anim(),
        "UiSwitch & RippleLayer must not animate under reduced_motion"
    );
    h_rm.run_step(&PreviewStep::ClickLabel("OpenDlg".to_owned()))
        .unwrap();
    assert!(h_rm.state().dialog_open);
    // Advance one 16ms tick to clear Masonry's initial WidgetState::new(request_anim = true);
    // with reduced_motion = true (0ms SlideTransition), no further anim frames may be requested.
    h_rm.run_step(&PreviewStep::AnimateMs(16)).unwrap();
    assert!(
        !h_rm.inner_harness_mut().render_root().needs_anim(),
        "Modal SlideTransition must not animate under reduced_motion"
    );
}

#[test]
fn memoize_with_theme_skips_unrelated_rebuilds_and_updates_on_data_or_theme_change() {
    use brocado::prelude::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use xilem_masonry::view::{flex_col, label};

    struct MemoState {
        theme: Theme,
        unrelated: u32,
        memo_key: String,
        rebuilds: Arc<AtomicUsize>,
    }

    let rebuilds = Arc::new(AtomicUsize::new(0));
    let mut h = build_ui_harness(
        MemoState {
            theme: Theme::light(),
            unrelated: 0,
            memo_key: "Alpha".to_owned(),
            rebuilds: rebuilds.clone(),
        },
        |s| s.theme.clone(),
        |s| {
            let key = s.memo_key.clone();
            let counter = s.rebuilds.clone();
            flex_col((
                label(format!("unrelated={}", s.unrelated)),
                memoize_with_theme(key, move |theme, data| {
                    counter.fetch_add(1, Ordering::SeqCst);
                    label(format!("{:?}:{data}", theme.brightness))
                }),
            ))
            .boxed()
        },
        PreviewConfig {
            width: 400,
            height: 240,
            ..PreviewConfig::default()
        },
    );

    assert_eq!(rebuilds.load(Ordering::SeqCst), 1);

    // Mutate unrelated state and trigger rebuilds: memoized subtree builder must NOT rerun.
    h.state_mut().unrelated += 1;
    h.rebuild();
    h.state_mut().unrelated += 1;
    h.rebuild();
    assert_eq!(rebuilds.load(Ordering::SeqCst), 1);

    // Mutate memoized key: subtree MUST rebuild.
    h.state_mut().memo_key = "Beta".to_owned();
    h.rebuild();
    assert_eq!(rebuilds.load(Ordering::SeqCst), 2);
    let tree = dump_layout_tree(&mut h);
    assert!(format_tree_text_contains(&tree, "Beta"));

    // Unrelated mutation again: still cached at 2.
    h.state_mut().unrelated += 1;
    h.rebuild();
    assert_eq!(rebuilds.load(Ordering::SeqCst), 2);

    // Switch theme brightness: subtree MUST rebuild even though memo_key is unchanged.
    h.state_mut().theme = Theme::dark();
    h.rebuild();
    assert_eq!(rebuilds.load(Ordering::SeqCst), 3);
}

fn format_tree_text_contains(node: &LayoutNode, needle: &str) -> bool {
    if node.text.as_deref().is_some_and(|t| t.contains(needle)) {
        return true;
    }
    node.children
        .iter()
        .any(|child| format_tree_text_contains(child, needle))
}

#[test]
fn virtual_list_mounts_only_visible_window_for_ten_thousand_items() {
    use brocado::prelude::*;
    use masonry::properties::Dimensions;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{label, sized_box};

    struct ListState {
        theme: Theme,
    }

    let mut h = build_ui_harness(
        ListState {
            theme: Theme::light(),
        },
        |s| s.theme.clone(),
        |_| {
            sized_box(virtual_list(10_000, |_, idx| {
                sized_box(label(format!("Virtual row #{idx}")))
                    .dims(Dimensions::height(masonry::layout::Length::px(36.0)))
            }))
            .dims(Dimensions::STRETCH)
            .boxed()
        },
        PreviewConfig {
            width: 480,
            height: 360,
            ..PreviewConfig::default()
        },
    );

    // Redraw so VirtualScroll measures the viewport and emits Fetch, then pump_actions + settle.
    h.settle_animations();
    let tree = dump_layout_tree(&mut h);

    fn count_labels(node: &LayoutNode) -> usize {
        let self_hit = usize::from(node.type_name == "Label");
        self_hit + node.children.iter().map(count_labels).sum::<usize>()
    }

    let mounted_labels = count_labels(&tree);
    assert!(
        mounted_labels > 0 && mounted_labels < 100,
        "expected O(viewport) mounted labels (between 1 and 99) for 10,000 virtual items, got {mounted_labels}"
    );
    let issues = lint_ui_layout(&tree, h.viewport());
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "virtual_list layout errors: {errors:#?}");
}

#[test]
fn preview_harness_reports_frame_timings_and_tab_cycle_reachability() {
    use brocado::prelude::*;
    use xilem_masonry::view::flex_col;

    struct QaState {
        theme: Theme,
        clicks: u32,
        toggled: bool,
    }

    let mut h = build_ui_harness(
        QaState {
            theme: Theme::light(),
            clicks: 0,
            toggled: false,
        },
        |s| s.theme.clone(),
        |s| {
            flex_col((
                primary_button("First", |st: &mut QaState| st.clicks += 1),
                secondary_button("Second", |st: &mut QaState| st.clicks += 1),
                switch(s.toggled, |st: &mut QaState, v| st.toggled = v),
            ))
            .boxed()
        },
        PreviewConfig {
            width: 480,
            height: 320,
            ..PreviewConfig::default()
        },
    );

    let timings = h.measure_frame_timings();
    assert!(timings.rebuild_us < 5_000_000);
    assert!(timings.layout_and_paint_us < 5_000_000);

    let cycle = h.collect_tab_cycle(6);
    let unique_focused: HashSet<_> = cycle.into_iter().flatten().collect();
    assert!(
        unique_focused.len() >= 2,
        "Tab cycle must visit multiple focusable controls, visited: {unique_focused:?}"
    );

    let a11y_issues = lint_ui_accessibility(&mut h);
    assert!(
        a11y_issues.is_empty(),
        "unexpected accessibility issues: {a11y_issues:#?}"
    );
}

#[test]
fn extreme_narrow_160px_all_components_ascii_stress() {
    use brocado::prelude::scroll_view;
    use brocado::prelude::*;
    use masonry::layout::AsUnit;
    use masonry::properties::Dimensions;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, flex_col, sized_box};

    #[derive(Clone)]
    struct StressState {
        theme: Theme,
        seg: usize,
        tab: usize,
        checked: bool,
        tri: ToggleableState,
        sw: bool,
        slider: f64,
        range: (f64, f64),
        sel_idx: usize,
        sel_open: bool,
        query: String,
        text_val: String,
        date: CalendarDate,
        time: TimeOfDay,
        show_dialog: bool,
        toasts: ToastHostState,
    }

    {
        let mut init_toasts = ToastHostState::new();
        init_toasts.show(
            "SupercalifragilisticToastMessage",
            Some("Undo"),
            ToastDuration::Indefinite,
            true,
        );
        let mut h = build_ui_harness(
            StressState {
                theme: Theme::light(),
                seg: 0,
                tab: 0,
                checked: true,
                tri: ToggleableState::Indeterminate,
                sw: true,
                slider: 0.4,
                range: (20.0, 80.0),
                sel_idx: 0,
                sel_open: false,
                query: String::new(),
                text_val: String::from("https://example.invalid/very-long-segment"),
                date: CalendarDate::new(2026, 11, 24),
                time: TimeOfDay::new(9, 30),
                show_dialog: true,
                toasts: init_toasts,
            },
            |s| s.theme.clone(),
            |s| {
                let page = flex_col(vec![
                    primary_button(
                        "VeryLongUnbreakablePrimaryActionButton",
                        |_: &mut StressState| {},
                    )
                    .boxed(),
                    segmented_button(
                        &["Overview", "Configuration", "Diagnostics"],
                        s.seg,
                        |st: &mut StressState, i| st.seg = i,
                    )
                    .boxed(),
                    tabs(
                        &["GeneralSettings", "Appearance", "Shortcuts"],
                        s.tab,
                        |st: &mut StressState, i| {
                            st.tab = i;
                        },
                    )
                    .boxed(),
                    wrap((
                        filter_chip("FilterOptionOne", s.checked, |st: &mut StressState| {
                            st.checked = !st.checked;
                        }),
                        input_chip("RemovableVeryLongTagChip", |_: &mut StressState| {})
                            .on_remove(|_: &mut StressState| {}),
                    ))
                    .gap(6.px())
                    .run_gap(6.px())
                    .boxed(),
                    checkbox(
                        "SupercalifragilisticCheckboxLabel",
                        s.checked,
                        |st: &mut StressState, v| {
                            st.checked = v;
                        },
                    )
                    .boxed(),
                    tri_state_checkbox(
                        "TriStateSynchronizationPreference",
                        s.tri,
                        |st: &mut StressState, next| st.tri = next,
                    )
                    .boxed(),
                    switch(s.sw, |st: &mut StressState, v| st.sw = v).boxed(),
                    clickable_list_item("SynchronizationDaemonEndpoint", |_: &mut StressState| {})
                        .supporting_text("BackgroundReplicationWorkerServiceStatus")
                        .boxed(),
                    slider(0.0, 1.0, s.slider, None, |st: &mut StressState, v| {
                        st.slider = v
                    })
                    .boxed(),
                    range_slider(0.0, 100.0, s.range, |st: &mut StressState, r| st.range = r)
                        .boxed(),
                    linear_progress(Some(0.6)).boxed(),
                    wavy_linear_progress(Some(0.6)).boxed(),
                    split_button(
                        "SupercalifragilisticSplitActionButton",
                        |_: &mut StressState| {},
                        |_: &mut StressState| {},
                    )
                    .boxed(),
                    button_group(
                        &["Day", "Week", "Month"],
                        s.seg,
                        |st: &mut StressState, i| st.seg = i,
                    )
                    .boxed(),
                    search_bar(s.query.clone(), |st: &mut StressState, q| st.query = q).boxed(),
                    text_field(s.text_val.clone(), |st: &mut StressState, v| {
                        st.text_val = v
                    })
                    .label("EndpointIdentifier")
                    .boxed(),
                    select(
                        &["AutomaticallySelectedBackend", "Vulkan", "Metal"],
                        s.sel_idx,
                        s.sel_open,
                        |st: &mut StressState| st.sel_open = !st.sel_open,
                        |st: &mut StressState, i| {
                            st.sel_idx = i;
                            st.sel_open = false;
                        },
                        |st: &mut StressState| st.sel_open = false,
                    )
                    .boxed(),
                    date_picker(s.date, |st: &mut StressState, d| st.date = d).boxed(),
                    time_picker(s.time, |st: &mut StressState, t| st.time = t).boxed(),
                    plain_tooltip(
                        "SupercalifragilisticTooltipTextWrappingCheck",
                        primary_button("HoverTipTarget", |_: &mut StressState| {}),
                    )
                    .boxed(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(8.px());

                let base = sized_box(scroll_view(page).constrain_horizontal(true))
                    .dims(Dimensions::STRETCH);
                let dlg = s.show_dialog.then(|| {
                    basic_dialog(
                        "ConfirmSupercalifragilisticAction?",
                        "ThisWillValidateLongTokenWrappingInsideModalCardsAt160px.",
                        "Confirm",
                        |st: &mut StressState| st.show_dialog = false,
                        |st: &mut StressState| st.show_dialog = false,
                    )
                    .dismiss_text("Cancel")
                });
                let tst = toast_host(&s.toasts, |st: &mut StressState| {
                    let _ = st.toasts.dismiss_current();
                })
                .on_action(|st: &mut StressState| {
                    let _ = st.toasts.dismiss_current();
                });

                xilem_masonry::view::zstack((base, tst, dlg)).boxed()
            },
            PreviewConfig {
                width: 1200,
                height: 800,
                ..PreviewConfig::default()
            },
        );

        for &(w, ht) in &[(1200_u32, 800_u32), (200, 480), (160, 480), (1200, 800)] {
            h.resize(w, ht);
            h.settle_animations();
            let tree = dump_layout_tree(&mut h);
            let issues = lint_ui_layout(&tree, h.viewport());
            let errors: Vec<_> = issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Error || i.kind == "overflow")
                .collect();
            assert!(
                errors.is_empty(),
                "[Md3@{w}x{ht}] ASCII 160px stress errors/overflows: {errors:#?}"
            );
            let a11y_issues = lint_ui_accessibility(&mut h);
            let a11y_errors: Vec<_> = a11y_issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Error)
                .collect();
            assert!(
                a11y_errors.is_empty(),
                "[Md3@{w}x{ht}] ASCII stress a11y errors: {a11y_errors:#?}"
            );
        }
    }
}
