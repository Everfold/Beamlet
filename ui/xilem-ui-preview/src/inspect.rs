//! Widget geometry/accessibility tree dumper and automated layout + WCAG contrast linter.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::Serialize;
use xilem::Color;
use xilem::masonry::app::RenderRoot;
use xilem::masonry::core::{Widget, WidgetId, WidgetRef};
use xilem_ui::prelude::DesignSpec;
use xilem_ui_core::foundation::AnchoredPopupHost;

use crate::harness::PreviewHarness;

/// Axis-aligned bounding rectangle in window logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LayoutRect {
    /// Left coordinate relative to window origin.
    pub x: f64,
    /// Top coordinate relative to window origin.
    pub y: f64,
    /// Border-box width.
    pub width: f64,
    /// Border-box height.
    pub height: f64,
}

impl LayoutRect {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    fn is_non_empty(self) -> bool {
        self.width > 0.5 && self.height > 0.5
    }
}

/// Serialized node summarizing a live Masonry widget's geometry, text, and accessibility state.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LayoutNode {
    /// Raw Masonry `WidgetId`.
    pub id: u64,
    /// Concrete Masonry widget `short_type_name()`.
    pub type_name: String,
    /// Optional debug/label text extracted from `Widget::get_debug_text()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Optional AccessKit accessibility role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Whether this widget holds focus.
    pub focused: bool,
    /// Whether this widget is disabled.
    pub disabled: bool,
    /// Computed window-space bounding rectangle.
    pub rect: LayoutRect,
    /// Child layout nodes in paint order.
    pub children: Vec<LayoutNode>,
}

/// Walks the laid-out widget tree (calling `redraw()` first) and returns the root [`LayoutNode`].
pub fn dump_layout_tree<State: 'static>(harness: &mut PreviewHarness<State>) -> LayoutNode {
    let inner = harness.inner_harness_mut();
    let _ = inner.redraw();
    let focused_id = inner.focused_widget_id();
    let mut ids = Vec::new();
    let mut tree = {
        let rr: &RenderRoot = inner.render_root();
        let root = rr.get_layer_root(0);
        snapshot_node(&root, rr, focused_id, &mut ids)
    };
    let roles: HashMap<u64, String> = ids
        .into_iter()
        .filter_map(|(raw, wid)| {
            inner
                .access_node(wid)
                .map(|a| (raw, format!("{:?}", a.role())))
        })
        .collect();
    attach_roles(&mut tree, &roles);
    tree
}

fn snapshot_node(
    node: &WidgetRef<'_, dyn Widget>,
    render_root: &RenderRoot,
    focused_id: Option<WidgetId>,
    ids: &mut Vec<(u64, WidgetId)>,
) -> LayoutNode {
    let wid = node.id();
    let raw = wid.to_raw();
    ids.push((raw, wid));

    let ctx = node.ctx();
    let box_rect = ctx.border_box();
    let origin = ctx.window_transform() * box_rect.origin();
    let text = node.inner().get_debug_text().filter(|s| !s.is_empty());

    let mut children: Vec<LayoutNode> = node
        .children()
        .iter()
        .map(|c| snapshot_node(c, render_root, focused_id, ids))
        .collect();
    if let Some(host) = node.downcast::<AnchoredPopupHost>()
        && let Some(layer_id) = host.inner().layer_id()
        && let Some(layer_node) = render_root.get_widget(layer_id)
    {
        children.push(snapshot_node(&layer_node, render_root, focused_id, ids));
    }

    LayoutNode {
        id: raw,
        type_name: node.inner().short_type_name().to_string(),
        text,
        role: None,
        focused: focused_id == Some(wid),
        disabled: ctx.is_disabled(),
        rect: LayoutRect {
            x: origin.x,
            y: origin.y,
            width: box_rect.width(),
            height: box_rect.height(),
        },
        children,
    }
}

fn attach_roles(node: &mut LayoutNode, roles: &HashMap<u64, String>) {
    node.role = roles.get(&node.id).cloned();
    for child in &mut node.children {
        attach_roles(child, roles);
    }
}

