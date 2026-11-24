//! Synthesizes Masonry [`DefaultProperties`] and shallow class `PropertyStack` layers from a [`Theme`].
//!
//! Every selector emitted here matches at most **one** semantic class (plus optional Masonry
//! pseudo-states `hovered`, `active`, `focused`, `disabled`) without descendant or sibling
//! combinators, keeping selector resolution constant-time and predictable across live theme
//! switches.

use masonry::core::{DefaultProperties, PropertySet, PropertyStack, Selector};
use masonry::layout::{AsUnit, Dim, Length};
use masonry::peniko::Color;
use masonry::properties::{
    Background, BorderColor, BorderWidth, BoxShadow, CaretColor, CheckmarkColor,
    CheckmarkStrokeWidth, ContentColor, CornerRadius, Dimensions, Gap, LineBreaking, Padding,
    PlaceholderColor, SelectionColor, ThumbColor, ThumbRadius, TrackColor, TrackThickness,
};
use masonry::widgets::{
    Button, Checkbox, Divider, Flex, Label, ProgressBar, Prose, RadioButton, SizedBox, Slider,
    Spinner, Switch, TextArea, TextInput,
};

use crate::foundation::state_layer::{resolve_interactive_colors, with_opacity};
use crate::foundation::{FocusFrame, ScrollBarStyle, ScrollViewWidget, UiScrollBar};
use crate::style_engine::classes;
use crate::theme::Theme;
use crate::theme::metrics::{
    ACTIVE_BUTTON_RADIUS, BUTTON_HEIGHT, BUTTON_PAD_H, BUTTON_RADIUS, CARD_PAD, CARD_RADIUS,
    CHECKBOX_RADIUS, CHIP_HEIGHT, CHIP_RADIUS, DIALOG_RADIUS, ElevationLevel, ICON_BTN_RADIUS,
    ICON_BTN_SIZE, INPUT_PAD_H, INPUT_PAD_V, INPUT_RADIUS, MENU_RADIUS, NAV_PILL_RADIUS,
    SEARCH_HEIGHT, SEARCH_RADIUS, SHEET_RADIUS, SWITCH_HEIGHT,
};
use crate::tokens::Brightness;

#[expect(
    clippy::too_many_arguments,
    reason = "Internal helper mapping explicit button variant styling parameters"
)]
fn insert_button_variant(
    stack: &mut PropertyStack,
    class: &'static str,
    theme: &Theme,
    base: Color,
    tint: Color,
    border: Option<(Color, f64)>,
    radius: f64,
    active_radius: Option<f64>,
    padding: Padding,
    dims: Option<Dimensions>,
    shadow: Option<BoxShadow>,
) {
    let states = resolve_interactive_colors(base, tint, theme.disabled_container());
    let mut base_set = PropertySet::new()
        .with(Background::Color(states.rest))
        .with(CornerRadius {
            radius: radius.px(),
        })
        .with(padding);
    if let Some(d) = dims {
        base_set = base_set.with(d);
    }
    if let Some(s) = shadow {
        base_set = base_set.with(s);
    }
    if let Some((b_col, b_w)) = border {
        base_set = base_set
            .with(BorderColor::new(b_col))
            .with(BorderWidth { width: b_w.px() });
    } else {
        base_set = base_set.with(BorderWidth { width: 0.px() });
    }

    stack.push_layer(Selector::classes(&[class]), base_set);
    stack.push_layer(
        Selector::classes(&[class]).with_hovered(true),
        PropertySet::new().with(Background::Color(states.hover)),
    );
    let mut active_set = PropertySet::new().with(Background::Color(states.pressed));
    if let Some(r) = active_radius {
        active_set = active_set.with(CornerRadius { radius: r.px() });
    }
    stack.push_layer(Selector::classes(&[class]).with_active(true), active_set);
    stack.push_layer(
        Selector::classes(&[class]).with_disabled(true),
        PropertySet::new().with(Background::Color(states.disabled)),
    );
}

