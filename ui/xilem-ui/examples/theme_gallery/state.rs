//! Domain-split application state for `theme_gallery`.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use xilem::WindowId;
use xilem_ui::prelude::*;

/// Top-level showcase navigation destinations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GalleryTab {
    #[default]
    Home,
    Layout,
    Components,
    Overlays,
    PlatformNative,
    PerfAndSpec,
    Workbench,
}

impl GalleryTab {
    pub const ALL: [Self; 7] = [
        Self::Home,
        Self::Layout,
        Self::Components,
        Self::Overlays,
        Self::PlatformNative,
        Self::PerfAndSpec,
        Self::Workbench,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Layout => 1,
            Self::Components => 2,
            Self::Overlays => 3,
            Self::PlatformNative => 4,
            Self::PerfAndSpec => 5,
            Self::Workbench => 6,
        }
    }

    #[must_use]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Home,
            1 => Self::Layout,
            2 => Self::Components,
            3 => Self::Overlays,
            4 => Self::PlatformNative,
            5 => Self::PerfAndSpec,
            _ => Self::Workbench,
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Home => "首页",
            Self::Layout => "布局系统",
            Self::Components => "组件与输入",
            Self::Overlays => "浮层与反馈",
            Self::PlatformNative => "平台原生",
            Self::PerfAndSpec => "性能与令牌",
            Self::Workbench => "工作台",
        }
    }

    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Home => icons::HOME,
            Self::Layout => icons::DOCK_LEFT,
            Self::Components => icons::EDIT,
            Self::Overlays => icons::MORE_HORIZ,
            Self::PlatformNative => icons::PALETTE,
            Self::PerfAndSpec => icons::TUNE,
            Self::Workbench => icons::FOLDER,
        }
    }
}

/// Interactive controls for the Layout System showcase page.
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

/// Interactive controls for cross-theme inputs, buttons, tabs, and selections.
#[derive(Clone, Debug)]
pub struct ComponentsState {
    pub click_count: u32,
    pub text_value: String,
    pub email_value: String,
    pub search_query: String,
    pub agree_terms: bool,
    pub tri_state: ToggleableState,
    pub notifications_on: bool,
    pub radio_choice: usize,
    pub slider_val: f64,
    pub range_val: (f64, f64),
    pub chip_flags: [bool; 3],
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
            text_value: "Beamlet 多主题界面".to_owned(),
            email_value: "invalid-email".to_owned(),
            search_query: String::new(),
            agree_terms: true,
            tri_state: ToggleableState::Indeterminate,
            notifications_on: true,
            radio_choice: 0,
            slider_val: 0.42,
            range_val: (25.0, 75.0),
            chip_flags: [true, false, true],
            multi_seg_mask: vec![true, false, true],
            input_tags: vec![
                "MD3".to_owned(),
                "WinUI3".to_owned(),
                "Aqua".to_owned(),
                "Adwaita".to_owned(),
            ],
            selected_tab: 0,
            secondary_tab: 0,
            select_open: false,
            select_index: 1,
        }
    }
}

/// Modal dialogs, drawers, pickers, and toast queue state.
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
        }
    }
}

/// Per-platform native widget state (Fluent 2, macOS HIG, GNOME Adwaita).
#[derive(Clone, Debug)]
pub struct PlatformNativeState {
    pub nav_collapsed: bool,
    pub info_bar_visible: bool,
    pub expander_open: bool,
    pub fluent_expander_open: bool,
    pub fluent_breadcrumb_depth: usize,
    pub fluent_number_val: f64,
    pub cmd_overflow_open: bool,
    pub show_hig_inspector: bool,
    pub hig_disclosure_open: bool,
    pub hig_stepper_val: i32,
    pub hig_path_depth: usize,
    pub hig_popup_open: bool,
    pub hig_popup_idx: usize,
    pub banner_visible: bool,
    pub native_switch_a: bool,
    pub native_switch_b: bool,
    pub adw_expander_open: bool,
    pub adw_combo_open: bool,
    pub adw_combo_idx: usize,
    pub adw_entry_val: String,
    pub adw_switcher_idx: usize,
}

impl Default for PlatformNativeState {
    fn default() -> Self {
        Self {
            nav_collapsed: false,
            info_bar_visible: true,
            expander_open: true,
            fluent_expander_open: true,
            fluent_breadcrumb_depth: 2,
            fluent_number_val: 24.0,
            cmd_overflow_open: false,
            show_hig_inspector: true,
            hig_disclosure_open: true,
            hig_stepper_val: 8,
            hig_path_depth: 2,
            hig_popup_open: false,
            hig_popup_idx: 0,
            banner_visible: true,
            native_switch_a: true,
            native_switch_b: false,
            adw_expander_open: true,
            adw_combo_open: false,
            adw_combo_idx: 0,
            adw_entry_val: "beamlet-workspace".to_owned(),
            adw_switcher_idx: 0,
        }
    }
}

/// Incremental memoization and 10,000-row virtual scrolling lab state.
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

/// Multi-pane desktop workbench sample state.
#[derive(Clone, Debug)]
pub struct WorkbenchState {
    pub selected_idx: usize,
    pub items: Vec<(&'static str, &'static str, &'static str)>,
    pub search_query: String,
    pub show_detail_pane: bool,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            selected_idx: 0,
            items: vec![
                (
                    "核心设计引擎",
                    "FocusFrame 与浅层 PropertyStack 默认样式表",
                    "运行中",
                ),
                (
                    "增量与虚拟化管道",
                    "memoize_with_spec 边界与万级 virtual_list",
                    "已就绪",
                ),
                (
                    "Windows Mica 外壳",
                    "可折叠 NavigationView + BreadcrumbBar + BeveledBorder",
                    "已同步",
                ),
                (
                    "macOS Aqua 检查器",
                    "Source List 侧栏 + 38px 工具栏 + PopUpButton",
                    "就绪",
                ),
                (
                    "GNOME Adwaita 分栏",
                    "AdwBreakpointBin + AdwClamp + ViewSwitcherBar",
                    "已更新",
                ),
            ],
            search_query: String::new(),
            show_detail_pane: true,
        }
    }
}

/// Root mutable showcase state composed of domain sub-structs.
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
    pub workbench: WorkbenchState,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            window_id: WindowId::next(),
            running: true,
            theme: UiTheme::native_light(),
            active_tab: GalleryTab::Home,
            accent_menu_open: false,
            layout: LayoutLabState::default(),
            components: ComponentsState::default(),
            overlays: OverlaysState::default(),
            native: PlatformNativeState::default(),
            perf: PerfLabState::default(),
            workbench: WorkbenchState::default(),
        }
    }
}
