//! Anchored menus and exposed dropdown selectors backed by `AnchoredPopupHost` (`PopupLayer`).

use std::marker::PhantomData;
use std::sync::Arc;

use xilem::kurbo::Vec2;
use xilem::masonry::core::ArcStr;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, FlexExt, button, flex_row, label};
use xilem::{AnyWidgetView, WidgetView};

use crate::foundation::overlay::{AnchoredPopupView, PopupEntrySpec, PopupLayerSpec};
use crate::foundation::{UiClassExt, icons, ui_icon};
use crate::style_engine::classes;
use crate::tokens::{DesignSpec, TypeRole};

/// Menu entry description consumed by [`dropdown_menu`] and [`select`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    /// Clickable menu row.
    Item {
        /// Primary caption.
        label: ArcStr,
        /// Optional leading SVG path.
        leading_icon: Option<&'static str>,
        /// Optional trailing shortcut caption.
        trailing_text: Option<ArcStr>,
        /// Highlighted as current value.
        selected: bool,
        /// Grayed out and non-clickable.
        disabled: bool,
    },
    /// Hairline horizontal divider.
    Divider,
}

/// Constructs an interactive [`MenuEntry::Item`].
#[must_use]
pub fn menu_item(label: impl Into<ArcStr>) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        leading_icon: None,
        trailing_text: None,
        selected: false,
        disabled: false,
    }
}

/// Constructs a [`MenuEntry::Divider`].
#[must_use]
pub fn menu_divider() -> MenuEntry {
    MenuEntry::Divider
}

impl MenuEntry {
    /// Attaches a leading icon to a [`MenuEntry::Item`].
    #[must_use]
    pub fn leading_icon(mut self, svg_path: &'static str) -> Self {
        if let Self::Item { leading_icon, .. } = &mut self {
            *leading_icon = Some(svg_path);
        }
        self
    }

    /// Attaches trailing shortcut text to a [`MenuEntry::Item`].
    #[must_use]
    pub fn trailing_text(mut self, text: impl Into<ArcStr>) -> Self {
        if let Self::Item { trailing_text, .. } = &mut self {
            *trailing_text = Some(text.into());
        }
        self
    }

    /// Marks the entry selected.
    #[must_use]
    pub fn selected(mut self, is_selected: bool) -> Self {
        if let Self::Item { selected, .. } = &mut self {
            *selected = is_selected;
        }
        self
    }

    /// Disables the entry.
    #[must_use]
    pub fn disabled(mut self, is_disabled: bool) -> Self {
        if let Self::Item { disabled, .. } = &mut self {
            *disabled = is_disabled;
        }
        self
    }
}

fn build_popup_spec(spec: &DesignSpec, entries: &[MenuEntry], min_width: f64) -> PopupLayerSpec {
    let p = &spec.palette;
    let mut item_idx = 0usize;
    let mapped = entries
        .iter()
        .map(|e| match e {
            MenuEntry::Divider => PopupEntrySpec::Divider,
            MenuEntry::Item {
                label,
                leading_icon,
                trailing_text,
                selected,
                disabled,
            } => {
                let idx = item_idx;
                item_idx += 1;
                let active_fg = if *selected {
                    p.on_secondary_container
                } else {
                    p.text_primary
                };
                PopupEntrySpec::Item {
                    action_idx: idx,
                    label: label.clone(),
                    leading_icon: *leading_icon,
                    trailing_text: trailing_text.clone(),
                    selected: *selected,
                    disabled: *disabled,
                    fg: spec.content_color(active_fg, *disabled),
                    class: if *selected {
                        classes::MENU_ITEM_SELECTED
                    } else {
                        classes::MENU_ITEM
                    },
                }
            }
        })
        .collect();

    PopupLayerSpec {
        min_width,
        max_width: (min_width * 2.0).max(280.0),
        surface_bg: p.surface_high,
        shadow_color: p.shadow,
        corner_radius: spec.metrics.menu_radius,
        divider_color: p.divider,
        padding_v: 6.0,
        padding_h: 6.0,
        header: None,
        entries: mapped,
    }
}

/// Anchors an interactive floating dropdown menu below `trigger` using Masonry's `LayerStack`.
pub fn dropdown_menu<State, Action, TriggerV, OnSelect, OnDismiss>(
    trigger: TriggerV,
    open: bool,
    entries: Vec<MenuEntry>,
    on_select: OnSelect,
    on_dismiss: OnDismiss,
) -> AnchoredPopupView<TriggerV, State, Action>
where
    State: 'static,
    Action: 'static,
    TriggerV: WidgetView<State, Action>,
    OnSelect: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    AnchoredPopupView {
        trigger,
        open,
        offset: Vec2::new(0.0, 4.0),
        spec_builder: Arc::new(move |spec| build_popup_spec(spec, &entries, 180.0)),
        on_select: Arc::new(on_select),
        on_dismiss: Arc::new(on_dismiss),
        phantom: PhantomData,
    }
}

/// Spec-styled exposed dropdown select trigger + anchored popup option list.
pub fn select<State, Action, OnToggle, OnSelect, OnDismiss>(
    options: &[&str],
    selected: usize,
    open: bool,
    on_toggle: OnToggle,
    on_select: OnSelect,
    on_dismiss: OnDismiss,
) -> Box<AnyWidgetView<State, Action>>
where
    State: 'static,
    Action: 'static,
    OnToggle: Fn(&mut State) -> Action + Send + Sync + 'static,
    OnSelect: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let current = options
        .get(selected)
        .copied()
        .unwrap_or("Select...")
        .to_string();
    let entries: Vec<MenuEntry> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let mut item = menu_item(*opt).selected(i == selected);
            if i == selected {
                item = item.leading_icon(icons::CHECK);
            }
            item
        })
        .collect();
    let on_toggle = Arc::new(on_toggle);

    let trigger = crate::style_engine::with_spec(move |spec: &DesignSpec, _| {
        let on_t = on_toggle.clone();
        let lbl = spec
            .typography
            .style_label(TypeRole::BodyMedium, label(current.clone()))
            .color(spec.palette.text_primary);
        let chevron = ui_icon(
            if open {
                icons::ARROW_DROP_UP
            } else {
                icons::ARROW_DROP_DOWN
            },
            16.0,
            spec.palette.text_secondary,
        );
        button(
            flex_row((lbl.flex(1.0), chevron))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
            move |s: &mut State| on_t(s),
        )
        .ui_class(classes::BTN_OUTLINED)
        .boxed()
    });

    dropdown_menu(trigger, open, entries, on_select, on_dismiss).boxed()
}