fn configure_buttons(props: &mut DefaultProperties, theme: &Theme) {
    let mut stack = PropertyStack::new();
    let s = &theme.scheme;
    let t = &theme.typography;
    let morph = Some(ACTIVE_BUTTON_RADIUS);
    let eff_btn_h = BUTTON_HEIGHT.max(f64::from(t.label_large.line_height) + 10.0);
    let eff_icon_sz = ICON_BTN_SIZE.max(f64::from(t.label_large.line_height) + 10.0);
    let eff_fab_h = (BUTTON_HEIGHT + 16.0)
        .max(56.0)
        .max(f64::from(t.label_large.line_height) + 20.0);
    let eff_chip_h = CHIP_HEIGHT.max(f64::from(t.label_large.line_height) + 8.0);
    let std_pad = Padding::from_vh(0.px(), BUTTON_PAD_H.px());
    let btn_dims = Some(Dimensions::height(eff_btn_h.px()));

    insert_button_variant(
        &mut stack,
        classes::BTN_PRIMARY,
        theme,
        s.primary,
        s.on_primary,
        None,
        BUTTON_RADIUS,
        morph,
        std_pad,
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_SECONDARY,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        Some((s.outline_variant, 1.0)),
        BUTTON_RADIUS,
        morph,
        std_pad,
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_ELEVATED,
        theme,
        s.surface_container_high,
        s.primary,
        Some((s.outline_variant, 1.0)),
        BUTTON_RADIUS,
        morph,
        std_pad,
        btn_dims,
        Some(theme.box_shadow(ElevationLevel::Level1)),
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_OUTLINED,
        theme,
        Color::TRANSPARENT,
        s.on_surface,
        Some((s.outline, 1.0)),
        BUTTON_RADIUS,
        morph,
        std_pad,
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_SUBTLE,
        theme,
        Color::TRANSPARENT,
        s.primary,
        None,
        BUTTON_RADIUS,
        morph,
        Padding::from_vh(0.px(), 10.px()),
        btn_dims,
        None,
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_ACTION_BTN]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(CornerRadius {
                radius: BUTTON_RADIUS.px(),
            })
            .with(Padding::from_vh(0.px(), 10.px()))
            .with(Dimensions::height(eff_btn_h.px()))
            .with(BorderWidth { width: 0.px() }),
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_ACTION_BTN]).with_hovered(true),
        PropertySet::new().with(Background::Color(with_opacity(s.primary_container, 0.14))),
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_ACTION_BTN]).with_active(true),
        PropertySet::new().with(Background::Color(with_opacity(s.primary_container, 0.24))),
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_DESTRUCTIVE,
        theme,
        s.error,
        s.on_error,
        None,
        BUTTON_RADIUS,
        morph,
        std_pad,
        btn_dims,
        None,
    );

    let icon_dims = Some(Dimensions::fixed(eff_icon_sz.px(), eff_icon_sz.px()));
    let icon_pad = Padding::ZERO;
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_STD,
        theme,
        Color::TRANSPARENT,
        s.on_surface_variant,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_CLOSE_BTN]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(CornerRadius {
                radius: ICON_BTN_RADIUS.px(),
            })
            .with(icon_pad)
            .with(Dimensions::fixed(eff_icon_sz.px(), eff_icon_sz.px()))
            .with(BorderWidth { width: 0.px() }),
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_CLOSE_BTN]).with_hovered(true),
        PropertySet::new().with(Background::Color(with_opacity(s.inverse_on_surface, 0.12))),
    );
    stack.push_layer(
        Selector::classes(&[classes::TOAST_CLOSE_BTN]).with_active(true),
        PropertySet::new().with(Background::Color(with_opacity(s.inverse_on_surface, 0.22))),
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_STD_SELECTED,
        theme,
        with_opacity(s.primary, 0.14),
        s.primary,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_FILLED,
        theme,
        s.primary,
        s.on_primary,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_FILLED_UNSELECTED,
        theme,
        s.surface_container_highest,
        s.primary,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_TONAL,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_TONAL_UNSELECTED,
        theme,
        s.surface_container_highest,
        s.on_surface_variant,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_OUTLINED,
        theme,
        Color::TRANSPARENT,
        s.on_surface_variant,
        Some((s.outline, 1.0)),
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_OUTLINED_SELECTED,
        theme,
        s.inverse_surface,
        s.inverse_on_surface,
        None,
        ICON_BTN_RADIUS,
        morph,
        icon_pad,
        icon_dims,
        None,
    );

    let fab_pad = Padding::all(16.px());
    let fab_dims = Some(Dimensions::height(eff_fab_h.px()));
    let fab_shadow = Some(theme.box_shadow(ElevationLevel::Level3));
    for (cls, bg, fg) in [
        (
            classes::FAB_PRIMARY,
            s.primary_container,
            s.on_primary_container,
        ),
        (classes::FAB_SURFACE, s.surface_container_high, s.primary),
        (
            classes::FAB_SECONDARY,
            s.secondary_container,
            s.on_secondary_container,
        ),
        (
            classes::FAB_TERTIARY,
            s.tertiary_container,
            s.on_tertiary_container,
        ),
    ] {
        insert_button_variant(
            &mut stack, cls, theme, bg, fg, None, 16.0, None, fab_pad, fab_dims, fab_shadow,
        );
    }

    let chip_pad = Padding::from_vh(0.px(), 12.px());
    let chip_dims = Some(Dimensions::height(eff_chip_h.px()));
    insert_button_variant(
        &mut stack,
        classes::CHIP_ASSIST,
        theme,
        s.surface,
        s.on_surface,
        Some((s.outline_variant, 1.0)),
        CHIP_RADIUS,
        None,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_FILTER,
        theme,
        Color::TRANSPARENT,
        s.on_surface,
        Some((s.outline, 1.0)),
        CHIP_RADIUS,
        None,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_FILTER_SELECTED,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        None,
        CHIP_RADIUS,
        None,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_INPUT,
        theme,
        s.surface_container,
        s.on_surface,
        Some((s.outline_variant, 1.0)),
        CHIP_RADIUS,
        None,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_SUGGESTION,
        theme,
        Color::TRANSPARENT,
        s.on_surface_variant,
        Some((s.outline_variant, 1.0)),
        CHIP_RADIUS,
        None,
        chip_pad,
        chip_dims,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::NAV_PILL,
        theme,
        Color::TRANSPARENT,
        s.on_surface,
        None,
        NAV_PILL_RADIUS,
        None,
        Padding::from_vh(6.px(), 10.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::NAV_PILL_ACTIVE,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        None,
        NAV_PILL_RADIUS,
        None,
        Padding::from_vh(6.px(), 10.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::SEG_BTN,
        theme,
        s.surface,
        s.on_surface,
        Some((s.outline_variant, 1.0)),
        BUTTON_RADIUS,
        None,
        Padding::from_vh(6.px(), 14.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::SEG_BTN_SELECTED,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        Some((s.outline, 1.0)),
        BUTTON_RADIUS,
        None,
        Padding::from_vh(6.px(), 14.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::LIST_ITEM,
        theme,
        Color::TRANSPARENT,
        s.on_surface,
        None,
        NAV_PILL_RADIUS,
        None,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::LIST_ITEM_SELECTED,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        None,
        NAV_PILL_RADIUS,
        None,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::MENU_ITEM,
        theme,
        Color::TRANSPARENT,
        s.on_surface,
        None,
        (MENU_RADIUS - 2.0).max(4.0),
        None,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::MENU_ITEM_SELECTED,
        theme,
        s.secondary_container,
        s.on_secondary_container,
        None,
        (MENU_RADIUS - 2.0).max(4.0),
        None,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::TAB_ITEM,
        theme,
        Color::TRANSPARENT,
        s.primary,
        None,
        (BUTTON_RADIUS - 2.0).max(4.0),
        None,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    let scrim_bg = with_opacity(s.scrim, 0.36);
    stack.push_layer(
        Selector::classes(&[classes::SCRIM]),
        PropertySet::new()
            .with(Background::Color(scrim_bg))
            .with(BorderWidth { width: 0.px() })
            .with(CornerRadius { radius: 0.px() })
            .with(Padding::ZERO),
    );
    stack.push_layer(
        Selector::classes(&[classes::SCRIM]).with_hovered(true),
        PropertySet::new().with(Background::Color(scrim_bg)),
    );
    stack.push_layer(
        Selector::classes(&[classes::SCRIM]).with_active(true),
        PropertySet::new().with(Background::Color(scrim_bg)),
    );
    stack.push_layer(
        Selector::classes(&[classes::SCRIM_TRANSPARENT]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderWidth { width: 0.px() })
            .with(CornerRadius { radius: 0.px() })
            .with(Padding::ZERO),
    );
    stack.push_layer(
        Selector::classes(&[classes::SCRIM_TRANSPARENT]).with_hovered(true),
        PropertySet::new().with(Background::Color(Color::TRANSPARENT)),
    );
    stack.push_layer(
        Selector::classes(&[classes::SCRIM_TRANSPARENT]).with_active(true),
        PropertySet::new().with(Background::Color(Color::TRANSPARENT)),
    );

    props.insert_stack::<Button>(stack);
}

fn configure_surfaces(props: &mut DefaultProperties, theme: &Theme) {
    let s = &theme.scheme;
    props.insert::<Flex, _>(Gap::ZERO);
    props.insert::<Divider, _>(ContentColor::new(s.outline_variant));

    let mut sized_stack = PropertyStack::new();
    let card_pad = Padding::all(CARD_PAD.px());
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_ELEVATED]),
        PropertySet::new()
            .with(Background::Color(s.surface_container_high))
            .with(CornerRadius {
                radius: CARD_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(card_pad)
            .with(theme.box_shadow(ElevationLevel::Level1)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_FILLED]),
        PropertySet::new()
            .with(Background::Color(s.surface_container))
            .with(CornerRadius {
                radius: CARD_RADIUS.px(),
            })
            .with(BorderWidth { width: 0.px() })
            .with(card_pad)
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_OUTLINED]),
        PropertySet::new()
            .with(Background::Color(s.surface))
            .with(CornerRadius {
                radius: CARD_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(card_pad)
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::GROUPED_CARD]),
        PropertySet::new()
            .with(Background::Color(s.surface))
            .with(CornerRadius {
                radius: CARD_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::DIALOG_SURFACE]),
        PropertySet::new()
            .with(Background::Color(s.surface_container_high))
            .with(CornerRadius {
                radius: DIALOG_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(theme.box_shadow(ElevationLevel::Level3)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::SHEET_SURFACE]),
        PropertySet::new()
            .with(Background::Color(s.surface_container_low))
            .with(CornerRadius {
                radius: SHEET_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(theme.box_shadow(ElevationLevel::Level2)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::POPOVER_SURFACE]),
        PropertySet::new()
            .with(Background::Color(s.surface_container_high))
            .with(CornerRadius {
                radius: MENU_RADIUS.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(s.outline_variant))
            .with(theme.box_shadow(ElevationLevel::Level2)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::HEADER_BAR]),
        PropertySet::new()
            .with(Background::Color(s.surface_container))
            .with(BorderWidth { width: 0.px() }),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BOTTOM_BAR]),
        PropertySet::new()
            .with(Background::Color(s.surface_container))
            .with(BorderWidth { width: 0.px() }),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BADGE_SMALL]),
        PropertySet::new()
            .with(Background::Color(s.error))
            .with(CornerRadius { radius: 999.px() })
            .with(BorderWidth { width: 0.px() })
            .with(Dimensions::fixed(6.px(), 6.px())),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BADGE_LARGE]),
        PropertySet::new()
            .with(Background::Color(s.error))
            .with(CornerRadius { radius: 999.px() })
            .with(BorderWidth { width: 0.px() })
            .with(Padding::from_vh(2.px(), 6.px())),
    );
    let eff_chip_h = CHIP_HEIGHT.max(f64::from(theme.typography.label_large.line_height) + 8.0);
    let eff_search_h =
        SEARCH_HEIGHT.max(f64::from(theme.typography.body_medium.line_height) + 12.0);
    sized_stack.push_layer(
        Selector::classes(&[classes::CHIP_ROW]),
        PropertySet::new().with(Dimensions::height(eff_chip_h.px())),
    );
    props.insert_stack::<SizedBox>(sized_stack);

    let mut focus_stack = PropertyStack::new();
    focus_stack.push_layer(
        Selector::classes(&[classes::SEARCH_BAR]),
        PropertySet::new()
            .with(Background::Color(s.surface_container_highest))
            .with(BorderColor::new(s.outline_variant))
            .with(BorderWidth { width: 1.px() })
            .with(CornerRadius {
                radius: SEARCH_RADIUS.px(),
            })
            .with(Dimensions::height(eff_search_h.px())),
    );
    props.insert_stack::<FocusFrame>(focus_stack);
}

