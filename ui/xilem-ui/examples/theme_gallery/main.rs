//! Multi-theme showcase gallery demonstrating Beamlet UI's layout system, components,
//! platform-native suites, and incremental/virtualization architecture.

#![warn(clippy::print_stdout, clippy::print_stderr)]

mod overlays;
mod pages;
mod shells;
mod state;

use xilem::dpi::LogicalSize;
use xilem::{AnyWidgetView, AppState, EventLoop, WidgetView, Xilem};
use xilem_ui::prelude::*;

use state::GalleryState;

impl AppState for GalleryState {
    fn keep_running(&self) -> bool {
        self.running
    }
}

fn app_view(state: &mut GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    match state.theme.language {
        DesignLanguage::Md3 => shells::md3_shell(state).boxed(),
        DesignLanguage::Fluent => shells::fluent_shell(state).boxed(),
        DesignLanguage::Hig => shells::hig_shell(state).boxed(),
        DesignLanguage::Adwaita => shells::adwaita_shell(state).boxed(),
    }
}

fn main() {
    let initial = GalleryState::default();
    let initial_props = initial.theme.default_properties();
    let app = Xilem::new(initial, |state: &mut GalleryState| {
        let wid = state.window_id;
        let theme = state.theme.clone();
        let title = format!("Beamlet 多主题 UI 展示台 ({})", theme.language.label());
        let view = app_view(state);
        std::iter::once(ui_window(wid, title, &theme, view).with_options(|opts| {
            opts.on_close(|s: &mut GalleryState| s.running = false)
                .with_initial_inner_size(LogicalSize::new(1240.0, 820.0))
        }))
    })
    .with_default_properties(initial_props);
    app.run_in(EventLoop::with_user_event())
        .expect("theme_gallery event loop exited cleanly");
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use xilem::masonry::properties::Dimensions;
    use xilem::style::Style as _;
    use xilem::view::sized_box;
    use xilem_preview::harness::{PreviewConfig, PreviewHarness};
    use xilem_preview::inspect::{
        A11yLintConfig, IssueSeverity, LintConfig, dump_layout_tree, lint_accessibility_with,
        lint_layout_with,
    };

    use super::*;
    use state::GalleryTab;

    #[test]
    fn gallery_all_tabs_and_themes_pass_layout_and_a11y_audit() {
        let layout_cfg = LintConfig::default()
            .with_visible_leaf_types([
                "UiCheckbox",
                "UiSwitch",
                "UiSlider",
                "UiRadioButton",
                "RangeSliderWidget",
                "LinearProgressWidget",
                "DeterminateCircularWidget",
            ])
            .with_overflow_containers([
                "EscapeCatcher",
                "FocusFrame",
                "Badged",
                "AnchoredPopupHost",
            ]);
        let a11y_cfg = A11yLintConfig::default()
            .with_interactive_types([
                "UiCheckbox",
                "UiSwitch",
                "UiSlider",
                "UiRadioButton",
                "RangeSliderWidget",
                "LinearProgressWidget",
                "DeterminateCircularWidget",
            ])
            .with_dialog_types(["EscapeCatcher"]);

        for lang in DesignLanguage::ALL {
            for tab in GalleryTab::ALL {
                let mut initial = GalleryState::default();
                initial.theme = UiTheme::new(lang, Brightness::Light);
                initial.active_tab = tab;

                let initial_spec = initial.theme.to_spec();
                let initial_props = initial.theme.default_properties();
                let last_spec = RefCell::new(initial_spec.clone());

                let mut cfg = PreviewConfig {
                    width: 1240,
                    height: 820,
                    ..PreviewConfig::default()
                };
                cfg.background = initial_spec.palette.window_bg;

                let mut h = PreviewHarness::with_dynamic_properties(
                    initial,
                    initial_props,
                    move |s| {
                        let next_spec = s.theme.to_spec();
                        let mut prev = last_spec.borrow_mut();
                        if next_spec != *prev {
                            *prev = next_spec.clone();
                            Some(cached_default_properties(&next_spec))
                        } else {
                            None
                        }
                    },
                    |s| {
                        let theme = s.theme.clone();
                        let bg = theme.to_spec().palette.window_bg;
                        ui_scope(
                            &theme,
                            sized_box(app_view(s))
                                .dims(Dimensions::STRETCH)
                                .background_color(bg),
                        )
                    },
                    cfg,
                );

                h.settle_animations();
                let tree = dump_layout_tree(&mut h);
                // `masonry_testing::TestHarness` sets `use_system_fonts: false` and loads only
                // Latin `ROBOTO`, so labels containing CJK characters cannot be shaped by Roboto in
                // headless unit tests (unlike the live desktop `EventLoop` which uses system fonts).
                let cjk_label_ids: HashSet<u64> = {
                    fn collect(node: &xilem_preview::inspect::LayoutNode, out: &mut HashSet<u64>) {
                        if node.type_name == "Label"
                            && let Some(txt) = &node.text
                            && txt.chars().any(|c| !c.is_ascii())
                        {
                            out.insert(node.id);
                        }
                        for c in &node.children {
                            collect(c, out);
                        }
                    }
                    let mut ids = HashSet::new();
                    collect(&tree, &mut ids);
                    ids
                };

                let layout_issues = lint_layout_with(&tree, h.viewport(), &layout_cfg);
                let layout_errors: Vec<_> = layout_issues
                    .iter()
                    .filter(|i| {
                        i.severity == IssueSeverity::Error
                            && !(i.kind == "zero-size" && cjk_label_ids.contains(&i.widget_id))
                    })
                    .collect();
                assert!(
                    layout_errors.is_empty(),
                    "[{lang:?} / {tab:?}] layout errors: {layout_errors:#?}\nTREE:\n{}",
                    xilem_preview::inspect::format_tree_text(&tree)
                );

                let a11y_issues = lint_accessibility_with(&mut h, &a11y_cfg);
                let a11y_errors: Vec<_> = a11y_issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::Error)
                    .collect();
                assert!(
                    a11y_errors.is_empty(),
                    "[{lang:?} / {tab:?}] a11y errors: {a11y_errors:#?}"
                );

                let img = h.render_image();
                let mut distinct = HashSet::new();
                for px in img.pixels().step_by(43) {
                    distinct.insert(px.0);
                    if distinct.len() >= 2 {
                        break;
                    }
                }
                assert!(
                    distinct.len() >= 2,
                    "[{lang:?} / {tab:?}] collapsed to blank frame"
                );
            }
        }
    }
}
