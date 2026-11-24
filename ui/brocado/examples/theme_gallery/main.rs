//! Multi-theme showcase gallery entrypoint and automated headless audit.

#![warn(clippy::print_stdout, clippy::print_stderr)]

mod overlays;
mod pages;
mod shells;
mod state;

use brocado::dpi::LogicalSize;
use brocado::prelude::*;

use state::GalleryState;

impl AppState for GalleryState {
    fn keep_running(&self) -> bool {
        self.running
    }
}

fn app_view(state: &mut GalleryState) -> Box<AnyWidgetView<GalleryState>> {
    shells::md3_shell(state).boxed()
}

fn main() {
    let initial = GalleryState::default();
    let initial_props = initial.theme.default_properties();
    AppRunner::new(initial, |state: &mut GalleryState| {
        let wid = state.window_id;
        let theme = state.theme.clone();
        let title = format!("Brocado ({})", theme.language.label());
        let view = app_view(state);
        std::iter::once(themed_window(wid, title, &theme, view).with_config(|cfg| {
            cfg.on_close(|s: &mut GalleryState| s.running = false)
                .with_initial_inner_size(LogicalSize::new(1200.0, 800.0))
        }))
    })
    .with_default_properties(initial_props)
    .run()
    .expect("theme_gallery event loop exited cleanly");
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use brocado_preview::harness::{PreviewConfig, PreviewHarness, PreviewStep};
    use brocado_preview::inspect::{
        A11yLintConfig, IssueSeverity, LayoutIssue, LayoutNode, LintConfig, dump_layout_tree,
        format_tree_text, lint_accessibility_with, lint_layout_with,
    };
    use masonry::properties::Dimensions;
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::sized_box;

    use super::*;
    use state::GalleryTab;

    fn effective_test_theme(theme: &UiTheme, fallback: &Option<FontFamily<'static>>) -> UiTheme {
        match (&theme.font_family, fallback) {
            (None, Some(ff)) => theme.clone().with_font_family(ff.clone()),
            _ => theme.clone(),
        }
    }

    fn make_gallery_harness(lang: DesignLanguage, tab: GalleryTab) -> PreviewHarness<GalleryState> {
        make_gallery_harness_with(lang, Brightness::Light, tab, 1200, 800)
    }

    fn make_gallery_harness_with(
        lang: DesignLanguage,
        brightness: Brightness,
        tab: GalleryTab,
        width: u32,
        height: u32,
    ) -> PreviewHarness<GalleryState> {
        let mut initial = GalleryState::default();
        initial.theme = UiTheme::new(lang, brightness);
        initial.active_tab = tab;

        let test_font_fallback: std::rc::Rc<RefCell<Option<FontFamily<'static>>>> =
            std::rc::Rc::new(RefCell::new(None));
        let hook_fallback = test_font_fallback.clone();
        let view_fallback = test_font_fallback.clone();

        let initial_spec = initial.theme.to_spec();
        let initial_props = initial.theme.default_properties();
        let last_spec = RefCell::new(initial_spec.clone());

        let mut cfg = PreviewConfig {
            width,
            height,
            ..PreviewConfig::default()
        };
        cfg.background = initial_spec.palette.window_bg;

        let mut h = PreviewHarness::with_dynamic_properties(
            initial,
            initial_props,
            move |s| {
                let eff = effective_test_theme(&s.theme, &hook_fallback.borrow());
                let next_spec = eff.to_spec();
                let mut prev = last_spec.borrow_mut();
                if next_spec != *prev {
                    *prev = next_spec.clone();
                    Some(cached_default_properties(&next_spec))
                } else {
                    None
                }
            },
            move |s| {
                let eff = effective_test_theme(&s.theme, &view_fallback.borrow());
                let bg = eff.to_spec().palette.window_bg;
                ui_scope(
                    &eff,
                    sized_box(app_view(s))
                        .dims(Dimensions::STRETCH)
                        .background_color(bg),
                )
            },
            cfg,
        );
        let cjk_families = h.register_system_cjk_fallback();
        if !cjk_families.is_empty() {
            let mut chain = vec![FontFamilyName::Generic(GenericFamily::SystemUi)];
            chain.extend(
                cjk_families
                    .into_iter()
                    .map(|name| FontFamilyName::Named(std::borrow::Cow::Owned(name))),
            );
            *test_font_fallback.borrow_mut() =
                Some(FontFamily::List(std::borrow::Cow::Owned(chain)));
            h.rebuild();
        }
        h
    }

    fn find_first_label_rect(node: &LayoutNode, needle: &str) -> Option<(f64, f64, f64, f64)> {
        if node.type_name == "Label" && node.text.as_deref() == Some(needle) {
            return Some((node.rect.x, node.rect.y, node.rect.width, node.rect.height));
        }
        node.children
            .iter()
            .find_map(|child| find_first_label_rect(child, needle))
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
                    if matches!(
                        c.type_name.as_str(),
                        "Portal" | "ScrollPortal" | "ScrollViewWidget"
                    ) {
                        collect_portal_horizontal_overflows(c, out);
                    } else {
                        check_children(portal_x0, portal_x1, c, out);
                    }
                }
            }
        }
        if matches!(
            node.type_name.as_str(),
            "Portal" | "ScrollPortal" | "ScrollViewWidget"
        ) && node.rect.width > 1.0
        {
            let has_h_scrollbar = node.children.iter().any(|c| {
                matches!(c.type_name.as_str(), "ScrollBar" | "UiScrollBar")
                    && c.rect.width > c.rect.height
            });
            if has_h_scrollbar {
                for c in &node.children {
                    collect_portal_horizontal_overflows(c, out);
                }
            } else {
                check_children(node.rect.x, node.rect.x + node.rect.width, node, out);
            }
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
            (240, 480),
            (200, 480),
            (160, 480),
            (160, 280),
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
        h.run_step(&PreviewStep::ClickLabel("高对比".into()))
            .unwrap();
        assert!(h.state().theme.high_contrast);
        h.run_step(&PreviewStep::ClickLabel("高对比".into()))
            .unwrap();
        assert!(!h.state().theme.high_contrast);

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

        // Platform confirm dialog
        {
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            assert!(h.state().overlays.show_platform_alert);
            assert_visual_invariants(&mut h, "MD3 Platform Confirm");
            let before_confirm = h.state().components.click_count;
            h.run_step(&PreviewStep::ClickLabel("确定".into())).unwrap();
            assert!(!h.state().overlays.show_platform_alert);
            assert_eq!(h.state().components.click_count, before_confirm + 1);
        }

        // 5. Native M3 Expressive showcase page
        h.run_step(&PreviewStep::ClickLabel("平台".into())).unwrap();
        assert_eq!(h.state().active_tab, GalleryTab::Native);

        let md3_before = h.state().components.click_count;
        h.run_step(&PreviewStep::ClickLabel("发布".into())).unwrap();
        assert_eq!(h.state().components.click_count, md3_before + 1);

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

        // 1-9: MD3 Overlays (dialogs, sheets, pickers, toast, tooltips)
        for (lbl, file) in [
            ("原生警告", "01_md3_confirm_dialog.png"),
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
                DesignLanguage::Md3,
                GalleryTab::Layout,
                360,
                640,
                "20_narrow_360x640_md3_layout.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Lab,
                360,
                640,
                "21_narrow_360x640_md3_lab.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Native,
                360,
                640,
                "22_narrow_360x640_md3_native.png",
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
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
            h.run_step(&PreviewStep::ClickLabel("默认".into())).unwrap();
            save(&mut h, "25_md3_accent_select_open.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
            h.run_step(&PreviewStep::ClickLabel("交互气泡".into()))
                .unwrap();
            save(&mut h, "26_md3_popover_open.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
            h.settle_animations();
            let host_id = *h
                .find_widget_ids("ContextMenuHost")
                .first()
                .expect("ContextMenuHost should exist on Overlays page");
            if let Some((hx, hy, hw, hh)) = h.widget_rect(host_id) {
                let inner = h.inner_harness_mut();
                inner.mouse_move(masonry::kurbo::Point::new(hx + hw * 0.5, hy + hh * 0.5));
                inner.mouse_button_press(Some(masonry::core::PointerButton::Secondary));
                inner.mouse_button_release(Some(masonry::core::PointerButton::Secondary));
                h.pump_actions();
            }
            save(&mut h, "27_md3_context_menu_open.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness_with(
                DesignLanguage::Md3,
                Brightness::Dark,
                GalleryTab::Layout,
                1180,
                800,
            );
            save(&mut h, "28_md3_dark_layout_split.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
            h.resize(320, 480);
            h.run_step(&PreviewStep::ClickLabel("交互气泡".into()))
                .unwrap();
            save(&mut h, "29_narrow_320x480_md3_popover.png", &mut issues);
        }
        {
            let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
            h.settle_animations();
            if let Some(&ta_id) = h.find_widget_ids("TextArea").get(2) {
                h.inner_harness_mut().scroll_into_view(ta_id);
            }
            save(
                &mut h,
                "30_md3_components_scrolled_textarea.png",
                &mut issues,
            );
        }

        assert!(
            issues.is_empty(),
            "visual alignment or viewport clipping detected:\n{}",
            issues.join("\n")
        );
    }

    #[test]
    fn gallery_extreme_narrow_160px_overlays_and_interactions() {
        let (layout_cfg, a11y_cfg) = gallery_lint_configs();

        // 1. Platform alert dialogs across all 4 design languages at 160x480
        for lang in DesignLanguage::ALL {
            let mut h = make_gallery_harness(lang, GalleryTab::Overlays);
            if !h.register_system_cjk_fallback().is_empty() {
                let tree = dump_layout_tree(&mut h);
                let cn_lbl = find_first_label_rect(&tree, "原生警告")
                    .expect("expected Chinese label '原生警告' in Overlays tab");
                assert!(
                    cn_lbl.2 > 0.0 && cn_lbl.3 > 0.0,
                    "[{lang:?}] expected non-zero shaped CJK label dimensions, got {cn_lbl:?}"
                );
            }
            h.resize(160, 480);
            h.run_step(&PreviewStep::ClickLabel("原生警告".into()))
                .unwrap();
            assert!(h.state().overlays.show_platform_alert);
            let overflows = audit_snapshot(
                &mut h,
                &layout_cfg,
                &a11y_cfg,
                &format!("160x480/{lang:?}/platform_alert"),
            );
            assert!(
                overflows.is_empty(),
                "[{lang:?}] platform_alert overflow at 160px: {overflows:#?}"
            );
            assert_visual_invariants(&mut h, &format!("160x480/{lang:?}/platform_alert"));
            let before = h.state().components.click_count;
            h.run_step(&PreviewStep::ClickLabel("确定".into())).unwrap();
            assert!(!h.state().overlays.show_platform_alert);
            assert_eq!(h.state().components.click_count, before + 1);
        }

        // 2. All MD3 modal overlays (dialogs, sheets, date/time pickers) at 160x480
        let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
        h.resize(160, 480);
        let modals: &[(&str, fn(&GalleryState) -> bool)] = &[
            ("基础弹窗", |s| s.overlays.show_basic_dialog),
            ("危险确认", |s| s.overlays.show_destructive_dialog),
            ("日期弹窗", |s| s.overlays.show_date_dialog),
            ("时间弹窗", |s| s.overlays.show_time_dialog),
            ("底抽屉", |s| s.overlays.show_bottom_sheet),
            ("侧抽屉", |s| s.overlays.show_side_sheet),
        ];
        for &(btn, is_open) in modals {
            h.run_step(&PreviewStep::ClickLabel(btn.into())).unwrap();
            assert!(is_open(h.state()), "{btn} failed to open at 160x480");
            let overflows = audit_snapshot(
                &mut h,
                &layout_cfg,
                &a11y_cfg,
                &format!("160x480/Md3/{btn}"),
            );
            assert!(
                overflows.is_empty(),
                "160x480/Md3/{btn} overflow: {overflows:#?}"
            );
            assert_visual_invariants(&mut h, &format!("160x480/Md3/{btn}"));
            h.run_steps(&[
                PreviewStep::HoverType {
                    type_name: "EscapeCatcher".into(),
                    index: 0,
                },
                PreviewStep::PressEscape,
            ])
            .unwrap();
            assert!(!is_open(h.state()), "{btn} failed Escape close at 160x480");
        }

        // 3. Popover, Tooltips, and Toast at 160x480
        h.run_step(&PreviewStep::ClickLabel("交互气泡".into()))
            .unwrap();
        assert!(h.state().overlays.popover_open);
        let pop_ov = audit_snapshot(&mut h, &layout_cfg, &a11y_cfg, "160x480/Md3/popover");
        assert!(pop_ov.is_empty(), "160x480/Md3/popover: {pop_ov:#?}");
        assert_visual_invariants(&mut h, "160x480/Md3/popover");
        h.run_step(&PreviewStep::ClickLabel("关闭".into())).unwrap();
        assert!(!h.state().overlays.popover_open);

        for tip in ["悬停提示", "富提示"] {
            h.run_step(&PreviewStep::HoverLabel(tip.into())).unwrap();
            assert_eq!(count_layer_type(&mut h, "TooltipLayer"), 1);
            let tip_ov = audit_snapshot(&mut h, &layout_cfg, &a11y_cfg, &format!("160x480/{tip}"));
            assert!(tip_ov.is_empty(), "160x480/{tip}: {tip_ov:#?}");
            assert_visual_invariants(&mut h, &format!("160x480/{tip}"));
        }

        h.run_step(&PreviewStep::ClickLabel("轻提示".into()))
            .unwrap();
        assert!(h.state().overlays.toasts.current().is_some());
        let toast_ov = audit_snapshot(&mut h, &layout_cfg, &a11y_cfg, "160x480/Md3/toast");
        assert!(toast_ov.is_empty(), "160x480/Md3/toast: {toast_ov:#?}");
        assert_visual_invariants(&mut h, "160x480/Md3/toast");

        // 4. Select dropdown on Components tab at 160x480
        let mut h_comp = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
        h_comp.resize(160, 480);
        h_comp
            .run_step(&PreviewStep::ClickLabel("默认".into()))
            .unwrap();
        assert_eq!(count_layer_type(&mut h_comp, "PopupLayer"), 1);
        let sel_ov = audit_snapshot(&mut h_comp, &layout_cfg, &a11y_cfg, "160x480/Md3/select");
        assert!(sel_ov.is_empty(), "160x480/Md3/select: {sel_ov:#?}");
        assert_visual_invariants(&mut h_comp, "160x480/Md3/select");
        h_comp
            .run_step(&PreviewStep::ClickLabel("蓝".into()))
            .unwrap();
        assert_eq!(h_comp.state().theme.accent, UiAccent::PRESETS[1]);
        assert_eq!(count_layer_type(&mut h_comp, "PopupLayer"), 0);
    }

    fn find_first_with_type<'a>(node: &'a LayoutNode, type_name: &str) -> Option<&'a LayoutNode> {
        if node.type_name == type_name {
            return Some(node);
        }
        node.children
            .iter()
            .find_map(|c| find_first_with_type(c, type_name))
    }

    fn find_ancestor_button_and_label<'a>(
        node: &'a LayoutNode,
        label_text: &str,
    ) -> Option<(&'a LayoutNode, &'a LayoutNode)> {
        if node.type_name == "Button" {
            fn find_lbl<'b>(n: &'b LayoutNode, needle: &str) -> Option<&'b LayoutNode> {
                if n.type_name == "Label" && n.text.as_deref() == Some(needle) {
                    return Some(n);
                }
                n.children.iter().find_map(|c| find_lbl(c, needle))
            }
            if let Some(lbl) = find_lbl(node, label_text) {
                return Some((node, lbl));
            }
        }
        node.children
            .iter()
            .find_map(|c| find_ancestor_button_and_label(c, label_text))
    }

    fn find_sized_box_and_label<'a>(
        node: &'a LayoutNode,
        label_text: &str,
    ) -> Option<(&'a LayoutNode, &'a LayoutNode)> {
        if node.type_name == "SizedBox"
            && let Some(lbl) = node
                .children
                .iter()
                .find(|c| c.type_name == "Label" && c.text.as_deref() == Some(label_text))
        {
            return Some((node, lbl));
        }
        for child in &node.children {
            if node.type_name == "SizedBox" && node.rect.width < 120.0 && node.rect.height < 48.0 {
                fn find_lbl<'b>(n: &'b LayoutNode, needle: &str) -> Option<&'b LayoutNode> {
                    if n.type_name == "Label" && n.text.as_deref() == Some(needle) {
                        return Some(n);
                    }
                    n.children.iter().find_map(|c| find_lbl(c, needle))
                }
                if let Some(lbl) = find_lbl(child, label_text) {
                    return Some((node, lbl));
                }
            }
            if let Some(hit) = find_sized_box_and_label(child, label_text) {
                return Some(hit);
            }
        }
        None
    }

    #[test]
    fn gallery_normal_size_alignment_centering_and_events() {
        // 1. Components tab @ 1200x800: rich_prose link click, prefix/badge vertical centering, select trigger alignment
        let mut h = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Components);
        h.settle_animations();
        let tree = dump_layout_tree(&mut h);

        // Select trigger ("默认") should keep label left and chevron Svg right (not centered together)
        if let Some((btn_node, lbl_node)) = find_ancestor_button_and_label(&tree, "默认")
            && let Some(svg_node) = find_first_with_type(btn_node, "Svg")
        {
            assert!(
                lbl_node.rect.x < btn_node.rect.x + btn_node.rect.width * 0.45,
                "select label should stay near leading edge: btn={:?}, lbl={:?}",
                btn_node.rect,
                lbl_node.rect
            );
            assert!(
                svg_node.rect.x > btn_node.rect.x + btn_node.rect.width * 0.55,
                "select chevron should stay near trailing edge: btn={:?}, svg={:?}",
                btn_node.rect,
                svg_node.rect
            );
        } else {
            panic!("expected wide select trigger button and chevron");
        }

        // Badge "0" vertical centering inside its SizedBox pill
        let (badge_box, badge_lbl) =
            find_sized_box_and_label(&tree, "0").expect("large_badge '0' present");
        let badge_box_cy = badge_box.rect.y + badge_box.rect.height * 0.5;
        let badge_lbl_cy = badge_lbl.rect.y + badge_lbl.rect.height * 0.5;
        assert!(
            (badge_box_cy - badge_lbl_cy).abs() <= 2.5,
            "large_badge '0' should be vertically centered: box={:?}, lbl={:?}",
            badge_box.rect,
            badge_lbl.rect
        );

        // Prefix "https://" vs adjacent TextArea vertical center
        let (_, py, _, ph) =
            find_first_label_rect(&tree, "https://").expect("prefix 'https://' present");
        let areas: Vec<_> = {
            fn collect_areas(n: &LayoutNode, out: &mut Vec<brocado_preview::inspect::LayoutRect>) {
                if n.type_name == "TextArea" {
                    out.push(n.rect);
                }
                for c in &n.children {
                    collect_areas(c, out);
                }
            }
            let mut v = Vec::new();
            collect_areas(&tree, &mut v);
            v
        };
        let matching_area = areas
            .into_iter()
            .find(|r| (r.y - py).abs() < 24.0)
            .expect("TextArea on prefix row");
        let prefix_cy = py + ph * 0.5;
        let area_cy = matching_area.y + matching_area.height * 0.5;
        assert!(
            (prefix_cy - area_cy).abs() <= 0.5
                && (py - matching_area.y).abs() <= 0.5
                && (ph - matching_area.height).abs() <= 0.5,
            "TextField prefix should baseline/vertically align with TextArea: prefix=({py:.2}, h={ph:.2}, cy={prefix_cy:.2}), area=({:.2}, h={:.2}, cy={area_cy:.2})",
            matching_area.y,
            matching_area.height
        );

        // RichTextWidget link click event ("设计规范链接")
        let rich_id = *h
            .find_widget_ids("RichTextWidget")
            .first()
            .expect("RichTextWidget mounted");
        h.inner_harness_mut().scroll_into_view(rich_id);
        h.pump_actions();
        let (rx, ry, rw, rh) = h.widget_rect(rich_id).expect("RichTextWidget rect");
        let mut x = rx + 20.0;
        while x < rx + rw.min(720.0) {
            h.click_point(x, ry + rh * 0.5);
            if h.state().components.last_link_clicked.is_some() {
                break;
            }
            x += 24.0;
        }
        assert_eq!(
            h.state().components.last_link_clicked.as_deref(),
            Some("spec-docs"),
            "rich_prose link should trigger on_link_click at 1200x800"
        );

        // 2. Tooltips @ 1200x800: compact bounds, inside viewport, survive internal move, dismiss on leave
        let mut h_ov = make_gallery_harness(DesignLanguage::Md3, GalleryTab::Overlays);
        for (btn_lbl, expect_rich) in [("悬停提示", false), ("富提示", true)] {
            h_ov.run_step(&PreviewStep::HoverLabel(btn_lbl.into()))
                .unwrap();
            h_ov.settle_animations();
            let tip_tree = dump_layout_tree(&mut h_ov);
            let tip_node =
                find_first_with_type(&tip_tree, "TooltipLayer").expect("TooltipLayer spawned");
            assert!(
                tip_node.rect.width >= 32.0 && tip_node.rect.width <= 340.0,
                "[{btn_lbl}] expected readable compact tooltip width, got {:?}",
                tip_node.rect
            );
            assert!(
                tip_node.rect.height >= 16.0
                    && tip_node.rect.x + tip_node.rect.width <= 1200.0
                    && tip_node.rect.y + tip_node.rect.height <= 800.0,
                "[{btn_lbl}] tooltip must stay inside viewport: {:?}",
                tip_node.rect
            );
            if expect_rich {
                assert!(
                    tree_has_text(tip_node, "移出自动关闭"),
                    "rich tooltip should render body text"
                );
            }
            // Internal pointer move inside the trigger anchor must keep the tooltip alive
            let (bx, by, bw, bh) = find_first_label_rect(&tip_tree, btn_lbl).unwrap();
            h_ov.inner_harness_mut()
                .mouse_move(masonry::kurbo::Point::new(bx + bw * 0.25, by + bh * 0.5));
            h_ov.pump_actions();
            assert_eq!(
                count_layer_type(&mut h_ov, "TooltipLayer"),
                1,
                "[{btn_lbl}] moving pointer inside anchor must not dismiss tooltip"
            );
            // Moving outside anchor dismisses tooltip
            h_ov.inner_harness_mut()
                .mouse_move(masonry::kurbo::Point::new(5.0, 5.0));
            h_ov.pump_actions();
            assert_eq!(
                count_layer_type(&mut h_ov, "TooltipLayer"),
                0,
                "[{btn_lbl}] moving outside anchor should dismiss tooltip"
            );
        }

        // DatePicker weekday centering ("M")
        let ov_tree = dump_layout_tree(&mut h_ov);
        let (wd_box, wd_lbl) =
            find_sized_box_and_label(&ov_tree, "M").expect("weekday 'M' box present");
        let wd_box_cy = wd_box.rect.y + wd_box.rect.height * 0.5;
        let wd_lbl_cy = wd_lbl.rect.y + wd_lbl.rect.height * 0.5;
        let wd_box_cx = wd_box.rect.x + wd_box.rect.width * 0.5;
        let wd_lbl_cx = wd_lbl.rect.x + wd_lbl.rect.width * 0.5;
        assert!(
            (wd_box_cy - wd_lbl_cy).abs() <= 2.5 && (wd_box_cx - wd_lbl_cx).abs() <= 2.5,
            "DatePicker weekday should be centered in cell: box={:?}, lbl={:?}",
            wd_box.rect,
            wd_lbl.rect
        );

        // BasicDialog action buttons right-aligned & scrim click dismisses dialog
        h_ov.run_step(&PreviewStep::ClickLabel("基础弹窗".into()))
            .unwrap();
        assert!(h_ov.state().overlays.show_basic_dialog);
        let dlg_tree = dump_layout_tree(&mut h_ov);
        let dlg_card = find_first_with_type(&dlg_tree, "SlideTransition")
            .expect("BasicDialog SlideTransition")
            .children
            .first()
            .expect("BasicDialog card");
        let (cancel_btn, _) =
            find_ancestor_button_and_label(&dlg_tree, "取消").expect("'取消' button");
        let (ok_btn, _) = find_ancestor_button_and_label(&dlg_tree, "恢复").expect("'恢复' button");
        let card_cx = dlg_card.rect.x + dlg_card.rect.width * 0.5;
        let card_right = dlg_card.rect.x + dlg_card.rect.width;
        assert!(
            cancel_btn.rect.x + cancel_btn.rect.width * 0.5 > card_cx
                && ok_btn.rect.x > cancel_btn.rect.x
                && card_right - (ok_btn.rect.x + ok_btn.rect.width) <= 24.0,
            "BasicDialog actions should align to trailing side of dialog card {:?}: cancel={:?}, ok={:?}",
            dlg_card.rect,
            cancel_btn.rect,
            ok_btn.rect
        );
        h_ov.click_point(20.0, 780.0);
        assert!(
            !h_ov.state().overlays.show_basic_dialog,
            "clicking dialog scrim should dismiss BasicDialog"
        );

        // DatePickerDialog, TimePickerDialog compact width (no dead space on right) + BottomSheet sizing
        h_ov.run_step(&PreviewStep::ClickLabel("日期弹窗".into()))
            .unwrap();
        assert!(h_ov.state().overlays.show_date_dialog);
        h_ov.settle_animations();
        let date_dlg_tree = dump_layout_tree(&mut h_ov);
        let date_card = find_first_with_type(&date_dlg_tree, "SlideTransition")
            .expect("DatePickerDialog SlideTransition")
            .children
            .first()
            .expect("DatePickerDialog card");
        assert!(
            date_card.rect.width <= 280.0 && date_card.rect.width >= 240.0,
            "DatePickerDialog should hug calendar width (~272px) without right-side void: {:?}",
            date_card.rect
        );
        let (date_ok_btn, _) =
            find_ancestor_button_and_label(&date_dlg_tree, "OK").expect("DatePicker 'OK' button");
        let date_bottom_gap = (date_card.rect.y + date_card.rect.height)
            - (date_ok_btn.rect.y + date_ok_btn.rect.height);
        assert!(
            date_bottom_gap <= 24.0,
            "DatePickerDialog should hug OK button bottom without dead void (gap={date_bottom_gap:.2}): card={:?}, ok={:?}",
            date_card.rect,
            date_ok_btn.rect
        );
        h_ov.run_step(&PreviewStep::ClickLabel("Cancel".into()))
            .unwrap();
        assert!(!h_ov.state().overlays.show_date_dialog);

        h_ov.run_step(&PreviewStep::ClickLabel("时间弹窗".into()))
            .unwrap();
        assert!(h_ov.state().overlays.show_time_dialog);
        h_ov.settle_animations();
        let time_dlg_tree = dump_layout_tree(&mut h_ov);
        let time_card = find_first_with_type(&time_dlg_tree, "SlideTransition")
            .expect("TimePickerDialog SlideTransition")
            .children
            .first()
            .expect("TimePickerDialog card");
        assert!(
            time_card.rect.width <= 264.0 && time_card.rect.width >= 220.0,
            "TimePickerDialog should stay compact (~256px) without right-side void: {:?}",
            time_card.rect
        );
        let (time_ok_btn, _) =
            find_ancestor_button_and_label(&time_dlg_tree, "OK").expect("TimePicker 'OK' button");
        let time_bottom_gap = (time_card.rect.y + time_card.rect.height)
            - (time_ok_btn.rect.y + time_ok_btn.rect.height);
        assert!(
            time_bottom_gap <= 24.0 && time_card.rect.height <= 230.0,
            "TimePickerDialog should have no phantom bottom whitespace (gap={time_bottom_gap:.2}): card={:?}, ok={:?}",
            time_card.rect,
            time_ok_btn.rect
        );
        let time_wrap = find_first_with_type(time_card, "WrapWidget")
            .expect("TimePicker WrapWidget inside dialog");
        let wrap_center = time_wrap.rect.x + time_wrap.rect.width * 0.5;
        let card_center = time_card.rect.x + time_card.rect.width * 0.5;
        assert!(
            (wrap_center - card_center).abs() <= 2.0,
            "TimePicker content should center inside dialog: wrap_cx={wrap_center:.2}, card_cx={card_center:.2}"
        );
        h_ov.run_step(&PreviewStep::ClickLabel("Cancel".into()))
            .unwrap();
        assert!(!h_ov.state().overlays.show_time_dialog);

        h_ov.run_step(&PreviewStep::ClickLabel("底抽屉".into()))
            .unwrap();
        assert!(h_ov.state().overlays.show_bottom_sheet);
        h_ov.settle_animations();
        let sheet_tree = dump_layout_tree(&mut h_ov);
        let sheet_card = find_first_with_type(&sheet_tree, "SlideTransition")
            .expect("BottomSheet SlideTransition")
            .children
            .first()
            .expect("BottomSheet card");
        assert!(
            sheet_card.rect.width <= 640.5
                && (sheet_card.rect.x - 280.0).abs() <= 2.0
                && sheet_card.rect.height < 200.0
                && sheet_card.rect.height >= 80.0,
            "BottomSheet at 1200x800 should respect 640px max width, center horizontally, and fit content height: {:?}",
            sheet_card.rect
        );
        h_ov.run_step(&PreviewStep::ClickLabel("关闭".into()))
            .unwrap();
        assert!(!h_ov.state().overlays.show_bottom_sheet);
    }

    #[test]
    #[ignore = "Manual visual audit helper dumping 160x480 PNGs to THEME_GALLERY_AUDIT_DIR"]
    fn audit_dump_extreme_narrow_160_pngs() {
        let out_dir_buf = std::env::var_os("THEME_GALLERY_AUDIT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("beamlet_160_audit"));
        let out_dir = out_dir_buf.as_path();
        std::fs::create_dir_all(out_dir).expect("create 160 audit_pngs dir");

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
            h.render_png(out_dir.join(name))
                .expect("write 160 audit png");
        };

        for (lang, tab, file) in [
            (
                DesignLanguage::Md3,
                GalleryTab::Components,
                "01_160_md3_components.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Layout,
                "02_160_md3_layout.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Overlays,
                "03_160_md3_overlays.png",
            ),
            (
                DesignLanguage::Md3,
                GalleryTab::Native,
                "04_160_md3_native.png",
            ),
            (DesignLanguage::Md3, GalleryTab::Lab, "15_160_md3_lab.png"),
        ] {
            let mut h = make_gallery_harness(lang, tab);
            h.resize(160, 480);
            save(&mut h, file, &mut issues);
        }

        for (lang, btn, file) in [
            (
                DesignLanguage::Md3,
                "基础弹窗",
                "08_160_md3_basic_dialog.png",
            ),
            (
                DesignLanguage::Md3,
                "日期弹窗",
                "11_160_md3_date_picker.png",
            ),
            (
                DesignLanguage::Md3,
                "时间弹窗",
                "12_160_md3_time_picker.png",
            ),
            (DesignLanguage::Md3, "侧抽屉", "13_160_md3_side_sheet.png"),
            (DesignLanguage::Md3, "轻提示", "14_160_md3_toast.png"),
        ] {
            let mut h = make_gallery_harness(lang, GalleryTab::Overlays);
            h.resize(160, 480);
            h.run_step(&PreviewStep::ClickLabel(btn.into())).unwrap();
            save(&mut h, file, &mut issues);
        }

        assert!(
            issues.is_empty(),
            "160px visual alignment or viewport clipping detected:\n{}",
            issues.join("\n")
        );
    }
}
