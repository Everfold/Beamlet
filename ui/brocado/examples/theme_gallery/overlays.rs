//! Root overlay compositor (`ZStack`) for dialogs, sheets, pickers, and toasts.

use brocado::prelude::*;
use masonry::layout::{AsUnit, UnitPoint};
use masonry::properties::Padding;
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, ZStackExt, flex_col, sized_box, zstack};

use crate::state::GalleryState;

pub fn wrap_with_overlays<V: WidgetView<GalleryState>>(
    state: &mut GalleryState,
    workspace: V,
) -> impl WidgetView<GalleryState, Widget = widgets::ZStack> + use<V> {
    let dialog_overlay = state.overlays.show_basic_dialog.then(|| {
        basic_dialog(
            "重置主题设置？",
            "恢复默认种子色与明暗模式。",
            "恢复",
            |s: &mut GalleryState| {
                s.theme = Theme::light();
                s.overlays.show_basic_dialog = false;
            },
            |s: &mut GalleryState| s.overlays.show_basic_dialog = false,
        )
        .icon(icons::PALETTE)
        .dismiss_text("取消")
    });

    let destructive_overlay = state.overlays.show_destructive_dialog.then(|| {
        basic_dialog(
            "确认删除？",
            "此操作不可撤销。",
            "删除",
            |s: &mut GalleryState| {
                s.components.click_count += 1;
                s.overlays.show_destructive_dialog = false;
            },
            |s: &mut GalleryState| s.overlays.show_destructive_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("取消")
    });

    let platform_alert_overlay: Option<Box<AnyWidgetView<GalleryState>>> =
        state.overlays.show_platform_alert.then(|| {
            basic_dialog(
                "确认提示",
                "遵循 Material 3 Expressive 动作顺序。",
                "确定",
                |s: &mut GalleryState| {
                    s.components.click_count += 1;
                    s.overlays.show_platform_alert = false;
                },
                |s: &mut GalleryState| s.overlays.show_platform_alert = false,
            )
            .icon(icons::INFO)
            .dismiss_text("取消")
            .boxed()
        });

    let date_overlay = state.overlays.show_date_dialog.then(|| {
        date_picker_dialog(
            state.overlays.selected_date,
            |s: &mut GalleryState, d| s.overlays.selected_date = d,
            |s: &mut GalleryState| s.overlays.show_date_dialog = false,
            |s: &mut GalleryState| s.overlays.show_date_dialog = false,
        )
    });

    let time_overlay = state.overlays.show_time_dialog.then(|| {
        time_picker_dialog(
            state.overlays.selected_time,
            |s: &mut GalleryState, t| s.overlays.selected_time = t,
            |s: &mut GalleryState| s.overlays.show_time_dialog = false,
            |s: &mut GalleryState| s.overlays.show_time_dialog = false,
        )
    });

    let sheet_overlay = state.overlays.show_bottom_sheet.then(|| {
        bottom_sheet(
            flex_col((
                title_medium("底部抽屉"),
                primary_button("关闭", |s: &mut GalleryState| {
                    s.overlays.show_bottom_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.overlays.show_bottom_sheet = false,
        )
    });

    let side_sheet_overlay = state.overlays.show_side_sheet.then(|| {
        side_sheet(
            flex_col((
                title_medium("侧边抽屉"),
                body_medium(format!("点击计数：{}", state.components.click_count)),
                primary_button("关闭", |s: &mut GalleryState| {
                    s.overlays.show_side_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.overlays.show_side_sheet = false,
        )
        .extent(320.0)
    });

    let toast_layer = sized_box(
        toast_host(&state.overlays.toasts, |s: &mut GalleryState| {
            let _ = s.overlays.toasts.dismiss_current();
        })
        .on_action(|s: &mut GalleryState| {
            let _ = s.overlays.toasts.dismiss_current();
        }),
    )
    .padding(Padding::all(8.px()))
    .alignment(UnitPoint::BOTTOM);

    let popover_overlay = (state.overlays.popover_open)
        .then_some(state.overlays.popover_anchor)
        .flatten()
        .map(|rect| {
            popover_surface(
                rect,
                flex_col((
                    title_small("视图过滤设置"),
                    checkbox(
                        "紧凑密度",
                        state.overlays.popover_compact,
                        |s: &mut GalleryState, v| {
                            s.overlays.popover_compact = v;
                        },
                    ),
                    wrap((
                        primary_button("应用", |s: &mut GalleryState| {
                            s.components.click_count += 1;
                            s.overlays.popover_open = false;
                        }),
                        outlined_button("关闭", |s: &mut GalleryState| {
                            s.overlays.popover_open = false;
                        }),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px())
                    .run_gap(6.px()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .gap(10.px()),
                |s: &mut GalleryState| {
                    s.overlays.popover_open = false;
                },
            )
        });

    zstack((
        workspace.alignment(UnitPoint::TOP_LEFT),
        toast_layer,
        popover_overlay.map(|p| p.alignment(UnitPoint::TOP_LEFT)),
        sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        side_sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        date_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        time_overlay.map(|t| t.alignment(UnitPoint::CENTER)),
        dialog_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        destructive_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        platform_alert_overlay.map(|a| a.alignment(UnitPoint::CENTER)),
    ))
}
