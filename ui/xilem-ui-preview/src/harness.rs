//! Headless CPU preview runner wrapping `masonry_testing::TestHarness` for Xilem views.

use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, bail};
use image::{ImageFormat, RgbaImage};
use masonry_testing::{TestHarness, TestHarnessParams};
use xilem::core::{
    DynMessage, MessageCtx, ProxyError, RawProxy, SendMessage, View, ViewId, ViewPathTracker as _,
};
use xilem::kurbo::Point;
use xilem::masonry::app::RenderRoot;
use xilem::masonry::core::keyboard::{Code, Key, KeyState, NamedKey};
use xilem::masonry::core::{KeyboardEvent, PointerButton, TextEvent, Widget, WidgetId, WidgetRef};
use xilem::masonry::dpi::PhysicalSize;
use xilem::masonry::properties::Dimensions;
use xilem::masonry::widgets::Passthrough;
use xilem::style::Style as _;
use xilem::view::sized_box;
use xilem::{AnyWidgetView, ViewCtx, WidgetView};
use xilem_ui::prelude::*;
use xilem_ui_core::foundation::AnchoredPopupHost;

#[derive(Debug)]
struct NoopRawProxy;

impl RawProxy for NoopRawProxy {
    fn send_message(&self, _path: Arc<[ViewId]>, _message: SendMessage) -> Result<(), ProxyError> {
        Ok(())
    }

    fn dyn_debug(&self) -> &dyn Debug {
        self
    }
}

/// Viewport and rasterization configuration for headless preview runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewConfig {
    /// Viewport width in physical pixels.
    pub width: u32,
    /// Viewport height in physical pixels.
    pub height: u32,
    /// Scale factor passed to Masonry (`1.0` = 96 DPI).
    pub scale_factor: f64,
    /// Maximum compressed PNG byte size allowed by snapshot checks.
    pub max_screenshot_size: u32,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            width: 1180,
            height: 800,
            scale_factor: 1.0,
            max_screenshot_size: 512 * 1024,
        }
    }
}

/// Scriptable UI interaction step replayed before capturing a preview frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewStep {
    /// Clicks the first `Button` whose subtree debug text matches `label`.
    ClickLabel(String),
    /// Clicks the `index`-th widget whose `short_type_name()` equals or contains `type_name`.
    ClickType {
        /// Masonry widget short type name.
        type_name: String,
        /// Zero-based match index.
        index: usize,
    },
    /// Moves keyboard focus to the `index`-th widget matching `type_name`.
    FocusType {
        /// Masonry widget short type name.
        type_name: String,
        /// Zero-based match index.
        index: usize,
    },
    /// Sends Unicode character input to the currently focused widget.
    TypeText(String),
    /// Moves the pointer to the center of the button matching `label`.
    HoverLabel(String),
    /// Moves the pointer to the center of the `index`-th widget matching `type_name`.
    HoverType {
        /// Masonry widget short type name.
        type_name: String,
        /// Zero-based match index.
        index: usize,
    },
    /// Advances animation timers by `ms` milliseconds.
    AnimateMs(u64),
    /// Dispatches an `Escape` key press.
    PressEscape,
    /// Dispatches a `Tab` (or `Shift+Tab`) key press.
    PressTab {
        /// Holds Shift modifier when `true`.
        shift: bool,
    },
}

fn parse_type_and_index(spec: &str) -> anyhow::Result<(String, usize)> {
    match spec.rsplit_once(':') {
        Some((ty, idx_str)) if !ty.is_empty() => {
            let idx = idx_str
                .parse::<usize>()
                .with_context(|| format!("invalid widget index '{idx_str}' in '{spec}'"))?;
            Ok((ty.to_string(), idx))
        }
        _ if !spec.is_empty() => Ok((spec.to_string(), 0)),
        _ => bail!("expected non-empty widget type specification"),
    }
}

