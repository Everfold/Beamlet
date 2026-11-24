//! macOS Apple Human Interface Guidelines (Aqua Light & Dark) theme backend and native components.

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]

pub mod components;
pub mod palette;
pub mod theme;

pub use brocado_core::components::*;
pub use brocado_core::foundation::{
    AnchoredPopupHost, AnchoredPopupView, Bounded, BoundedAnyView, BoundedViewExt, BoundedX,
    BoundedXAnyView, BoundedXSeq, BoundedY, BoundedYSeq, ColumnView, ContainerView, EscapeCatcher,
    EscapeCatcherView, EscapePressed, FitAxis, FocusFrame, FocusFrameView, HasMinAxis,
    HoverTooltipHost, HoverTooltipView, InteractiveColors, PopupAction, PopupEntrySpec, PopupLayer,
    PopupLayerSpec, RowView, ScrollAxis, ScrollBarStyle, ScrollBarVisibility, ScrollView,
    ScrollViewWidget, SlideTransition, TooltipContentSpec, UiClassExt, UiScrollBar, UiTransition,
    UncheckedBounded, UncheckedBoundedX, UncheckedBoundedY, UnminAxis, WithClass, assume_bounded,
    assume_bounded_x, assume_bounded_y, blend_over, bounded, bounded_x, bounded_y, column,
    container, escape_catcher, focus_frame, icons, resolve_interactive_colors, row,
    scroll_horizontal, scroll_vertical, scroll_view, ui_icon, ui_transition, with_opacity,
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
pub use components::{
    MacosAlertDialog, disclosure_group, form_group, form_row, help_button, inspector_panel,
    inspector_panel_with_width, macos_alert_dialog, macos_toolbar, macos_toolbar_with_center,
    path_control, popup_button, source_list_sidebar, stepper,
};
pub use palette::{HigAccent, hig_palette};
pub use theme::{HigTheme, hig_scope};
