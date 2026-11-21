//! Mutable showcase application state for `theme_gallery`.

use xilem::WindowId;
use xilem_ui::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GalleryTab {
    #[default]
    Overview,
    Inputs,
    Overlays,
    PlatformNative,
}

impl GalleryTab {
    pub const fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Inputs => 1,
            Self::Overlays => 2,
            Self::PlatformNative => 3,
        }
    }

    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Overview,
            1 => Self::Inputs,
            2 => Self::Overlays,
            _ => Self::PlatformNative,
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

    // Overlays & pickers
    pub selected_tab: usize,
    pub select_open: bool,
    pub select_index: usize,
    pub selected_date: CalendarDate,
    pub selected_time: TimeOfDay,
    pub show_basic_dialog: bool,
    pub show_bottom_sheet: bool,
    pub toasts: ToastHostState,

    // Platform native state
    pub info_bar_visible: bool,
    pub expander_open: bool,
    pub banner_visible: bool,
    pub native_switch_a: bool,
    pub native_switch_b: bool,
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
            text_value: "Multi-theme Xilem UI".to_string(),
            email_value: "invalid-email".to_string(),
            search_query: String::new(),
            agree_terms: true,
            tri_state: ToggleableState::Indeterminate,
            notifications_on: true,
            radio_choice: 0,
            slider_val: 0.42,
            range_val: (25.0, 75.0),
            chip_flags: [true, false, true],
            selected_tab: 0,
            select_open: false,
            select_index: 1,
            selected_date: CalendarDate::new(2026, 11, 22),
            selected_time: TimeOfDay::new(14, 30),
            show_basic_dialog: false,
            show_bottom_sheet: false,
            toasts: ToastHostState::new(),
            info_bar_visible: true,
            expander_open: true,
            banner_visible: true,
            native_switch_a: true,
            native_switch_b: false,
            click_count: 0,
        }
    }
}
