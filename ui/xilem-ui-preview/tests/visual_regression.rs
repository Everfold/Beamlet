//! Headless golden snapshot and layout/contrast regression tests for Beamlet multi-theme UI.

use xilem_ui::prelude::{Brightness, DesignLanguage, UiTheme};
use xilem_ui_preview::harness::{PreviewConfig, PreviewStep};
use xilem_ui_preview::inspect::{IssueSeverity, dump_layout_tree, lint_layout, lint_spec_contrast};
use xilem_ui_preview::scenes::build_scene_harness;

fn assert_scene_snapshot(
    scene_id: &str,
    theme: UiTheme,
    steps: &[PreviewStep],
    snapshot_name: &str,
) {
    let config = PreviewConfig {
        width: 960,
        height: 640,
        ..PreviewConfig::default()
    };
    let mut harness =
        build_scene_harness(scene_id, theme, config).expect("scene harness should build");
    harness
        .run_steps(steps)
        .expect("interaction steps should replay cleanly");
    if !steps
        .iter()
        .any(|step| matches!(step, PreviewStep::AnimateMs(_)))
    {
        harness.settle_animations();
    }

    let tree = dump_layout_tree(&mut harness);
    let layout_errors: Vec<_> = lint_layout(&tree, harness.viewport())
        .into_iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .collect();
    assert!(
        layout_errors.is_empty(),
        "expected zero layout errors for {snapshot_name}, got: {layout_errors:#?}"
    );

    let contrast_issues = lint_spec_contrast(harness.current_spec());
    assert!(
        contrast_issues.is_empty(),
        "expected zero contrast issues for {snapshot_name}, got: {contrast_issues:#?}"
    );

    harness.check_render_snapshot(env!("CARGO_MANIFEST_DIR"), snapshot_name);
}

#[test]
fn golden_md3_light_workspace() {
    assert_scene_snapshot(
        "workspace",
        UiTheme::new(DesignLanguage::Md3, Brightness::Light),
        &[],
        "md3_light_workspace",
    );
}

#[test]
fn golden_fluent_dark_workspace() {
    assert_scene_snapshot(
        "workspace",
        UiTheme::new(DesignLanguage::Fluent, Brightness::Dark),
        &[],
        "fluent_dark_workspace",
    );
}

#[test]
fn golden_hig_light_workspace() {
    assert_scene_snapshot(
        "workspace",
        UiTheme::new(DesignLanguage::Hig, Brightness::Light),
        &[],
        "hig_light_workspace",
    );
}

#[test]
fn golden_adwaita_light_workspace() {
    assert_scene_snapshot(
        "workspace",
        UiTheme::new(DesignLanguage::Adwaita, Brightness::Light),
        &[],
        "adwaita_light_workspace",
    );
}

#[test]
fn golden_fluent_light_inputs_focused() {
    assert_scene_snapshot(
        "inputs",
        UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
        &[PreviewStep::FocusType {
            type_name: "TextArea".to_owned(),
            index: 0,
        }],
        "fluent_light_inputs_focused",
    );
}

#[test]
fn golden_hig_light_dialog_open() {
    assert_scene_snapshot(
        "overlays",
        UiTheme::new(DesignLanguage::Hig, Brightness::Light),
        &[PreviewStep::ClickLabel("Open Basic Dialog".to_owned())],
        "hig_light_dialog_open",
    );
}