fn configure_inputs(props: &mut DefaultProperties, theme: &Theme) {
    let s = &theme.scheme;

    props.insert::<Label, _>(ContentColor::new(s.on_surface));
    props.insert::<Label, _>(LineBreaking::WordWrap);
    props.insert::<Prose, _>(ContentColor::new(s.on_surface));
    props.insert::<TextArea<true>, _>(CaretColor { color: s.primary });
    props.insert::<TextArea<true>, _>(SelectionColor {
        color: with_opacity(s.primary, 0.28),
    });

    // Switches, Checkboxes, Radios, Sliders, Progress
    let mut cb_stack = PropertyStack::new();
    cb_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(s.outline))
            .with(BorderWidth { width: 2.px() })
            .with(CornerRadius {
                radius: CHECKBOX_RADIUS.px(),
            })
            .with(CheckmarkColor::new(s.on_primary))
            .with(CheckmarkStrokeWidth { width: 2.0 }),
    );
    cb_stack.push_layer(
        Selector::classes(&[classes::CHECKED]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(s.primary))
            .with(BorderWidth { width: 2.px() })
            .with(CheckmarkColor::new(s.primary)),
    );
    cb_stack.push_layer(
        Selector::new().with_disabled(true),
        PropertySet::new()
            .with(Background::Color(theme.disabled_container()))
            .with(BorderColor::new(theme.disabled_content()))
            .with(CheckmarkColor::new(s.surface)),
    );
    props.insert_stack::<Checkbox>(cb_stack);

    let mut sw_stack = PropertyStack::new();
    sw_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(s.surface_container_highest))
            .with(BorderColor::new(s.outline))
            .with(BorderWidth { width: 1.5.px() })
            .with(ThumbColor(s.on_surface_variant))
            .with(ThumbRadius(Length::px((SWITCH_HEIGHT * 0.30).max(6.0))))
            .with(TrackThickness(Length::px(SWITCH_HEIGHT)))
            .with(CornerRadius {
                radius: (SWITCH_HEIGHT / 2.0).px(),
            }),
    );
    let toggled_cls = "#toggled";
    sw_stack.push_layer(
        Selector::classes(&[toggled_cls]),
        PropertySet::new()
            .with(Background::Color(s.primary))
            .with(BorderColor::new(s.primary))
            .with(ThumbColor(s.on_primary)),
    );
    props.insert_stack::<Switch>(sw_stack);

    let mut rb_stack = PropertyStack::new();
    rb_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(s.outline))
            .with(BorderWidth { width: 2.px() })
            .with(CheckmarkColor::new(s.primary)),
    );
    rb_stack.push_layer(
        Selector::classes(&[classes::CHECKED]),
        PropertySet::new().with(BorderColor::new(s.primary)),
    );
    props.insert_stack::<RadioButton>(rb_stack);

    props.insert::<Slider, _>(TrackColor {
        active: s.primary,
        inactive: s.surface_container_highest,
    });
    props.insert::<Slider, _>(ThumbColor(s.primary));
    props.insert::<Slider, _>(TrackThickness(Length::px(4.0)));
    props.insert::<Slider, _>(ThumbRadius(Length::px(9.0)));

    props.insert::<ProgressBar, _>(TrackColor {
        active: s.primary,
        inactive: s.secondary_container,
    });
    props.insert::<ProgressBar, _>(CornerRadius { radius: 999.px() });
    props.insert::<Spinner, _>(TrackColor {
        active: s.primary,
        inactive: s.secondary_container,
    });
    props.insert::<Spinner, _>(TrackThickness(Length::px(3.5)));

    // TextInput + composite UiTextField Flex host share the same shallow outlined/filled rules.
    let base_pad = Padding::from_vh(INPUT_PAD_V.px(), INPUT_PAD_H.px());
    let mut tf_stack = PropertyStack::new();
    push_text_field_rules(&mut tf_stack, theme, base_pad, true);
    tf_stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_FLUSH]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(Color::TRANSPARENT))
            .with(BorderWidth { width: 0.px() })
            .with(CornerRadius { radius: 0.px() })
            .with(Padding::ZERO)
            .with(Dimensions::height(Dim::MaxContent))
            .with(PlaceholderColor::new(s.on_surface_variant)),
    );
    props.insert_stack::<TextInput>(tf_stack);

    let mut flex_stack = PropertyStack::new();
    push_text_field_rules(&mut flex_stack, theme, base_pad, false);
    props.insert_stack::<Flex>(flex_stack);
}

