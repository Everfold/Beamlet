//! Active platform-native signature widgets + open custom slot card.

use brocado::components::typography::ui_label;
use brocado::prelude::*;
use masonry::layout::AsUnit;
use masonry::parley::LineHeight;
use masonry::peniko::Color;
use masonry::properties::{LineBreaking, Padding};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{CrossAxisAlignment, FlexExt, flex_col, flex_row, sized_box};

use crate::state::GalleryState;

pub fn native_page(state: &mut GalleryState) -> impl WidgetView<GalleryState> + use<> {
    let clicks = state.components.click_count;
    let md3_group_selected = state.native.md3_group_selected;
    let md3_fab_menu_open = state.native.md3_fab_menu_open;
    let md3_last_action = state.native.md3_last_action.clone();

    let make_custom_items = |wide: bool| {
        let save_btn = button_view(
            ButtonVariant::Primary,
            with_theme(|theme, _| {
                let shortcut = format_shortcut_primary("S");
                let on_primary = theme.scheme.on_primary;
                let text = clip_box(
                    ui_label(TypeRole::LabelLarge, "保存")
                        .color(on_primary)
                        .line_breaking(LineBreaking::Clip),
                );
                clip_box(
                    flex_row((
                        ui_icon(icons::CHECK, 16.0, on_primary),
                        text.flex(1.0),
                        sized_box(
                            ui_label(TypeRole::LabelSmall, shortcut)
                                .line_height(LineHeight::MetricsRelative(1.0))
                                .color(on_primary)
                                .line_breaking(LineBreaking::Clip),
                        )
                        .padding(Padding::from_vh(3.px(), 6.px()))
                        .border(on_primary, 1.px())
                        .corner_radius(4.px()),
                    ))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
                )
            }),
            |s: &mut GalleryState| s.components.click_count += 1,
        )
        .padding(Padding::from_vh(8.px(), 12.px()));
        let wide_btn_base =
            primary_button(format!("宽方角 ({clicks})"), |s: &mut GalleryState| {
                s.components.click_count += 1;
            })
            .corner_radius(3.px());
        let wide_btn: Box<AnyWidgetView<GalleryState>> = if wide {
            wide_btn_base.width(150.px()).boxed()
        } else {
            wide_btn_base.boxed()
        };
        let list_row = list_item_view(
            flex_row((
                ui_icon(icons::INFO, 16.0, Color::from_rgb8(0x4E, 0x8C, 0xE8)),
                body_medium("list_item_view").flex(1.0),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px()),
        );
        (save_btn, wide_btn, list_row)
    };
    let (sb_w, wb_w, lr_w) = make_custom_items(true);
    let (sb_n, wb_n, lr_n) = make_custom_items(false);

    let custom_card = outlined_card(
        flex_col((
            title_medium("自定义插槽与样式覆写"),
            breakpoint_bin(
                520.0,
                flex_row((sb_w, wb_w, lr_w))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(10.px()),
                flex_col((sb_n, wb_n, lr_n))
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .gap(8.px()),
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    let make_fabs = || {
        (
            small_fab(icons::EDIT, FabColor::Surface, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            fab(icons::ADD, FabColor::Primary, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            large_fab(icons::SEND, FabColor::Tertiary, |s: &mut GalleryState| {
                s.components.click_count += 1;
            }),
            extended_fab(
                icons::EDIT,
                "撰写",
                FabColor::Secondary,
                |s: &mut GalleryState| s.components.click_count += 1,
            ),
        )
    };
    let (sf_w, f_w, lf_w, ef_w) = make_fabs();
    let (sf_n, f_n, lf_n, ef_n) = make_fabs();
    let suite = filled_card(
        flex_col((
            title_medium("Material 3 Expressive"),
            top_app_bar_slots(
                TopAppBarVariant::CenterAligned,
                "TopAppBar",
                Some(
                    icon_button(icons::MENU, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .boxed(),
                ),
                vec![
                    icon_button(icons::SEARCH, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                    })
                    .boxed(),
                ],
            ),
            breakpoint_bin(
                340.0,
                flex_row((sf_w, f_w, lf_w, ef_w))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(12.px()),
                wrap((sf_n, f_n, lf_n, ef_n))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px())
                    .run_gap(8.px()),
            ),
            floating_toolbar_with_variant(
                FloatingToolbarVariant::Vibrant,
                vec![
                    icon_button(icons::ARROW_BACK, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                        s.native.md3_last_action = "返回".to_owned();
                    })
                    .boxed(),
                    icon_button(icons::COPY, |s: &mut GalleryState| {
                        s.components.click_count += 1;
                        s.native.md3_last_action = "复制".to_owned();
                    })
                    .boxed(),
                    primary_button("发布", |s: &mut GalleryState| {
                        s.components.click_count += 1;
                        s.native.md3_last_action = "发布".to_owned();
                    })
                    .boxed(),
                ],
            ),
            wrap((
                split_button(
                    "发布更新",
                    |s: &mut GalleryState| {
                        s.components.click_count += 1;
                        s.native.md3_last_action = "发布主操作".to_owned();
                    },
                    |s: &mut GalleryState| {
                        s.components.click_count += 1;
                        s.native.md3_last_action = "展开下拉菜单".to_owned();
                    },
                )
                .boxed(),
                label_medium(format!("最近动作: {md3_last_action}")).boxed(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(12.px())
            .run_gap(8.px()),
            button_group(
                &["日", "周", "月", "年"],
                md3_group_selected,
                |s: &mut GalleryState, idx| {
                    s.native.md3_group_selected = idx;
                    s.native.md3_last_action = format!("分段 #{idx}");
                },
            ),
            wavy_linear_progress(Some(0.68)),
            fab_menu(
                md3_fab_menu_open,
                |s: &mut GalleryState| {
                    s.native.md3_fab_menu_open = !s.native.md3_fab_menu_open;
                },
                vec![
                    FabMenuItem::new(icons::ADD, "新建笔记", |s: &mut GalleryState| {
                        s.native.md3_fab_menu_open = false;
                        s.native.md3_last_action = "新建笔记".to_owned();
                    }),
                    FabMenuItem::new(icons::INFO, "查看详情", |s: &mut GalleryState| {
                        s.native.md3_fab_menu_open = false;
                        s.native.md3_last_action = "查看详情".to_owned();
                    }),
                ],
            ),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(10.px()),
    );

    flex_col((suite, custom_card))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(16.px())
}
