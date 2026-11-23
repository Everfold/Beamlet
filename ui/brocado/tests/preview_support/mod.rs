//! Shared `brocado` preview harness wrapper, widget lint configuration, and WCAG contrast audit.

use std::cell::RefCell;

use brocado::prelude::*;
use brocado_preview::harness::{PreviewConfig, PreviewHarness};
use brocado_preview::inspect::{
    A11yIssue, A11yLintConfig, ContrastCheck, ContrastIssue, LayoutIssue, LayoutNode, LintConfig,
    lint_accessibility_with, lint_contrast, lint_layout_with,
};
use masonry::properties::Dimensions;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::sized_box;

pub mod scenes;

/// Returns a [`LintConfig`] extended with `brocado` / `brocado-core` custom widgets and
/// overlay/decorator containers.
#[must_use]
pub fn ui_lint_config() -> LintConfig {
    LintConfig::default()
        .with_visible_leaf_types([
            "UiCheckbox",
            "UiSwitch",
            "UiSlider",
            "UiRadioButton",
            "RangeSliderWidget",
            "LinearProgressWidget",
            "DeterminateCircularWidget",
        ])
        .with_overflow_containers(["EscapeCatcher", "FocusFrame", "Badged", "AnchoredPopupHost"])
}

/// Runs [`lint_layout_with`] using [`ui_lint_config()`].
#[must_use]
pub fn lint_ui_layout(root: &LayoutNode, viewport: (f64, f64)) -> Vec<LayoutIssue> {
    lint_layout_with(root, viewport, &ui_lint_config())
}

/// Returns an [`A11yLintConfig`] extended with `brocado` / `brocado-core` custom widgets and
/// modal dialog wrappers.
#[must_use]
pub fn ui_a11y_lint_config() -> A11yLintConfig {
    A11yLintConfig::default()
        .with_interactive_types([
            "UiCheckbox",
            "UiSwitch",
            "UiSlider",
            "UiRadioButton",
            "RangeSliderWidget",
            "LinearProgressWidget",
            "DeterminateCircularWidget",
        ])
        .with_dialog_types(["EscapeCatcher"])
}

/// Runs [`lint_accessibility_with`] using [`ui_a11y_lint_config()`].
pub fn lint_ui_accessibility<State: 'static>(
    harness: &mut PreviewHarness<State>,
) -> Vec<A11yIssue> {
    lint_accessibility_with(harness, &ui_a11y_lint_config())
}

/// Audits semantic foreground/background pairs inside `spec` against WCAG 2.1 thresholds.
#[must_use]
pub fn lint_spec_contrast(spec: &DesignSpec) -> Vec<ContrastIssue> {
    let p = &spec.palette;
    let checks = [
        ContrastCheck::new("text_primary/surface", p.text_primary, p.surface, 4.5),
        ContrastCheck::new("text_primary/window_bg", p.text_primary, p.window_bg, 4.5),
        ContrastCheck::new("text_secondary/surface", p.text_secondary, p.surface, 3.0),
        ContrastCheck::new("on_accent/accent", p.on_accent, p.accent, 3.0),
        ContrastCheck::new(
            "on_accent_container/accent_container",
            p.on_accent_container,
            p.accent_container,
            4.5,
        ),
        ContrastCheck::new("on_error/error", p.on_error, p.error, 3.0),
        ContrastCheck::new("on_success/success", p.on_success, p.success, 3.0),
        ContrastCheck::new("on_warning/warning", p.on_warning, p.warning, 3.0),
    ];
    lint_contrast(&checks)
}

/// Builds a [`PreviewHarness`] wired to `brocado`'s `ui_scope`, dynamic `DefaultProperties`
/// switching, and window background color.
pub fn build_ui_harness<State: 'static>(
    state: State,
    theme_fn: fn(&State) -> UiTheme,
    logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
    mut config: PreviewConfig,
) -> PreviewHarness<State> {
    let initial_theme = theme_fn(&state);
    let initial_spec = initial_theme.to_spec();
    let initial_props = initial_theme.default_properties();
    config.background = initial_spec.palette.window_bg;

    let last_spec = RefCell::new(initial_spec);
    PreviewHarness::with_dynamic_properties(
        state,
        initial_props,
        move |s| {
            let next_spec = theme_fn(s).to_spec();
            let mut prev = last_spec.borrow_mut();
            if next_spec != *prev {
                *prev = next_spec.clone();
                Some(cached_default_properties(&next_spec))
            } else {
                None
            }
        },
        move |s| {
            let theme = theme_fn(s);
            let spec = theme.to_spec();
            ui_scope(
                &theme,
                sized_box(logic(s))
                    .dims(Dimensions::STRETCH)
                    .background_color(spec.palette.window_bg),
            )
        },
        config,
    )
}
