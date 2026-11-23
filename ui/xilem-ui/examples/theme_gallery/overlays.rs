//! Root overlay compositor stack (`ZStack`) for dialogs, platform alerts, sheets, pickers, and toasts.

use xilem::masonry::layout::{AsUnit, UnitPoint};
use xilem::masonry::properties::Padding;
use xilem::masonry::widgets;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, ZStackExt, flex_col, sized_box, zstack};
use xilem::{AnyWidgetView, WidgetView};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn wrap_with_overlays<V: WidgetView<GalleryState>>(
    state: &mut GalleryState,
    workspace: V,
    floating_action: Option<Box<AnyWidgetView<GalleryState>>>,
) -> impl WidgetView<GalleryState, Widget = widgets::ZStack> + use<V> {
    let dialog_overlay = state.overlays.show_basic_dialog.then(|| {
        basic_dialog(
            "重置主题偏好设置？",
            "将恢复宿主系统默认的设计语言与强调色预设。",
            "恢复默认",
            |s: &mut GalleryState| {
                s.theme = UiTheme::native_light();
                s.overlays.show_basic_dialog = false;
            },
            |s: &mut GalleryState| s.overlays.show_basic_dialog = false,
        )
        .icon(icons::PALETTE)
        .dismiss_text("取消")
    });

    let destructive_overlay = state.overlays.show_destructive_dialog.then(|| {
        basic_dialog(
            "移除选中的工作区目标？",
            "将从展示列表中移除当前选中的工作区目标条目。",
            "确认移除",
            |s: &mut GalleryState| {
                if !s.workbench.items.is_empty() {
                    let idx = s
                        .workbench
                        .selected_idx
                        .min(s.workbench.items.len().saturating_sub(1));
                    s.workbench.items.remove(idx);
                    s.workbench.selected_idx = 0;
                }
                s.overlays.show_destructive_dialog = false;
            },
            |s: &mut GalleryState| s.overlays.show_destructive_dialog = false,
        )
        .icon(icons::DELETE)
        .destructive(true)
        .dismiss_text("取消")
    });

    let platform_alert_overlay: Option<Box<AnyWidgetView<GalleryState>>> = state
        .overlays
        .show_platform_alert
        .then(|| match state.theme.language {
            DesignLanguage::Adwaita => alert_dialog(
                "AdwAlertDialog 原生警告框",
                "遵循 GNOME Libadwaita Pill 按钮与模态焦点陷阱规范。",
                "取消",
                "继续执行",
                |s: &mut GalleryState| {
                    s.components.click_count += 1;
                    s.overlays.show_platform_alert = false;
                },
                |s: &mut GalleryState| s.overlays.show_platform_alert = false,
            )
            .destructive(false)
            .boxed(),
            DesignLanguage::Hig => macos_alert_dialog(
                "NSAlert 原生警告框",
                "遵循 macOS Apple HIG 垂直居中图标、加粗标题与 Trailing 确认按钮排布。",
                "确定",
                "取消",
                |s: &mut GalleryState| {
                    s.components.click_count += 1;
                    s.overlays.show_platform_alert = false;
                },
                |s: &mut GalleryState| s.overlays.show_platform_alert = false,
            )
            .boxed(),
            DesignLanguage::Fluent | DesignLanguage::Md3 => basic_dialog(
                "平台自适应确认对话框",
                "Fluent 下自动采用 PrimaryLeading 主按钮居左，MD3 下遵循 PrimaryTrailing 规范。",
                "确认",
                |s: &mut GalleryState| {
                    s.components.click_count += 1;
                    s.overlays.show_platform_alert = false;
                },
                |s: &mut GalleryState| s.overlays.show_platform_alert = false,
            )
            .icon(icons::INFO)
            .dismiss_text("取消")
            .boxed(),
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
                headline_small("底部抽屉面板"),
                body_medium("通过 SlideTransition + EscapeCatcher 实现的共享模态底部抽屉。"),
                primary_button("关闭抽屉", |s: &mut GalleryState| {
                    s.overlays.show_bottom_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.overlays.show_bottom_sheet = false,
        )
    });

    let side_sheet_overlay = state.overlays.show_side_sheet.then(|| {
        let selected_title = state
            .workbench
            .items
            .get(state.workbench.selected_idx)
            .map_or("未选中任何项目", |(t, _, _)| *t);
        side_sheet(
            flex_col((
                headline_small("目标详情侧边抽屉"),
                title_medium(selected_title),
                body_medium("由 modal_shell + EscapeCatcher 驱动的右侧检查器抽屉。"),
                primary_button("关闭侧边抽屉", |s: &mut GalleryState| {
                    s.overlays.show_side_sheet = false;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(12.px()),
            |s: &mut GalleryState| s.overlays.show_side_sheet = false,
        )
        .extent(360.0)
    });

    let fab_layer = floating_action.map(|f| {
        sized_box(f)
            .padding(Padding::all(20.px()))
            .alignment(UnitPoint::BOTTOM_RIGHT)
    });
    let toast_layer = sized_box(
        toast_host(&state.overlays.toasts, |s: &mut GalleryState| {
            let _ = s.overlays.toasts.dismiss_current();
        })
        .on_action(|s: &mut GalleryState| {
            let _ = s.overlays.toasts.dismiss_current();
        }),
    )
    .padding(Padding::all(16.px()))
    .alignment(UnitPoint::BOTTOM);

    zstack((
        workspace.alignment(UnitPoint::TOP_LEFT),
        fab_layer,
        toast_layer,
        sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        side_sheet_overlay.map(|s| s.alignment(UnitPoint::CENTER)),
        date_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        time_overlay.map(|t| t.alignment(UnitPoint::CENTER)),
        dialog_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        destructive_overlay.map(|d| d.alignment(UnitPoint::CENTER)),
        platform_alert_overlay.map(|a| a.alignment(UnitPoint::CENTER)),
    ))
}
