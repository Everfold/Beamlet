//! Spec-driven cross-theme component suite consuming ambient [`DesignSpec`](crate::tokens::DesignSpec).

pub mod badge;
pub mod button;
pub mod card;
pub mod chip;
pub mod date_time;
pub mod dialog;
pub mod divider;
pub mod icon_button;
pub mod list;
pub mod menu;
pub mod navigation;
pub mod progress;
pub mod range_slider;
pub mod scaffold;
pub mod search_bar;
pub mod segmented_button;
pub mod selection;
pub mod sheet;
pub mod split;
pub mod tabs;
pub mod text_field;
pub mod toast;
pub mod tooltip;
pub mod typography;

pub use badge::{large_badge, small_badge, with_badge};
pub use button::{
    ButtonVariant, UiButton, UiCustomButton, button_view, destructive_button, elevated_button,
    outlined_button, primary_button, secondary_button, subtle_button,
};
pub use card::{
    CardVariant, card, elevated_card, filled_card, grouped_card, outlined_card, surface_box,
};
pub use chip::{assist_chip, filter_chip, input_chip, suggestion_chip};
pub use date_time::{
    CalendarDate, TimeOfDay, date_picker, date_picker_dialog, time_picker, time_picker_dialog,
};
pub use dialog::{basic_dialog, dialog};
pub use divider::{horizontal_divider, inset_divider, vertical_divider};
pub use icon_button::{
    IconButtonVariant, UiCustomIconButton, UiIconButton, filled_icon_button, icon_button,
    icon_button_view, outlined_icon_button, tonal_icon_button,
};
pub use list::{
    UiListItem, clickable_list_item, clickable_list_item_view, list_item, list_item_view,
    virtual_list,
};
pub use menu::{MenuEntry, dropdown_menu, menu_divider, menu_item, select};
pub use navigation::{
    NavDestination, modal_navigation_drawer, navigation_bar, navigation_drawer, navigation_rail,
    top_bar,
};
pub use progress::{
    DeterminateCircularWidget, LinearProgressWidget, circular_progress,
    determinate_circular_progress, linear_progress, small_circular_progress,
};
pub use range_slider::{RangeSlider, RangeSliderWidget, range_slider};
pub use scaffold::scaffold;
pub use search_bar::{UiSearchBar, search_bar};
pub use segmented_button::{multi_choice_segmented_button, segmented_button};
pub use selection::{
    ToggleableState, checkbox, radio, radio_group, slider, switch, tri_state_checkbox,
};
pub use sheet::{bottom_sheet, side_sheet};
pub use split::{UiSplit, split_view};
pub use tabs::{scrollable_tabs, secondary_tabs, tabs};
pub use text_field::{
    TextFieldVariant, UiTextArea, UiTextField, filled_text_area, filled_text_field, text_area,
    text_field,
};
pub use toast::{ToastData, ToastDuration, ToastHostState, toast, toast_host};
pub use tooltip::{plain_tooltip, rich_tooltip};
pub use typography::{
    body_large, body_medium, body_small, display_large, display_medium, display_small,
    headline_large, headline_medium, headline_small, label_large, label_medium, label_small,
    title_large, title_medium, title_small, ui_label, ui_prose,
};
pub use xilem::InsertNewline;
