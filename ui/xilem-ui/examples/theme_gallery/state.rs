//! Mutable showcase application state for `theme_gallery`.

use xilem::WindowId;
use xilem_ui::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GalleryTab {
    #[default]
    Overview,
    Inputs,
    Overlays,
    Tokens,
    PlatformNative,
    Workspace,
}

impl GalleryTab {
    pub const fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Inputs => 1,
            Self::Overlays => 2,
            Self::Tokens => 3,
            Self::PlatformNative => 4,
            Self::Workspace => 5,
        }
    }

    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Overview,
            1 => Self::Inputs,
            2 => Self::Overlays,
            3 => Self::Tokens,
            4 => Self::PlatformNative,
            _ => Self::Workspace,
        }
    }
}

pub struct GalleryState {
    pub window_id: WindowId,
    pub running: bool,
    pub theme: UiTheme,
    pub active_tab: GalleryTab,
    pub accent_menu_open: bool,

    // Inputs & selection
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

    // Overlays & pickers
    pub selected_tab: usize,
    pub secondary_tab: usize,
    pub select_open: bool,
    pub select_index: usize,
    pub selected_date: CalendarDate,
    pub selected_time: TimeOfDay,
    pub show_basic_dialog: bool,
    pub show_destructive_dialog: bool,
    pub show_bottom_sheet: bool,
    pub show_side_sheet: bool,
    pub show_date_dialog: bool,
    pub show_time_dialog: bool,
    pub toasts: ToastHostState,

    // Platform native state
    pub info_bar_visible: bool,
    pub expander_open: bool,
    pub fluent_expander_open: bool,
    pub fluent_breadcrumb_depth: usize,
    pub fluent_number_val: f64,
    pub show_hig_inspector: bool,
    pub hig_disclosure_open: bool,
    pub hig_stepper_val: i32,
    pub hig_path_depth: usize,
    pub banner_visible: bool,
    pub native_switch_a: bool,
    pub native_switch_b: bool,
    pub adw_expander_open: bool,
    pub adw_combo_open: bool,
    pub adw_combo_idx: usize,
    pub adw_entry_val: String,

    // Workbench list state
    pub selected_workspace_idx: usize,
    pub workspace_items: Vec<(&'static str, &'static str, &'static str)>,
    pub click_count: u32,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            window_id: WindowId::next(),
            running: true,
            theme: UiTheme::native_light(),
            active_tab: GalleryTab::Overview,
            accent_menu_open: false,
            text_value: "Beamlet 多主题界面".to_string(),
            email_value: "invalid-email".to_string(),
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
                "MD3".into(),
                "WinUI3".into(),
                "Aqua".into(),
                "Adwaita".into(),
            ],
            selected_tab: 0,
            secondary_tab: 0,
            select_open: false,
            select_index: 1,
            selected_date: CalendarDate::new(2026, 11, 22),
            selected_time: TimeOfDay::new(14, 30),
            show_basic_dialog: false,
            show_destructive_dialog: false,
            show_bottom_sheet: false,
            show_side_sheet: false,
            show_date_dialog: false,
            show_time_dialog: false,
            toasts: ToastHostState::new(),
            info_bar_visible: true,
            expander_open: true,
            fluent_expander_open: true,
            fluent_breadcrumb_depth: 2,
            fluent_number_val: 24.0,
            show_hig_inspector: true,
            hig_disclosure_open: true,
            hig_stepper_val: 8,
            hig_path_depth: 2,
            banner_visible: true,
            native_switch_a: true,
            native_switch_b: false,
            adw_expander_open: true,
            adw_combo_open: false,
            adw_combo_idx: 0,
            adw_entry_val: "beamlet-workspace".to_string(),
            selected_workspace_idx: 0,
            workspace_items: vec![
                (
                    "核心设计引擎",
                    "FocusFrame 与 PropertyStack 默认样式表",
                    "运行中",
                ),
                (
                    "Windows Mica 外壳",
                    "NavigationView + BreadcrumbBar 导航栏",
                    "已同步",
                ),
                (
                    "macOS Aqua 检查器",
                    "Source List 侧栏 + 38px 工具栏 + 路径栏",
                    "就绪",
                ),
                (
                    "GNOME Adwaita 盒式布局",
                    "AdwClamp + HeaderBar + EntryRow",
                    "已更新",
                ),
            ],
            click_count: 0,
        }
    }
}