/// Formats a [`LayoutNode`] subtree as concise human- and agent-readable text.
#[must_use]
pub fn format_tree_text(node: &LayoutNode) -> String {
    let mut out = String::new();
    write_node(&mut out, node, 0);
    out
}

fn write_node(out: &mut String, node: &LayoutNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let _ = write!(
        out,
        "{indent}#{} {} [{:.1}, {:.1} {:.1}x{:.1}]",
        node.id, node.type_name, node.rect.x, node.rect.y, node.rect.width, node.rect.height
    );
    if let Some(role) = &node.role {
        let _ = write!(out, " role={role}");
    }
    if node.focused {
        out.push_str(" *FOCUSED*");
    }
    if node.disabled {
        out.push_str(" (disabled)");
    }
    if let Some(txt) = &node.text {
        let escaped = txt.replace('\n', "\\n");
        let _ = write!(out, " \"{escaped}\"");
    }
    out.push('\n');
    for child in &node.children {
        write_node(out, child, depth + 1);
    }
}

/// Severity tier for automated layout diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum IssueSeverity {
    /// Potential visual defect worth reviewing.
    Warning,
    /// Definite layout bug (zero-size visible control, overflow, or sibling overlap).
    Error,
}

/// Automated layout diagnostic produced by [`lint_layout`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LayoutIssue {
    /// Severity tier.
    pub severity: IssueSeverity,
    /// Short diagnostic category (`"zero-size"`, `"overflow"`, `"sibling-overlap"`, `"out-of-viewport"`).
    pub kind: &'static str,
    /// Offending widget ID.
    pub widget_id: u64,
    /// Offending widget type name.
    pub widget_type: String,
    /// Human-readable description of the layout anomaly.
    pub message: String,
}

fn is_visible_leaf_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "Button"
            | "TextInput"
            | "Checkbox"
            | "Switch"
            | "Slider"
            | "RadioButton"
            | "Svg"
            | "RangeSliderWidget"
            | "LinearProgressWidget"
            | "DeterminateCircularWidget"
    )
}

fn allows_child_overflow(type_name: &str) -> bool {
    matches!(
        type_name,
        "Portal"
            | "ZStack"
            | "EscapeCatcher"
            | "Passthrough"
            | "FocusFrame"
            | "ScrollBar"
            | "Badged"
            | "AnchoredPopupHost"
    )
}

/// Scans `root` against `viewport` `(width, height)` for zero-sized controls, parent overflow,
/// sibling overlaps inside `Flex`, and offscreen non-scrollable controls.
#[must_use]
pub fn lint_layout(root: &LayoutNode, viewport: (f64, f64)) -> Vec<LayoutIssue> {
    let mut issues = Vec::new();
    lint_walk(root, viewport, false, &mut issues);
    issues
}