impl PreviewStep {
    /// Parses a CLI `--step` specification such as `"click:Open Basic Dialog"` or `"focus:TextArea:0"`.
    pub fn parse(spec: &str) -> anyhow::Result<Self> {
        if let Some(arg) = spec.strip_prefix("click:") {
            anyhow::ensure!(!arg.is_empty(), "click: requires a non-empty button label");
            return Ok(Self::ClickLabel(arg.to_string()));
        }
        if let Some(arg) = spec.strip_prefix("click-type:") {
            let (type_name, index) = parse_type_and_index(arg)?;
            return Ok(Self::ClickType { type_name, index });
        }
        if let Some(arg) = spec.strip_prefix("focus:") {
            let (type_name, index) = parse_type_and_index(arg)?;
            return Ok(Self::FocusType { type_name, index });
        }
        if let Some(arg) = spec.strip_prefix("type:") {
            return Ok(Self::TypeText(arg.to_string()));
        }
        if let Some(arg) = spec.strip_prefix("hover:") {
            anyhow::ensure!(!arg.is_empty(), "hover: requires a non-empty label");
            return Ok(Self::HoverLabel(arg.to_string()));
        }
        if let Some(arg) = spec.strip_prefix("hover-type:") {
            let (type_name, index) = parse_type_and_index(arg)?;
            return Ok(Self::HoverType { type_name, index });
        }
        if let Some(arg) = spec.strip_prefix("animate:") {
            let ms = arg
                .parse::<u64>()
                .with_context(|| format!("invalid millisecond duration '{arg}'"))?;
            return Ok(Self::AnimateMs(ms));
        }
        if let Some(arg) = spec.strip_prefix("key:") {
            return match arg.to_ascii_lowercase().as_str() {
                "escape" | "esc" => Ok(Self::PressEscape),
                "tab" => Ok(Self::PressTab { shift: false }),
                "shift-tab" => Ok(Self::PressTab { shift: true }),
                other => bail!("unsupported key step '{other}' (expected escape, tab, shift-tab)"),
            };
        }
        bail!(
            "invalid preview step '{spec}'; expected click:, click-type:, focus:, type:, hover:, hover-type:, animate:, or key:"
        )
    }
}

type RootViewState<State> = <Box<AnyWidgetView<State>> as View<State, (), ViewCtx>>::ViewState;

/// Reusable headless Xilem/Masonry harness capable of CPU rasterization, DOM inspection, and interaction replay.
#[expect(
    missing_debug_implementations,
    reason = "Holds Masonry TestHarness, tokio runtime, and opaque view state"
)]
pub struct PreviewHarness<State: 'static> {
    state: State,
    theme_fn: fn(&State) -> UiTheme,
    logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
    view_ctx: ViewCtx,
    current_view: Box<AnyWidgetView<State>>,
    view_state: RootViewState<State>,
    last_spec: DesignSpec,
    config: PreviewConfig,
    harness: TestHarness<Passthrough>,
}

