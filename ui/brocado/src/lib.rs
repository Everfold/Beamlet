//! Unified multi-theme UI facade supporting Material Design 3, Windows Fluent, macOS HIG, and GNOME Adwaita.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod prelude;
pub mod theme;

pub use brocado_core as core_ui;
pub use brocado_core::components;
pub use brocado_core::foundation::{
    BeveledBorder, BeveledBorderView, ContextMenuHost, ContextMenuView, EscapeCatcher, FocusFrame,
    FocusFrameView, HoverTooltipHost, PopoverAnchorRect, PopoverAnchorView, PopoverAnchorWidget,
    RippleLayer, RippleLayerView, UiClassExt, WrapState, WrapView, WrapWidget, beveled_surface,
    focus_frame, icons, popover_anchor, popover_place, ripple_layer, ui_icon, ui_transition, wrap,
};
pub use brocado_core::style_engine::{
    MemoizeWithSpec, build_default_properties, cached_default_properties, classes, current_spec,
    memoize_with_spec, with_spec,
};
pub use brocado_core::tokens;
pub use brocado_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TypeRole, TypeScale, WindowBackdrop,
    WindowSizeClass,
};
pub use brocado_core::{
    AnyWidgetView, Color, FontWeight, InitialRootWidget, InsertNewline, MasonryRoot, Pod,
    TextAlign, ViewCtx, WidgetView, WidgetViewSequence, core, dpi, kurbo, masonry, reactive, style,
    view,
};
pub use theme::{DesignLanguage, UiAccent, UiTheme, ui_scope};
#[cfg(feature = "winit")]
pub use theme::{themed_window, themed_window_with_backdrop};

#[cfg(feature = "winit")]
pub use brocado_winit as host;
#[cfg(feature = "winit")]
pub use brocado_winit::{
    AppRunner, AppState, DesktopWindow, EventLoop, EventLoopBuilder, ExitOnClose, WindowChrome,
    WindowConfig, WindowId, desktop_window, desktop_window_arc,
};

#[cfg(feature = "md3")]
pub use brocado_md3 as md3;

#[cfg(feature = "fluent")]
pub use brocado_fluent as fluent;

#[cfg(feature = "hig")]
pub use brocado_hig as hig;

#[cfg(feature = "adwaita")]
pub use brocado_adwaita as adwaita;
