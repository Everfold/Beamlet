//! Spec-driven icon buttons across Standard, Filled, Tonal, and Outlined variants.

use std::sync::Arc;

use xilem::view::button;
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::{UiClassExt, ui_icon};
use crate::style_engine::classes;
use crate::tokens::DesignSpec;

/// Visual emphasis style for [`UiIconButton`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconButtonVariant {
    /// Transparent background until hovered/selected.
    #[default]
    Standard,
    /// Accent background fill.
    Filled,
    /// Secondary container background fill.
    Tonal,
    /// Bordered outline ring.
    Outlined,
}

/// Callback signature for [`UiIconButton`].
pub type IconButtonCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// Spec-aware square icon button supporting optional toggle selection.
#[must_use = "View values do nothing unless provided to Xilem."]
#[expect(missing_debug_implementations, reason = "Contains closure callback")]
pub struct UiIconButton<State, Action> {
    variant: IconButtonVariant,
    svg_path: &'static str,
    selected: bool,
    disabled: bool,
    on_click: IconButtonCallback<State, Action>,
}

impl<State, Action> UiIconButton<State, Action> {
    /// Marks this icon button as toggle-selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Disables activation when `true`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn render_with_spec(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let (cls, active_fg) = match (self.variant, self.selected) {
            (IconButtonVariant::Standard, false) => (classes::ICON_BTN_STD, p.text_secondary),
            (IconButtonVariant::Standard, true) => (classes::ICON_BTN_STD_SELECTED, p.accent),
            (IconButtonVariant::Filled, true) => (classes::ICON_BTN_FILLED, p.on_accent),
            (IconButtonVariant::Filled, false) => (classes::ICON_BTN_FILLED_UNSELECTED, p.accent),
            (IconButtonVariant::Tonal, true) => (classes::ICON_BTN_TONAL, p.on_secondary_container),
            (IconButtonVariant::Tonal, false) => {
                (classes::ICON_BTN_TONAL_UNSELECTED, p.text_secondary)
            }
            (IconButtonVariant::Outlined, false) => (classes::ICON_BTN_OUTLINED, p.text_secondary),
            (IconButtonVariant::Outlined, true) => {
                (classes::ICON_BTN_OUTLINED_SELECTED, p.inverse_on_surface)
            }
        };
        let fg = spec.content_color(active_fg, self.disabled);
        let icon_size = (spec.metrics.icon_btn_size * 0.55).clamp(16.0, 22.0);
        let on_click = self.on_click.clone();
        button(
            ui_icon(self.svg_path, icon_size, fg),
            move |state: &mut State| on_click(state),
        )
        .disabled(self.disabled)
        .ui_class(cls)
        .boxed()
    }
}

crate::impl_spec_view!(UiIconButton<State, Action>);

fn make_icon_button<State, Action, F>(
    variant: IconButtonVariant,
    svg_path: &'static str,
    selected_default: bool,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    UiIconButton {
        variant,
        svg_path,
        selected: selected_default,
        disabled: false,
        on_click: Arc::new(on_click),
    }
}

/// Standard icon button (unselected by default).
pub fn icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Standard, svg_path, false, on_click)
}

/// Filled high-emphasis icon button (defaults `selected = true`).
pub fn filled_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Filled, svg_path, true, on_click)
}

/// Tonal icon button (defaults `selected = true`).
pub fn tonal_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Tonal, svg_path, true, on_click)
}

/// Outlined icon button (unselected by default).
pub fn outlined_icon_button<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    on_click: F,
) -> UiIconButton<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    make_icon_button(IconButtonVariant::Outlined, svg_path, false, on_click)
}
