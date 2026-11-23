//! Synthesizes Masonry [`DefaultProperties`] and shallow class `PropertyStack` layers from a [`DesignSpec`].
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
    CheckmarkStrokeWidth, ContentColor, CornerRadius, Dimensions, Gap, Padding, PlaceholderColor,
    SelectionColor, ThumbColor, ThumbRadius, TrackColor, TrackThickness,
};
use masonry::widgets::{
    Button, Checkbox, Divider, Flex, Label, ProgressBar, Prose, RadioButton, SizedBox, Slider,
    Spinner, Switch, TextArea, TextInput,
};

use crate::foundation::FocusFrame;
use crate::foundation::state_layer::{resolve_interactive_colors, with_opacity};
use crate::style_engine::classes;
use crate::tokens::{Brightness, ControlBorderStyle, DesignSpec, ElevationLevel};

#[expect(
    clippy::too_many_arguments,
    reason = "Internal helper mapping explicit button variant styling parameters"
)]
fn insert_button_variant(
    stack: &mut PropertyStack,
    class: &'static str,
    spec: &DesignSpec,
    base: Color,
    tint: Color,
    border: Option<(Color, f64)>,
    radius: f64,
    padding: Padding,
    dims: Option<Dimensions>,
    shadow: Option<BoxShadow>,
) {
    let states = resolve_interactive_colors(
        base,
        tint,
        spec.disabled_container(),
        spec.brightness,
        spec.feedback,
    );
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
    stack.push_layer(
        Selector::classes(&[class]).with_active(true),
        PropertySet::new().with(Background::Color(states.pressed)),
    );
    stack.push_layer(
        Selector::classes(&[class]).with_disabled(true),
        PropertySet::new().with(Background::Color(states.disabled)),
    );
}

