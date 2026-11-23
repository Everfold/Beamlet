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

        let mut portal_overflows = Vec::new();
        collect_portal_horizontal_overflows(&tree, &mut portal_overflows);
        assert!(
            portal_overflows.is_empty(),
            "[{label}] horizontal portal overflows:\n{}",
            portal_overflows.join("\n")
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

    fn collect_portal_horizontal_overflows(node: &LayoutNode, out: &mut Vec<String>) {
        fn check_children(portal_x0: f64, portal_x1: f64, n: &LayoutNode, out: &mut Vec<String>) {
            for c in &n.children {
                if c.rect.width > 1.0 && c.rect.height > 1.0 {
                    if c.rect.x < portal_x0 - 2.0 || c.rect.x + c.rect.width > portal_x1 + 2.0 {
                        out.push(format!(
                            "{} #{} ({:.1}..{:.1}) overflows portal ({:.1}..{:.1})",
                            c.type_name,
                            c.id,
                            c.rect.x,
                            c.rect.x + c.rect.width,
                            portal_x0,
                            portal_x1
                        ));
                    }
                    check_children(portal_x0, portal_x1, c, out);
                }
            }
        }
        if (node.type_name == "Portal" || node.type_name == "ScrollPortal") && node.rect.width > 1.0
        {
            check_children(node.rect.x, node.rect.x + node.rect.width, node, out);
            return;
        }
        for c in &node.children {
            collect_portal_horizontal_overflows(c, out);
        }
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
        let viewports = [
            (1440, 900),
            (960, 720),
            (760, 600),
            (640, 360),
            (520, 560),
            (480, 640),
            (400, 600),
            (360, 640),
            (320, 480),
            (1200, 800),
        ];

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
                    if !overflows.is_empty() {
                        all_overflows.push((lang, tab, w, height, overflows));
                    }
                }
            }
        }
        assert!(
            all_overflows.is_empty(),
            "unexpected overflow warnings across resized viewports: {all_overflows:#?}"
        );
    }

    #[test]
    fn gallery_all_pages_interactive_events_match_targets() {
        let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
        h.settle_animations();

        // 1. Switcher bar: dark mode, reduced motion, accent select, theme switching
        assert_eq!(h.state().theme.brightness, Brightness::Light);
        h.run_step(&PreviewStep::ClickLabel("默认".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
        assert_visual_invariants(&mut h, "Top accent select open");
        h.run_step(&PreviewStep::ClickLabel("蓝".into())).unwrap();
        assert_eq!(h.state().theme.accent, UiAccent::PRESETS[1]);
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);

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
        h.run_step(&PreviewStep::ClickLabel("浅色".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
        assert_visual_invariants(&mut h, "MD3 Components + open Select");
        h.run_step(&PreviewStep::ClickLabel("自动".into())).unwrap();
        assert_eq!(h.state().components.select_index, 0);
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);

        // Resizing across the 680px shell breakpoint with an open popup must keep at most 1 live layer
        h.run_step(&PreviewStep::ClickLabel("蓝".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
        h.resize(400, 600);
        assert_visual_invariants(&mut h, "Narrow shell after resize with open popup");
        h.run_step(&PreviewStep::ClickLabel("绿".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);
        h.resize(1200, 800);

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

        // Tooltips (plain + rich) must each spawn exactly one TooltipLayer
        for tip_lbl in ["悬停提示", "富提示"] {
            h.run_step(&PreviewStep::HoverLabel(tip_lbl.into()))
                .unwrap();
            assert_eq!(count_layer_type(&mut h, "TooltipLayer"), 1);
            assert_visual_invariants(&mut h, &format!("Tooltip on {tip_lbl}"));
        }

        // Native alert across Adwaita & HIG
        for lang_btn in ["Adwaita", "HIG"] {
            h.run_step(&PreviewStep::ClickLabel(lang_btn.into()))
                .unwrap();
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            assert!(h.state().overlays.show_platform_alert);
            assert_visual_invariants(&mut h, &format!("{lang_btn} Native Alert"));
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
        let fluent_hosts = h.find_widget_ids("AnchoredPopupHost");
        if let Some(&overflow_host) = fluent_hosts.get(1) {
            let rr = h.inner_harness_mut().render_root();
            let overflow_btn = rr.get_widget(overflow_host).and_then(|host_ref| {
                fn find_btn(
                    n: &xilem::masonry::core::WidgetRef<'_, dyn xilem::masonry::core::Widget>,
                ) -> Option<xilem::masonry::core::WidgetId> {
                    if n.inner().short_type_name() == "Button" {
                        return Some(n.id());
                    }
                    n.children().iter().find_map(find_btn)
                }
                find_btn(&host_ref)
            });
            if let Some(btn_id) = overflow_btn {
                h.click_widget_id(btn_id);
                assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
                assert_visual_invariants(&mut h, "Fluent CommandBar overflow open");
                h.run_step(&PreviewStep::ClickLabel("导出".into())).unwrap();
                assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);
            }
        }
        let fluent_before = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("保存".into())).unwrap();
        assert!(h.state().components.click_count >= fluent_before + 1);

        h.run_step(&PreviewStep::ClickLabel("HIG".into())).unwrap();
        h.run_step(&PreviewStep::ClickLabel("自动".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
        assert_visual_invariants(&mut h, "HIG Native + open popup_button");
        h.run_step(&PreviewStep::ClickLabel("Metal".into()))
            .unwrap();
        assert_eq!(h.state().native.hig_popup_idx, 1);
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);
        let sw_before = h.state().native.native_switch;
        h.run_step(&PreviewStep::ClickType {
            type_name: "UiSwitch".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(h.state().native.native_switch, !sw_before);

        h.run_step(&PreviewStep::ClickLabel("Adwaita".into()))
            .unwrap();
        h.run_step(&PreviewStep::ClickLabel("WGPU".into())).unwrap();
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 1);
        assert_visual_invariants(&mut h, "Adwaita Native + open combo_row");
        h.run_step(&PreviewStep::ClickLabel("CPU".into())).unwrap();
        assert_eq!(h.state().native.adw_combo_idx, 1);
        assert_eq!(count_layer_type(&mut h, "PopupLayer"), 0);
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
            memo_after_mount + 2,
            "memoize_with_spec must rebuild when memo_tag changes (wide + narrow shell branches)"
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
        let mut align_issues = Vec::new();
        collect_icon_misalignments(&lab_tree, &mut align_issues);
        assert!(
            align_issues.is_empty(),
            "icon misalignments on Lab page:\n{}",
            align_issues.join("\n")
        );
    }

    fn is_direct_icon_leaf(n: &LayoutNode) -> bool {
        if n.type_name == "Svg" {
            return true;
        }
        if n.type_name == "SizedBox" && n.children.len() == 1 {
            return is_direct_icon_leaf(&n.children[0]);
        }
        false
    }

    fn collect_icon_misalignments(node: &LayoutNode, out: &mut Vec<String>) {
        if node.type_name == "Flex" && node.children.len() >= 2 {
            for pair in node.children.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                let side_by_side = (a.rect.x + a.rect.width <= b.rect.x + 1.5
                    || b.rect.x + b.rect.width <= a.rect.x + 1.5)
                    && a.rect.height > 1.0
                    && b.rect.height > 1.0;
                if side_by_side {
                    let svg_vs_lbl = if is_direct_icon_leaf(a)
                        && b.type_name == "Label"
                        && b.rect.height > 1.0
                    {
                        Some((
                            a.rect.y + a.rect.height * 0.5,
                            b.rect.y + b.rect.height * 0.5,
                            a.id,
                            b.id,
                        ))
                    } else if is_direct_icon_leaf(b)
                        && a.type_name == "Label"
                        && a.rect.height > 1.0
                    {
                        Some((
                            b.rect.y + b.rect.height * 0.5,
                            a.rect.y + a.rect.height * 0.5,
                            b.id,
                            a.id,
                        ))
                    } else {
                        None
                    };
                    if let Some((svg_cy, lbl_cy, sid, lid)) = svg_vs_lbl
                        && (svg_cy - lbl_cy).abs() > 4.0
                    {
                        out.push(format!(
                            "Flex #{}: Svg-container #{sid} cy={svg_cy:.1} vs Label #{lid} cy={lbl_cy:.1} (diff={:.1}px)",
                            node.id,
                            (svg_cy - lbl_cy).abs()
                        ));
                    }
                }
            }
        }
        for c in &node.children {
            collect_icon_misalignments(c, out);
        }
    }

    fn collect_clipped_layers(node: &LayoutNode, vw: f64, vh: f64, out: &mut Vec<String>) {
        if (node.type_name == "PopupLayer" || node.type_name == "TooltipLayer")
            && node.rect.width > 1.0
            && node.rect.height > 1.0
        {
            if node.rect.x < 0.0
                || node.rect.y < 0.0
                || node.rect.x + node.rect.width > vw + 1.0
                || node.rect.y + node.rect.height > vh + 1.0
            {
                let child_info = node
                    .children
                    .first()
                    .map(|c| {
                        format!(
                            " child=({:.1}, {:.1}, {:.1}x{:.1})",
                            c.rect.x, c.rect.y, c.rect.width, c.rect.height
                        )
                    })
                    .unwrap_or_default();
                out.push(format!(
                    "{} #{} out of bounds: ({:.1}, {:.1}, {:.1}x{:.1}) in ({vw:.0}x{vh:.0}){child_info}",
                    node.type_name,
                    node.id,
                    node.rect.x,
                    node.rect.y,
                    node.rect.width,
                    node.rect.height
                ));
            }
        }
        for c in &node.children {
            collect_clipped_layers(c, vw, vh, out);
        }
    }

    fn count_widgets(node: &LayoutNode, target: &str) -> usize {
        let self_hit = usize::from(node.type_name == target);
        self_hit
            + node
                .children
                .iter()
                .map(|c| count_widgets(c, target))
                .sum::<usize>()
    }

    fn count_layer_type(h: &mut PreviewHarness<GalleryState>, target: &str) -> usize {
        h.settle_animations();
        let tree = dump_layout_tree(h);
        count_widgets(&tree, target)
    }

    fn assert_visual_invariants(h: &mut PreviewHarness<GalleryState>, ctx: &str) {
        h.settle_animations();
        let (vw, vh) = h.viewport();
        let tree = dump_layout_tree(h);
        let mut issues = Vec::new();
        let popup_count = count_widgets(&tree, "PopupLayer");
        if popup_count > 1 {
            issues.push(format!(
                "expected at most 1 PopupLayer, found {popup_count}"
            ));
        }
        let tooltip_count = count_widgets(&tree, "TooltipLayer");
        if tooltip_count > 1 {
            issues.push(format!(
                "expected at most 1 TooltipLayer, found {tooltip_count}"
            ));
        }
        collect_icon_misalignments(&tree, &mut issues);
        collect_clipped_layers(&tree, vw, vh, &mut issues);
        collect_portal_horizontal_overflows(&tree, &mut issues);
        assert!(
            issues.is_empty(),
            "visual invariant failures ({ctx}):\n{}",
            issues.join("\n")
        );
    }

    #[test]
    #[ignore = "Manual visual audit helper dumping PNGs to THEME_GALLERY_AUDIT_DIR or temp dir"]
    fn audit_dump_all_triggered_pngs() {
        let out_dir_buf = std::env::var_os("THEME_GALLERY_AUDIT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("beamlet_theme_gallery_audit"));
        let out_dir = out_dir_buf.as_path();
        std::fs::create_dir_all(out_dir).expect("create audit_pngs dir");

        let mut issues = Vec::new();
        let save = |h: &mut PreviewHarness<GalleryState>, name: &str, issues: &mut Vec<String>| {
            h.settle_animations();
            let (vw, vh) = h.viewport();
            let tree = dump_layout_tree(h);
            let mut local = Vec::new();
            let popup_count = count_widgets(&tree, "PopupLayer");
            if popup_count > 1 {
                local.push(format!(
                    "expected at most 1 PopupLayer, found {popup_count}"
                ));
            }
            let tooltip_count = count_widgets(&tree, "TooltipLayer");
            if tooltip_count > 1 {
                local.push(format!(
                    "expected at most 1 TooltipLayer, found {tooltip_count}"
                ));
            }
            collect_icon_misalignments(&tree, &mut local);
            collect_clipped_layers(&tree, vw, vh, &mut local);
            collect_portal_horizontal_overflows(&tree, &mut local);
            for m in local {
                issues.push(format!("[{name}] {m}"));
            }
            h.render_png(out_dir.join(name)).expect("write audit png");
        };

        // 1-3: Overlays -> Native Alert across Adwaita, HIG, Fluent
        for (lang, file) in [
            (DesignLanguage::Adwaita, "01_adwaita_native_alert.png"),
            (DesignLanguage::Hig, "02_hig_native_alert.png"),
            (DesignLanguage::Fluent, "03_fluent_native_alert.png"),
        ] {
            let mut h = make_gallery_harness(lang, GalleryTab::Overlays);
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            save(&mut h, file, &mut issues);
        }

        // 4-12: MD3 Overlays (dialogs, sheets, pickers, toast, tooltips)
        for (lbl, file) in [
            ("基础弹窗", "04_md3_basic_dialog.png"),
            ("危险确认", "05_md3_destructive_dialog.png"),
            ("底抽屉", "06_md3_bottom_sheet.png"),
            ("侧抽屉", "07_md3_side_sheet.png"),
            ("日期弹窗", "08_md3_date_dialog.png"),
            ("时间弹窗", "09_md3_time_dialog.png"),
            ("轻提示", "10_md3_toast.png"),
        ] {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
            h.run_step(&PreviewStep::ClickLabel(lbl.into())).unwrap();
            save(&mut h, file, &mut issues);
        }

        for (lbl, file) in [
            ("悬停提示", "11_md3_plain_tooltip.png"),
            ("富提示", "12_md3_rich_tooltip.png"),
        ] {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
            h.run_step(&PreviewStep::HoverLabel(lbl.into())).unwrap();
            save(&mut h, file, &mut issues);
        }

        // 13 & 17: MD3 Components + open select
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
            save(&mut h, "17_md3_components.png", &mut issues);
            h.run_step(&PreviewStep::ClickLabel("浅色".into())).unwrap();
            save(&mut h, "13_md3_select_open.png", &mut issues);
        }

        // 14: Fluent Native overflow menu open
        {
            let mut h = make_gallery_harness(DesignLanguage::Fluent, GalleryTab::Native);
            let popup_hosts = h.find_widget_ids("AnchoredPopupHost");
            if let Some(&host_id) = popup_hosts.get(1) {
                let rr = h.inner_harness_mut().render_root();
                if let Some(host_ref) = rr.get_widget(host_id) {
                    fn first_button(
                        n: &xilem::masonry::core::WidgetRef<'_, dyn xilem::masonry::core::Widget>,
                    ) -> Option<xilem::masonry::core::WidgetId> {
                        if n.inner().short_type_name() == "Button" {
                            return Some(n.id());
                        }
                        n.children().iter().find_map(first_button)
                    }
                    if let Some(btn_id) = first_button(&host_ref) {
                        h.click_widget_id(btn_id);
                    }
                }
            }
            save(&mut h, "14_fluent_native_overflow.png", &mut issues);
        }

        // 15: HIG Native popup_button open
        {
            let mut h = make_gallery_harness(DesignLanguage::Hig, GalleryTab::Native);
            h.run_step(&PreviewStep::ClickLabel("自动".into())).unwrap();
            save(&mut h, "15_hig_native_popup.png", &mut issues);
        }

        // 16: Adwaita Native combo_row open
        {
            let mut h = make_gallery_harness(DesignLanguage::Adwaita, GalleryTab::Native);
            h.run_step(&PreviewStep::ClickLabel("WGPU".into())).unwrap();
            save(&mut h, "16_adwaita_native_combo.png", &mut issues);
        }

        // 18: Adwaita Layout page
        {
            let mut h = make_gallery_harness(DesignLanguage::Adwaita, GalleryTab::Layout);
            save(&mut h, "18_adwaita_layout.png", &mut issues);
        }

        // 19-24: Narrow & short responsive screenshots (360x640, 320x480, 640x360)
        for (lang, tab, w, height, file) in [
            (
                DesignLanguage::Md3,
                GalleryTab::Components,
                360,
                640,
                "19_narrow_360x640_md3_components.png",
            ),
            (
                DesignLanguage::Adwaita,
                GalleryTab::Layout,
                360,
                640,
                "20_narrow_360x640_adwaita_layout.png",
            ),
            (
                DesignLanguage::Fluent,
                GalleryTab::Lab,
                360,
                640,
                "21_narrow_360x640_fluent_lab.png",
            ),
            (
                DesignLanguage::Hig,
                GalleryTab::Native,
                360,
                640,
                "22_narrow_360x640_hig_native.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Overlays,
                640,
                360,
                "24_short_640x360_md3_overlays.png",
            ),
        ] {
            let mut h = make_gallery_harness(lang, tab);
            h.resize(w, height);
            save(&mut h, file, &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Adwaita, GalleryTab::Overlays);
            h.resize(320, 480);
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            save(&mut h, "23_narrow_320x480_adwaita_alert.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
            h.run_step(&PreviewStep::ClickLabel("默认".into())).unwrap();
            save(&mut h, "25_md3_accent_select_open.png", &mut issues);
        }

        assert!(
            issues.is_empty(),
            "visual alignment or viewport clipping detected:\n{}",
            issues.join("\n")
        );
    }
}
