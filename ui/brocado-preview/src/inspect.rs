//! Widget geometry/accessibility tree dumper and automated layout + WCAG contrast linter.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use masonry::accesskit::Role;
use masonry::app::RenderRoot;
use masonry::core::{Widget, WidgetId, WidgetRef};
use masonry::peniko::Color;
use serde::Serialize;

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
    /// Child layout nodes in paint order (plus any active overlay layers attached at the root).
    pub children: Vec<LayoutNode>,
}

/// Walks the laid-out widget tree and any active overlay layers (calling `redraw()` first)
/// and returns the root [`LayoutNode`].
pub fn dump_layout_tree<State: 'static>(harness: &mut PreviewHarness<State>) -> LayoutNode {
    let inner = harness.inner_harness_mut();
    let (visual_layers, _) = inner.redraw();
    let focused_id = inner.focused_widget_id();
    let mut visited = HashSet::new();
    let mut ids = Vec::new();
    let mut tree = {
        let rr = inner.render_root();
        let root = rr.get_layer_root(0);
        let mut root_node = snapshot_node(&root, focused_id, &mut visited, &mut ids);
        for layer in &visual_layers.layers {
            if !visited.contains(&layer.widget_id)
                && let Some(layer_root) = rr.get_widget(layer.widget_id)
            {
                root_node.children.push(snapshot_node(
                    &layer_root,
                    focused_id,
                    &mut visited,
                    &mut ids,
                ));
            }
        }
        root_node
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
    focused_id: Option<WidgetId>,
    visited: &mut HashSet<WidgetId>,
    ids: &mut Vec<(u64, WidgetId)>,
) -> LayoutNode {
    let wid = node.id();
    visited.insert(wid);
    let raw = wid.to_raw();
    ids.push((raw, wid));

    let ctx = node.ctx();
    let box_rect = ctx.border_box();
    let origin = ctx.window_transform() * box_rect.origin();
    let text = node.inner().get_debug_text().filter(|s| !s.is_empty());

    let mut children = Vec::new();
    for c in node.children() {
        if !c.ctx().is_stashed() && !visited.contains(&c.id()) {
            children.push(snapshot_node(&c, focused_id, visited, ids));
        }
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

/// Automated layout diagnostic produced by [`lint_layout`] or [`lint_layout_with`].
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

/// Customizes widget type allowlists evaluated by [`lint_layout_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintConfig {
    /// Widget `short_type_name()` values treated as interactive/visible leaf controls that must
    /// not collapse to zero size or sit completely outside the viewport.
    pub visible_leaf_types: Vec<String>,
    /// Container `short_type_name()` values allowed to clip, scroll, or intentionally overflow their
    /// children beyond their own border-box.
    pub overflow_containers: Vec<String>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            visible_leaf_types: [
                "Button",
                "TextInput",
                "Checkbox",
                "Switch",
                "Slider",
                "RadioButton",
                "Svg",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            overflow_containers: [
                "Portal",
                "ScrollViewWidget",
                "VirtualScroll",
                "ZStack",
                "Passthrough",
                "ScrollBar",
                "UiScrollBar",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}

impl LintConfig {
    /// Appends additional visible leaf widget type names to inspect for zero-size and offscreen bounds.
    #[must_use]
    pub fn with_visible_leaf_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.visible_leaf_types
            .extend(types.into_iter().map(Into::into));
        self
    }

    /// Appends additional container widget type names allowed to overflow their border-box.
    #[must_use]
    pub fn with_overflow_containers<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.overflow_containers
            .extend(types.into_iter().map(Into::into));
        self
    }

    fn is_visible_leaf_type(&self, type_name: &str) -> bool {
        self.visible_leaf_types.iter().any(|t| t == type_name)
    }

    fn allows_child_overflow(&self, type_name: &str) -> bool {
        self.overflow_containers.iter().any(|t| t == type_name)
    }
}

/// Scans `root` against `viewport` `(width, height)` using [`LintConfig::default()`].
#[must_use]
pub fn lint_layout(root: &LayoutNode, viewport: (f64, f64)) -> Vec<LayoutIssue> {
    lint_layout_with(root, viewport, &LintConfig::default())
}

/// Scans `root` against `viewport` `(width, height)` using custom widget classification `config`
/// for zero-sized controls, parent overflow, sibling overlaps inside `Flex`, and offscreen controls.
#[must_use]
pub fn lint_layout_with(
    root: &LayoutNode,
    viewport: (f64, f64),
    config: &LintConfig,
) -> Vec<LayoutIssue> {
    let mut issues = Vec::new();
    lint_walk(root, viewport, false, config, &mut issues);
    issues
}

fn lint_walk(
    node: &LayoutNode,
    viewport: (f64, f64),
    inside_portal: bool,
    config: &LintConfig,
    issues: &mut Vec<LayoutIssue>,
) {
    let in_portal = inside_portal
        || node.type_name == "Portal"
        || node.type_name == "ScrollViewWidget"
        || node.type_name == "VirtualScroll";

    let is_nonempty_label = node.type_name == "Label" && node.text.is_some();
    if (config.is_visible_leaf_type(&node.type_name) || is_nonempty_label)
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
        && (config.is_visible_leaf_type(&node.type_name) || is_nonempty_label)
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

    if node.rect.is_non_empty() && !config.allows_child_overflow(&node.type_name) {
        const TOL: f64 = 2.5;
        for child in &node.children {
            if !child.rect.is_non_empty()
                || child.type_name == "ScrollBar"
                || child.type_name == "UiScrollBar"
            {
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
        lint_walk(child, viewport, in_portal, config, issues);
    }
}

/// Specification for one WCAG 2.1 foreground/background contrast check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastCheck {
    /// Semantic color pair name.
    pub pair: &'static str,
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Required minimum WCAG 2.1 contrast ratio.
    pub minimum: f64,
}

impl ContrastCheck {
    /// Creates a new contrast requirement between `fg` and `bg`.
    #[must_use]
    pub const fn new(pair: &'static str, fg: Color, bg: Color, minimum: f64) -> Self {
        Self {
            pair,
            fg,
            bg,
            minimum,
        }
    }
}

/// WCAG 2.1 contrast failure reported by [`lint_contrast`].
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

/// Computes the WCAG 2.1 relative-luminance contrast ratio between `fg` and `bg` (`1.0..=21.0`).
#[must_use]
pub fn contrast_ratio(fg: Color, bg: Color) -> f64 {
    let l1 = srgb_luminance(fg);
    let l2 = srgb_luminance(bg);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

/// Audits arbitrary foreground/background [`ContrastCheck`] pairs against WCAG 2.1 thresholds.
#[must_use]
pub fn lint_contrast(checks: &[ContrastCheck]) -> Vec<ContrastIssue> {
    checks
        .iter()
        .filter_map(|check| {
            let ratio = contrast_ratio(check.fg, check.bg);
            (ratio < check.minimum).then_some(ContrastIssue {
                pair: check.pair,
                ratio,
                minimum: check.minimum,
            })
        })
        .collect()
}

/// Automated AccessKit semantics diagnostic produced by [`lint_accessibility`] or
/// [`lint_accessibility_with`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct A11yIssue {
    /// Severity tier.
    pub severity: IssueSeverity,
    /// Diagnostic category (`"missing-access-node"`, `"invalid-role"`, `"numeric-out-of-range"`,
    /// `"non-modal-dialog"`).
    pub kind: &'static str,
    /// Offending widget ID.
    pub widget_id: u64,
    /// Offending widget type name.
    pub widget_type: String,
    /// Human-readable description of the accessibility anomaly.
    pub message: String,
}

/// Customizes widget type allowlists inspected by [`lint_accessibility_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct A11yLintConfig {
    /// Widget `short_type_name()` values treated as interactive/semantic controls that must
    /// publish an AccessKit node with a concrete semantic `Role`.
    pub interactive_types: Vec<String>,
    /// Container `short_type_name()` values that represent modal dialog wrappers when exposing
    /// `Role::Dialog`.
    pub dialog_types: Vec<String>,
}

impl Default for A11yLintConfig {
    fn default() -> Self {
        Self {
            interactive_types: [
                "Button",
                "TextArea",
                "Checkbox",
                "Switch",
                "Slider",
                "RadioButton",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            dialog_types: Vec::new(),
        }
    }
}

impl A11yLintConfig {
    /// Appends additional interactive widget type names to verify for valid AccessKit roles.
    #[must_use]
    pub fn with_interactive_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.interactive_types
            .extend(types.into_iter().map(Into::into));
        self
    }

    /// Appends widget type names checked for `modal = true` when exposing `Role::Dialog`.
    #[must_use]
    pub fn with_dialog_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dialog_types.extend(types.into_iter().map(Into::into));
        self
    }

    fn is_interactive(&self, type_name: &str) -> bool {
        self.interactive_types.iter().any(|t| t == type_name)
    }

    fn is_dialog_container(&self, type_name: &str) -> bool {
        self.dialog_types.iter().any(|t| t == type_name)
    }
}

