//! Convenience prelude exposing concrete cross-theme components, focus primitives, and per-platform APIs.

pub use crate::components::*;
pub use crate::theme::{DesignLanguage, UiAccent, UiTheme, ui_scope};
#[cfg(feature = "winit")]
pub use crate::theme::{themed_window, themed_window_with_backdrop};
pub use crate::{
    AdaptivePairView, AdaptivePairWidget, AnyWidgetView, BeveledBorder, BeveledBorderView, Bounded,
    BoundedAnyView, BoundedViewExt, BoundedX, BoundedXAnyView, BoundedXSeq, BoundedY, BoundedYSeq,
    BreakpointBin, BreakpointBinView, Brightness, Clamp, ClampView, ClipBoxView, ClipBoxWidget,
    Color, ColumnView, ConstrainedBox, ConstrainedBoxView, ContainerView, ContextMenuHost,
    ContextMenuView, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder,
    ElevationLevel, EscapeCatcher, FitAxis, FocusFrame, FocusFrameView, FontFamily, FontFamilyName,
    FontWeight, GenericFamily, HasMinAxis, HoverTooltipHost, InputFocusStyle, InsertNewline,
    ModalAxisSizing, MotionDuration, MotionEasing, MotionScheme, NavIndicatorStyle, Pod,
    PopoverAnchorRect, PopoverAnchorView, PopoverAnchorWidget, RippleLayer, RippleLayerView,
    RowView, ScrollAxis, ScrollBarStyle, ScrollBarVisibility, ScrollView, ScrollViewWidget,
    SemanticPalette, ShortcutConvention, SpringSpec, StateFeedbackMode, TabIndicatorStyle,
    TextAlign, TypeRole, TypeScale, UiClassExt, UiLocale, UiScrollBar, UncheckedBounded,
    UncheckedBoundedX, UncheckedBoundedY, UnminAxis, ViewCtx, WidgetView, WidgetViewSequence,
    WindowBackdrop, WindowSizeClass, WrapState, WrapView, WrapWidget, adaptive_pair,
    assume_bounded, assume_bounded_x, assume_bounded_y, beveled_surface, bounded, bounded_clamp,
    bounded_x, bounded_y, breakpoint_bin, build_default_properties, cached_default_properties,
    clamp, classes, clip_box, column, constrained_box, container, current_spec, focus_frame, icons,
    memoize_with_spec, popover_anchor, popover_place, ripple_layer, row, scroll_horizontal,
    scroll_vertical, scroll_view, ui_icon, ui_transition, with_spec, wrap,
};

#[cfg(feature = "winit")]
pub use crate::{
    AppRunner, AppState, DesktopWindow, EventLoop, EventLoopBuilder, ExitOnClose, WindowChrome,
    WindowConfig, WindowId, desktop_window, desktop_window_arc,
};

#[cfg(feature = "md3")]
pub use brocado_md3::{
    ColorScheme, ContrastLevel, FabColor, FabMenuItem, FabSize, FloatingToolbarVariant,
    Md3ColorVariant, Md3Style, Md3Theme, TopAppBarVariant, UiWavyLinearProgress,
    WavyLinearProgressWidget, bottom_app_bar, button_group, center_aligned_top_app_bar,
    extended_fab, fab, fab_menu, floating_toolbar, floating_toolbar_with_variant, large_fab,
    large_top_app_bar, md3_scope, medium_top_app_bar, navigation_rail_with_fab, sized_fab,
    small_fab, split_button, top_app_bar, top_app_bar_slots, wavy_linear_progress,
};