impl<State: 'static> PreviewHarness<State> {
    /// Builds a headless preview window at `config` viewport using `theme_fn` and root `logic`.
    pub fn new(
        mut state: State,
        theme_fn: fn(&State) -> UiTheme,
        logic: fn(&mut State) -> Box<AnyWidgetView<State>>,
        config: PreviewConfig,
    ) -> Self {
        let rt = Arc::new(tokio::runtime::Runtime::new().expect("tokio preview runtime"));
        let mut view_ctx = ViewCtx::new(Arc::new(NoopRawProxy), rt);
        let theme = theme_fn(&state);
        let spec = theme.to_spec();
        let scoped_view = ui_scope(
            &theme,
            sized_box(logic(&mut state))
                .dims(Dimensions::STRETCH)
                .background_color(spec.palette.window_bg),
        )
        .boxed();

        let (pod, view_state) = scoped_view.build(&mut view_ctx, &mut state);
        let params = TestHarnessParams::default()
            .with_size(PhysicalSize::new(config.width, config.height))
            .with_background(spec.palette.window_bg)
            .with_padding(0)
            .with_scale(config.scale_factor)
            .with_max_screenshot_size(config.max_screenshot_size);
        let harness = TestHarness::create_with(theme.default_properties(), pod.new_widget, params);

        Self {
            state,
            theme_fn,
            logic,
            view_ctx,
            current_view: scoped_view,
            view_state,
            last_spec: spec,
            config,
            harness,
        }
    }

    /// Rebuilds the view tree cleanly after state mutations, updating Masonry `DefaultProperties` on theme changes.
    pub fn rebuild(&mut self) {
        let theme = (self.theme_fn)(&self.state);
        let next_spec = theme.to_spec();
        if next_spec != self.last_spec {
            let props = cached_default_properties(&next_spec);
            self.harness.render_root().set_default_properties(props);
            self.last_spec = next_spec.clone();
        }
        let next_view = ui_scope(
            &theme,
            sized_box((self.logic)(&mut self.state))
                .dims(Dimensions::STRETCH)
                .background_color(next_spec.palette.window_bg),
        )
        .boxed();

        self.view_ctx.reset_changed_props();
        self.view_ctx.reset_changed_transforms();
        let current_view = &self.current_view;
        let view_state = &mut self.view_state;
        let view_ctx = &mut self.view_ctx;
        let app_state = &mut self.state;
        self.harness.edit_root_widget(|mut root| {
            next_view.rebuild(
                current_view,
                view_state,
                view_ctx,
                root.reborrow_mut(),
                app_state,
            );
        });
        self.current_view = next_view;
    }

    /// Pumps all queued Masonry actions through Xilem message handling and triggers `rebuild()` if handled.
    pub fn pump_actions(&mut self) {
        while let Some((action, widget_id)) = self.harness.pop_action_erased() {
            let Some(path) = self.view_ctx.get_id_path(widget_id).cloned() else {
                continue;
            };
            let env = std::mem::take(self.view_ctx.environment());
            let mut msg_ctx = MessageCtx::new(env, path, DynMessage(action));
            let view = &self.current_view;
            let view_state = &mut self.view_state;
            let app_state = &mut self.state;
            let result = self.harness.edit_root_widget(|mut root| {
                view.message(view_state, &mut msg_ctx, root.reborrow_mut(), app_state)
            });
            *self.view_ctx.environment() = msg_ctx.finish().0;
            if matches!(result, xilem::core::MessageResult::Action(())) {
                self.rebuild();
            }
        }
    }

    fn find_button_id_by_label(&mut self, needle: &str) -> Option<WidgetId> {
        let rr: &RenderRoot = self.harness.render_root();
        let root = rr.get_layer_root(0);
        find_button_exact_then_contains(&root, rr, needle)
    }

    fn find_widget_id_by_type(&mut self, type_name: &str, index: usize) -> Option<WidgetId> {
        let rr: &RenderRoot = self.harness.render_root();
        let root = rr.get_layer_root(0);
        let mut exact = Vec::new();
        let mut partial = Vec::new();
        collect_matching_type(&root, rr, type_name, &mut exact, &mut partial);
        exact
            .get(index)
            .copied()
            .or_else(|| partial.get(index).copied())
    }

    fn move_pointer_to_widget(&mut self, id: WidgetId) {
        let center = {
            let w = self.harness.get_widget_with_id(id);
            w.ctx().window_transform() * w.ctx().border_box().center()
        };
        self.harness.mouse_move(center);
    }

    /// Replays one [`PreviewStep`] against the active widget tree and pumps resulting actions.
    pub fn run_step(&mut self, step: &PreviewStep) -> anyhow::Result<()> {
        let _ = self.harness.redraw();
        match step {
            PreviewStep::ClickLabel(label) => {
                let id = self
                    .find_button_id_by_label(label)
                    .with_context(|| format!("button matching label '{label}' not found"))?;
                self.click_widget_id(id);
            }
            PreviewStep::ClickType { type_name, index } => {
                let id = self
                    .find_widget_id_by_type(type_name, *index)
                    .with_context(|| {
                        format!("widget matching type '{type_name}' index {index} not found")
                    })?;
                self.click_widget_id(id);
            }
            PreviewStep::FocusType { type_name, index } => {
                let id = self
                    .find_widget_id_by_type(type_name, *index)
                    .with_context(|| {
                        format!("widget matching type '{type_name}' index {index} not found")
                    })?;
                self.harness.focus_on(Some(id));
                self.pump_actions();
            }
            PreviewStep::TypeText(text) => {
                self.harness.keyboard_type_chars(text);
                self.pump_actions();
            }
            PreviewStep::HoverLabel(label) => {
                let id = self
                    .find_button_id_by_label(label)
                    .with_context(|| format!("hover target matching label '{label}' not found"))?;
                self.move_pointer_to_widget(id);
                self.pump_actions();
            }
            PreviewStep::HoverType { type_name, index } => {
                let id = self
                    .find_widget_id_by_type(type_name, *index)
                    .with_context(|| {
                        format!("hover target matching type '{type_name}' index {index} not found")
                    })?;
                self.move_pointer_to_widget(id);
                self.pump_actions();
            }
            PreviewStep::AnimateMs(ms) => {
                self.harness.animate_ms(*ms);
                self.pump_actions();
            }
            PreviewStep::PressEscape => {
                self.harness
                    .process_text_event(TextEvent::Keyboard(KeyboardEvent {
                        state: KeyState::Down,
                        key: Key::Named(NamedKey::Escape),
                        code: Code::Escape,
                        ..KeyboardEvent::default()
                    }));
                self.pump_actions();
            }
            PreviewStep::PressTab { shift } => {
                self.harness.press_tab_key(*shift);
                self.pump_actions();
            }
        }
        Ok(())
    }

    /// Replays a sequence of [`PreviewStep`]s in order.
    pub fn run_steps(&mut self, steps: &[PreviewStep]) -> anyhow::Result<()> {
        for step in steps {
            self.run_step(step)?;
        }
        Ok(())
    }

    /// Advances Masonry animation timers by 500ms so mount transitions settle to full opacity.
    pub fn settle_animations(&mut self) {
        let _ = self.harness.redraw();
        self.harness.animate_ms(500);
        self.pump_actions();
    }

    /// Simulates an OS window focus gain/loss event (`TextEvent::WindowFocusChange`) and repaints.
    pub fn set_window_focused(&mut self, focused: bool) {
        self.harness
            .process_text_event(TextEvent::WindowFocusChange(focused));
        self.pump_actions();
        let _ = self.harness.redraw();
    }

    /// Rasterizes the current widget tree on CPU using `vello_cpu`.
    pub fn render_image(&mut self) -> RgbaImage {
        self.harness.render()
    }

    /// Rasterizes the current frame and writes a PNG file at `path`.
    pub fn render_png(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let img = self.render_image();
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        img.save_with_format(path, ImageFormat::Png)
            .with_context(|| format!("failed writing preview PNG to {}", path.display()))
    }

    /// Compares the rendered frame against `<manifest_dir>/screenshots/<test_name>.png` (updating when `MASONRY_TEST_BLESS=1`).
    pub fn check_render_snapshot(&mut self, manifest_dir: &str, test_name: &str) {
        self.harness
            .check_render_snapshot(manifest_dir, test_name, false);
    }

    /// Returns a mutable handle to the underlying `TestHarness` for custom low-level assertions.
    pub fn inner_harness_mut(&mut self) -> &mut TestHarness<Passthrough> {
        &mut self.harness
    }

    /// Active [`DesignSpec`] corresponding to the latest build/rebuild.
    #[must_use]
    pub fn current_spec(&self) -> &DesignSpec {
        &self.last_spec
    }

    /// Configured viewport dimensions `(width, height)` in logical pixels.
    #[must_use]
    pub fn viewport(&self) -> (f64, f64) {
        (f64::from(self.config.width), f64::from(self.config.height))
    }

    /// Shared reference to application state.
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Mutable reference to application state.
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// Moves the pointer to window coordinates `(x, y)`, clicks the primary button, and pumps actions.
    pub fn click_point(&mut self, x: f64, y: f64) {
        let _ = self.harness.redraw();
        self.harness.mouse_move(Point::new(x, y));
        self.harness
            .mouse_button_press(Some(PointerButton::Primary));
        self.harness
            .mouse_button_release(Some(PointerButton::Primary));
        self.pump_actions();
    }

    /// Returns all widget IDs in tree order matching `type_name` (exact match first, substring fallback).
    pub fn find_widget_ids(&mut self, type_name: &str) -> Vec<WidgetId> {
        let _ = self.harness.redraw();
        let rr: &RenderRoot = self.harness.render_root();
        let root = rr.get_layer_root(0);
        let mut exact = Vec::new();
        let mut partial = Vec::new();
        collect_matching_type(&root, rr, type_name, &mut exact, &mut partial);
        if exact.is_empty() { partial } else { exact }
    }

    /// Clicks widget `id` with the primary mouse button (scrolling into view if needed) and pumps actions.
    pub fn click_widget_id(&mut self, id: WidgetId) {
        if let Some((x, y, w, h)) = self.widget_rect(id) {
            let (vw, vh) = self.viewport();
            if x < 0.0 || y < 0.0 || x + w > vw || y + h > vh {
                self.harness.scroll_into_view(id);
                let _ = self.harness.redraw();
            }
            if let Some((sx, sy, sw, sh)) = self.widget_rect(id) {
                self.click_point(sx + sw * 0.5, sy + sh * 0.5);
                return;
            }
        }
        self.harness
            .mouse_click_on(id, Some(PointerButton::Primary));
        self.pump_actions();
    }

    /// Returns a widget's `(x, y, width, height)` border-box in window coordinates, if present.
    pub fn widget_rect(&mut self, id: WidgetId) -> Option<(f64, f64, f64, f64)> {
        let _ = self.harness.redraw();
        let w = self.harness.try_get_widget(id)?;
        let box_rect = w.ctx().border_box();
        let origin = w.ctx().window_transform() * box_rect.origin();
        Some((origin.x, origin.y, box_rect.width(), box_rect.height()))
    }

    /// Clicks inside the `index`-th widget matching `type_name` at fractional `(x_frac, y_frac)`.
    pub fn click_type_at(
        &mut self,
        type_name: &str,
        index: usize,
        x_frac: f64,
        y_frac: f64,
    ) -> anyhow::Result<()> {
        let id = self
            .find_widget_id_by_type(type_name, index)
            .with_context(|| format!("widget '{type_name}' index {index} not found"))?;
        let (x, y, w, h) = self
            .widget_rect(id)
            .with_context(|| format!("failed reading rect for '{type_name}' index {index}"))?;
        self.click_point(x + w * x_frac, y + h * y_frac);
        Ok(())
    }

    /// Drags inside the `index`-th widget matching `type_name` between fractional `(from)` and `(to)` positions.
    pub fn drag_type(
        &mut self,
        type_name: &str,
        index: usize,
        from: (f64, f64),
        to: (f64, f64),
    ) -> anyhow::Result<()> {
        let id = self
            .find_widget_id_by_type(type_name, index)
            .with_context(|| format!("widget '{type_name}' index {index} not found"))?;
        let (x, y, w, h) = self
            .widget_rect(id)
            .with_context(|| format!("failed reading rect for '{type_name}' index {index}"))?;
        let p0 = Point::new(x + w * from.0, y + h * from.1);
        let p1 = Point::new(
            x + w * ((from.0 + to.0) * 0.5),
            y + h * ((from.1 + to.1) * 0.5),
        );
        let p2 = Point::new(x + w * to.0, y + h * to.1);
        self.harness.mouse_move(p0);
        self.harness
            .mouse_button_press(Some(PointerButton::Primary));
        self.harness.mouse_move(p1);
        self.harness.mouse_move(p2);
        self.harness
            .mouse_button_release(Some(PointerButton::Primary));
        self.pump_actions();
        Ok(())
    }
}