fn lint_walk(
    node: &LayoutNode,
    viewport: (f64, f64),
    inside_portal: bool,
    issues: &mut Vec<LayoutIssue>,
) {
    let in_portal = inside_portal || node.type_name == "Portal";

    let is_nonempty_label = node.type_name == "Label" && node.text.is_some();
    if (is_visible_leaf_type(&node.type_name) || is_nonempty_label)
        && (node.rect.width <= 0.5 || node.rect.height <= 0.5)
    {
        issues.push(LayoutIssue {
            severity: IssueSeverity::Error,
            kind: "zero-size",
            widget_id: node.id,
            widget_type: node.type_name.clone(),
            message: format!(
                "{} #{} rendered at {:.1}x{:.1}",
                node.type_name, node.id, node.rect.width, node.rect.height
            ),
        });
    }

    if !in_portal
        && (is_visible_leaf_type(&node.type_name) || is_nonempty_label)
        && node.rect.is_non_empty()
        && (node.rect.right() < 0.0
            || node.rect.bottom() < 0.0
            || node.rect.x > viewport.0
            || node.rect.y > viewport.1)
    {
        issues.push(LayoutIssue {
            severity: IssueSeverity::Warning,
            kind: "out-of-viewport",
            widget_id: node.id,
            widget_type: node.type_name.clone(),
            message: format!(
                "{} #{} ({:.1},{:.1} {:.1}x{:.1}) lies completely outside {:.0}x{:.0} viewport",
                node.type_name,
                node.id,
                node.rect.x,
                node.rect.y,
                node.rect.width,
                node.rect.height,
                viewport.0,
                viewport.1
            ),
        });
    }

    if node.rect.is_non_empty() && !allows_child_overflow(&node.type_name) {
        const TOL: f64 = 2.5;
        for child in &node.children {
            if !child.rect.is_non_empty() || child.type_name == "ScrollBar" {
                continue;
            }
            let overflows = child.rect.x < node.rect.x - TOL
                || child.rect.y < node.rect.y - TOL
                || child.rect.right() > node.rect.right() + TOL
                || child.rect.bottom() > node.rect.bottom() + TOL;
            if overflows {
                issues.push(LayoutIssue {
                    severity: IssueSeverity::Warning,
                    kind: "overflow",
                    widget_id: child.id,
                    widget_type: child.type_name.clone(),
                    message: format!(
                        "{} #{} ({:.1}..{:.1}, {:.1}..{:.1}) overflows {} #{} ({:.1}..{:.1}, {:.1}..{:.1})",
                        child.type_name,
                        child.id,
                        child.rect.x,
                        child.rect.right(),
                        child.rect.y,
                        child.rect.bottom(),
                        node.type_name,
                        node.id,
                        node.rect.x,
                        node.rect.right(),
                        node.rect.y,
                        node.rect.bottom()
                    ),
                });
            }
        }
    }

    if node.type_name == "Flex" && node.children.len() >= 2 {
        const OVERLAP_TOL: f64 = 2.5;
        for i in 0..node.children.len() {
            let a = &node.children[i];
            if !a.rect.is_non_empty() {
                continue;
            }
            for b in &node.children[(i + 1)..] {
                if !b.rect.is_non_empty() {
                    continue;
                }
                let ox = a.rect.right().min(b.rect.right()) - a.rect.x.max(b.rect.x);
                let oy = a.rect.bottom().min(b.rect.bottom()) - a.rect.y.max(b.rect.y);
                if ox > OVERLAP_TOL && oy > OVERLAP_TOL {
                    issues.push(LayoutIssue {
                        severity: IssueSeverity::Error,
                        kind: "sibling-overlap",
                        widget_id: a.id,
                        widget_type: a.type_name.clone(),
                        message: format!(
                            "Flex #{} siblings {} #{} and {} #{} overlap by {:.1}x{:.1}px",
                            node.id, a.type_name, a.id, b.type_name, b.id, ox, oy
                        ),
                    });
                }
            }
        }
    }

    for child in &node.children {
        lint_walk(child, viewport, in_portal, issues);
    }
}

/// WCAG 2.1 contrast failure reported by [`lint_spec_contrast`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContrastIssue {
    /// Semantic color pair name.
    pub pair: &'static str,
    /// Computed WCAG 2.1 contrast ratio.
    pub ratio: f64,
    /// Required minimum contrast ratio.
    pub minimum: f64,
}

fn srgb_luminance(color: Color) -> f64 {
    let rgba = color.to_rgba8();
    let lin = |c: u8| {
        let s = f64::from(c) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgba.r) + 0.7152 * lin(rgba.g) + 0.0722 * lin(rgba.b)
}

fn contrast_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = srgb_luminance(fg);
    let l2 = srgb_luminance(bg);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

