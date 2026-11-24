//! Reusable Material Design 3 Expressive UI infrastructure: color blending, SVG icon cache, transitions,
//! Masonry class injection, and window overlay primitives.

pub mod adaptive_pair;
pub mod bounded;
pub mod breakpoint;
pub mod clamp;
pub mod class_view;
pub mod clip_box;
pub mod constrained_box;
pub mod focus_frame;
pub mod icon;
pub mod overlay;
pub mod ripple;
pub mod scroll_view;
pub mod state_layer;
pub mod transition;
pub mod wrap;

pub use adaptive_pair::{AdaptivePairView, AdaptivePairWidget, adaptive_pair};
pub use bounded::{
    Bounded, BoundedAnyView, BoundedViewExt, BoundedX, BoundedXAnyView, BoundedXSeq, BoundedY,
    BoundedYSeq, ColumnView, ContainerView, FitAxis, HasMinAxis, RowView, ScrollAxis,
    UncheckedBounded, UncheckedBoundedX, UncheckedBoundedY, UnminAxis, assume_bounded,
    assume_bounded_x, assume_bounded_y, bounded, bounded_x, bounded_y, column, container, row,
};
pub use breakpoint::{BreakpointBin, BreakpointBinView, breakpoint_bin};
pub use clamp::{Clamp, ClampView, bounded_clamp, clamp};
pub use class_view::{UiClassExt, WithClass};
pub use clip_box::{ClipBoxView, ClipBoxWidget, clip_box};
pub use constrained_box::{ConstrainedBox, ConstrainedBoxView, constrained_box};
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
pub use scroll_view::{
    ScrollBarStyle, ScrollBarVisibility, ScrollView, ScrollViewWidget, UiScrollBar,
    scroll_horizontal, scroll_vertical, scroll_view,
};
pub use state_layer::{InteractiveColors, blend_over, resolve_interactive_colors, with_opacity};
pub use transition::{ModalAxisSizing, SlideTransition, UiTransition, ui_transition};
pub use wrap::{WrapState, WrapView, WrapWidget, wrap};