fn subtree_text(node: &WidgetRef<'_, dyn Widget>, needle: &str, exact: bool) -> bool {
    if let Some(txt) = node.inner().get_debug_text() {
        if exact {
            if txt == needle {
                return true;
            }
        } else if txt.contains(needle) {
            return true;
        }
    }
    node.children()
        .iter()
        .any(|c| subtree_text(c, needle, exact))
}

fn find_button_exact_then_contains(
    root: &WidgetRef<'_, dyn Widget>,
    render_root: &RenderRoot,
    needle: &str,
) -> Option<WidgetId> {
    fn walk(
        node: &WidgetRef<'_, dyn Widget>,
        render_root: &RenderRoot,
        needle: &str,
        exact: bool,
    ) -> Option<WidgetId> {
        if node.ctx().is_stashed() {
            return None;
        }
        if node.inner().short_type_name().contains("Button") && subtree_text(node, needle, exact) {
            return Some(node.id());
        }
        for child in node.children() {
            if let Some(id) = walk(&child, render_root, needle, exact) {
                return Some(id);
            }
        }
        if let Some(host) = node.downcast::<AnchoredPopupHost>()
            && let Some(layer_id) = host.inner().layer_id()
            && let Some(layer_node) = render_root.get_widget(layer_id)
            && let Some(id) = walk(&layer_node, render_root, needle, exact)
        {
            return Some(id);
        }
        None
    }
    walk(root, render_root, needle, true).or_else(|| walk(root, render_root, needle, false))
}

