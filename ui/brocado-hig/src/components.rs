//! macOS signature components: `macos_toolbar`, `macos_toolbar_with_center`, `source_list_sidebar`, `form_group`, `form_row`, `help_button`, `popup_button`, `MacosAlertDialog`.

use std::marker::PhantomData;
use std::sync::Arc;

use brocado_core::components::divider::{horizontal_divider, vertical_divider};
use brocado_core::components::typography::ui_label;
use brocado_core::components::{
    MenuEntry, destructive_button, dropdown_menu, menu_item, primary_button, secondary_button,
};
use brocado_core::foundation::{
    AnchoredPopupView, ConstrainedBox, EscapeCatcher, ModalAxisSizing, ModalShellConfig,
    ModalShellState, UiClassExt, adaptive_pair, clip_box, constrained_box, icons,
    modal_shell_build, modal_shell_message, modal_shell_rebuild, modal_shell_teardown, ui_icon,
    wrap,
};
use brocado_core::style_engine::{classes, current_spec_arc, with_spec};
use brocado_core::tokens::{DesignSpec, MotionDuration, TypeRole};
use masonry::core::ArcStr;
use masonry::kurbo::Vec2;
use masonry::layout::{AsUnit, UnitPoint};
use masonry::peniko::Color;
use masonry::properties::{Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::AnyWidgetView;
use xilem_masonry::Pod;
use xilem_masonry::ViewCtx;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, button, flex_col, flex_row, label, sized_box,
};

