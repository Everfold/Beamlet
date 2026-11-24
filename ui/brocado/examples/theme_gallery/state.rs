//! Lean showcase application state for `theme_gallery`.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use brocado::prelude::*;

/// Showcase navigation tabs (5 focused views).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GalleryTab {
    #[default]
    Components,
    Layout,
    Overlays,
    Native,
    Lab,
}

impl GalleryTab {
    pub const ALL: [Self; 5] = [
        Self::Components,
        Self::Layout,
        Self::Overlays,
        Self::Native,
        Self::Lab,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Components => 0,
            Self::Layout => 1,
            Self::Overlays => 2,
            Self::Native => 3,
            Self::Lab => 4,
        }
    }

    #[must_use]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Components,
            1 => Self::Layout,
            2 => Self::Overlays,
            3 => Self::Native,
            _ => Self::Lab,
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Components => "组件",
            Self::Layout => "布局",
            Self::Overlays => "浮层",
            Self::Native => "平台",
            Self::Lab => "性能",
        }
    }

    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Components => icons::EDIT,
            Self::Layout => icons::DOCK_LEFT,
            Self::Overlays => icons::MORE_HORIZ,
            Self::Native => icons::PALETTE,
            Self::Lab => icons::TUNE,
        }
    }
}

/// Interactive controls for the Layout page.
#[derive(Clone, Debug)]
pub struct LayoutLabState {
    pub spacing_idx: usize,
    pub align_idx: usize,
    pub clamp_idx: usize,
    pub simulate_compact: bool,
}

impl Default for LayoutLabState {
    fn default() -> Self {
        Self {
            spacing_idx: 2,
            align_idx: 2,
            clamp_idx: 1,
            simulate_compact: false,
        }
    }
}

/// Cross-theme components and inputs state.
#[derive(Clone, Debug)]
pub struct ComponentsState {
    pub click_count: u32,
    pub text_value: String,
    pub email_value: String,
    pub notes_value: String,
    pub last_link_clicked: Option<String>,
    pub search_query: String,
    pub agree_terms: bool,
    pub tri_state: ToggleableState,
    pub notifications_on: bool,
    pub radio_choice: usize,
    pub slider_val: f64,
    pub range_val: (f64, f64),
    pub chip_flags: [bool; 2],
    pub multi_seg_mask: Vec<bool>,
    pub input_tags: Vec<String>,
    pub selected_tab: usize,
    pub secondary_tab: usize,
    pub select_open: bool,
    pub select_index: usize,
}

impl Default for ComponentsState {
    fn default() -> Self {
        Self {
            click_count: 0,
            text_value: "Beamlet UI".to_owned(),
            email_value: "invalid-email".to_owned(),
            notes_value: "支持多行输入、Shift+Enter / Enter 策略以及垂直滚动。".to_owned(),
            last_link_clicked: None,
            search_query: String::new(),
            agree_terms: true,
            tri_state: ToggleableState::Indeterminate,
            notifications_on: true,
            radio_choice: 0,
            slider_val: 0.42,
            range_val: (25.0, 75.0),
            chip_flags: [true, false],
            multi_seg_mask: vec![true, false, true],
            input_tags: vec![
                "MD3".into(),
                "Expressive".into(),
                "OKLCH".into(),
                "Spring".into(),
            ],
            selected_tab: 0,
            secondary_tab: 0,
            select_open: false,
            select_index: 1,
        }
    }
}

/// Overlays and picker state.
pub struct OverlaysState {
    pub show_basic_dialog: bool,
    pub show_destructive_dialog: bool,
    pub show_platform_alert: bool,
    pub show_bottom_sheet: bool,
    pub show_side_sheet: bool,
    pub show_date_dialog: bool,
    pub show_time_dialog: bool,
    pub selected_date: CalendarDate,
    pub selected_time: TimeOfDay,
    pub toasts: ToastHostState,
    pub context_action: Option<String>,
    pub popover_open: bool,
    pub popover_anchor: Option<PopoverAnchorRect>,
    pub popover_compact: bool,
}

impl Default for OverlaysState {
    fn default() -> Self {
        Self {
            show_basic_dialog: false,
            show_destructive_dialog: false,
            show_platform_alert: false,
            show_bottom_sheet: false,
            show_side_sheet: false,
            show_date_dialog: false,
            show_time_dialog: false,
            selected_date: CalendarDate::new(2026, 11, 23),
            selected_time: TimeOfDay::new(16, 0),
            toasts: ToastHostState::new(),
            context_action: None,
            popover_open: false,
            popover_anchor: None,
            popover_compact: false,
        }
    }
}

/// Active Material 3 Expressive showcase state.
#[derive(Clone, Debug)]
pub struct PlatformNativeState {
    pub md3_group_selected: usize,
    pub md3_fab_menu_open: bool,
    pub md3_last_action: String,
}

impl Default for PlatformNativeState {
    fn default() -> Self {
        Self {
            md3_group_selected: 0,
            md3_fab_menu_open: false,
            md3_last_action: "等待操作".to_owned(),
        }
    }
}

/// Incremental memoization and 10,000-row virtual list lab state.
#[derive(Clone, Debug)]
pub struct PerfLabState {
    pub unrelated_ticks: u32,
    pub memo_tag: String,
    pub unmemoized_builds: Arc<AtomicUsize>,
    pub memoized_builds: Arc<AtomicUsize>,
    pub virtual_jump_idx: Option<usize>,
    pub selected_virtual_row: Option<usize>,
}

impl Default for PerfLabState {
    fn default() -> Self {
        Self {
            unrelated_ticks: 0,
            memo_tag: "Alpha".to_owned(),
            unmemoized_builds: Arc::new(AtomicUsize::new(0)),
            memoized_builds: Arc::new(AtomicUsize::new(0)),
            virtual_jump_idx: None,
            selected_virtual_row: None,
        }
    }
}

/// Root application state for `theme_gallery`.
pub struct GalleryState {
    pub window_id: WindowId,
    pub running: bool,
    pub theme: UiTheme,
    pub active_tab: GalleryTab,
    pub accent_menu_open: bool,
    pub layout: LayoutLabState,
    pub components: ComponentsState,
    pub overlays: OverlaysState,
    pub native: PlatformNativeState,
    pub perf: PerfLabState,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            window_id: WindowId::next(),
            running: true,
            theme: UiTheme::native_light(),
            active_tab: GalleryTab::Components,
            accent_menu_open: false,
            layout: LayoutLabState::default(),
            components: ComponentsState::default(),
            overlays: OverlaysState::default(),
            native: PlatformNativeState::default(),
            perf: PerfLabState::default(),
        }
    }
}
