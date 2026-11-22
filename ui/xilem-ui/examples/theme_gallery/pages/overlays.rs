//! Overlays, tooltips, modal dialogs/sheets, toasts, and date/time pickers page.

use xilem::WidgetView;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{CrossAxisAlignment, flex_col, flex_row};
use xilem_ui::prelude::*;

use crate::state::GalleryState;

pub fn overlays_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let trigger_row = elevated_card(
        flex_col((
            title_medium("模态框、提示与轻提示队列 (Toast)"),
            flex_row((
                primary_button("打开基础弹窗", |s: &mut GalleryState| {
                    s.show_basic_dialog = true;
                }),
                destructive_button("危险确认弹窗", |s: &mut GalleryState| {
                    s.show_destructive_dialog = true;
                })
                .leading_icon(icons::DELETE),
                secondary_button("底部抽屉", |s: &mut GalleryState| {
                    s.show_bottom_sheet = true;
                }),
                secondary_button("侧边抽屉", |s: &mut GalleryState| {
                    s.show_side_sheet = true;
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
            flex_row((
                outlined_button("日期选择弹窗", |s: &mut GalleryState| {
                    s.show_date_dialog = true;
                })
                .leading_icon(icons::CALENDAR),
                outlined_button("时间选择弹窗", |s: &mut GalleryState| {
                    s.show_time_dialog = true;
                })
                .leading_icon(icons::SCHEDULE),
                outlined_button("弹出轻提示 (Toast)", |s: &mut GalleryState| {
                    let theme_name = s.theme.language.label();
                    s.toasts.show(
                        format!("当前激活主题：{theme_name}"),
                        Some("知道了"),
                        ToastDuration::Short,
                        true,
                    );
                }),
                plain_tooltip(
                    "由 LayerStack 承载的悬停轻量提示",
                    subtle_button("悬停普通提示", |s: &mut GalleryState| {
                        s.click_count += 1
                    }),
                ),
                rich_tooltip(
                    Some("富文本提示"),
                    "鼠标移出控件范围后浮层会自动卸载销毁。",
                    subtle_button("悬停富提示", |s: &mut GalleryState| s.click_count += 1),
                ),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px()),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(12.px()),
    );

    let pickers_row = flex_row((
        outlined_card(
            flex_col((
                title_medium(format!("日期选择器 ({})", state.selected_date.format_iso())),
                date_picker(state.selected_date, |s: &mut GalleryState, d| {
                    s.selected_date = d;
                }),
            ))
            .gap(8.px()),
        ),
        outlined_card(
            flex_col((
                title_medium(format!("时间选择器 ({})", state.selected_time.format_24h())),
                time_picker(state.selected_time, |s: &mut GalleryState, t| {
                    s.selected_time = t;
                }),
            ))
            .gap(8.px()),
        ),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(16.px());

    flex_col((
        headline_small("分层浮层与日期时间选择器"),
        trigger_row,
        pickers_row,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(16.px())
}
