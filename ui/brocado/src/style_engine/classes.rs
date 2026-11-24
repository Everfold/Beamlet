//! Canonical semantic Masonry style classes mapped by `style_engine::builder`.

/// Primary filled/emphasized action button.
pub const BTN_PRIMARY: &str = "ui.btn.primary";
/// Secondary tonal action button.
pub const BTN_SECONDARY: &str = "ui.btn.secondary";
/// Elevated action button.
pub const BTN_ELEVATED: &str = "ui.btn.elevated";
/// Outlined action button.
pub const BTN_OUTLINED: &str = "ui.btn.outlined";
/// Borderless subtle/text action button.
pub const BTN_SUBTLE: &str = "ui.btn.subtle";
/// High-emphasis destructive / danger confirmation action button.
pub const BTN_DESTRUCTIVE: &str = "ui.btn.destructive";

/// Standard transparent icon button (unselected).
pub const ICON_BTN_STD: &str = "ui.icon_btn.std";
/// Standard icon button (selected).
pub const ICON_BTN_STD_SELECTED: &str = "ui.icon_btn.std.selected";
/// Filled accent icon button (selected).
pub const ICON_BTN_FILLED: &str = "ui.icon_btn.filled";
/// Filled icon button (unselected).
pub const ICON_BTN_FILLED_UNSELECTED: &str = "ui.icon_btn.filled.unselected";
/// Tonal icon button (selected).
pub const ICON_BTN_TONAL: &str = "ui.icon_btn.tonal";
/// Tonal icon button (unselected).
pub const ICON_BTN_TONAL_UNSELECTED: &str = "ui.icon_btn.tonal.unselected";
/// Outlined icon button (unselected).
pub const ICON_BTN_OUTLINED: &str = "ui.icon_btn.outlined";
/// Outlined icon button (selected).
pub const ICON_BTN_OUTLINED_SELECTED: &str = "ui.icon_btn.outlined.selected";

/// Primary Floating Action Button surface (`56px`).
pub const FAB_PRIMARY: &str = "ui.fab.primary";
/// Surface Floating Action Button.
pub const FAB_SURFACE: &str = "ui.fab.surface";
/// Secondary Floating Action Button.
pub const FAB_SECONDARY: &str = "ui.fab.secondary";
/// Tertiary Floating Action Button.
pub const FAB_TERTIARY: &str = "ui.fab.tertiary";

/// Elevated container card.
pub const CARD_ELEVATED: &str = "ui.card.elevated";
/// Filled container card.
pub const CARD_FILLED: &str = "ui.card.filled";
/// Outlined container card.
pub const CARD_OUTLINED: &str = "ui.card.outlined";
/// Desktop grouped list container (macOS `Form` / Libadwaita `boxed-list` / WinUI `SettingsCard`).
pub const GROUPED_CARD: &str = "ui.card.grouped";
/// Modal dialog surface.
pub const DIALOG_SURFACE: &str = "ui.dialog.surface";
/// Bottom or side sheet surface.
pub const SHEET_SURFACE: &str = "ui.sheet.surface";
/// Anchored floating popover container surface.
pub const POPOVER_SURFACE: &str = "ui.popover.surface";

/// Assist chip.
pub const CHIP_ASSIST: &str = "ui.chip.assist";
/// Filter chip (unselected).
pub const CHIP_FILTER: &str = "ui.chip.filter";
/// Filter chip (selected).
pub const CHIP_FILTER_SELECTED: &str = "ui.chip.filter.selected";
/// Input tag chip.
pub const CHIP_INPUT: &str = "ui.chip.input";
/// Suggestion chip.
pub const CHIP_SUGGESTION: &str = "ui.chip.suggestion";
/// Chip row wrapper bounding container height (`chip_height`).
pub const CHIP_ROW: &str = "ui.chip.row";

/// Checked state marker for checkboxes, radios, and toggles.
pub const CHECKED: &str = "ui.checked";
/// Search bar container surface.
pub const SEARCH_BAR: &str = "ui.search_bar";
/// Filled text field input styling.
pub const TEXT_FIELD_FILLED: &str = "ui.tf.filled";
/// Outlined / desktop crisp text field input styling.
pub const TEXT_FIELD_OUTLINED: &str = "ui.tf.outlined";
/// Flush borderless inner text field (when wrapped in a custom outer chrome container).
pub const TEXT_FIELD_FLUSH: &str = "ui.tf.flush";
/// Error highlight override for text fields.
pub const TEXT_FIELD_ERROR: &str = "ui.tf.error";

/// Navigation destination button (unselected).
pub const NAV_PILL: &str = "ui.nav.pill";
/// Navigation destination button (active).
pub const NAV_PILL_ACTIVE: &str = "ui.nav.pill.active";
/// Segmented button item (unselected).
pub const SEG_BTN: &str = "ui.seg_btn";
/// Segmented button item (selected).
pub const SEG_BTN_SELECTED: &str = "ui.seg_btn.selected";

/// Small dot badge (`6×6`).
pub const BADGE_SMALL: &str = "ui.badge.small";
/// Numeric pill badge.
pub const BADGE_LARGE: &str = "ui.badge.large";

/// Clickable list item row (unselected).
pub const LIST_ITEM: &str = "ui.list_item";
/// Clickable list item row (selected).
pub const LIST_ITEM_SELECTED: &str = "ui.list_item.selected";
/// Popup menu row (unselected).
pub const MENU_ITEM: &str = "ui.menu_item";
/// Popup menu row (selected).
pub const MENU_ITEM_SELECTED: &str = "ui.menu_item.selected";
/// Tab strip button item.
pub const TAB_ITEM: &str = "ui.tab.item";
/// Inverse-surface toast/snackbar trailing text action button.
pub const TOAST_ACTION_BTN: &str = "ui.toast.action_btn";
/// Inverse-surface toast/snackbar icon close button.
pub const TOAST_CLOSE_BTN: &str = "ui.toast.close_btn";
/// Top window header bar / toolbar surface.
pub const HEADER_BAR: &str = "ui.header_bar";
/// Bottom app bar / footer bar surface.
pub const BOTTOM_BAR: &str = "ui.bottom_bar";
/// Modal scrim overlay button.
pub const SCRIM: &str = "ui.scrim";
/// Transparent click-away scrim button used for light-dismiss popovers.
pub const SCRIM_TRANSPARENT: &str = "ui.scrim.transparent";
