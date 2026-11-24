//! Convenience prelude exposing Material Design 3 Expressive components, theme APIs, and layout primitives.

pub use crate::components::*;
#[cfg(feature = "winit")]
pub use crate::theme::themed_window;
pub use crate::theme::{
    ColorScheme, ContrastLevel, DEFAULT_SEED, ElevationLevel, Theme, contrast_ratio,
    format_shortcut_primary, format_shortcut_primary_shift, metrics, relative_luminance, seeds,
    theme_scope,
};
pub use crate::{
    AdaptivePairView, AdaptivePairWidget, AnyWidgetView, Bounded, BoundedAnyView, BoundedViewExt,
    BoundedX, BoundedXAnyView, BoundedXSeq, BoundedY, BoundedYSeq, BreakpointBin,
    BreakpointBinView, Brightness, Clamp, ClampView, ClipBoxView, ClipBoxWidget, Color, ColumnView,
    ConstrainedBox, ConstrainedBoxView, ContainerView, ContextMenuHost, ContextMenuView,
    EscapeCatcher, FitAxis, FocusFrame, FocusFrameView, FontFamily, FontFamilyName, FontWeight,
    GenericFamily, HasMinAxis, HoverTooltipHost, InsertNewline, MemoizeWithTheme, ModalAxisSizing,
    MotionDuration, MotionEasing, Pod, PopoverAnchorRect, PopoverAnchorView, PopoverAnchorWidget,
    RippleLayer, RippleLayerView, RowView, SPACE_2, SPACE_4, SPACE_6, SPACE_8, SPACE_12, SPACE_16,
    SPACE_20, SPACE_24, SPACE_32, ScrollAxis, ScrollBarStyle, ScrollBarVisibility, ScrollView,
    ScrollViewWidget, SpringSpec, TextAlign, TextStyleSpec, ThemeScope, TypeRole, TypeScale,
    UiClassExt, UiLocale, UiScrollBar, UncheckedBounded, UncheckedBoundedX, UncheckedBoundedY,
    UnminAxis, ViewCtx, WidgetView, WidgetViewSequence, WindowSizeClass, WithTheme, WrapState,
    WrapView, WrapWidget, adaptive_pair, assume_bounded, assume_bounded_x, assume_bounded_y,
    bounded, bounded_clamp, bounded_x, bounded_y, breakpoint_bin, build_default_properties,
    cached_default_properties, clamp, classes, clip_box, column, constrained_box, container,
    current_theme, current_theme_arc, focus_frame, icons, memoize_with_theme, popover_anchor,
    popover_place, ripple_layer, row, scroll_horizontal, scroll_vertical, scroll_view,
    theme_scope_arc, ui_icon, ui_transition, with_theme, wrap,
};

#[cfg(feature = "winit")]
pub use crate::{
    AppRunner, AppState, DesktopWindow, EventLoop, EventLoopBuilder, ExitOnClose, WindowChrome,
    WindowConfig, WindowId, desktop_window, desktop_window_arc,
};
