//! Reusable cross-theme UI infrastructure: color blending, SVG icon cache, transitions,
//! Masonry class injection, and window overlay primitives.

pub mod bevel;
pub mod class_view;
pub mod focus_frame;
pub mod icon;
pub mod overlay;
pub mod ripple;
pub mod state_layer;
pub mod transition;
pub mod wrap;

pub use bevel::{BeveledBorder, BeveledBorderView, beveled_surface};
pub use class_view::{UiClassExt, WithClass};
pub use focus_frame::{FocusFrame, FocusFrameView, focus_frame};
pub use icon::{icons, ui_icon};
pub use overlay::{
    AnchoredPopupHost, AnchoredPopupView, ContextMenuHost, ContextMenuView, EscapeCatcher,
    EscapeCatcherView, EscapePressed, HoverTooltipHost, HoverTooltipView, ModalShellConfig,
    ModalShellState, PopoverAnchorAction, PopoverAnchorRect, PopoverAnchorView,
    PopoverAnchorWidget, PopoverPlacementView, PopoverPlacementWidget, PopupAction, PopupEntrySpec,
    PopupLayer, PopupLayerSpec, TooltipContentSpec, escape_catcher, modal_shell_build,
    modal_shell_message, modal_shell_rebuild, modal_shell_teardown, popover_anchor, popover_place,
    ui_scrim,
};
pub use ripple::{RippleLayer, RippleLayerView, ripple_layer};
pub use state_layer::{InteractiveColors, blend_over, resolve_interactive_colors, with_opacity};
pub use transition::{SlideTransition, UiTransition, ui_transition};
pub use wrap::{WrapState, WrapView, WrapWidget, wrap};