/// Audits semantic foreground/background pairs inside `spec` against WCAG 2.1 thresholds.
#[must_use]
pub fn lint_spec_contrast(spec: &DesignSpec) -> Vec<ContrastIssue> {
    let p = &spec.palette;
    let checks = [
        ("text_primary/surface", p.text_primary, p.surface, 4.5),
        ("text_primary/window_bg", p.text_primary, p.window_bg, 4.5),
        ("text_secondary/surface", p.text_secondary, p.surface, 3.0),
        ("on_accent/accent", p.on_accent, p.accent, 3.0),
        (
            "on_accent_container/accent_container",
            p.on_accent_container,
            p.accent_container,
            4.5,
        ),
        ("on_error/error", p.on_error, p.error, 3.0),
        ("on_success/success", p.on_success, p.success, 3.0),
        ("on_warning/warning", p.on_warning, p.warning, 3.0),
    ];

    checks
        .into_iter()
        .filter_map(|(pair, fg, bg, minimum)| {
            let ratio = contrast_ratio(fg, bg);
            (ratio < minimum).then_some(ContrastIssue {
                pair,
                ratio,
                minimum,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use xilem::WidgetView;
    use xilem::masonry::layout::AsUnit;
    use xilem::style::Style as _;
    use xilem::view::{flex_col, sized_box};
    use xilem_ui::prelude::*;

    use super::*;
    use crate::harness::{PreviewConfig, PreviewHarness};

    fn dummy_node(
        id: u64,
        type_name: &str,
        rect: LayoutRect,
        children: Vec<LayoutNode>,
    ) -> LayoutNode {
        LayoutNode {
            id,
            type_name: type_name.into(),
            text: if type_name == "Label" {
                Some("Text".into())
            } else {
                None
            },
            role: None,
            focused: false,
            disabled: false,
            rect,
            children,
        }
    }

    #[test]
    fn lint_layout_flags_zero_size_overflow_and_overlap() {
        let zero_btn = dummy_node(
            2,
            "Button",
            LayoutRect {
                x: 10.0,
                y: 10.0,
                width: 0.0,
                height: 32.0,
            },
            vec![],
        );
        let overflow_lbl = dummy_node(
            3,
            "Label",
            LayoutRect {
                x: 10.0,
                y: 50.0,
                width: 240.0,
                height: 20.0,
            },
            vec![],
        );
        let overlap_b = dummy_node(
            4,
            "Label",
            LayoutRect {
                x: 20.0,
                y: 55.0,
                width: 80.0,
                height: 20.0,
            },
            vec![],
        );
        let root = dummy_node(
            1,
            "Flex",
            LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 100.0,
            },
            vec![zero_btn, overflow_lbl, overlap_b],
        );

        let issues = lint_layout(&root, (800.0, 600.0));
        assert!(issues.iter().any(|i| i.kind == "zero-size"));
        assert!(issues.iter().any(|i| i.kind == "overflow"));
        assert!(issues.iter().any(|i| i.kind == "sibling-overlap"));
    }

    #[test]
    fn dump_and_lint_clean_tree_reports_zero_issues() {
        struct S {
            theme: UiTheme,
        }
        let mut h = PreviewHarness::new(
            S {
                theme: UiTheme::new(DesignLanguage::Fluent, Brightness::Light),
            },
            |s| s.theme.clone(),
            |_| {
                sized_box(
                    flex_col((
                        title_medium("Clean Heading"),
                        primary_button("Action", |_: &mut S| {}),
                    ))
                    .gap(8.px()),
                )
                .padding(16.px())
                .boxed()
            },
            PreviewConfig {
                width: 480,
                height: 320,
                ..Default::default()
            },
        );
        let tree = dump_layout_tree(&mut h);
        let text = format_tree_text(&tree);
        assert!(text.contains("Clean Heading"));
        assert!(text.contains("Button"));

        let layout_issues = lint_layout(&tree, h.viewport());
        assert!(
            layout_issues.is_empty(),
            "unexpected layout issues: {layout_issues:?}"
        );

        let contrast_issues = lint_spec_contrast(h.current_spec());
        assert!(
            contrast_issues.is_empty(),
            "unexpected contrast issues: {contrast_issues:?}"
        );
    }
}
