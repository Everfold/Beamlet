//! Headless foundation, `DesignSpec` styling engine, and overlay primitives for multi-theme Brocado UIs.
//!
//! Upstream provenance: Brocado's retained widget runtime and reactive tree reconciliation reuse
//! upstream Linebender crates (`masonry`, `xilem_core`, `xilem_masonry`).

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod foundation;
pub mod style_engine;
pub mod tokens;

pub use masonry;
pub use masonry::dpi;
pub use masonry::kurbo;
pub use masonry::palette;
pub use masonry::parley::Alignment as TextAlign;
pub use masonry::parley::GenericFamily;
pub use masonry::parley::style::{FontFamily, FontFamilyName, FontWeight};
pub use masonry::peniko::{self, Blob, Color, ImageBrush, ImageFormat};
pub use masonry::widgets::InsertNewline;
pub use xilem_core as core;
pub use xilem_masonry as reactive;
pub use xilem_masonry::style;
pub use xilem_masonry::view;
pub use xilem_masonry::{
    AnyWidgetView, InitialRootWidget, MasonryRoot, Pod, ViewCtx, WidgetView, WidgetViewSequence,
};

pub use foundation::{
    AdaptivePairView, AdaptivePairWidget, BeveledBorder, BeveledBorderView, ClipBoxView,
    ClipBoxWidget, ContextMenuHost, ContextMenuView, EscapeCatcher, FocusFrame, FocusFrameView,
    HoverTooltipHost, PopoverAnchorRect, PopoverAnchorView, PopoverAnchorWidget, RippleLayer,
    RippleLayerView, UiClassExt, WrapState, WrapView, WrapWidget, adaptive_pair, beveled_surface,
    clip_box, focus_frame, icons, popover_anchor, popover_place, ripple_layer, ui_icon,
    ui_transition, wrap,
};
pub use style_engine::{
    MemoizeWithSpec, build_default_properties, current_spec, memoize_with_spec, ui_scope, with_spec,
};
pub use tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale, WindowBackdrop,
    WindowSizeClass,
};
