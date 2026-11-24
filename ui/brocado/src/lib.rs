//! Material Design 3 Expressive UI library built on Linebender Masonry & Xilem.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod foundation;
#[cfg(feature = "winit")]
pub mod host;
pub mod prelude;
pub mod style_engine;
pub mod theme;
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
    AdaptivePairView, AdaptivePairWidget, Bounded, BoundedAnyView, BoundedViewExt, BoundedX,
    BoundedXAnyView, BoundedXSeq, BoundedY, BoundedYSeq, BreakpointBin, BreakpointBinView, Clamp,
    ClampView, ClipBoxView, ClipBoxWidget, ColumnView, ConstrainedBox, ConstrainedBoxView,
    ContainerView, ContextMenuHost, ContextMenuView, EscapeCatcher, FitAxis, FocusFrame,
    FocusFrameView, HasMinAxis, HoverTooltipHost, ModalAxisSizing, PopoverAnchorRect,
    PopoverAnchorView, PopoverAnchorWidget, RippleLayer, RippleLayerView, RowView, ScrollAxis,
    ScrollBarStyle, ScrollBarVisibility, ScrollView, ScrollViewWidget, UiClassExt, UiScrollBar,
    UncheckedBounded, UncheckedBoundedX, UncheckedBoundedY, UnminAxis, WrapState, WrapView,
    WrapWidget, adaptive_pair, assume_bounded, assume_bounded_x, assume_bounded_y, bounded,
    bounded_clamp, bounded_x, bounded_y, breakpoint_bin, clamp, clip_box, column, constrained_box,
    container, focus_frame, icons, popover_anchor, popover_place, ripple_layer, row,
    scroll_horizontal, scroll_vertical, scroll_view, ui_icon, ui_transition, wrap,
};
pub use style_engine::{
    MemoizeWithTheme, ThemeScope, WithTheme, build_default_properties, cached_default_properties,
    classes, current_theme, current_theme_arc, memoize_with_theme, theme_scope_arc, with_theme,
};
#[cfg(feature = "winit")]
pub use theme::themed_window;
pub use theme::{
    ColorScheme, ContrastLevel, DEFAULT_SEED, ElevationLevel, Theme, contrast_ratio,
    format_shortcut_primary, format_shortcut_primary_shift, metrics, relative_luminance, seeds,
    theme_scope,
};
pub use tokens::{
    Brightness, MotionDuration, MotionEasing, SPACE_2, SPACE_4, SPACE_6, SPACE_8, SPACE_12,
    SPACE_16, SPACE_20, SPACE_24, SPACE_32, SpringSpec, TextStyleSpec, TypeRole, TypeScale,
    UiLocale, WindowSizeClass,
};

#[cfg(feature = "winit")]
pub use host::{
    AppRunner, AppState, DesktopWindow, EventLoop, EventLoopBuilder, ExitOnClose, WindowChrome,
    WindowConfig, WindowId, desktop_window, desktop_window_arc,
};