fn collect_matching_type(
    node: &WidgetRef<'_, dyn Widget>,
    render_root: &RenderRoot,
    needle: &str,
    exact: &mut Vec<WidgetId>,
    partial: &mut Vec<WidgetId>,
) {
    if node.ctx().is_stashed() {
        return;
    }
    let name = node.inner().short_type_name();
    if name == needle {
        exact.push(node.id());
    } else if name.contains(needle) {
        partial.push(node.id());
    }
    for child in node.children() {
        collect_matching_type(&child, render_root, needle, exact, partial);
    }
    if let Some(host) = node.downcast::<AnchoredPopupHost>()
        && let Some(layer_id) = host.inner().layer_id()
        && let Some(layer_node) = render_root.get_widget(layer_id)
    {
        collect_matching_type(&layer_node, render_root, needle, exact, partial);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_preview_steps_handles_supported_verbs() {
        assert_eq!(
            PreviewStep::parse("click:Save").unwrap(),
            PreviewStep::ClickLabel("Save".into())
        );
        assert_eq!(
            PreviewStep::parse("focus:TextArea").unwrap(),
            PreviewStep::FocusType {
                type_name: "TextArea".into(),
                index: 0,
            }
        );
        assert_eq!(
            PreviewStep::parse("click-type:Button:2").unwrap(),
            PreviewStep::ClickType {
                type_name: "Button".into(),
                index: 2,
            }
        );
        assert_eq!(
            PreviewStep::parse("animate:450").unwrap(),
            PreviewStep::AnimateMs(450)
        );
        assert_eq!(
            PreviewStep::parse("key:escape").unwrap(),
            PreviewStep::PressEscape
        );
        assert_eq!(
            PreviewStep::parse("key:shift-tab").unwrap(),
            PreviewStep::PressTab { shift: true }
        );
        assert!(PreviewStep::parse("unknown:foo").is_err());
    }
}