/// Audits the live AccessKit tree of `harness` using [`A11yLintConfig::default()`].
pub fn lint_accessibility<State: 'static>(harness: &mut PreviewHarness<State>) -> Vec<A11yIssue> {
    lint_accessibility_with(harness, &A11yLintConfig::default())
}

/// Audits the live AccessKit tree of `harness` using `config` for semantic roles, numeric bounds,
/// and modal dialog invariants.
pub fn lint_accessibility_with<State: 'static>(
    harness: &mut PreviewHarness<State>,
    config: &A11yLintConfig,
) -> Vec<A11yIssue> {
    let inner = harness.inner_harness_mut();
    let (visual_layers, _) = inner.redraw();
    let mut visited = HashSet::new();
    let mut nodes: Vec<(WidgetId, String)> = Vec::new();
    {
        let rr: &RenderRoot = inner.render_root();
        let root = rr.get_layer_root(0);
        collect_visible_widgets(&root, &mut visited, &mut nodes);
        for layer in &visual_layers.layers {
            if !visited.contains(&layer.widget_id)
                && let Some(layer_root) = rr.get_widget(layer.widget_id)
            {
                collect_visible_widgets(&layer_root, &mut visited, &mut nodes);
            }
        }
    }

    let mut issues = Vec::new();
    for (wid, type_name) in nodes {
        let raw_id = wid.to_raw();
        let is_interactive = config.is_interactive(&type_name);
        let access_node = inner.access_node(wid);

        if is_interactive {
            let Some(ref a11y) = access_node else {
                issues.push(A11yIssue {
                    severity: IssueSeverity::Error,
                    kind: "missing-access-node",
                    widget_id: raw_id,
                    widget_type: type_name,
                    message: format!("interactive widget #{raw_id} lacks an AccessKit node"),
                });
                continue;
            };
            let role = a11y.role();
            if matches!(role, Role::Unknown | Role::GenericContainer) {
                issues.push(A11yIssue {
                    severity: IssueSeverity::Error,
                    kind: "invalid-role",
                    widget_id: raw_id,
                    widget_type: type_name.clone(),
                    message: format!(
                        "{type_name} #{raw_id} published non-semantic AccessKit role {role:?}"
                    ),
                });
            }
            if matches!(role, Role::Slider | Role::ProgressIndicator)
                && let (Some(val), Some(min), Some(max)) = (
                    a11y.numeric_value(),
                    a11y.data().min_numeric_value(),
                    a11y.data().max_numeric_value(),
                )
                && (val < min - 1e-6 || val > max + 1e-6)
            {
                issues.push(A11yIssue {
                    severity: IssueSeverity::Error,
                    kind: "numeric-out-of-range",
                    widget_id: raw_id,
                    widget_type: type_name.clone(),
                    message: format!(
                        "{type_name} #{raw_id} numeric_value={val} outside [{min}, {max}]"
                    ),
                });
            }
        }

        if let Some(a11y) = access_node
            && (a11y.role() == Role::Dialog
                || (config.is_dialog_container(&type_name) && a11y.role() == Role::Dialog))
            && !a11y.is_modal()
        {
            issues.push(A11yIssue {
                severity: IssueSeverity::Error,
                kind: "non-modal-dialog",
                widget_id: raw_id,
                widget_type: type_name,
                message: format!("dialog #{raw_id} exposed Role::Dialog without is_modal=true"),
            });
        }
    }
    issues
}

