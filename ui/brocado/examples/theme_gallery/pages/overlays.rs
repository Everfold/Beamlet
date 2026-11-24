//! Overlays, tooltips, toasts, and date/time pickers page.

use brocado::prelude::*;
use masonry::layout::AsUnit;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, flex_col, flex_row};

use crate::state::GalleryState;

pub fn overlays_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let basic_btn = |lbl| {
        primary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_basic_dialog = true;
        })
    };
    let native_alert_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_platform_alert = true;
        })
    };
    let delete_btn = |lbl| {
        destructive_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_destructive_dialog = true;
        })
        .leading_icon(icons::DELETE)
    };
    let bottom_sheet_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_bottom_sheet = true;
        })
    };
    let side_sheet_btn = |lbl| {
        secondary_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_side_sheet = true;
        })
    };
    let date_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_date_dialog = true;
        })
        .leading_icon(icons::CALENDAR)
    };
    let time_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            s.overlays.show_time_dialog = true;
        })
        .leading_icon(icons::SCHEDULE)
    };
    let toast_btn = |lbl| {
        outlined_button(lbl, |s: &mut GalleryState| {
            let theme_name = s.theme.language.label();
            s.overlays.toasts.show(
                format!("主题：{theme_name}"),
                Some("关闭"),
                ToastDuration::Short,
                true,
            );
        })
    };
    let plain_tip_btn = || {
        plain_tooltip(
            "普通提示",
            subtle_button("悬停提示", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
        )
    };
    let rich_tip_btn = || {
        rich_tooltip(
            Some("富提示"),
            "移出自动关闭。",
            subtle_button("富提示", |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
        )
    };

    let popover_btn = || {
        popover_anchor(
            outlined_button(
                if state.overlays.popover_open {
                    "关闭气泡"
                } else {
                    "交互气泡"
                },
                |s: &mut GalleryState| {
                    s.overlays.popover_open = !s.overlays.popover_open;
                },
            ),
            |s: &mut GalleryState, rect| {
                if let Some(r) = rect {
                    s.overlays.popover_anchor = Some(r);
                } else {
                    s.overlays.popover_open = false;
                    s.overlays.popover_anchor = None;
                }
            },
        )
    };

    let wide_triggers = wrap((
        basic_btn("基础弹窗"),
        native_alert_btn("原生警告"),
        delete_btn("危险确认"),
        bottom_sheet_btn("底抽屉"),
        side_sheet_btn("侧抽屉"),
        date_btn("日期弹窗"),
        time_btn("时间弹窗"),
        toast_btn("轻提示"),
        popover_btn(),
        plain_tip_btn(),
        rich_tip_btn(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(10.px())
    .run_gap(10.px());

    let narrow_triggers = wrap((
        basic_btn("基础弹窗"),
        native_alert_btn("原生警告"),
        delete_btn("危险确认"),
        bottom_sheet_btn("底抽屉"),
        side_sheet_btn("侧抽屉"),
        toast_btn("轻提示"),
        date_btn("日期弹窗"),
        time_btn("时间弹窗"),
        popover_btn(),
        plain_tip_btn(),
        rich_tip_btn(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(6.px())
    .run_gap(6.px());

    let ctx_status = state
        .overlays
        .context_action
        .as_deref()
        .unwrap_or("右键点击此区域（或按 Shift+F10）打开上下文菜单");
    let ctx_card = context_menu(
        filled_card(
            flex_col((title_medium("ContextMenu 右键菜单"), body_small(ctx_status)))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(6.px()),
        ),
        vec![
            menu_item("标记书签").leading_icon(icons::BOOKMARK),
            menu_item("重命名").leading_icon(icons::EDIT),
            menu_divider(),
            menu_item("移除").leading_icon(icons::DELETE),
        ],
        |s: &mut GalleryState, idx| {
            let label = match idx {
                0 => "上次操作：标记书签",
                1 => "上次操作：重命名",
                _ => "上次操作：移除",
            };
            s.overlays.context_action = Some(label.to_owned());
            s.components.click_count += 1;
        },
    );

    let trigger_row = elevated_card(
        flex_col((
            title_medium("弹窗、抽屉与提示"),
            breakpoint_bin(520.0, wide_triggers, narrow_triggers),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    let date_card = |d: CalendarDate| {
        outlined_card(
            flex_col((
                title_medium(d.format_iso()),
                date_picker(d, |s: &mut GalleryState, next| {
                    s.overlays.selected_date = next;
                }),
            ))
            .gap(8.px()),
        )
    };
    let time_card = |t: TimeOfDay| {
        outlined_card(
            flex_col((
                title_medium(t.format_24h()),
                time_picker(t, |s: &mut GalleryState, next| {
                    s.overlays.selected_time = next;
                }),
            ))
            .gap(8.px()),
        )
    };

    let pickers_row = breakpoint_bin(
        640.0,
        flex_row((
            date_card(state.overlays.selected_date),
            time_card(state.overlays.selected_time),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(16.px()),
        flex_col((
            date_card(state.overlays.selected_date),
            time_card(state.overlays.selected_time),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(12.px()),
    );

    flex_col((trigger_row, ctx_card, pickers_row))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