fn configure_buttons(props: &mut DefaultProperties, spec: &DesignSpec) {
    let mut stack = PropertyStack::new();
    let p = &spec.palette;
    let m = &spec.metrics;
    let std_pad = Padding::from_vh(0.px(), m.button_pad_h.px());
    let btn_dims = Some(Dimensions::height(m.button_height.px()));

    insert_button_variant(
        &mut stack,
        classes::BTN_PRIMARY,
        spec,
        p.accent,
        p.on_accent,
        None,
        m.button_radius,
        std_pad,
        btn_dims,
        None,
    );
    let secondary_border = if spec.control_border == ControlBorderStyle::BorderlessFlat {
        None
    } else {
        Some((p.border, 1.0))
    };
    insert_button_variant(
        &mut stack,
        classes::BTN_SECONDARY,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        secondary_border,
        m.button_radius,
        std_pad,
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_ELEVATED,
        spec,
        p.surface_high,
        p.accent,
        Some((p.border, 1.0)),
        m.button_radius,
        std_pad,
        btn_dims,
        Some(spec.box_shadow(ElevationLevel::Level1)),
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_OUTLINED,
        spec,
        Color::TRANSPARENT,
        p.text_primary,
        Some((p.border_strong, 1.0)),
        m.button_radius,
        std_pad,
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_SUBTLE,
        spec,
        Color::TRANSPARENT,
        p.accent,
        None,
        m.button_radius,
        Padding::from_vh(0.px(), 10.px()),
        btn_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::BTN_DESTRUCTIVE,
        spec,
        p.error,
        p.on_error,
        None,
        m.button_radius,
        std_pad,
        btn_dims,
        None,
    );

    let icon_dims = Some(Dimensions::fixed(
        m.icon_btn_size.px(),
        m.icon_btn_size.px(),
    ));
    let icon_pad = Padding::ZERO;
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_STD,
        spec,
        Color::TRANSPARENT,
        p.text_secondary,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_STD_SELECTED,
        spec,
        with_opacity(p.accent, 0.14),
        p.accent,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_FILLED,
        spec,
        p.accent,
        p.on_accent,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_FILLED_UNSELECTED,
        spec,
        p.surface_highest,
        p.accent,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_TONAL,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_TONAL_UNSELECTED,
        spec,
        p.surface_highest,
        p.text_secondary,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_OUTLINED,
        spec,
        Color::TRANSPARENT,
        p.text_secondary,
        Some((p.border_strong, 1.0)),
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::ICON_BTN_OUTLINED_SELECTED,
        spec,
        p.inverse_surface,
        p.inverse_on_surface,
        None,
        m.icon_btn_radius,
        icon_pad,
        icon_dims,
        None,
    );

    let fab_pad = Padding::all(16.px());
    let fab_dims = Some(Dimensions::height(56.px()));
    let fab_shadow = Some(spec.box_shadow(ElevationLevel::Level3));
    for (cls, bg, fg) in [
        (
            classes::FAB_PRIMARY,
            p.accent_container,
            p.on_accent_container,
        ),
        (classes::FAB_SURFACE, p.surface_high, p.accent),
        (
            classes::FAB_SECONDARY,
            p.secondary_container,
            p.on_secondary_container,
        ),
        (
            classes::FAB_TERTIARY,
            p.tertiary_container,
            p.on_tertiary_container,
        ),
    ] {
        insert_button_variant(
            &mut stack, cls, spec, bg, fg, None, 16.0, fab_pad, fab_dims, fab_shadow,
        );
    }

    let chip_pad = Padding::from_vh(0.px(), 12.px());
    let chip_dims = Some(Dimensions::height(m.chip_height.px()));
    insert_button_variant(
        &mut stack,
        classes::CHIP_ASSIST,
        spec,
        p.surface,
        p.text_primary,
        Some((p.border, 1.0)),
        m.chip_radius,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_FILTER,
        spec,
        Color::TRANSPARENT,
        p.text_primary,
        Some((p.border_strong, 1.0)),
        m.chip_radius,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_FILTER_SELECTED,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        None,
        m.chip_radius,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_INPUT,
        spec,
        p.surface_container,
        p.text_primary,
        Some((p.border, 1.0)),
        m.chip_radius,
        chip_pad,
        chip_dims,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::CHIP_SUGGESTION,
        spec,
        Color::TRANSPARENT,
        p.text_secondary,
        Some((p.border, 1.0)),
        m.chip_radius,
        chip_pad,
        chip_dims,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::NAV_PILL,
        spec,
        Color::TRANSPARENT,
        p.text_primary,
        None,
        m.nav_pill_radius,
        Padding::from_vh(6.px(), 10.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::NAV_PILL_ACTIVE,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        None,
        m.nav_pill_radius,
        Padding::from_vh(6.px(), 10.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::SEG_BTN,
        spec,
        p.surface,
        p.text_primary,
        Some((p.border, 1.0)),
        m.button_radius,
        Padding::from_vh(6.px(), 14.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::SEG_BTN_SELECTED,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        Some((p.border_strong, 1.0)),
        m.button_radius,
        Padding::from_vh(6.px(), 14.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::LIST_ITEM,
        spec,
        Color::TRANSPARENT,
        p.text_primary,
        None,
        m.nav_pill_radius,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::LIST_ITEM_SELECTED,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        None,
        m.nav_pill_radius,
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::MENU_ITEM,
        spec,
        Color::TRANSPARENT,
        p.text_primary,
        None,
        (m.menu_radius - 2.0).max(4.0),
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );
    insert_button_variant(
        &mut stack,
        classes::MENU_ITEM_SELECTED,
        spec,
        p.secondary_container,
        p.on_secondary_container,
        None,
        (m.menu_radius - 2.0).max(4.0),
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    insert_button_variant(
        &mut stack,
        classes::TAB_ITEM,
        spec,
        Color::TRANSPARENT,
        p.accent,
        None,
        (m.button_radius - 2.0).max(4.0),
        Padding::from_vh(8.px(), 12.px()),
        None,
        None,
    );

    let scrim_bg = with_opacity(p.scrim, 0.36);
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

fn configure_surfaces(props: &mut DefaultProperties, spec: &DesignSpec) {
    let p = &spec.palette;
    let m = &spec.metrics;
    props.insert::<Flex, _>(Gap::ZERO);
    props.insert::<Divider, _>(ContentColor::new(p.divider));

    let mut sized_stack = PropertyStack::new();
    let card_pad = Padding::all(m.card_pad.px());
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_ELEVATED]),
        PropertySet::new()
            .with(Background::Color(p.surface_high))
            .with(CornerRadius {
                radius: m.card_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(card_pad)
            .with(spec.box_shadow(ElevationLevel::Level1)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_FILLED]),
        PropertySet::new()
            .with(Background::Color(p.surface_container))
            .with(CornerRadius {
                radius: m.card_radius.px(),
            })
            .with(BorderWidth { width: 0.px() })
            .with(card_pad)
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::CARD_OUTLINED]),
        PropertySet::new()
            .with(Background::Color(p.surface))
            .with(CornerRadius {
                radius: m.card_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(card_pad)
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::GROUPED_CARD]),
        PropertySet::new()
            .with(Background::Color(p.surface))
            .with(CornerRadius {
                radius: m.card_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(BoxShadow::default()),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::DIALOG_SURFACE]),
        PropertySet::new()
            .with(Background::Color(p.surface_high))
            .with(CornerRadius {
                radius: m.dialog_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(spec.box_shadow(ElevationLevel::Level3)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::SHEET_SURFACE]),
        PropertySet::new()
            .with(Background::Color(p.surface_low))
            .with(CornerRadius {
                radius: m.sheet_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(spec.box_shadow(ElevationLevel::Level2)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::POPOVER_SURFACE]),
        PropertySet::new()
            .with(Background::Color(p.surface_high))
            .with(CornerRadius {
                radius: m.menu_radius.px(),
            })
            .with(BorderWidth { width: 1.px() })
            .with(BorderColor::new(p.border))
            .with(spec.box_shadow(ElevationLevel::Level2)),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::HEADER_BAR]),
        PropertySet::new()
            .with(Background::Color(p.header_bar_bg))
            .with(BorderWidth { width: 0.px() }),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BOTTOM_BAR]),
        PropertySet::new()
            .with(Background::Color(p.surface_container))
            .with(BorderWidth { width: 0.px() }),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BADGE_SMALL]),
        PropertySet::new()
            .with(Background::Color(p.error))
            .with(CornerRadius { radius: 999.px() })
            .with(BorderWidth { width: 0.px() })
            .with(Dimensions::fixed(6.px(), 6.px())),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::BADGE_LARGE]),
        PropertySet::new()
            .with(Background::Color(p.error))
            .with(CornerRadius { radius: 999.px() })
            .with(BorderWidth { width: 0.px() })
            .with(Padding::from_vh(2.px(), 6.px())),
    );
    sized_stack.push_layer(
        Selector::classes(&[classes::CHIP_ROW]),
        PropertySet::new().with(Dimensions::height(m.chip_height.px())),
    );
    props.insert_stack::<SizedBox>(sized_stack);

    let mut focus_stack = PropertyStack::new();
    focus_stack.push_layer(
        Selector::classes(&[classes::SEARCH_BAR]),
        PropertySet::new()
            .with(Background::Color(p.surface_highest))
            .with(BorderColor::new(p.border))
            .with(BorderWidth { width: 1.px() })
            .with(CornerRadius {
                radius: m.search_radius.px(),
            })
            .with(Dimensions::height(m.search_height.px())),
    );
    props.insert_stack::<FocusFrame>(focus_stack);
}