fn collect_visible_widgets(
    node: &WidgetRef<'_, dyn Widget>,
    visited: &mut HashSet<WidgetId>,
    out: &mut Vec<(WidgetId, String)>,
) {
    let wid = node.id();
    if !visited.insert(wid) || node.ctx().is_stashed() {
        return;
    }
    out.push((wid, node.inner().short_type_name().to_string()));
    for child in node.children() {
        collect_visible_widgets(&child, visited, out);
    }
}

#[cfg(test)]
mod tests {
    use masonry::layout::AsUnit;
    use masonry::theme::{BACKGROUND_COLOR, TEXT_COLOR};
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{flex_col, label, sized_box, text_button};

    use super::*;
    use crate::harness::PreviewConfig;

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
    fn custom_lint_config_and_contrast_checks() {
        let zero_custom = dummy_node(
            2,
            "CustomControl",
            LayoutRect {
                x: 10.0,
                y: 10.0,
                width: 0.0,
                height: 24.0,
            },
            vec![],
        );
        let wide_child = dummy_node(
            4,
            "Label",
            LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 20.0,
            },
            vec![],
        );
        let custom_overlay = dummy_node(
            3,
            "CustomOverlay",
            LayoutRect {
                x: 10.0,
                y: 40.0,
                width: 50.0,
                height: 20.0,
            },
            vec![wide_child],
        );
        let root = dummy_node(
            1,
            "Flex",
            LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 200.0,
            },
            vec![zero_custom, custom_overlay],
        );

        let config = LintConfig::default()
            .with_visible_leaf_types(["CustomControl"])
            .with_overflow_containers(["CustomOverlay"]);
        let issues = lint_layout_with(&root, (300.0, 200.0), &config);
        assert!(issues.iter().any(|i| i.kind == "zero-size"));
        assert!(!issues.iter().any(|i| i.kind == "overflow"));

        let bw = contrast_ratio(Color::BLACK, Color::WHITE);
        assert!((bw - 21.0).abs() < 0.05);

        let failed = lint_contrast(&[
            ContrastCheck::new("black/white", Color::BLACK, Color::WHITE, 4.5),
            ContrastCheck::new(
                "gray/gray",
                Color::from_rgb8(0x88, 0x88, 0x88),
                Color::from_rgb8(0x99, 0x99, 0x99),
                4.5,
            ),
        ]);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].pair, "gray/gray");
    }

    #[test]
    fn dump_and_lint_clean_pure_brocado_tree_reports_zero_issues() {
        struct S;
        let mut h = PreviewHarness::new(
            S,
            |_| {
                sized_box(
                    flex_col((
                        label("Clean Heading"),
                        text_button("Action", |_: &mut S| {}),
                    ))
                    .gap(8.px()),
                )
                .padding(16.px())
            },
            PreviewConfig {
                width: 480,
                height: 320,
                ..PreviewConfig::default()
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

        let contrast_issues = lint_contrast(&[ContrastCheck::new(
            "fg/bg",
            TEXT_COLOR,
            BACKGROUND_COLOR,
            4.5,
        )]);
        assert!(
            contrast_issues.is_empty(),
            "unexpected contrast issues: {contrast_issues:?}"
        );

        let a11y_issues = lint_accessibility(&mut h);
        assert!(
            a11y_issues.is_empty(),
            "unexpected accessibility issues: {a11y_issues:?}"
        );
    }
}
