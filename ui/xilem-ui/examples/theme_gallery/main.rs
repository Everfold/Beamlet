//! Multi-theme showcase gallery entrypoint and automated headless audit.

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
        let title = format!("Beamlet UI ({})", theme.language.label());
        let view = app_view(state);
        std::iter::once(ui_window(wid, title, &theme, view).with_options(|opts| {
            opts.on_close(|s: &mut GalleryState| s.running = false)
                .with_initial_inner_size(LogicalSize::new(1200.0, 800.0))
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
    use xilem_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
    use xilem_preview::inspect::{
        A11yLintConfig, IssueSeverity, LayoutIssue, LayoutNode, LintConfig, dump_layout_tree,
        format_tree_text, lint_accessibility_with, lint_layout_with,
    };

    use super::*;
    use state::GalleryTab;

    fn make_gallery_harness(lang: DesignLanguage, tab: GalleryTab) -> PreviewHarness<GalleryState> {
        let mut initial = GalleryState::default();
        initial.theme = UiTheme::new(lang, Brightness::Light);
        initial.active_tab = tab;

        let initial_spec = initial.theme.to_spec();
        let initial_props = initial.theme.default_properties();
        let last_spec = RefCell::new(initial_spec.clone());

        let mut cfg = PreviewConfig {
            width: 1200,
            height: 800,
            ..PreviewConfig::default()
        };
        cfg.background = initial_spec.palette.window_bg;

        PreviewHarness::with_dynamic_properties(
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
        )
    }

    fn gallery_lint_configs() -> (LintConfig, A11yLintConfig) {
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
        (layout_cfg, a11y_cfg)
    }

    fn audit_snapshot(
        h: &mut PreviewHarness<GalleryState>,
        layout_cfg: &LintConfig,
        a11y_cfg: &A11yLintConfig,
        label: &str,
    ) -> Vec<LayoutIssue> {
        h.settle_animations();
        let tree = dump_layout_tree(h);

        // `masonry_testing::TestHarness` sets `use_system_fonts: false` with Latin-only `ROBOTO`.
        let cjk_label_ids: HashSet<u64> = {
            fn collect(node: &LayoutNode, out: &mut HashSet<u64>) {
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

        let layout_issues = lint_layout_with(&tree, h.viewport(), layout_cfg);
        let layout_errors: Vec<_> = layout_issues
            .iter()
            .filter(|i| {
                i.severity == IssueSeverity::Error
                    && !(i.kind == "zero-size" && cjk_label_ids.contains(&i.widget_id))
            })
            .collect();
        assert!(
            layout_errors.is_empty(),
            "[{label}] layout errors: {layout_errors:#?}\nTREE:\n{}",
            format_tree_text(&tree)
        );

        let a11y_issues = lint_accessibility_with(h, a11y_cfg);
        let a11y_errors: Vec<_> = a11y_issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .collect();
        assert!(
            a11y_errors.is_empty(),
            "[{label}] a11y errors: {a11y_errors:#?}"
        );

        let (vw, vh) = h.viewport();
        assert!((tree.rect.width - vw).abs() < 1.0 && (tree.rect.height - vh).abs() < 1.0);
        let img = h.render_image();
        assert!(img.width() > 0 && img.height() > 0);
        let mut distinct = HashSet::new();
        for px in img.pixels().step_by(43) {
            distinct.insert(px.0);
            if distinct.len() >= 2 {
                break;
            }
        }
        assert!(distinct.len() >= 2, "[{label}] collapsed to blank frame");

        layout_issues
            .into_iter()
            .filter(|i| i.kind == "overflow")
            .collect()
    }

    fn tree_has_text(node: &LayoutNode, needle: &str) -> bool {
        if node.text.as_deref().is_some_and(|t| t.contains(needle)) {
            return true;
        }
        node.children.iter().any(|c| tree_has_text(c, needle))
    }

    #[test]
    fn gallery_all_tabs_and_themes_pass_layout_and_a11y_audit() {
        let (layout_cfg, a11y_cfg) = gallery_lint_configs();
        for lang in DesignLanguage::ALL {
            for tab in GalleryTab::ALL {
                let mut h = make_gallery_harness(lang, tab);
                let overflows = audit_snapshot(
                    &mut h,
                    &layout_cfg,
                    &a11y_cfg,
                    &format!("{lang:?}/{tab:?}@1200x800"),
                );
                assert!(
                    overflows.is_empty(),
                    "[{lang:?}/{tab:?}@1200x800] unexpected overflow warnings: {overflows:#?}"
                );
            }
        }
    }

    #[test]
    fn gallery_window_resize_preserves_layout_integrity() {
        let (layout_cfg, a11y_cfg) = gallery_lint_configs();
        let viewports = [(1440, 900), (960, 720), (760, 600), (600, 520), (1200, 800)];

        let mut all_overflows = Vec::new();
        for lang in DesignLanguage::ALL {
            for tab in GalleryTab::ALL {
                let mut h = make_gallery_harness(lang, tab);
                for &(w, height) in &viewports {
                    h.resize(w, height);
                    let overflows = audit_snapshot(
                        &mut h,
                        &layout_cfg,
                        &a11y_cfg,
                        &format!("{lang:?}/{tab:?}@{w}x{height}"),
                    );
                    if !overflows.is_empty() && w >= 600 {
                        all_overflows.push((lang, tab, w, height, overflows));
                    }
                }
            }
        }
        assert!(
            all_overflows.is_empty(),
            "unexpected overflow warnings at standard desktop widths (>= 960px): {all_overflows:#?}"
        );
    }

    #[test]
    fn gallery_all_pages_interactive_events_match_targets() {
        let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
        h.settle_animations();

        // 1. Switcher bar: dark mode, reduced motion, theme switching
        assert_eq!(h.state().theme.brightness, Brightness::Light);
        h.run_step(&PreviewStep::ClickLabel("深色".into())).unwrap();
        assert_eq!(h.state().theme.brightness, Brightness::Dark);
        h.run_step(&PreviewStep::ClickLabel("减动效".into()))
            .unwrap();
        assert!(h.state().theme.reduced_motion);
        h.run_step(&PreviewStep::ClickLabel("Fluent".into()))
            .unwrap();
        assert_eq!(h.state().theme.language, DesignLanguage::Fluent);

        // Fluent collapsible navigation toggle
        assert!(!h.state().native.nav_collapsed);
        h.run_step(&PreviewStep::ClickType {
            type_name: "Button".into(),
            index: 0,
        })
        .unwrap();
        assert!(h.state().native.nav_collapsed);

        // Return to MD3 light
        h.run_step(&PreviewStep::ClickLabel("MD3".into())).unwrap();
        assert_eq!(h.state().theme.language, DesignLanguage::Md3);

        // 2. Components page: buttons, disabled no-op, destructive dialog + Escape,
        //    checkbox/switch/radio/sliders/tabs/chips
        let clicks_before = h.state().components.click_count;
        for lbl in ["主要", "次要", "浮起", "描边", "文本", "涟漪"] {
            h.run_step(&PreviewStep::ClickLabel(lbl.into())).unwrap();
        }
        assert_eq!(h.state().components.click_count, clicks_before + 6);

        let before_disabled = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("禁用".into())).unwrap();
        assert_eq!(h.state().components.click_count, before_disabled);

        h.run_step(&PreviewStep::ClickLabel("删除".into())).unwrap();
        assert!(h.state().overlays.show_destructive_dialog);
        h.run_steps(&[
            PreviewStep::HoverType {
                type_name: "EscapeCatcher".into(),
                index: 0,
            },
            PreviewStep::PressEscape,
        ])
        .unwrap();
        assert!(!h.state().overlays.show_destructive_dialog);

        let agreed_before = h.state().components.agree_terms;
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiCheckbox".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(h.state().components.agree_terms, !agreed_before);

        let notif_before = h.state().components.notifications_on;
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiSwitch".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(h.state().components.notifications_on, !notif_before);

        assert_eq!(h.state().components.radio_choice, 0);
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiRadioButton".into(),
            index: 1,
        })
        .unwrap();
        assert_eq!(h.state().components.radio_choice, 1);

        h.drag_type("UiSlider", 0, (0.2, 0.5), (0.85, 0.5)).unwrap();
        assert!(h.state().components.slider_val > 0.7);
        h.drag_type("RangeSliderWidget", 0, (0.25, 0.5), (0.55, 0.5))
            .unwrap();
        assert!(h.state().components.range_val.0 > 35.0);

        h.run_step(&PreviewStep::ClickLabel("外观".into())).unwrap();
        assert_eq!(h.state().components.selected_tab, 1);
        h.run_step(&PreviewStep::ClickLabel("筛选 B".into()))
            .unwrap();
        assert!(h.state().components.chip_flags[1]);

        // 3. Layout page: spacing, clamp, compact BreakpointBin simulation
        h.run_step(&PreviewStep::ClickLabel("布局".into())).unwrap();
        assert_eq!(h.state().active_tab, GalleryTab::Layout);
        h.run_step(&PreviewStep::ClickLabel("24".into())).unwrap();
        assert_eq!(h.state().layout.spacing_idx, 4);
        h.run_step(&PreviewStep::ClickLabel("880px".into()))
            .unwrap();
        assert_eq!(h.state().layout.clamp_idx, 2);

        let wide_tree = dump_layout_tree(&mut h);
        assert!(tree_has_text(&wide_tree, "宽屏：双栏 A"));
        h.run_step(&PreviewStep::ClickLabel("模拟窄屏 420px".into()))
            .unwrap();
        assert!(h.state().layout.simulate_compact);
        h.settle_animations();
        let narrow_tree = dump_layout_tree(&mut h);
        assert!(tree_has_text(&narrow_tree, "窄屏：堆叠 A"));

        // 4. Overlays page: dialogs, platform alerts, sheets, pickers, toast
        h.run_step(&PreviewStep::ClickLabel("浮层".into())).unwrap();
        assert_eq!(h.state().active_tab, GalleryTab::Overlays);

        let overlay_checks: &[(&str, fn(&GalleryState) -> bool)] = &[
            ("基础弹窗", |s| s.overlays.show_basic_dialog),
            ("底抽屉", |s| s.overlays.show_bottom_sheet),
            ("侧抽屉", |s| s.overlays.show_side_sheet),
            ("日期弹窗", |s| s.overlays.show_date_dialog),
            ("时间弹窗", |s| s.overlays.show_time_dialog),
        ];
        for &(btn, check_open) in overlay_checks {
            h.run_step(&PreviewStep::ClickLabel(btn.into())).unwrap();
            assert!(check_open(h.state()), "expected {btn} to open overlay");
            h.run_steps(&[
                PreviewStep::HoverType {
                    type_name: "EscapeCatcher".into(),
                    index: 0,
                },
                PreviewStep::PressEscape,
            ])
            .unwrap();
            assert!(!check_open(h.state()), "expected Escape to close {btn}");
        }

        h.run_step(&PreviewStep::ClickLabel("轻提示".into()))
            .unwrap();
        assert!(h.state().overlays.toasts.current().is_some());

        // Native alert across Adwaita & HIG
        for lang_btn in ["Adwaita", "HIG"] {
            h.run_step(&PreviewStep::ClickLabel(lang_btn.into()))
                .unwrap();
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            assert!(h.state().overlays.show_platform_alert);
            let before_confirm = h.state().components.click_count;
            h.run_step(&PreviewStep::ClickLabel("确定".into())).unwrap();
            assert!(!h.state().overlays.show_platform_alert);
            assert_eq!(h.state().components.click_count, before_confirm + 1);
        }

        // 5. Native page across all 4 platforms
        h.run_step(&PreviewStep::ClickLabel("平台".into())).unwrap();
        assert_eq!(h.state().active_tab, GalleryTab::Native);

        h.run_step(&PreviewStep::ClickLabel("MD3".into())).unwrap();
        let md3_before = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("发布".into())).unwrap();
        assert_eq!(h.state().components.click_count, md3_before + 1);

        h.run_step(&PreviewStep::ClickLabel("Fluent".into()))
            .unwrap();
        let fluent_before = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("保存".into())).unwrap();
        assert!(h.state().components.click_count >= fluent_before + 1);

        h.run_step(&PreviewStep::ClickLabel("HIG".into())).unwrap();
        let sw_before = h.state().native.native_switch;
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiSwitch".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(h.state().native.native_switch, !sw_before);

        h.run_step(&PreviewStep::ClickLabel("Adwaita".into()))
            .unwrap();
        let adw_before = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("同步".into())).unwrap();
        assert_eq!(h.state().components.click_count, adw_before + 1);

        // 6. Lab page: memoize_with_spec counters & virtual_list
        h.run_step(&PreviewStep::ClickLabel("性能".into())).unwrap();
        assert_eq!(h.state().active_tab, GalleryTab::Lab);
        h.settle_animations();

        let memo_after_mount = h
            .state()
            .perf
            .memoized_builds
            .load(std::sync::atomic::Ordering::Relaxed);
        let unmemo_after_mount = h
            .state()
            .perf
            .unmemoized_builds
            .load(std::sync::atomic::Ordering::Relaxed);

        h.run_step(&PreviewStep::ClickLabel("无关状态 +1".into()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("无关状态 +1".into()))
            .unwrap();
        assert_eq!(h.state().perf.unrelated_ticks, 2);
        assert!(
            h.state()
                .perf
                .unmemoized_builds
                .load(std::sync::atomic::Ordering::Relaxed)
                >= unmemo_after_mount + 2,
            "unmemoized with_spec must rebuild on unrelated ticks"
        );
        assert_eq!(
            h.state()
                .perf
                .memoized_builds
                .load(std::sync::atomic::Ordering::Relaxed),
            memo_after_mount,
            "memoize_with_spec must NOT rebuild on unrelated ticks"
        );

        h.run_step(&PreviewStep::ClickLabel("切换 Key".into()))
            .unwrap();
        assert_eq!(h.state().perf.memo_tag, "Beta");
        assert_eq!(
            h.state()
                .perf
                .memoized_builds
                .load(std::sync::atomic::Ordering::Relaxed),
            memo_after_mount + 1,
            "memoize_with_spec must rebuild when memo_tag changes"
        );

        h.run_step(&PreviewStep::ClickLabel("#5000".into()))
            .unwrap();
        assert_eq!(h.state().perf.virtual_jump_idx, Some(5000));
        h.settle_animations();
        let lab_tree = dump_layout_tree(&mut h);
        assert!(
            tree_has_text(&lab_tree, "Row #50"),
            "virtual_list should render jumped neighborhood around #5000"
        );
    }
}