fn configure_inputs(props: &mut DefaultProperties, spec: &DesignSpec) {
    let p = &spec.palette;
    let m = &spec.metrics;

    props.insert::<Label, _>(ContentColor::new(p.text_primary));
    props.insert::<Prose, _>(ContentColor::new(p.text_primary));
    props.insert::<TextArea<true>, _>(CaretColor { color: p.accent });
    props.insert::<TextArea<true>, _>(SelectionColor {
        color: with_opacity(p.accent, 0.28),
    });

    // Switches, Checkboxes, Radios, Sliders, Progress
    let mut cb_stack = PropertyStack::new();
    cb_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(p.border_strong))
            .with(BorderWidth { width: 2.px() })
            .with(CornerRadius {
                radius: m.checkbox_radius.px(),
            })
            .with(CheckmarkColor::new(p.on_accent))
            .with(CheckmarkStrokeWidth { width: 2.0 }),
    );
    cb_stack.push_layer(
        Selector::classes(&[classes::CHECKED]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(p.accent))
            .with(BorderWidth { width: 2.px() })
            .with(CheckmarkColor::new(p.accent)),
    );
    cb_stack.push_layer(
        Selector::new().with_disabled(true),
        PropertySet::new()
            .with(Background::Color(spec.disabled_container()))
            .with(BorderColor::new(spec.disabled_content()))
            .with(CheckmarkColor::new(p.surface)),
    );
    props.insert_stack::<Checkbox>(cb_stack);

    let mut sw_stack = PropertyStack::new();
    sw_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(p.surface_highest))
            .with(BorderColor::new(p.border_strong))
            .with(BorderWidth { width: 1.5.px() })
            .with(ThumbColor(p.text_secondary))
            .with(ThumbRadius(Length::px((m.switch_height * 0.30).max(6.0))))
            .with(TrackThickness(Length::px(m.switch_height)))
            .with(CornerRadius {
                radius: (m.switch_height / 2.0).px(),
            }),
    );
    let toggled_cls = "#toggled";
    sw_stack.push_layer(
        Selector::classes(&[toggled_cls]),
        PropertySet::new()
            .with(Background::Color(p.accent))
            .with(BorderColor::new(p.accent))
            .with(ThumbColor(p.on_accent)),
    );
    props.insert_stack::<Switch>(sw_stack);

    let mut rb_stack = PropertyStack::new();
    rb_stack.push_layer(
        Selector::new(),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(p.border_strong))
            .with(BorderWidth { width: 2.px() })
            .with(CheckmarkColor::new(p.accent)),
    );
    rb_stack.push_layer(
        Selector::classes(&[classes::CHECKED]),
        PropertySet::new().with(BorderColor::new(p.accent)),
    );
    props.insert_stack::<RadioButton>(rb_stack);

    props.insert::<Slider, _>(TrackColor {
        active: p.accent,
        inactive: p.surface_highest,
    });
    props.insert::<Slider, _>(ThumbColor(p.accent));
    props.insert::<Slider, _>(TrackThickness(Length::px(4.0)));
    props.insert::<Slider, _>(ThumbRadius(Length::px(9.0)));

    props.insert::<ProgressBar, _>(TrackColor {
        active: p.accent,
        inactive: p.secondary_container,
    });
    props.insert::<ProgressBar, _>(CornerRadius { radius: 999.px() });
    props.insert::<Spinner, _>(TrackColor {
        active: p.accent,
        inactive: p.secondary_container,
    });
    props.insert::<Spinner, _>(TrackThickness(Length::px(3.5)));

    // TextInput + composite UiTextField Flex host share the same shallow outlined/filled rules.
    let base_pad = Padding::from_vh(m.input_pad_v.px(), m.input_pad_h.px());
    let mut tf_stack = PropertyStack::new();
    push_text_field_rules(&mut tf_stack, spec, base_pad, true);
    tf_stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_FLUSH]),
        PropertySet::new()
            .with(Background::Color(Color::TRANSPARENT))
            .with(BorderColor::new(Color::TRANSPARENT))
            .with(BorderWidth { width: 0.px() })
            .with(CornerRadius { radius: 0.px() })
            .with(Padding::ZERO)
            .with(Dimensions::height(Dim::MaxContent))
            .with(PlaceholderColor::new(p.text_secondary)),
    );
    props.insert_stack::<TextInput>(tf_stack);

    let mut flex_stack = PropertyStack::new();
    push_text_field_rules(&mut flex_stack, spec, base_pad, false);
    props.insert_stack::<Flex>(flex_stack);
}

