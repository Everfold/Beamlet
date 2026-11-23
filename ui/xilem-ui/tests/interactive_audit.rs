//! Exhaustive headless interactive & layout audit across all Beamlet UI components and themes.

use std::collections::HashSet;

use xilem_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use xilem_preview::inspect::{IssueSeverity, LayoutNode, dump_layout_tree};
use xilem_ui::prelude::{Brightness, DesignLanguage, ToggleableState, UiTheme};

#[allow(dead_code)]
mod preview_support;

use preview_support::scenes::{ALL_SCENES, SceneState, build_scene_harness};
use preview_support::{
    build_ui_harness, lint_spec_contrast, lint_ui_accessibility, lint_ui_layout,
};

fn default_harness(scene_id: &str, language: DesignLanguage) -> PreviewHarness<SceneState> {
    build_scene_harness(
        scene_id,
        UiTheme::new(language, Brightness::Light),
        PreviewConfig::default(),
    )
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
    let contrast = lint_spec_contrast(&harness.state().theme.to_spec());
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
    for &lang in &DesignLanguage::ALL {
        let mut h = default_harness("overview", lang);

        let before = h.state().click_count;
        for lbl in ["Primary", "Secondary", "Elevated", "Outlined", "Subtle"] {
            h.run_step(&PreviewStep::ClickLabel(lbl.to_owned()))
                .unwrap_or_else(|e| panic!("[{lang:?}] clicking {lbl}: {e}"));
        }
        assert_eq!(
            h.state().click_count,
            before + 5,
            "[{lang:?}] standard buttons should increment click_count"
        );

        let disabled_before = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("Disabled".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().click_count,
            disabled_before,
            "[{lang:?}] disabled button must not fire callback"
        );

        h.run_step(&PreviewStep::ClickLabel("Delete".to_owned()))
            .unwrap();
        assert!(
            h.state().show_destructive_dialog,
            "[{lang:?}] Delete button should open destructive dialog"
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
            "[{lang:?}] Escape should close destructive dialog"
        );

        let icon_ids = find_buttons_without_label(&mut h);
        // 5 icon buttons in overview + 4 removable chip close buttons = 9 icon-only buttons;
        // click the first 5 (icon actions row).
        assert!(
            icon_ids.len() >= 9,
            "[{lang:?}] expected >= 9 icon buttons in overview, got {}",
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
            "[{lang:?}] all 5 icon buttons should fire"
        );

        // Primary tabs
        h.run_step(&PreviewStep::ClickLabel("Appearance".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 1, "[{lang:?}] primary tab -> 1");
        h.run_step(&PreviewStep::ClickLabel("Shortcuts".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 2, "[{lang:?}] primary tab -> 2");
        h.run_step(&PreviewStep::ClickLabel("General".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_tab, 0, "[{lang:?}] primary tab -> 0");

        // Secondary tabs
        h.run_step(&PreviewStep::ClickLabel("Changelog".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 2, "[{lang:?}] secondary tab -> 2");
        h.run_step(&PreviewStep::ClickLabel("Overview".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 0, "[{lang:?}] secondary tab -> 0");
        h.run_step(&PreviewStep::ClickLabel("Spec".to_owned()))
            .unwrap();
        assert_eq!(h.state().secondary_tab, 1, "[{lang:?}] secondary tab -> 1");

        // Multi-choice segmented buttons: initial [true, false, true]
        for lbl in ["Bold", "Italic", "Code"] {
            h.run_step(&PreviewStep::ClickLabel(lbl.to_owned()))
                .unwrap();
        }
        assert_eq!(
            h.state().multi_seg_mask,
            [false, true, false],
            "[{lang:?}] segmented buttons should toggle independently"
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
            "[{lang:?}] assist/suggestion/input chip clicks should fire"
        );

        let flag0 = h.state().chip_flags[0];
        h.run_step(&PreviewStep::ClickLabel("Filter A".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().chip_flags[0],
            !flag0,
            "[{lang:?}] Filter A chip should toggle"
        );

        // Last icon-only button in overview belongs to the last removable input chip ("Adwaita").
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
            "[{lang:?}] chip remove button should remove tag"
        );

        assert_healthy(&mut h, &format!("overview/{lang:?}"));
    }
}

#[test]
fn audit_inputs_all_controls_across_all_themes() {
    for &lang in &DesignLanguage::ALL {
        let mut h = default_harness("inputs", lang);

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
            "[{lang:?}] search bar should record typed text"
        );
        let clear_btns = find_buttons_without_label(&mut h);
        assert!(
            !clear_btns.is_empty(),
            "[{lang:?}] non-empty search bar should show clear button"
        );
        let clear_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == clear_btns[0])
            .expect("clear button WidgetId");
        h.click_widget_id(clear_wid);
        assert!(
            h.state().search_query.is_empty(),
            "[{lang:?}] clicking search clear button should clear query"
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
            "[{lang:?}] Workspace Name text_field should update on TypeText"
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
            "[{lang:?}] filled_text_field should update on TypeText"
        );

        // 3. Checkbox
        let cb_ids = h.find_widget_ids("Checkbox");
        assert!(!cb_ids.is_empty(), "[{lang:?}] Checkbox should exist");
        let init_agree = h.state().agree_terms;
        h.click_widget_id(cb_ids[0]);
        assert_eq!(
            h.state().agree_terms,
            !init_agree,
            "[{lang:?}] Checkbox should toggle"
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
            "[{lang:?}] tri_state_checkbox should transition Indeterminate -> On"
        );
        h.run_step(&PreviewStep::ClickLabel("Group Sync".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().tri_state,
            ToggleableState::Off,
            "[{lang:?}] tri_state_checkbox should transition On -> Off"
        );

        // 5. Switch
        let sw_ids = h.find_widget_ids("Switch");
        assert!(!sw_ids.is_empty(), "[{lang:?}] Switch should exist");
        let init_sw = h.state().switch_on;
        h.click_widget_id(sw_ids[0]);
        assert_eq!(
            h.state().switch_on,
            !init_sw,
            "[{lang:?}] Switch should toggle"
        );

        // 6. Radio buttons
        h.run_step(&PreviewStep::ClickLabel("Compact Desktop".to_owned()))
            .unwrap();
        assert_eq!(h.state().radio_choice, 1, "[{lang:?}] radio -> 1");
        h.run_step(&PreviewStep::ClickLabel("Standard Density".to_owned()))
            .unwrap();
        assert_eq!(h.state().radio_choice, 0, "[{lang:?}] radio -> 0");

        // 7. Slider drag
        h.drag_type("Slider", 0, (0.45, 0.5), (0.85, 0.5)).unwrap();
        assert!(
            h.state().slider_val > 0.65,
            "[{lang:?}] slider drag right should increase slider_val (got {})",
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
            "[{lang:?}] RangeSlider drag should update range_val"
        );

        // 9. Select dropdown open -> choose item -> close
        assert!(!h.state().select_open);
        h.run_step(&PreviewStep::ClickLabel("Force Light".to_owned()))
            .unwrap();
        assert!(h.state().select_open, "[{lang:?}] select should open");
        assert_healthy(&mut h, &format!("inputs/{lang:?}/select-open"));
        h.run_step(&PreviewStep::ClickLabel("Force Dark".to_owned()))
            .unwrap();
        assert_eq!(h.state().select_index, 2);
        assert!(!h.state().select_open, "[{lang:?}] select should close");

        assert_healthy(&mut h, &format!("inputs/{lang:?}"));
    }
}

#[test]
fn audit_overlays_and_pickers_across_all_themes() {
    for &lang in &DesignLanguage::ALL {
        let mut h = default_harness("overlays", lang);

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
            "[{lang:?}] basic dialog card height should be compact (<420px), got {heights:?}"
        );
        assert_healthy(&mut h, &format!("overlays/{lang:?}/basic-dialog"));
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
            "[{lang:?}] clicking scrim at (10,10) should dismiss modal"
        );

        // 2. Destructive dialog
        h.run_step(&PreviewStep::ClickLabel("Destructive Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_destructive_dialog);
        assert_healthy(&mut h, &format!("overlays/{lang:?}/destructive-dialog"));
        h.run_step(&PreviewStep::ClickLabel("Delete".to_owned()))
            .unwrap();
        assert!(!h.state().show_destructive_dialog);

        // 3. Bottom sheet
        h.run_step(&PreviewStep::ClickLabel("Bottom Sheet".to_owned()))
            .unwrap();
        assert!(h.state().show_bottom_sheet);
        assert_healthy(&mut h, &format!("overlays/{lang:?}/bottom-sheet"));
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
            "[{lang:?}] side sheet should have 0 lint errors & warnings, got: {sheet_issues:#?}"
        );
        assert_healthy(&mut h, &format!("overlays/{lang:?}/side-sheet"));
        h.run_step(&PreviewStep::ClickLabel("Close Drawer".to_owned()))
            .unwrap();
        assert!(!h.state().show_side_sheet);

        // 5. Toast trigger & dismiss
        h.run_step(&PreviewStep::ClickLabel("Show Toast".to_owned()))
            .unwrap();
        assert!(
            h.state().toasts.current().is_some(),
            "[{lang:?}] Show Toast should enqueue toast"
        );
        assert_healthy(&mut h, &format!("overlays/{lang:?}/toast-visible"));
        h.run_step(&PreviewStep::ClickLabel("Dismiss".to_owned()))
            .unwrap();
        assert!(
            h.state().toasts.current().is_none(),
            "[{lang:?}] Toast action button should dismiss toast"
        );

        // 6. Inline DatePicker day select & next month
        h.run_step(&PreviewStep::ClickLabel("15".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_date.day, 15, "[{lang:?}] day -> 15");
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
            "[{lang:?}] next month arrow should advance 11 -> 12"
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
            "[{lang:?}] date dialog card height should be compact (<540px), got {date_heights:?}"
        );
        assert_healthy(&mut h, &format!("overlays/{lang:?}/date-dialog"));
        h.run_step(&PreviewStep::ClickLabel("OK".to_owned()))
            .unwrap();
        assert!(!h.state().show_date_dialog);

        // 8. Inline TimePicker AM / PM + hour/minute steppers
        h.run_step(&PreviewStep::ClickLabel("AM".to_owned()))
            .unwrap();
        assert!(h.state().selected_time.hour < 12, "[{lang:?}] AM toggle");
        h.run_step(&PreviewStep::ClickLabel("PM".to_owned()))
            .unwrap();
        assert!(h.state().selected_time.hour >= 12, "[{lang:?}] PM toggle");
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
            "[{lang:?}] TimePicker hour+1 button"
        );

        // 9. Time dialog
        h.run_step(&PreviewStep::ClickLabel("Time Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_time_dialog);
        assert_healthy(&mut h, &format!("overlays/{lang:?}/time-dialog"));
        h.run_step(&PreviewStep::ClickLabel("Cancel".to_owned()))
            .unwrap();
        assert!(!h.state().show_time_dialog);
    }
}

#[test]
fn audit_platform_native_all_suites() {
    for &lang in &DesignLanguage::ALL {
        let mut h = default_harness("platform-native", lang);

        // MD3: Extended FAB + Floating Toolbar Publish + FAB icon buttons
        let clicks_before = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("Compose".to_owned()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("Publish".to_owned()))
            .unwrap();
        assert_eq!(
            h.state().click_count,
            clicks_before + 2,
            "[{lang:?}] MD3 Compose/Publish"
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
            "[{lang:?}] MD3 FAB sizes + toolbar icons"
        );

        // Fluent: Breadcrumb + NumberBox (icon +/-) + Expander
        h.run_step(&PreviewStep::ClickLabel("Settings".to_owned()))
            .unwrap();
        let num_before = h.state().fluent_number_val;
        // Icon buttons after the 5 MD3 FAB/toolbar icons: index 5 is NumberBox decrement, index 6 is increment.
        assert!(
            icon_btns.len() >= 7,
            "[{lang:?}] expected NumberBox +/- icon buttons"
        );
        let all_btns = h.find_widget_ids("Button");
        let nb_dec = *all_btns
            .iter()
            .find(|w| w.to_raw() == icon_btns[5])
            .unwrap();
        let nb_inc = *all_btns
            .iter()
            .find(|w| w.to_raw() == icon_btns[6])
            .unwrap();
        h.click_widget_id(nb_inc);
        assert_eq!(h.state().fluent_number_val, num_before + 4.0);
        h.click_widget_id(nb_dec);
        assert_eq!(h.state().fluent_number_val, num_before);

        let exp_before = h.state().expander_open;
        let exp_btn = *all_btns
            .iter()
            .find(|w| w.to_raw() == icon_btns[7])
            .expect("Fluent expander chevron button");
        h.click_widget_id(exp_btn);
        assert_eq!(
            h.state().expander_open,
            !exp_before,
            "[{lang:?}] Fluent expander should toggle"
        );
        let exp_btn_after = find_buttons_without_label(&mut h)[7];
        let exp_wid_after = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == exp_btn_after)
            .unwrap();
        h.click_widget_id(exp_wid_after);
        assert_eq!(h.state().expander_open, exp_before);

        // HIG: Stepper (icon - / +) & DisclosureGroup
        let step_before = h.state().hig_stepper_val;
        let pn_icons = find_buttons_without_label(&mut h);
        assert!(
            pn_icons.len() >= 10,
            "[{lang:?}] expected HIG stepper icon buttons, got {}",
            pn_icons.len()
        );
        let pn_btns = h.find_widget_ids("Button");
        let hig_dec = *pn_btns.iter().find(|w| w.to_raw() == pn_icons[8]).unwrap();
        let hig_inc = *pn_btns.iter().find(|w| w.to_raw() == pn_icons[9]).unwrap();
        h.click_widget_id(hig_inc);
        assert_eq!(h.state().hig_stepper_val, step_before + 1);
        h.click_widget_id(hig_dec);
        assert_eq!(h.state().hig_stepper_val, step_before);

        let disc_before = h.state().hig_disclosure_open;
        h.run_step(&PreviewStep::ClickLabel("Details".to_owned()))
            .unwrap();
        assert_eq!(h.state().hig_disclosure_open, !disc_before);
        h.run_step(&PreviewStep::ClickLabel("Details".to_owned()))
            .unwrap();
        assert_eq!(h.state().hig_disclosure_open, disc_before);

        // Adwaita: EntryRow + ComboRow + ExpanderRow + SwitchRow
        h.run_steps(&[
            PreviewStep::FocusType {
                type_name: "TextArea".to_owned(),
                index: 0,
            },
            PreviewStep::TypeText("-v2".to_owned()),
        ])
        .unwrap();
        assert!(
            h.state().adw_entry_val.contains("-v2"),
            "[{lang:?}] AdwEntryRow should update on TypeText"
        );

        assert!(!h.state().adw_combo_open);
        // Scroll the bottommost Switch inside AdwExpanderRow into view first so ComboRow sits
        // well above the viewport bottom edge and its PopupLayer opens in-bounds.
        let bottom_sw = h.find_widget_ids("Switch")[0];
        h.inner_harness_mut().scroll_into_view(bottom_sw);
        h.run_step(&PreviewStep::ClickLabel("Vello WGPU".to_owned()))
            .unwrap();
        assert!(h.state().adw_combo_open, "[{lang:?}] ComboRow should open");
        h.run_step(&PreviewStep::ClickLabel("Vello CPU".to_owned()))
            .unwrap();
        assert_eq!(h.state().adw_combo_idx, 1);
        assert!(
            !h.state().adw_combo_open,
            "[{lang:?}] ComboRow should close"
        );

        // Switch inside SwitchRow while ExpanderRow is open
        let sw_ids = h.find_widget_ids("Switch");
        assert!(
            !sw_ids.is_empty(),
            "[{lang:?}] Adw SwitchRow Switch should exist"
        );
        let sw_before = h.state().switch_on;
        h.click_widget_id(sw_ids[0]);
        assert_eq!(h.state().switch_on, !sw_before);

        let adw_exp_before = h.state().adw_expander_open;
        let adw_exp_raw = *find_buttons_without_label(&mut h)
            .last()
            .expect("AdwExpanderRow chevron button");
        let adw_exp_wid = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == adw_exp_raw)
            .unwrap();
        h.click_widget_id(adw_exp_wid);
        assert_eq!(h.state().adw_expander_open, !adw_exp_before);
        let adw_exp_raw2 = *find_buttons_without_label(&mut h)
            .last()
            .expect("AdwExpanderRow chevron button");
        let adw_exp_wid2 = h
            .find_widget_ids("Button")
            .into_iter()
            .find(|w| w.to_raw() == adw_exp_raw2)
            .unwrap();
        h.click_widget_id(adw_exp_wid2);
        assert_eq!(h.state().adw_expander_open, adw_exp_before);

        assert_healthy(&mut h, &format!("platform-native/{lang:?}"));
    }
}

#[test]
fn audit_workspaces_all_four_platforms() {
    // 1. MD3 Workspace
    {
        let mut h = default_harness("workspace", DesignLanguage::Md3);
        h.run_step(&PreviewStep::ClickLabel("Tokens".to_owned()))
            .unwrap();
        assert_eq!(h.state().active_tab, 2);
        let clicks = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("New Action".to_owned()))
            .unwrap();
        assert_eq!(h.state().click_count, clicks + 1);
        h.run_step(&PreviewStep::ClickLabel("Windows Mica Shell".to_owned()))
            .unwrap();
        assert_eq!(h.state().selected_row, 1);
        assert_healthy(&mut h, "workspace/md3");
    }

    // 2. Fluent Workspace
    {
        let mut h = default_harness("workspace", DesignLanguage::Fluent);
        h.run_step(&PreviewStep::ClickLabel("Native".to_owned()))
            .unwrap();
        assert_eq!(h.state().active_tab, 3);
        let clicks = h.state().click_count;
        h.run_step(&PreviewStep::ClickLabel("New".to_owned()))
            .unwrap();
        assert_eq!(h.state().click_count, clicks + 1);
        h.run_step(&PreviewStep::ClickLabel("Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_basic_dialog);
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".to_owned(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(!h.state().show_basic_dialog);
        assert_healthy(&mut h, "workspace/fluent");
    }

    // 3. HIG Workspace
    {
        let mut h = default_harness("workspace", DesignLanguage::Hig);
        h.run_step(&PreviewStep::ClickLabel("Inputs".to_owned()))
            .unwrap();
        assert_eq!(h.state().active_tab, 1);
        h.run_step(&PreviewStep::ClickLabel("?".to_owned()))
            .unwrap();
        assert!(h.state().show_basic_dialog);
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".to_owned(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(!h.state().show_basic_dialog);
        assert_healthy(&mut h, "workspace/hig");
    }

    // 4. Adwaita Workspace
    {
        let mut h = default_harness("workspace", DesignLanguage::Adwaita);
        h.run_step(&PreviewStep::ClickLabel("Tokens".to_owned()))
            .unwrap();
        assert_eq!(h.state().active_tab, 2);
        h.run_step(&PreviewStep::ClickLabel("Hide".to_owned()))
            .unwrap();
        assert!(!h.state().banner_visible);
        h.run_step(&PreviewStep::ClickLabel("Dialog".to_owned()))
            .unwrap();
        assert!(h.state().show_basic_dialog);
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".to_owned(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(!h.state().show_basic_dialog);
        assert_healthy(&mut h, "workspace/adwaita");
    }
}

#[test]
fn audit_desktop_all_scenes_themes_brightness() {
    for scene in ALL_SCENES {
        for &lang in &DesignLanguage::ALL {
            for brightness in [Brightness::Light, Brightness::Dark] {
                let mut h = build_scene_harness(
                    scene.id,
                    UiTheme::new(lang, brightness),
                    PreviewConfig::default(),
                )
                .expect("scene builds");
                h.settle_animations();
                let tree = dump_layout_tree(&mut h);
                let issues = lint_ui_layout(&tree, h.viewport());
                assert!(
                    issues.is_empty(),
                    "desktop matrix expected 0 layout issues for {}/{lang:?}/{brightness:?}, got: {issues:#?}",
                    scene.id
                );
                let contrast = lint_spec_contrast(&h.state().theme.to_spec());
                assert!(
                    contrast.is_empty(),
                    "desktop matrix expected 0 contrast issues for {}/{lang:?}/{brightness:?}, got: {contrast:#?}",
                    scene.id
                );
            }
        }
    }
}

#[test]
fn audit_toast_host_without_outer_max_dims_stays_compact() {
    use masonry::layout::{AsUnit, UnitPoint};
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{ZStackExt, sized_box, zstack};
    use xilem_ui::prelude::{ToastDuration, ToastHostState, primary_button, toast_host};

    #[derive(Clone)]
    struct ToastMountState {
        theme: UiTheme,
        toasts: ToastHostState,
    }

    let initial = ToastMountState {
        theme: UiTheme::new(DesignLanguage::Md3, Brightness::Light),
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
        xilem_preview::inspect::LayoutRect,
        xilem_preview::inspect::LayoutRect,
    )> {
        if node.type_name == "FocusFrame" {
            fn find_text_area(n: &LayoutNode) -> Option<xilem_preview::inspect::LayoutRect> {
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

    for &lang in &DesignLanguage::ALL {
        let mut h = default_harness("inputs", lang);

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
                "[{lang:?} {stage}] TextArea height ({}) must be strictly smaller than FocusFrame height ({})",
                area_rect.height,
                frame_rect.height
            );
            let frame_cy = frame_rect.y + frame_rect.height * 0.5;
            let area_cy = area_rect.y + area_rect.height * 0.5;
            assert!(
                (area_cy - frame_cy).abs() <= 2.0,
                "[{lang:?} {stage}] search TextArea center y ({area_cy:.2}) must match FocusFrame center y ({frame_cy:.2}) within 2px (frame={frame_rect:?}, area={area_rect:?})"
            );
            assert_healthy(&mut h, &format!("search_bar_center/{lang:?}/{stage}"));
        }
    }
}

#[test]
fn audit_phase2_custom_controls_disabled_bounds_and_drag() {
    use masonry::layout::AsUnit;
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, flex_col};
    use xilem_ui::prelude::{checkbox, radio, radio_group, range_slider, slider, switch};

    #[derive(Clone)]
    struct ControlStressState {
        theme: UiTheme,
        sw_disabled: bool,
        cb_disabled: bool,
        radio_idx: usize,
        slider_val: f64,
        range_val: (f64, f64),
        disabled_clicks: u32,
    }

    for &lang in &DesignLanguage::ALL {
        let initial = ControlStressState {
            theme: UiTheme::new(lang, Brightness::Light),
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
            "[{lang:?}] disabled custom controls must never fire callbacks"
        );
        assert_eq!(h.state().radio_idx, 0);

        // Extreme out-of-bounds slider drags must clamp cleanly to [min, max] without panicking.
        h.drag_type("UiSlider", 0, (0.5, 0.5), (-0.5, 0.5)).unwrap();
        assert!(
            (h.state().slider_val - 0.0).abs() < 1e-6,
            "[{lang:?}] out-of-bounds left drag should clamp to 0.0 (got {})",
            h.state().slider_val
        );
        h.drag_type("UiSlider", 0, (0.05, 0.5), (1.5, 0.5)).unwrap();
        assert!(
            (h.state().slider_val - 1.0).abs() < 1e-6,
            "[{lang:?}] out-of-bounds right drag should clamp to 1.0 (got {})",
            h.state().slider_val
        );

        // RangeSlider extreme crossing drag must clamp and prevent crossover.
        h.drag_type("RangeSliderWidget", 0, (0.25, 0.5), (1.4, 0.5))
            .unwrap();
        assert!(
            h.state().range_val.0 <= h.state().range_val.1
                && h.state().range_val.0 >= 0.0
                && h.state().range_val.1 <= 100.0,
            "[{lang:?}] range slider bounds invariant violated: {:?}",
            h.state().range_val
        );
    }
}

#[test]
fn audit_phase3_adwaita_breakpoint_boxed_list_and_alert_dialog() {
    use masonry::layout::{AsUnit, UnitPoint};
    use xilem_masonry::WidgetView;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, FlexExt, ZStackExt, flex_col, zstack};
    use xilem_ui::prelude::{
        alert_dialog, breakpoint_bin, clickable_action_row, header_bar_with_title, icons,
        preferences_group, primary_button, switch_row, view_switcher, view_switcher_bar,
    };

    #[derive(Clone)]
    struct AdwResponsiveState {
        theme: UiTheme,
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
            AdwResponsiveState {
                theme: UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
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
                    let list = preferences_group(
                        Some("Prefs"),
                        None,
                        vec![
                            switch_row(
                                "Opt A",
                                None::<&str>,
                                toggle,
                                |st: &mut AdwResponsiveState, v| {
                                    st.toggle = v;
                                },
                            )
                            .boxed(),
                            clickable_action_row(
                                "Run Row",
                                None::<&str>,
                                Some(icons::CHECK_CIRCLE),
                                None::<xilem_ui::prelude::UiButton<AdwResponsiveState, ()>>,
                                |st: &mut AdwResponsiveState| st.row_clicks += 1,
                            )
                            .boxed(),
                        ],
                    );
                    flex_col((
                        list,
                        primary_button("Open Alert", |st: &mut AdwResponsiveState| {
                            st.show_alert = true;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(12.px())
                };

                let wide = flex_col((
                    header_bar_with_title(
                        view_switcher(
                            &[("WidePrefs", icons::TUNE), ("WideStatus", icons::EXPLORE)],
                            s.tab,
                            |st: &mut AdwResponsiveState, idx| st.tab = idx,
                        ),
                        vec![],
                        vec![],
                    ),
                    make_body().flex(1.0),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);

                let narrow = flex_col((
                    make_body().flex(1.0),
                    view_switcher_bar(
                        &[
                            ("NarrowPrefs", icons::TUNE),
                            ("NarrowStatus", icons::EXPLORE),
                        ],
                        s.tab,
                        |st: &mut AdwResponsiveState, idx| st.tab = idx,
                    ),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch);

                let bin = breakpoint_bin(600.0, wide, narrow);
                let dlg = s.show_alert.then(|| {
                    alert_dialog(
                        "Confirm Reset?",
                        "This resets Adwaita preferences.",
                        "Cancel",
                        "Reset",
                        |st: &mut AdwResponsiveState| {
                            st.confirmed += 1;
                            st.show_alert = false;
                        },
                        |st: &mut AdwResponsiveState| st.show_alert = false,
                    )
                    .destructive(true)
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

    // 1. Wide layout (>= 600px): header ViewSwitcher visible, bottom ViewSwitcherBar stashed.
    let mut wide_h = make_harness(1180);
    wide_h
        .run_step(&PreviewStep::ClickLabel("WideStatus".to_owned()))
        .unwrap();
    assert_eq!(wide_h.state().tab, 1);
    assert!(
        wide_h
            .run_step(&PreviewStep::ClickLabel("NarrowPrefs".to_owned()))
            .is_err(),
        "narrow ViewSwitcherBar must be stashed when width >= 600px"
    );

    // 2. Narrow layout (< 600px): ViewSwitcherBar visible & interactive, alert_dialog opens and confirms.
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
    let issues = lint_ui_layout(&tree, narrow_h.viewport());
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "narrow Adwaita alert errors: {errors:#?}"
    );
    narrow_h
        .run_step(&PreviewStep::ClickLabel("Reset".to_owned()))
        .unwrap();
    assert!(!narrow_h.state().show_alert);
    assert_eq!(narrow_h.state().confirmed, 1);

    // Also render wide Adwaita tree to verify 0 errors.
    let wide_tree = dump_layout_tree(&mut wide_h);
    let wide_errors: Vec<_> = lint_ui_layout(&wide_tree, wide_h.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(
        wide_errors.is_empty(),
        "wide Adwaita errors: {wide_errors:#?}"
    );
}

#[test]
fn audit_phase4_md3_fixed_colors_ripple_and_text_field() {
    use masonry::layout::AsUnit;
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, flex_col};
    use xilem_ui::prelude::{filled_text_field, primary_button, ripple_layer, text_field};

    struct Phase4State {
        theme: UiTheme,
        clicks: usize,
        outlined: String,
        filled: String,
    }

    let build = |reduced_motion: bool| {
        build_ui_harness(
            Phase4State {
                theme: UiTheme::new(DesignLanguage::Md3, Brightness::Light)
                    .with_reduced_motion(reduced_motion),
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
fn audit_phase5_fluent_and_hig_desktop_controls() {
    use masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
    use masonry::core::{KeyboardEvent, TextEvent};
    use masonry::layout::{AsUnit, UnitPoint};
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{
        CrossAxisAlignment, FlexExt as _, ZStackExt, flex_col, flex_row, zstack,
    };
    use xilem_ui::prelude::{
        NavDestination, command_bar_with_overflow, icons, macos_alert_dialog, menu_item,
        navigation_view_collapsible, number_box, popup_button, primary_button, switch, text_field,
    };

    fn find_button_center(node: &LayoutNode, needle: &str) -> Option<(f64, f64)> {
        if node.type_name == "Button" && has_matching_text(node, needle) {
            return Some((
                node.rect.x + node.rect.width * 0.5,
                node.rect.y + node.rect.height * 0.5,
            ));
        }
        for c in &node.children {
            if let Some(pt) = find_button_center(c, needle) {
                return Some(pt);
            }
        }
        None
    }

    fn has_matching_text(node: &LayoutNode, needle: &str) -> bool {
        if node.text.as_deref().is_some_and(|t| t.contains(needle)) {
            return true;
        }
        node.children.iter().any(|c| has_matching_text(c, needle))
    }

    // 1. Windows Fluent 2: collapsible NavigationView (280px <-> 52px), CommandBar overflow popup,
    //    and NumberBox ArrowUp/ArrowDown clamped stepping.
    struct FluentPhase5 {
        theme: UiTheme,
        collapsed: bool,
        nav: usize,
        overflow_open: bool,
        overflow_picked: Option<usize>,
        num: f64,
    }

    let mut fh = build_ui_harness(
        FluentPhase5 {
            theme: UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
            collapsed: false,
            nav: 0,
            overflow_open: false,
            overflow_picked: None,
            num: 9.0,
        },
        |s| s.theme.clone(),
        |s| {
            let nav_items = [
                NavDestination::new("Home", icons::HOME),
                NavDestination::new("Settings", icons::SETTINGS),
            ];
            let footer = [NavDestination::new("Update", icons::REFRESH).with_badge("1")];
            let nav = navigation_view_collapsible(
                Some("WinUI"),
                &nav_items,
                &footer,
                s.nav,
                s.collapsed,
                |st: &mut FluentPhase5| st.collapsed = !st.collapsed,
                |st: &mut FluentPhase5, i| st.nav = i,
            );
            let cmd = command_bar_with_overflow(
                vec![primary_button("Save", |st: &mut FluentPhase5| st.num += 0.0).boxed()],
                vec![menu_item("OverflowOne"), menu_item("OverflowTwo")],
                s.overflow_open,
                |st: &mut FluentPhase5| st.overflow_open = !st.overflow_open,
                |st: &mut FluentPhase5, idx| {
                    st.overflow_picked = Some(idx);
                    st.overflow_open = false;
                },
                |st: &mut FluentPhase5| st.overflow_open = false,
            );
            let nb = number_box(s.num, 0.0, 10.0, 1.0, |st: &mut FluentPhase5, v| {
                st.num = v;
            });
            let right = flex_col((cmd, nb))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(12.px())
                .padding(Padding::all(16.px()));
            flex_row((nav, right.flex(1.0)))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .boxed()
        },
        PreviewConfig {
            width: 820,
            height: 480,
            ..PreviewConfig::default()
        },
    );

    let wide_tree = dump_layout_tree(&mut fh);
    let wide_nav_w = wide_tree.children[0].children[0].children[0].children[0]
        .rect
        .width;
    assert!(
        (wide_nav_w - 280.0).abs() < 2.0,
        "expanded NavigationView width expected 280px, got {wide_nav_w}"
    );
    // Toggle hamburger collapse
    fh.run_step(&PreviewStep::ClickType {
        type_name: "Button".to_owned(),
        index: 0,
    })
    .unwrap();
    assert!(fh.state().collapsed);
    let compact_tree = dump_layout_tree(&mut fh);
    let compact_nav_w = compact_tree.children[0].children[0].children[0].children[0]
        .rect
        .width;
    assert!(
        (compact_nav_w - 52.0).abs() < 2.0,
        "collapsed NavigationView width expected 52px, got {compact_nav_w}"
    );
    let compact_errors: Vec<_> = lint_ui_layout(&compact_tree, fh.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(
        compact_errors.is_empty(),
        "compact NavigationView errors: {compact_errors:#?}"
    );

    // CommandBar overflow menu open + item click
    fh.run_step(&PreviewStep::ClickType {
        type_name: "AnchoredPopupHost".to_owned(),
        index: 0,
    })
    .unwrap();
    assert!(fh.state().overflow_open);
    let popup_tree = dump_layout_tree(&mut fh);
    let (ox, oy) = find_button_center(&popup_tree, "OverflowTwo")
        .expect("OverflowTwo popup menu item should be visible");
    fh.click_point(ox, oy);
    assert!(!fh.state().overflow_open);
    assert_eq!(fh.state().overflow_picked, Some(1));

    // NumberBox ArrowUp -> clamped at 10.0, then ArrowDown -> 9.0
    fh.run_step(&PreviewStep::FocusType {
        type_name: "NumberStepCatcher".to_owned(),
        index: 0,
    })
    .unwrap();
    for _ in 0..3 {
        fh.inner_harness_mut()
            .process_text_event(TextEvent::Keyboard(KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::ArrowUp),
                code: Code::ArrowUp,
                ..KeyboardEvent::default()
            }));
        fh.pump_actions();
    }
    assert!((fh.state().num - 10.0).abs() < 1e-6);
    fh.inner_harness_mut()
        .process_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::ArrowDown),
            code: Code::ArrowDown,
            ..KeyboardEvent::default()
        }));
    fh.pump_actions();
    assert!((fh.state().num - 9.0).abs() < 1e-6);

    // 2. macOS HIG: FocusFrame + UiSwitch inactive-window pixel diff, popup_button, and macos_alert_dialog
    struct HigPhase5 {
        theme: UiTheme,
        text: String,
        toggle: bool,
        pop_sel: usize,
        pop_open: bool,
        show_alert: bool,
        confirmed: usize,
        dismissed: usize,
    }

    let mut hh = build_ui_harness(
        HigPhase5 {
            theme: UiTheme::new(DesignLanguage::Hig, Brightness::Light),
            text: "Aqua".to_owned(),
            toggle: true,
            pop_sel: 0,
            pop_open: false,
            show_alert: false,
            confirmed: 0,
            dismissed: 0,
        },
        |s| s.theme.clone(),
        |s| {
            let main = flex_col((
                text_field(s.text.clone(), |st: &mut HigPhase5, v| st.text = v).label("Field"),
                switch(s.toggle, |st: &mut HigPhase5, v| st.toggle = v),
                popup_button(
                    &["PDF", "PNG", "SVG"],
                    s.pop_sel,
                    s.pop_open,
                    |st: &mut HigPhase5| st.pop_open = !st.pop_open,
                    |st: &mut HigPhase5, i| {
                        st.pop_sel = i;
                        st.pop_open = false;
                    },
                    |st: &mut HigPhase5| st.pop_open = false,
                ),
                primary_button("Open Mac Alert", |st: &mut HigPhase5| {
                    st.show_alert = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px())
            .padding(Padding::all(16.px()));

            let alert = s.show_alert.then(|| {
                macos_alert_dialog(
                    "Export Document?",
                    "Select primary or cancel.",
                    "Export",
                    "Cancel",
                    |st: &mut HigPhase5| {
                        st.confirmed += 1;
                        st.show_alert = false;
                    },
                    |st: &mut HigPhase5| {
                        st.dismissed += 1;
                        st.show_alert = false;
                    },
                )
            });

            zstack((
                main.alignment(UnitPoint::TOP_LEFT),
                alert.map(|a| a.alignment(UnitPoint::CENTER)),
            ))
            .boxed()
        },
        PreviewConfig {
            width: 560,
            height: 440,
            ..PreviewConfig::default()
        },
    );

    hh.run_step(&PreviewStep::ClickType {
        type_name: "TextInput".to_owned(),
        index: 0,
    })
    .unwrap();
    let active_img = hh.render_image();
    hh.set_window_focused(false);
    let inactive_img = hh.render_image();
    assert_ne!(
        active_img.as_raw(),
        inactive_img.as_raw(),
        "macOS inactive window must visually remove OuterHalo3px focus ring & desaturate controls"
    );
    hh.set_window_focused(true);

    // PopUpButton select "SVG"
    hh.run_step(&PreviewStep::ClickType {
        type_name: "AnchoredPopupHost".to_owned(),
        index: 0,
    })
    .unwrap();
    assert!(hh.state().pop_open);
    let pop_tree = dump_layout_tree(&mut hh);
    let (px, py) = find_button_center(&pop_tree, "SVG").expect("SVG menu item should appear");
    hh.click_point(px, py);
    assert_eq!(hh.state().pop_sel, 2);
    assert!(!hh.state().pop_open);

    // macOS AlertDialog confirm + Escape
    hh.run_step(&PreviewStep::ClickLabel("Open Mac Alert".to_owned()))
        .unwrap();
    assert!(hh.state().show_alert);
    hh.settle_animations();
    let alert_tree = dump_layout_tree(&mut hh);
    let alert_errors: Vec<_> = lint_ui_layout(&alert_tree, hh.viewport())
        .into_iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    assert!(
        alert_errors.is_empty(),
        "macOS alert layout errors: {alert_errors:#?}"
    );
    hh.run_step(&PreviewStep::ClickLabel("Export".to_owned()))
        .unwrap();
    assert!(!hh.state().show_alert);
    assert_eq!(hh.state().confirmed, 1);

    hh.run_step(&PreviewStep::ClickLabel("Open Mac Alert".to_owned()))
        .unwrap();
    hh.run_step(&PreviewStep::FocusType {
        type_name: "EscapeCatcher".to_owned(),
        index: 0,
    })
    .unwrap();
    hh.run_step(&PreviewStep::PressEscape).unwrap();
    assert!(!hh.state().show_alert);
    assert_eq!(hh.state().dismissed, 1);
}

#[test]
fn audit_phase6_accessibility_keyboard_and_reduced_motion() {
    use masonry::accesskit::Role;
    use masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
    use masonry::core::{KeyboardEvent, TextEvent};
    use masonry::layout::{AsUnit, UnitPoint};
    use masonry::properties::Padding;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, ZStackExt, flex_col, zstack};
    use xilem_ui::prelude::{
        basic_dialog, determinate_circular_progress, dropdown_menu, linear_progress, menu_item,
        primary_button, range_slider, ripple_layer, slider, switch, ui_transition,
    };

    // 1. Modal Focus Trap, Instant Escape (without manual FocusType), & Double-Escape safety
    struct ModalTrapState {
        theme: UiTheme,
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
                theme: UiTheme::new(DesignLanguage::Md3, Brightness::Light)
                    .with_reduced_motion(reduced_motion),
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
            xilem_preview::inspect::format_tree_text(&tree_before)
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
fn memoize_with_spec_skips_unrelated_rebuilds_and_updates_on_data_or_theme_change() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::view::{flex_col, label};
    use xilem_ui::prelude::*;

    struct MemoState {
        theme: UiTheme,
        unrelated: u32,
        memo_key: String,
        rebuilds: Arc<AtomicUsize>,
    }

    let rebuilds = Arc::new(AtomicUsize::new(0));
    let mut h = build_ui_harness(
        MemoState {
            theme: UiTheme::new(DesignLanguage::Md3, Brightness::Light),
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
                memoize_with_spec(key, move |spec, data| {
                    counter.fetch_add(1, Ordering::SeqCst);
                    label(format!("{}:{data}", spec.name))
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

    // Switch DesignSpec theme/brightness: subtree MUST rebuild even though memo_key is unchanged.
    h.state_mut().theme = UiTheme::new(DesignLanguage::Fluent, Brightness::Dark);
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
    use masonry::properties::Dimensions;
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{label, sized_box};
    use xilem_ui::prelude::*;

    struct ListState {
        theme: UiTheme,
    }

    let mut h = build_ui_harness(
        ListState {
            theme: UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
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
    use xilem_masonry::WidgetView as _;
    use xilem_masonry::view::flex_col;
    use xilem_ui::prelude::*;

    struct QaState {
        theme: UiTheme,
        clicks: u32,
        toggled: bool,
    }

    let mut h = build_ui_harness(
        QaState {
            theme: UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
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