fn push_text_field_rules(
    stack: &mut PropertyStack,
    theme: &Theme,
    pad: Padding,
    include_placeholder: bool,
) {
    let s = &theme.scheme;
    let mut outlined = PropertySet::new()
        .with(Background::Color(match theme.brightness {
            Brightness::Light => s.surface,
            Brightness::Dark => s.surface_container_low,
        }))
        .with(BorderColor::new(s.outline))
        .with(BorderWidth { width: 1.px() })
        .with(CornerRadius {
            radius: INPUT_RADIUS.px(),
        })
        .with(pad);
    let mut filled = PropertySet::new()
        .with(Background::Color(s.surface_container_highest))
        .with(BorderColor::new(s.outline_variant))
        .with(BorderWidth { width: 1.px() })
        .with(CornerRadius {
            radius: INPUT_RADIUS.px(),
        })
        .with(pad);
    if include_placeholder {
        outlined = outlined.with(PlaceholderColor::new(s.on_surface_variant));
        filled = filled.with(PlaceholderColor::new(s.on_surface_variant));
    }
    stack.push_layer(Selector::classes(&[classes::TEXT_FIELD_OUTLINED]), outlined);
    stack.push_layer(Selector::classes(&[classes::TEXT_FIELD_FILLED]), filled);
    for cls in [classes::TEXT_FIELD_OUTLINED, classes::TEXT_FIELD_FILLED] {
        stack.push_layer(
            Selector::classes(&[cls]).with_hovered(true),
            PropertySet::new().with(BorderColor::new(s.on_surface)),
        );
        stack.push_layer(
            Selector::classes(&[cls]).with_focused(true),
            PropertySet::new()
                .with(BorderColor::new(s.primary))
                .with(BorderWidth { width: 1.px() }),
        );
    }
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_ERROR]),
        PropertySet::new()
            .with(BorderColor::new(s.error))
            .with(BorderWidth { width: 1.px() }),
    );
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_OUTLINED]).with_disabled(true),
        PropertySet::new()
            .with(Background::Color(theme.disabled_container()))
            .with(BorderColor::new(with_opacity(s.on_surface, 0.16))),
    );
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_FILLED]).with_disabled(true),
        PropertySet::new()
            .with(Background::Color(theme.disabled_container()))
            .with(BorderColor::new(Color::TRANSPARENT)),
    );
}

fn configure_scrollbars(props: &mut DefaultProperties, theme: &Theme) {
    let style = ScrollBarStyle::from_theme(theme);
    props.insert::<ScrollViewWidget, ScrollBarStyle>(style);
    props.insert::<UiScrollBar, ScrollBarStyle>(style);
}

/// Builds a complete Masonry [`DefaultProperties`] bundle for `theme`.
#[must_use]
pub fn build_default_properties(theme: &Theme) -> DefaultProperties {
    let mut props = DefaultProperties::new();
    configure_buttons(&mut props, theme);
    configure_surfaces(&mut props, theme);
    configure_inputs(&mut props, theme);
    configure_scrollbars(&mut props, theme);
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_default_properties_handles_light_and_dark_themes_deterministically() {
        for theme in [Theme::light(), Theme::dark()] {
            let a = format!("{:?}", build_default_properties(&theme));
            let b = format!("{:?}", build_default_properties(&theme));
            assert!(!a.is_empty());
            assert!(a.contains("PropertyStack"));
            assert_eq!(a.len(), b.len());
        }
    }
}