fn push_text_field_rules(
    stack: &mut PropertyStack,
    spec: &DesignSpec,
    pad: Padding,
    include_placeholder: bool,
) {
    let p = &spec.palette;
    let m = &spec.metrics;
    let mut outlined = PropertySet::new()
        .with(Background::Color(match spec.brightness {
            Brightness::Light => p.surface,
            Brightness::Dark => p.surface_low,
        }))
        .with(BorderColor::new(p.border_strong))
        .with(BorderWidth { width: 1.px() })
        .with(CornerRadius {
            radius: m.input_radius.px(),
        })
        .with(pad);
    let mut filled = PropertySet::new()
        .with(Background::Color(p.surface_highest))
        .with(BorderColor::new(p.border))
        .with(BorderWidth { width: 1.px() })
        .with(CornerRadius {
            radius: m.input_radius.px(),
        })
        .with(pad);
    if include_placeholder {
        outlined = outlined.with(PlaceholderColor::new(p.text_secondary));
        filled = filled.with(PlaceholderColor::new(p.text_secondary));
    }
    stack.push_layer(Selector::classes(&[classes::TEXT_FIELD_OUTLINED]), outlined);
    stack.push_layer(Selector::classes(&[classes::TEXT_FIELD_FILLED]), filled);
    for cls in [classes::TEXT_FIELD_OUTLINED, classes::TEXT_FIELD_FILLED] {
        stack.push_layer(
            Selector::classes(&[cls]).with_hovered(true),
            PropertySet::new().with(BorderColor::new(p.text_primary)),
        );
        stack.push_layer(
            Selector::classes(&[cls]).with_focused(true),
            PropertySet::new()
                .with(BorderColor::new(p.accent))
                .with(BorderWidth { width: 1.px() }),
        );
    }
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_ERROR]),
        PropertySet::new()
            .with(BorderColor::new(p.error))
            .with(BorderWidth { width: 1.px() }),
    );
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_OUTLINED]).with_disabled(true),
        PropertySet::new()
            .with(Background::Color(spec.disabled_container()))
            .with(BorderColor::new(with_opacity(p.text_primary, 0.16))),
    );
    stack.push_layer(
        Selector::classes(&[classes::TEXT_FIELD_FILLED]).with_disabled(true),
        PropertySet::new()
            .with(Background::Color(spec.disabled_container()))
            .with(BorderColor::new(Color::TRANSPARENT)),
    );
}

/// Builds a complete Masonry [`DefaultProperties`] bundle for `spec`.
#[must_use]
pub fn build_default_properties(spec: &DesignSpec) -> DefaultProperties {
    let mut props = DefaultProperties::new();
    configure_buttons(&mut props, spec);
    configure_surfaces(&mut props, spec);
    configure_inputs(&mut props, spec);
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_default_properties_handles_light_and_dark_specs_deterministically() {
        for brightness in [Brightness::Light, Brightness::Dark] {
            let spec = DesignSpec::neutral_fallback(brightness);
            let a = format!("{:?}", build_default_properties(&spec));
            let b = format!("{:?}", build_default_properties(&spec));
            assert!(!a.is_empty());
            assert!(a.contains("PropertyStack"));
            assert_eq!(a.len(), b.len());
        }
    }
}
