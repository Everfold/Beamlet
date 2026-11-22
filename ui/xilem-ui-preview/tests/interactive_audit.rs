//! Exhaustive headless interactive & layout audit across all Beamlet UI components and themes.

use std::collections::HashSet;

use xilem_ui::prelude::{Brightness, DesignLanguage, ToggleableState, UiTheme};
use xilem_ui_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
use xilem_ui_preview::inspect::{
    IssueSeverity, LayoutNode, dump_layout_tree, lint_layout, lint_spec_contrast,
};
use xilem_ui_preview::scenes::{ALL_SCENES, SceneState, build_scene_harness};

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
    let issues = lint_layout(&tree, harness.viewport());
    let errors: Vec<_> = issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "layout errors in {ctx_label}: {errors:#?}"
    );
    let contrast = lint_spec_contrast(harness.current_spec());
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
        let sheet_issues = lint_layout(&sheet_tree, h.viewport());
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
                let issues = lint_layout(&tree, h.viewport());
                assert!(
                    issues.is_empty(),
                    "desktop matrix expected 0 layout issues for {}/{lang:?}/{brightness:?}, got: {issues:#?}",
                    scene.id
                );
                let contrast = lint_spec_contrast(h.current_spec());
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
    use xilem::WidgetView;
    use xilem::masonry::layout::{AsUnit, UnitPoint};
    use xilem::masonry::properties::Padding;
    use xilem::style::Style as _;
    use xilem::view::{ZStackExt, sized_box, zstack};
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

    let mut h = PreviewHarness::new(
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
        xilem_ui_preview::inspect::LayoutRect,
        xilem_ui_preview::inspect::LayoutRect,
    )> {
        if node.type_name == "FocusFrame" {
            fn find_text_area(n: &LayoutNode) -> Option<xilem_ui_preview::inspect::LayoutRect> {
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
