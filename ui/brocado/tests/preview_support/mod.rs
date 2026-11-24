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

/// Returns a [`LintConfig`] extended with `brocado` custom widgets and
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
            "WavyLinearProgressWidget",
            "DeterminateCircularWidget",
        ])
        .with_overflow_containers(["EscapeCatcher", "FocusFrame", "Badged", "AnchoredPopupHost"])
}

/// Runs [`lint_layout_with`] using [`ui_lint_config()`].
#[must_use]
pub fn lint_ui_layout(root: &LayoutNode, viewport: (f64, f64)) -> Vec<LayoutIssue> {
    lint_layout_with(root, viewport, &ui_lint_config())
}

/// Returns an [`A11yLintConfig`] extended with `brocado` custom widgets and
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
            "WavyLinearProgressWidget",
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

/// Audits semantic foreground/background pairs inside `theme.scheme` against WCAG 2.1 thresholds.
#[must_use]
pub fn lint_theme_contrast(theme: &Theme) -> Vec<ContrastIssue> {
    let s = &theme.scheme;
    let checks = [
        ContrastCheck::new(
            "on_surface/surface_container",
            s.on_surface,
            s.surface_container,
            4.5,
        ),
        ContrastCheck::new("on_surface/surface", s.on_surface, s.surface, 4.5),
        ContrastCheck::new(
            "on_surface_variant/surface_container",
            s.on_surface_variant,
            s.surface_container,
            3.0,
        ),
        ContrastCheck::new("on_primary/primary", s.on_primary, s.primary, 3.0),
        ContrastCheck::new(
            "on_primary_container/primary_container",
            s.on_primary_container,
            s.primary_container,
            4.5,
        ),
        ContrastCheck::new("on_error/error", s.on_error, s.error, 3.0),
        ContrastCheck::new("on_success/success", s.on_success, s.success, 3.0),
        ContrastCheck::new("on_warning/warning", s.on_warning, s.warning, 3.0),
    ];
    lint_contrast(&checks)
}

/// Builds a [`PreviewHarness`] wired to `brocado`'s `theme_scope`, dynamic `DefaultProperties`
/// switching, and window background color.
pub fn build_ui_harness<State: 'static>(
    state: State,
    theme_fn: fn(&State) -> Theme,
    logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
    mut config: PreviewConfig,
) -> PreviewHarness<State> {
    let initial_theme = theme_fn(&state);
    let initial_props = initial_theme.default_properties();
    config.background = initial_theme.scheme.surface;

    let last_theme = RefCell::new(initial_theme);
    PreviewHarness::with_dynamic_properties(
        state,
        initial_props,
        move |s| {
            let next_theme = theme_fn(s);
            let mut prev = last_theme.borrow_mut();
            if next_theme != *prev {
                *prev = next_theme.clone();
                Some(cached_default_properties(&next_theme))
            } else {
                None
            }
        },
        move |s| {
            let theme = theme_fn(s);
            theme_scope(
                &theme,
                sized_box(logic(s))
                    .dims(Dimensions::STRETCH)
                    .background_color(theme.scheme.surface),
            )
        },
        config,
    )
}
