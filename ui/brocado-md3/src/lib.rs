//! Material Design 3 theme backend and signature Material 3 components for Brocado.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod color;
pub mod components;
pub mod theme;

pub use brocado_core::components::*;
pub use brocado_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, Bounded, BoundedAnyView, BoundedViewExt, BoundedX,
    BoundedXAnyView, BoundedXSeq, BoundedY, BoundedYSeq, ColumnView, ContainerView, EscapeCatcher,
    EscapeCatcherView, EscapePressed, FitAxis, FocusFrame, FocusFrameView, HasMinAxis,
    HoverTooltipHost, HoverTooltipView, InteractiveColors, PopupAction, PopupEntrySpec, PopupLayer,
    PopupLayerSpec, RippleLayer, RippleLayerView, RowView, ScrollAxis, ScrollBarStyle,
    ScrollBarVisibility, ScrollView, ScrollViewWidget, SlideTransition, TooltipContentSpec,
    UiClassExt, UiScrollBar, UiTransition, UncheckedBounded, UncheckedBoundedX, UncheckedBoundedY,
    UnminAxis, WithClass, assume_bounded, assume_bounded_x, assume_bounded_y, blend_over, bounded,
    bounded_x, bounded_y, column, container, escape_catcher, focus_frame, icons,
    resolve_interactive_colors, ripple_layer, row, scroll_view, ui_icon, ui_transition,
    with_opacity,
};
pub use brocado_core::style_engine::{
    MemoizeWithSpec, build_default_properties, cached_default_properties, classes, current_spec,
    memoize_with_spec, with_spec,
};
pub use brocado_core::tokens::{
    Brightness, ControlBorderStyle, ControlMetrics, DesignSpec, DialogActionOrder, ElevationLevel,
    InputFocusStyle, MotionDuration, MotionEasing, NavIndicatorStyle, SemanticPalette,
    ShortcutConvention, StateFeedbackMode, TabIndicatorStyle, TextStyleSpec, TypeRole, TypeScale,
    WindowBackdrop, WindowSizeClass,
};
pub use color::{ColorScheme, ContrastLevel, contrast_ratio, relative_luminance};
pub use components::{
    FabColor, FabSize, TopAppBarVariant, bottom_app_bar, center_aligned_top_app_bar, extended_fab,
    fab, floating_toolbar, large_fab, large_top_app_bar, medium_top_app_bar,
    navigation_rail_with_fab, sized_fab, small_fab, top_app_bar, top_app_bar_slots,
};
pub use theme::{Md3Theme, md3_scope};