/// Unified macOS compact `38px` window toolbar hosting an arbitrary custom center view (e.g. search bar or segmented switcher).
pub fn macos_toolbar_with_center<State: 'static, Action: 'static, C>(
    leading_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    center: C,
    trailing_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    C: WidgetView<State, Action>,
{
    flex_col((
        sized_box(
            flex_row((
                (!leading_actions.is_empty()).then(|| {
                    flex_row(leading_actions)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px())
                }),
                clip_box(center).fill_available(true).flex(1.0),
                (!trailing_actions.is_empty()).then(|| {
                    flex_row(trailing_actions)
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px())
                }),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .padding(Padding::from_vh(4.px(), 6.px())),
        )
        .dims(Dimensions::height(38.px()))
        .ui_class(classes::HEADER_BAR),
        horizontal_divider(),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
}

/// Unified macOS compact `38px` window toolbar with leading actions, title, and trailing actions.
pub fn macos_toolbar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    leading_actions: Vec<Box<AnyWidgetView<State, Action>>>,
    trailing_actions: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> {
    let title: ArcStr = title.into();
    let title_view = with_spec(move |spec: &DesignSpec, _| {
        clip_box(
            ui_label(TypeRole::TitleSmall, title.clone())
                .color(spec.palette.text_primary)
                .line_breaking(LineBreaking::Clip),
        )
    });
    macos_toolbar_with_center(leading_actions, title_view, trailing_actions)
}

/// Frosted macOS Source List sidebar grouping labeled sections with rounded row selection.
pub fn source_list_sidebar<State: 'static, Action: 'static, F>(
    sections: &[(&str, Vec<(&str, &'static str)>)],
    selected: (usize, usize),
    on_select: F,
) -> impl WidgetView<State, Action, Widget = ConstrainedBox> + use<State, Action, F>
where
    F: Fn(&mut State, (usize, usize)) -> Action + Send + Sync + 'static,
{
    let owned_sections: Vec<(ArcStr, Vec<(ArcStr, &'static str)>)> = sections
        .iter()
        .map(|(title, items)| {
            (
                ArcStr::from(*title),
                items
                    .iter()
                    .map(|(name, icon)| (ArcStr::from(*name), *icon))
                    .collect(),
            )
        })
        .collect();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let mut nodes: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
        for (sec_idx, (sec_title, items)) in owned_sections.iter().enumerate() {
            nodes.push(
                spec.typography
                    .style_label(TypeRole::LabelSmall, label(sec_title.clone()))
                    .color(p.text_secondary)
                    .padding(Padding::from_vh(4.px(), 10.px()))
                    .boxed(),
            );
            for (item_idx, (item_name, icon)) in items.iter().enumerate() {
                let active = selected == (sec_idx, item_idx);
                let cb = cb.clone();
                let fg = if active {
                    p.text_primary
                } else {
                    p.text_secondary
                };
                let icon_c = if active { p.accent } else { fg };
                let lbl = spec
                    .typography
                    .style_label(TypeRole::BodyMedium, label(item_name.clone()))
                    .color(fg);
                let cls = if active {
                    classes::NAV_PILL_ACTIVE
                } else {
                    classes::NAV_PILL
                };
                nodes.push(
                    button(
                        flex_row((ui_icon(icon, 16.0, icon_c), lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(8.px()),
                        move |s: &mut State| cb(s, (sec_idx, item_idx)),
                    )
                    .ui_class(cls)
                    .boxed(),
                );
            }
        }

        let w = spec.metrics.sidebar_width;
        constrained_box(
            flex_row((
                sized_box(
                    flex_col(nodes)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(4.px())
                        .padding(Padding::all(10.px())),
                )
                .dims(Dimensions::STRETCH)
                .background_color(p.sidebar_bg)
                .flex(1.0),
                vertical_divider(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .max_width(w + 1.0)
        .fill_max_width(true)
    })
}

/// macOS grouped Settings `Form` container separating child [`form_row`] items with inset dividers.
pub fn form_group<State: 'static, Action: 'static>(
    header: Option<&str>,
    rows: Vec<Box<AnyWidgetView<State, Action>>>,
    footer: Option<&str>,
) -> impl WidgetView<State, Action, Widget = widgets::Flex> + use<State, Action> {
    let header_owned = header.map(ArcStr::from);
    let footer_owned = footer.map(ArcStr::from);

    let head_view = header_owned.map(|h| {
        with_spec(move |spec: &DesignSpec, _| {
            ui_label(TypeRole::LabelMedium, h.clone())
                .color(spec.palette.text_secondary)
                .padding(Padding::from_vh(2.px(), 6.px()))
        })
    });

    let mut card_children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
    for (idx, row) in rows.into_iter().enumerate() {
        if idx > 0 {
            card_children.push(horizontal_divider().boxed());
        }
        card_children.push(row);
    }

    let card = sized_box(flex_col(card_children).cross_axis_alignment(CrossAxisAlignment::Stretch))
        .ui_class(classes::GROUPED_CARD);

    let foot_view = footer_owned.map(|f| {
        with_spec(move |spec: &DesignSpec, _| {
            ui_label(TypeRole::BodySmall, f.clone())
                .color(spec.palette.text_secondary)
                .padding(Padding::from_vh(2.px(), 6.px()))
        })
    });

    flex_col((head_view, card, foot_view))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(6.px())
}

/// macOS `Form` row aligning label on the leading edge and `control` on the trailing edge.
pub fn form_row<State: 'static, Action: 'static, C>(
    label_text: impl Into<ArcStr>,
    control: C,
) -> impl WidgetView<State, Action, Widget = brocado_core::foundation::AdaptivePairWidget>
where
    C: WidgetView<State, Action>,
{
    let text: ArcStr = label_text.into();
    let lbl = with_spec(move |spec: &DesignSpec, _| {
        ui_label(TypeRole::BodyMedium, text.clone()).color(spec.palette.text_primary)
    });

    adaptive_pair(lbl, control)
        .gap(12.px())
        .run_gap(8.px())
        .min_leading(56.0)
        .padding(Padding::from_vh(10.px(), 12.px()))
}

/// Classic circular macOS `?` help button (`24×24`), returning `Pod<widgets::Button>` directly.
pub fn help_button<State: 'static, Action: 'static, F>(
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let cb = cb.clone();
        button(
            spec.typography
                .style_label(TypeRole::LabelMedium, label("?"))
                .color(p.text_primary),
            move |s: &mut State| cb(s),
        )
        .ui_class(classes::BTN_OUTLINED)
        .corner_radius(999.px())
        .background_color(Color::TRANSPARENT)
        .dims(Dimensions::fixed(24.px(), 24.px()))
        .padding(Padding::ZERO)
    })
}

/// macOS SwiftUI `DisclosureGroup` revealing indented custom `content` under a chevron header.
pub fn disclosure_group<State: 'static, Action: 'static, C, F>(
    title: impl Into<ArcStr>,
    expanded: bool,
    on_toggle: F,
    content: C,
) -> impl WidgetView<State, Action, Widget = widgets::Flex>
where
    C: WidgetView<State, Action>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let title: ArcStr = title.into();
    let cb = Arc::new(on_toggle);
    let header_btn = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let cb = cb.clone();
        let chevron = if expanded {
            icons::ARROW_DROP_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        let lbl = clip_box(
            ui_label(TypeRole::TitleSmall, title.clone())
                .color(p.text_primary)
                .line_breaking(LineBreaking::Clip),
        );
        button(
            clip_box(
                flex_row((ui_icon(chevron, 16.0, p.text_secondary), lbl.flex(1.0)))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(6.px()),
            )
            .fill_available(true),
            move |s: &mut State| cb(s),
        )
        .ui_class(classes::BTN_SUBTLE)
    });

    let body = expanded.then(|| sized_box(content).padding(Padding::left(20.px())));
    flex_col((header_btn, body))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(6.px())
}

/// macOS `NSStepper` / SwiftUI `Stepper` pairing a leading label, numeric readout, and compact `−` / `+` buttons.
pub fn stepper<State: 'static, Action: 'static, F>(
    label_text: impl Into<ArcStr>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: F,
) -> impl WidgetView<State, Action, Widget = brocado_core::foundation::AdaptivePairWidget>
where
    F: Fn(&mut State, i32) -> Action + Send + Sync + 'static,
{
    let label_text: ArcStr = label_text.into();
    let step = step.abs().max(1);
    let clamped = value.clamp(min, max);
    let can_dec = clamped > min;
    let can_inc = clamped < max;
    let cb = Arc::new(on_change);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let dec_cb = cb.clone();
        let inc_cb = cb.clone();
        let next_dec = clamped.saturating_sub(step).clamp(min, max);
        let next_inc = clamped.saturating_add(step).clamp(min, max);

        let lbl = ui_label(TypeRole::BodyMedium, label_text.clone()).color(p.text_primary);
        let readout = ui_label(TypeRole::BodyMedium, format!("{clamped}"))
            .color(p.text_secondary)
            .line_breaking(LineBreaking::Clip);

        let dec_btn = button(
            ui_icon(
                icons::REMOVE,
                14.0,
                spec.content_color(p.text_primary, !can_dec),
            ),
            move |s: &mut State| dec_cb(s, next_dec),
        )
        .disabled(!can_dec)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(24.px(), 24.px()))
        .padding(Padding::all(0.px()));

        let inc_btn = button(
            ui_icon(
                icons::ADD,
                14.0,
                spec.content_color(p.text_primary, !can_inc),
            ),
            move |s: &mut State| inc_cb(s, next_inc),
        )
        .disabled(!can_inc)
        .ui_class(classes::ICON_BTN_STD)
        .dims(Dimensions::fixed(24.px(), 24.px()))
        .padding(Padding::all(0.px()));

        let pill = sized_box(
            flex_row((
                dec_btn,
                vertical_divider().dims(Dimensions::height(16.px())),
                inc_btn,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(0.px()),
        )
        .background_color(p.surface)
        .border(p.border_strong, 1.px())
        .corner_radius(spec.metrics.button_radius.px());

        adaptive_pair(
            lbl,
            flex_row((readout, pill))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(8.px()),
        )
        .gap(10.px())
        .run_gap(6.px())
        .min_leading(48.0)
    })
}

/// Trailing macOS Inspector pane (`260px` preferred width, shrinking cleanly on narrower viewports).
pub fn inspector_panel<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    sections: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = ConstrainedBox> {
    inspector_panel_with_width(title, sections, 260.0)
}

/// Trailing macOS Inspector pane with configurable preferred max width (`width + 1px` incl. divider).
pub fn inspector_panel_with_width<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
    sections: Vec<Box<AnyWidgetView<State, Action>>>,
    width: f64,
) -> impl WidgetView<State, Action, Widget = ConstrainedBox> {
    let title: ArcStr = title.into();
    let hdr = with_spec(move |spec: &DesignSpec, _| {
        spec.typography
            .style_label(TypeRole::TitleSmall, label(title.clone()))
            .color(spec.palette.text_primary)
    });

    constrained_box(
        flex_row((
            vertical_divider(),
            sized_box(
                flex_col((
                    hdr,
                    horizontal_divider(),
                    flex_col(sections)
                        .cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .gap(12.px()),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(10.px()),
            )
            .ui_class(classes::CARD_FILLED)
            .corner_radius(0.px())
            .padding(Padding::all(12.px()))
            .dims(Dimensions::STRETCH)
            .flex(1.0),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch),
    )
    .max_width(width + 1.0)
    .fill_max_width(true)
}

/// macOS Finder-style bottom `NSPathControl` bar with `14px` segment icons and chevron separators.
pub fn path_control<State: 'static, Action: 'static, F>(
    segments: &[(&str, &'static str)],
    on_select: F,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> + use<State, Action, F>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let items: Vec<(ArcStr, &'static str)> = segments
        .iter()
        .map(|(name, ic)| (ArcStr::from(*name), *ic))
        .collect();
    let cb = Arc::new(on_select);

    with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let last_idx = items.len().saturating_sub(1);
        let mut children: Vec<Box<AnyWidgetView<State, Action>>> = Vec::new();
        for (idx, (name, icon_path)) in items.iter().enumerate() {
            if idx > 0 {
                children.push(ui_icon(icons::CHEVRON_RIGHT, 12.0, p.text_secondary).boxed());
            }
            let cb = cb.clone();
            let fg = if idx == last_idx {
                p.text_primary
            } else {
                p.text_secondary
            };
            let lbl = clip_box(
                ui_label(TypeRole::BodySmall, name.clone())
                    .color(fg)
                    .line_breaking(LineBreaking::Clip),
            );
            children.push(
                button(
                    clip_box(
                        flex_row((ui_icon(icon_path, 14.0, fg), lbl.flex(1.0)))
                            .cross_axis_alignment(CrossAxisAlignment::Center)
                            .gap(4.px()),
                    ),
                    move |s: &mut State| cb(s, idx),
                )
                .ui_class(classes::BTN_SUBTLE)
                .dims(Dimensions::height(22.px()))
                .boxed(),
            );
        }

        sized_box(
            flex_col((
                horizontal_divider(),
                wrap(children)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(4.px())
                    .run_gap(4.px())
                    .padding(Padding::from_vh(3.px(), 8.px())),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .background_color(p.header_bar_bg)
    })
}

/// macOS `NSPopUpButton` compact pill with trailing accent-tinted double-chevron badge
/// backed by an anchored [`PopupLayer`](brocado_core::foundation::PopupLayer).
pub fn popup_button<State: 'static, Action: 'static, OnToggle, OnSelect, OnDismiss>(
    options: &[&str],
    selected: usize,
    open: bool,
    on_toggle: OnToggle,
    on_select: OnSelect,
    on_dismiss: OnDismiss,
) -> AnchoredPopupView<Box<AnyWidgetView<State, Action>>, State, Action>
where
    OnToggle: Fn(&mut State) -> Action + Send + Sync + 'static,
    OnSelect: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
    OnDismiss: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let current = options
        .get(selected)
        .copied()
        .or_else(|| options.first().copied())
        .unwrap_or("选择…");
    let current_text = ArcStr::from(current);
    let toggle_cb = Arc::new(on_toggle);

    let trigger = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let toggle_cb = toggle_cb.clone();
        let lbl = clip_box(
            ui_label(TypeRole::BodyMedium, current_text.clone())
                .color(p.text_primary)
                .line_breaking(LineBreaking::Clip),
        );
        let chevrons = sized_box(
            flex_col((
                ui_icon(icons::ARROW_DROP_UP, 8.0, p.on_accent),
                ui_icon(icons::ARROW_DROP_DOWN, 8.0, p.on_accent),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(0.px()),
        )
        .dims(Dimensions::fixed(18.px(), 18.px()))
        .background_color(p.accent)
        .corner_radius(4.px());

        button(
            clip_box(
                flex_row((lbl.flex(1.0), chevrons))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(8.px()),
            ),
            move |s: &mut State| toggle_cb(s),
        )
        .ui_class(classes::BTN_OUTLINED)
        .padding(Padding::from_vh(2.px(), 8.px()))
    })
    .boxed();

    let entries: Vec<MenuEntry> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let entry = menu_item(*opt).selected(i == selected);
            if i == selected {
                entry.leading_icon(icons::CHECK)
            } else {
                entry
            }
        })
        .collect();

    dropdown_menu(trigger, open, entries, on_select, on_dismiss)
}

/// Callback invoked by [`MacosAlertDialog`].
pub type MacosAlertCallback<State, Action> = Arc<dyn Fn(&mut State) -> Action + Send + Sync>;

/// macOS 11+ (Big Sur / Sonoma) centered alert dialog with icon, centered copy,
/// and stacked full-width primary/secondary action buttons.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(missing_debug_implementations, reason = "Contains closure callbacks")]
pub struct MacosAlertDialog<State, Action> {
    title: ArcStr,
    message: ArcStr,
    primary_text: ArcStr,
    secondary_text: ArcStr,
    icon_svg: Option<&'static str>,
    destructive: bool,
    dismiss_on_scrim: bool,
    on_primary: MacosAlertCallback<State, Action>,
    on_secondary: MacosAlertCallback<State, Action>,
    phantom: PhantomData<fn(&mut State) -> Action>,
}

impl<State, Action> MacosAlertDialog<State, Action> {
    /// Sets or clears the centered header glyph.
    pub fn icon(mut self, svg_path: Option<&'static str>) -> Self {
        self.icon_svg = svg_path;
        self
    }

    /// Uses destructive emphasis for the primary stacked button when `true`.
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Enables or disables closing when clicking outside the alert card (defaults to `true`).
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    fn build_content(&self, spec: &DesignSpec) -> Box<AnyWidgetView<State, Action>>
    where
        State: 'static,
        Action: 'static,
    {
        let p = &spec.palette;
        let glyph = self.icon_svg.map(|path| {
            let tint = if self.destructive { p.error } else { p.accent };
            sized_box(ui_icon(path, 36.0, tint))
                .padding(Padding::all(8.px()))
                .background_color(p.surface_low)
                .corner_radius(10.px())
        });

        let title_lbl = ui_label(TypeRole::TitleMedium, self.title.clone()).color(p.text_primary);
        let msg_lbl = ui_label(TypeRole::BodySmall, self.message.clone()).color(p.text_secondary);

        let on_primary = self.on_primary.clone();
        let primary_btn: Box<AnyWidgetView<State, Action>> = if self.destructive {
            destructive_button(self.primary_text.clone(), move |s: &mut State| {
                on_primary(s)
            })
            .boxed()
        } else {
            primary_button(self.primary_text.clone(), move |s: &mut State| {
                on_primary(s)
            })
            .boxed()
        };

        let on_secondary = self.on_secondary.clone();
        let secondary_btn = secondary_button(self.secondary_text.clone(), move |s: &mut State| {
            on_secondary(s)
        });

        let actions = flex_col((primary_btn, secondary_btn))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(8.px());

        let header = flex_col((glyph, title_lbl, msg_lbl))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(10.px());

        flex_col((header, actions))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(14.px())
            .boxed()
    }
}

impl<State: 'static, Action: 'static> ViewMarker for MacosAlertDialog<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for MacosAlertDialog<State, Action>
{
    type Element = Pod<EscapeCatcher>;
    type ViewState = (
        Box<AnyWidgetView<State, Action>>,
        ModalShellState<
            State,
            Action,
            <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState,
        >,
    );

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        let spec = current_spec_arc(ctx);
        let content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            width: ModalAxisSizing::FitContent {
                min: Some(260.0),
                max: Some(360.0),
            },
            height: ModalAxisSizing::FitContent {
                min: None,
                max: None,
            },
            viewport_margin: None,
            padding: Padding::all(16.px()),
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 12.0),
            duration: MotionDuration::Medium1,
            dismiss_on_scrim: self.dismiss_on_scrim,
        };
        let (el, shell) =
            modal_shell_build(ctx, app_state, cfg, &content, self.on_secondary.clone());
        (el, (content, shell))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        (prev_content, shell): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        let spec = current_spec_arc(ctx);
        let next_content = self.build_content(&spec);
        let cfg = ModalShellConfig {
            class: classes::DIALOG_SURFACE,
            width: ModalAxisSizing::FitContent {
                min: Some(260.0),
                max: Some(360.0),
            },
            height: ModalAxisSizing::FitContent {
                min: None,
                max: None,
            },
            viewport_margin: None,
            padding: Padding::all(16.px()),
            alignment: UnitPoint::CENTER,
            enter_offset: Vec2::new(0.0, 12.0),
            duration: MotionDuration::Medium1,
            dismiss_on_scrim: self.dismiss_on_scrim,
        };
        modal_shell_rebuild(
            shell,
            ctx,
            element,
            app_state,
            cfg,
            prev_content,
            &next_content,
            self.on_secondary.clone(),
        );
        *prev_content = next_content;
    }

    fn teardown(
        &self,
        (content, shell): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        modal_shell_teardown(shell, ctx, element, content);
    }

    fn message(
        &self,
        (content, shell): &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        modal_shell_message(
            shell,
            message,
            element,
            app_state,
            content,
            &self.on_secondary,
        )
    }
}

/// Constructs a macOS 11+ [`MacosAlertDialog`] with centered icon, title, copy, and stacked full-width buttons.
pub fn macos_alert_dialog<State: 'static, Action: 'static, F1, F2>(
    title: impl Into<ArcStr>,
    message: impl Into<ArcStr>,
    primary_text: impl Into<ArcStr>,
    secondary_text: impl Into<ArcStr>,
    on_primary: F1,
    on_secondary: F2,
) -> MacosAlertDialog<State, Action>
where
    F1: Fn(&mut State) -> Action + Send + Sync + 'static,
    F2: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    MacosAlertDialog {
        title: title.into(),
        message: message.into(),
        primary_text: primary_text.into(),
        secondary_text: secondary_text.into(),
        icon_svg: Some(icons::INFO),
        destructive: false,
        dismiss_on_scrim: true,
        on_primary: Arc::new(on_primary),
        on_secondary: Arc::new(on_secondary),
        phantom: PhantomData,
    }
}
