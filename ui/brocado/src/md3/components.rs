//! Material Design 3 signature components (FAB, Extended FAB, Top/Bottom App Bars) with concrete `WidgetView`s.

use std::sync::Arc;

use crate::components::{
    NavDestination, headline_medium, headline_small, surface_box, title_large, vertical_divider,
};
use crate::foundation::{ConstrainedBox, UiClassExt, clip_box, constrained_box, ui_icon, wrap};
use crate::style_engine::{classes, with_spec};
use crate::tokens::{DesignSpec, ElevationLevel, TypeRole};
use masonry::core::ArcStr;
use masonry::layout::AsUnit;
use masonry::properties::{Dimensions, LineBreaking, Padding};
use masonry::widgets;
use xilem_masonry::AnyWidgetView;
use xilem_masonry::WidgetView;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt, MainAxisAlignment, button, flex_col, flex_row, label, sized_box,
};

use crate::TextAlign;

/// Material 3 FAB tonal color mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FabColor {
    /// Primary accent container (`FAB_PRIMARY`).
    #[default]
    Primary,
    /// Elevated surface container (`FAB_SURFACE`).
    Surface,
    /// Secondary container (`FAB_SECONDARY`).
    Secondary,
    /// Tertiary container (`FAB_TERTIARY`).
    Tertiary,
}

impl FabColor {
    const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => classes::FAB_PRIMARY,
            Self::Surface => classes::FAB_SURFACE,
            Self::Secondary => classes::FAB_SECONDARY,
            Self::Tertiary => classes::FAB_TERTIARY,
        }
    }

    const fn foreground(self, spec: &DesignSpec) -> masonry::peniko::Color {
        let p = &spec.palette;
        match self {
            Self::Primary => p.on_accent_container,
            Self::Surface => p.accent,
            Self::Secondary => p.on_secondary_container,
            Self::Tertiary => p.on_tertiary_container,
        }
    }
}

/// Material 3 FAB size tokens (`Small 40px`, `Medium 56px`, `Large 96px`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FabSize {
    /// Compact `40×40` FAB (`12px` radius, `20px` icon).
    Small,
    /// Standard `56×56` FAB (`16px` radius, `24px` icon).
    #[default]
    Medium,
    /// Prominent `96×96` FAB (`28px` radius, `36px` icon).
    Large,
}

impl FabSize {
    const fn box_px(self) -> f64 {
        match self {
            Self::Small => 40.0,
            Self::Medium => 56.0,
            Self::Large => 96.0,
        }
    }

    const fn icon_px(self) -> f64 {
        match self {
            Self::Small => 20.0,
            Self::Medium => 24.0,
            Self::Large => 36.0,
        }
    }

    const fn radius_px(self) -> f64 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 16.0,
            Self::Large => 28.0,
        }
    }
}

/// Material 3 Floating Action Button parameterized by [`FabSize`] and [`FabColor`], returning `Pod<widgets::Button>`.
pub fn sized_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    size: FabSize,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let fg = color.foreground(spec);
        let cb = cb.clone();
        button(
            ui_icon(svg_path, size.icon_px(), fg),
            move |s: &mut State| cb(s),
        )
        .ui_class(color.class_name())
        .corner_radius(size.radius_px().px())
        .dims(Dimensions::fixed(size.box_px().px(), size.box_px().px()))
    })
}

/// Material 3 `40×40` Small Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn small_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Small, color, on_click)
}

/// Material 3 `56×56` Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Medium, color, on_click)
}

/// Material 3 `96×96` Large Floating Action Button returning `Pod<widgets::Button>` directly.
pub fn large_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    sized_fab(svg_path, FabSize::Large, color, on_click)
}

/// Material 3 extended pill FAB combining icon + text label, returning `Pod<widgets::Button>` directly.
pub fn extended_fab<State: 'static, Action: 'static, F>(
    svg_path: &'static str,
    text: impl Into<ArcStr>,
    color: FabColor,
    on_click: F,
) -> impl WidgetView<State, Action, Widget = widgets::Button>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let text: ArcStr = text.into();
    let cb = Arc::new(on_click);
    with_spec(move |spec: &DesignSpec, _| {
        let fg = color.foreground(spec);
        let cb = cb.clone();
        let lbl = spec
            .typography
            .style_label(TypeRole::LabelLarge, label(text.clone()))
            .color(fg);
        button(
            flex_row((ui_icon(svg_path, 20.0, fg), lbl))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(10.px()),
            move |s: &mut State| cb(s),
        )
        .ui_class(color.class_name())
    })
}

/// Material 3 Top App Bar height and layout variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TopAppBarVariant {
    /// Standard `64px` small top app bar.
    #[default]
    Small,
    /// `64px` bar with centered title.
    CenterAligned,
    /// Two-row `112px` medium expanding top app bar.
    Medium,
    /// Two-row `152px` large expanding top app bar.
    Large,
}

/// Material 3 Top App Bar with open `leading` and `trailing` custom view slots.
pub fn top_app_bar_slots<State: 'static, Action: 'static>(
    variant: TopAppBarVariant,
    title: impl Into<ArcStr>,
    leading: Option<Box<AnyWidgetView<State, Action>>>,
    trailing: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    let title: ArcStr = title.into();
    let trail_row = (!trailing.is_empty()).then(|| {
        flex_row(trailing)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
    });

    let (height, body): (f64, Box<AnyWidgetView<State, Action>>) = match variant {
        TopAppBarVariant::Small => (
            64.0,
            flex_row((
                leading,
                clip_box(title_large(title).line_breaking(LineBreaking::Clip))
                    .fill_available(true)
                    .flex(1.0),
                trail_row,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .padding(Padding::from_vh(8.px(), 6.px()))
            .boxed(),
        ),
        TopAppBarVariant::CenterAligned => (
            64.0,
            flex_row((
                leading,
                clip_box(
                    title_large(title)
                        .text_alignment(TextAlign::Center)
                        .line_breaking(LineBreaking::Clip),
                )
                .fill_available(true)
                .flex(1.0),
                trail_row,
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .padding(Padding::from_vh(8.px(), 6.px()))
            .boxed(),
        ),
        TopAppBarVariant::Medium => {
            let nav_row = flex_row((leading, sized_box(label("")).flex(1.0), trail_row))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px());
            (
                112.0,
                flex_col((
                    nav_row,
                    clip_box(headline_small(title).line_breaking(LineBreaking::Clip)),
                ))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .padding(Padding::from_vh(14.px(), 12.px()))
                .boxed(),
            )
        }
        TopAppBarVariant::Large => {
            let nav_row = flex_row((leading, sized_box(label("")).flex(1.0), trail_row))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px());
            (
                152.0,
                flex_col((
                    nav_row,
                    clip_box(headline_medium(title).line_breaking(LineBreaking::Clip)),
                ))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .padding(Padding::from_vh(18.px(), 12.px()))
                .boxed(),
            )
        }
    };

    sized_box(constrained_box(body).min_height_from_role(TypeRole::TitleLarge, 20.0, height))
        .ui_class(classes::HEADER_BAR)
}

/// Material 3 Top App Bar supporting small, center-aligned, medium, and large variants.
pub fn top_app_bar<State: 'static, Action: 'static>(
    variant: TopAppBarVariant,
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar_slots(variant, title, None, Vec::new())
}

/// Center-aligned Material 3 top app bar.
pub fn center_aligned_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::CenterAligned, title)
}

/// Medium Material 3 top app bar.
pub fn medium_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::Medium, title)
}

/// Large Material 3 top app bar.
pub fn large_top_app_bar<State: 'static, Action: 'static>(
    title: impl Into<ArcStr>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    top_app_bar(TopAppBarVariant::Large, title)
}

/// Material 3 Bottom App Bar docking secondary icon actions alongside an optional FAB.
pub fn bottom_app_bar<State: 'static, Action: 'static>(
    actions: Vec<Box<AnyWidgetView<State, Action>>>,
    trailing_fab: Option<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
    sized_box(
        flex_row((
            wrap(actions)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(4.px())
                .run_gap(4.px())
                .flex(1.0),
            trailing_fab,
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(6.px())
        .padding(Padding::from_vh(10.px(), 10.px())),
    )
    .ui_class(classes::BOTTOM_BAR)
}

/// Material 3 Navigation Rail hosting an optional top header/FAB slot above pill navigation destinations.
pub fn navigation_rail_with_fab<State: 'static, Action: 'static, H, F>(
    header: Option<H>,
    destinations: &[NavDestination],
    selected: usize,
    on_select: F,
) -> impl WidgetView<State, Action, Widget = ConstrainedBox> + use<State, Action, H, F>
where
    H: WidgetView<State, Action>,
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let dests = destinations.to_vec();
    let cb = Arc::new(on_select);
    let nav_items = with_spec(move |spec: &DesignSpec, _| {
        let p = &spec.palette;
        let items: Vec<Box<AnyWidgetView<State, Action>>> = dests
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let active = i == selected;
                let cb = cb.clone();
                let fg = if active {
                    p.on_secondary_container
                } else {
                    p.text_secondary
                };
                let pill_bg = if active {
                    p.secondary_container
                } else {
                    masonry::peniko::Color::TRANSPARENT
                };
                let pill = sized_box(ui_icon(d.icon, 18.0, fg))
                    .padding(Padding::from_vh(5.px(), 14.px()))
                    .background_color(pill_bg)
                    .corner_radius(spec.metrics.nav_pill_radius.px());
                let txt = spec
                    .typography
                    .style_label(TypeRole::LabelSmall, label(d.label.clone()))
                    .color(if active {
                        p.text_primary
                    } else {
                        p.text_secondary
                    });
                button(
                    flex_col((pill, txt))
                        .cross_axis_alignment(CrossAxisAlignment::Center)
                        .gap(4.px()),
                    move |s: &mut State| cb(s, i),
                )
                .ui_class(classes::TAB_ITEM)
                .boxed()
            })
            .collect();
        flex_col(items)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(8.px())
    });

    constrained_box(
        flex_row((
            surface_box(
                ElevationLevel::Level1,
                flex_col((header, nav_items))
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(14.px())
                    .padding(Padding::from_vh(14.px(), 6.px())),
            )
            .radius(0.0)
            .padding(0.0)
            .dims(Dimensions::STRETCH)
            .flex(1.0),
            vertical_divider(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch),
    )
    .max_width(81.0)
    .fill_max_width(true)
}

pub use expressive::{
    FabMenuItem, FloatingToolbarVariant, UiWavyLinearProgress, WavyLinearProgressWidget,
    button_group, fab_menu, floating_toolbar, floating_toolbar_with_variant, split_button,
    wavy_linear_progress,
};

mod expressive {
    use std::f64::consts::TAU;
    use std::marker::PhantomData;
    use std::sync::Arc;

    use crate::components::{filled_icon_button, primary_button, secondary_button};
    use crate::foundation::{BoundedX, BoundedY, ClipBoxWidget, WrapWidget, clip_box, icons, wrap};
    use crate::style_engine::current_spec_arc;
    use crate::tokens::ElevationLevel;
    use masonry::accesskit::{Node, Role};
    use masonry::core::{
        AccessCtx, ArcStr, ChildrenIds, LayoutCtx, MeasureCtx, NoAction, PaintCtx, PropertiesMut,
        PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetMut,
    };
    use masonry::imaging::Painter;
    use masonry::kurbo::{Axis, BezPath, Cap, Line, Point, Size, Stroke};
    use masonry::layout::{AsUnit, LenReq, Length};
    use masonry::peniko::Color;
    use masonry::properties::{Background, BoxShadow, CornerRadius, Padding};
    use masonry::widgets;
    use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
    use xilem_masonry::style::Style as _;
    use xilem_masonry::view::{CrossAxisAlignment, FlexExt as _, flex_col, flex_row};
    use xilem_masonry::{AnyWidgetView, Pod, ViewCtx, WidgetView};

    use super::{FabColor, fab};

    const WAVY_WAVE_LENGTH: f64 = 22.0;

    /// Visual surface style for [`floating_toolbar_with_variant`].
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum FloatingToolbarVariant {
        /// Tonal `Level3` surface container (default M3 toolbar).
        #[default]
        Standard,
        /// Expressive vibrant primary/accent container toolbar.
        Vibrant,
    }

    struct FloatingToolbarSurface<V, State, Action> {
        variant: FloatingToolbarVariant,
        child: V,
        phantom: PhantomData<fn(State) -> Action>,
    }

    impl<V, State, Action> FloatingToolbarSurface<V, State, Action> {
        fn resolve_style(
            variant: FloatingToolbarVariant,
            spec: &crate::tokens::DesignSpec,
        ) -> (Color, BoxShadow) {
            let p = &spec.palette;
            let bg = match variant {
                FloatingToolbarVariant::Standard => {
                    ElevationLevel::Level3.tinted_surface(p.surface, p.accent)
                }
                FloatingToolbarVariant::Vibrant => p.accent_container,
            };
            let shadow = spec.box_shadow(ElevationLevel::Level3);
            (bg, shadow)
        }
    }

    impl<V, State, Action> ViewMarker for FloatingToolbarSurface<V, State, Action> {}

    impl<V, State, Action> View<State, Action, ViewCtx> for FloatingToolbarSurface<V, State, Action>
    where
        State: 'static,
        Action: 'static,
        V: WidgetView<State, Action>,
    {
        type Element = Pod<widgets::SizedBox>;
        type ViewState = ((Color, BoxShadow), V::ViewState);

        fn build(
            &self,
            ctx: &mut ViewCtx,
            app_state: &mut State,
        ) -> (Self::Element, Self::ViewState) {
            let spec = current_spec_arc(ctx);
            let (bg, shadow) = Self::resolve_style(self.variant, &spec);
            let (child_pod, child_state) = self.child.build(ctx, app_state);
            let mut pod = ctx.create_pod(widgets::SizedBox::new(child_pod.new_widget.erased()));
            pod.new_widget.properties.insert(Background::Color(bg));
            pod.new_widget
                .properties
                .insert(CornerRadius { radius: 28.px() });
            pod.new_widget.properties.insert(Padding::all(8.px()));
            pod.new_widget.properties.insert(shadow);
            (pod, ((bg, shadow), child_state))
        }

        fn rebuild(
            &self,
            prev: &Self,
            ((prev_bg, prev_shadow), child_state): &mut Self::ViewState,
            ctx: &mut ViewCtx,
            mut element: Mut<'_, Self::Element>,
            app_state: &mut State,
        ) {
            let spec = current_spec_arc(ctx);
            let (bg, shadow) = Self::resolve_style(self.variant, &spec);
            let id = element.id();
            if *prev_bg != bg {
                ctx.mark_prop_changed::<Background>(id);
                element.insert_prop(Background::Color(bg));
                *prev_bg = bg;
            }
            if *prev_shadow != shadow {
                ctx.mark_prop_changed::<BoxShadow>(id);
                element.insert_prop(shadow);
                *prev_shadow = shadow;
            }
            let mut child = widgets::SizedBox::child_mut(&mut element)
                .expect("FloatingToolbarSurface SizedBox always holds a child");
            self.child
                .rebuild(&prev.child, child_state, ctx, child.downcast(), app_state);
        }

        fn teardown(
            &self,
            (_, child_state): &mut Self::ViewState,
            ctx: &mut ViewCtx,
            mut element: Mut<'_, Self::Element>,
        ) {
            let mut child = widgets::SizedBox::child_mut(&mut element)
                .expect("FloatingToolbarSurface SizedBox always holds a child");
            self.child.teardown(child_state, ctx, child.downcast());
        }

        fn message(
            &self,
            (_, child_state): &mut Self::ViewState,
            message: &mut MessageCtx,
            mut element: Mut<'_, Self::Element>,
            app_state: &mut State,
        ) -> MessageResult<Action> {
            let mut child = widgets::SizedBox::child_mut(&mut element)
                .expect("FloatingToolbarSurface SizedBox always holds a child");
            self.child
                .message(child_state, message, child.downcast(), app_state)
        }
    }

    /// Material 3 Expressive pill-shaped floating toolbar hosting an action row with `Level3` elevation.
    pub fn floating_toolbar<State: 'static, Action: 'static>(
        actions: Vec<Box<AnyWidgetView<State, Action>>>,
    ) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
        floating_toolbar_with_variant(FloatingToolbarVariant::Standard, actions)
    }

    /// Material 3 Expressive pill-shaped floating toolbar parameterized by [`FloatingToolbarVariant`].
    pub fn floating_toolbar_with_variant<State: 'static, Action: 'static>(
        variant: FloatingToolbarVariant,
        actions: Vec<Box<AnyWidgetView<State, Action>>>,
    ) -> impl WidgetView<State, Action, Widget = widgets::SizedBox> {
        FloatingToolbarSurface {
            variant,
            child: wrap(actions)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(6.px())
                .run_gap(6.px())
                .padding(Padding::from_vh(4.px(), 8.px())),
            phantom: PhantomData,
        }
    }

    /// Material 3 Expressive split button pairing a clipped primary action with an attached menu trigger.
    pub fn split_button<State: 'static, Action: 'static, FPrimary, FMenu>(
        label: impl Into<ArcStr>,
        on_primary: FPrimary,
        on_menu: FMenu,
    ) -> impl WidgetView<State, Action, Widget = ClipBoxWidget>
    where
        FPrimary: Fn(&mut State) -> Action + Send + Sync + 'static,
        FMenu: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        clip_box(
            flex_row((
                clip_box(primary_button(label, on_primary).corner_radius(999.px())).flex(1.0),
                filled_icon_button(icons::ARROW_DROP_DOWN, on_menu).corner_radius(12.px()),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(2.px()),
        )
    }

    /// Material 3 Expressive connected button group morphing active segments to full pills (`999px`)
    /// and inactive segments to soft rounded rectangles (`12px`).
    pub fn button_group<State: 'static, Action: 'static, F>(
        items: &[&str],
        selected: usize,
        on_select: F,
    ) -> impl WidgetView<State, Action, Widget = WrapWidget>
    where
        F: Fn(&mut State, usize) -> Action + Send + Sync + Clone + 'static,
    {
        let buttons: Vec<Box<AnyWidgetView<State, Action>>> = items
            .iter()
            .enumerate()
            .map(|(idx, &label)| {
                let on_select = on_select.clone();
                if idx == selected {
                    primary_button(label.to_owned(), move |state| on_select(state, idx))
                        .corner_radius(999.px())
                        .boxed()
                } else {
                    secondary_button(label.to_owned(), move |state| on_select(state, idx))
                        .corner_radius(12.px())
                        .boxed()
                }
            })
            .collect();
        wrap(buttons)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(4.px())
            .run_gap(6.px())
    }

    /// Action entry displayed inside an expanded Material 3 Expressive [`fab_menu`].
    #[expect(missing_debug_implementations, reason = "Holds closure callback")]
    pub struct FabMenuItem<State, Action> {
        /// SVG path rendered as the leading icon of this action pill.
        pub icon: &'static str,
        /// Text label displayed alongside the icon.
        pub label: ArcStr,
        /// Callback invoked when the action pill is pressed.
        pub on_select: Arc<dyn Fn(&mut State) -> Action + Send + Sync>,
    }

    impl<State, Action> FabMenuItem<State, Action> {
        /// Creates a new [`FabMenuItem`] with `icon`, `label`, and click callback.
        pub fn new<F>(icon: &'static str, label: impl Into<ArcStr>, on_select: F) -> Self
        where
            F: Fn(&mut State) -> Action + Send + Sync + 'static,
        {
            Self {
                icon,
                label: label.into(),
                on_select: Arc::new(on_select),
            }
        }
    }

    /// Material 3 Expressive expandable FAB menu stacking tonal action pills above a primary toggle FAB.
    pub fn fab_menu<State: 'static, Action: 'static, FToggle>(
        open: bool,
        on_toggle: FToggle,
        items: Vec<FabMenuItem<State, Action>>,
    ) -> impl WidgetView<State, Action, Widget = widgets::Flex>
    where
        FToggle: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        let item_views = open.then(|| {
            let pills: Vec<Box<AnyWidgetView<State, Action>>> = items
                .into_iter()
                .map(|item| {
                    let cb = item.on_select;
                    secondary_button(item.label, move |state| cb(state))
                        .leading_icon(item.icon)
                        .corner_radius(999.px())
                        .boxed()
                })
                .collect();
            flex_col(pills)
                .cross_axis_alignment(CrossAxisAlignment::End)
                .gap(8.px())
        });
        let toggle_icon = if open { icons::CLOSE } else { icons::ADD };
        flex_col((item_views, fab(toggle_icon, FabColor::Primary, on_toggle)))
            .cross_axis_alignment(CrossAxisAlignment::End)
            .gap(12.px())
    }

    /// Masonry widget painting a Material 3 Expressive sinusoidal wavy linear progress indicator.
    #[derive(Debug)]
    pub struct WavyLinearProgressWidget {
        progress: Option<f64>,
        track_color: Color,
        indicator_color: Color,
        reduced_motion: bool,
        phase: f64,
    }

    impl WavyLinearProgressWidget {
        fn new(
            progress: Option<f64>,
            track_color: Color,
            indicator_color: Color,
            reduced_motion: bool,
        ) -> Self {
            Self {
                progress: progress.map(|p| p.clamp(0.0, 1.0)),
                track_color,
                indicator_color,
                reduced_motion,
                phase: 0.0,
            }
        }

        fn set_progress(this: &mut WidgetMut<'_, Self>, progress: Option<f64>) {
            let next = progress.map(|p| p.clamp(0.0, 1.0));
            if this.widget.progress != next {
                this.widget.progress = next;
                if next.is_none() && !this.widget.reduced_motion {
                    this.ctx.request_anim_frame();
                }
                this.ctx.request_accessibility_update();
                this.ctx.request_paint_only();
            }
        }

        fn set_colors(this: &mut WidgetMut<'_, Self>, track: Color, indicator: Color) {
            if this.widget.track_color != track || this.widget.indicator_color != indicator {
                this.widget.track_color = track;
                this.widget.indicator_color = indicator;
                this.ctx.request_paint_only();
            }
        }

        fn set_reduced_motion(this: &mut WidgetMut<'_, Self>, reduced: bool) {
            if this.widget.reduced_motion != reduced {
                this.widget.reduced_motion = reduced;
                if !reduced && this.widget.progress.is_none() {
                    this.ctx.request_anim_frame();
                }
                this.ctx.request_paint_only();
            }
        }
    }

    impl Widget for WavyLinearProgressWidget {
        type Action = NoAction;

        fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::new()
        }

        fn update(
            &mut self,
            ctx: &mut UpdateCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            event: &Update,
        ) {
            if matches!(event, Update::WidgetAdded)
                && self.progress.is_none()
                && !self.reduced_motion
            {
                ctx.request_anim_frame();
            }
        }

        fn on_anim_frame(
            &mut self,
            ctx: &mut UpdateCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            interval: u64,
        ) {
            if self.progress.is_none() && !self.reduced_motion {
                let dt = (interval as f64) / 1_000_000_000.0;
                self.phase = (self.phase + dt * 1.4).rem_euclid(1.0);
                ctx.request_anim_frame();
                ctx.request_paint_only();
            }
        }

        fn accessibility_role(&self) -> Role {
            Role::ProgressIndicator
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            node: &mut Node,
        ) {
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(1.0);
            if let Some(p) = self.progress {
                node.set_numeric_value(p);
            }
        }

        fn measure(
            &mut self,
            _ctx: &mut MeasureCtx<'_>,
            _props: &PropertiesRef<'_>,
            axis: Axis,
            len_req: LenReq,
            _cross_length: Option<Length>,
        ) -> Length {
            match axis {
                Axis::Horizontal => match len_req {
                    LenReq::MinContent => Length::ZERO,
                    LenReq::MaxContent => Length::px(240.0),
                    LenReq::FitContent(space) => space,
                },
                Axis::Vertical => Length::px(12.0),
            }
        }

        fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

        fn paint(
            &mut self,
            ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            painter: &mut Painter<'_>,
        ) {
            let size = ctx.border_box().size();
            if size.width <= 2.0 || size.height <= 2.0 {
                return;
            }
            let mid_y = size.height * 0.5;
            let pad_x = 2.0_f64.min(size.width * 0.25);
            let span = (size.width - 2.0 * pad_x).max(0.0);
            if span <= 0.5 {
                return;
            }
            let track_stroke = Stroke::new(3.5).with_caps(Cap::Round);
            let track_line = Line::new(Point::new(pad_x, mid_y), Point::new(pad_x + span, mid_y));
            painter
                .stroke(track_line, &track_stroke, self.track_color)
                .draw();

            let (start_frac, end_frac) = match self.progress {
                Some(p) => (0.0, p.clamp(0.0, 1.0)),
                None => {
                    let head = self.phase * 1.35;
                    ((head - 0.35).clamp(0.0, 1.0), head.clamp(0.0, 1.0))
                }
            };
            let start_x = span * start_frac;
            let end_x = span * end_frac;
            if end_x - start_x <= 0.5 {
                return;
            }

            let amp = if self.reduced_motion {
                0.0
            } else {
                (size.height * 0.28).min(3.2)
            };
            let step: f64 = 2.0;
            let mut path = BezPath::new();
            let mut offset = 0.0_f64;
            let active_len = end_x - start_x;
            let first_y = mid_y + amp * ((start_x / WAVY_WAVE_LENGTH + self.phase) * TAU).sin();
            path.move_to(Point::new(pad_x + start_x, first_y));
            while offset < active_len {
                offset = (offset + step).min(active_len);
                let x_local = start_x + offset;
                let y = mid_y + amp * ((x_local / WAVY_WAVE_LENGTH + self.phase) * TAU).sin();
                path.line_to(Point::new(pad_x + x_local, y));
            }
            painter
                .stroke(&path, &track_stroke, self.indicator_color)
                .draw();
        }
    }

    /// Material 3 Expressive wavy linear progress indicator view constructed by [`wavy_linear_progress`].
    #[must_use = "View values do nothing unless provided to Brocado."]
    #[derive(Clone, Copy, Debug)]
    pub struct UiWavyLinearProgress {
        progress: Option<f64>,
    }

    impl BoundedX for UiWavyLinearProgress {}
    impl BoundedY for UiWavyLinearProgress {}

    impl ViewMarker for UiWavyLinearProgress {}

    impl<State: 'static, Action: 'static> View<State, Action, ViewCtx> for UiWavyLinearProgress {
        type Element = Pod<WavyLinearProgressWidget>;
        type ViewState = (Color, Color, bool);

        fn build(
            &self,
            ctx: &mut ViewCtx,
            _app_state: &mut State,
        ) -> (Self::Element, Self::ViewState) {
            let spec = current_spec_arc(ctx);
            let track = spec.palette.accent_container;
            let indicator = spec.palette.accent;
            let reduced = spec.reduced_motion;
            let pod = ctx.create_pod(WavyLinearProgressWidget::new(
                self.progress,
                track,
                indicator,
                reduced,
            ));
            (pod, (track, indicator, reduced))
        }

        fn rebuild(
            &self,
            prev: &Self,
            (prev_track, prev_indicator, prev_reduced): &mut Self::ViewState,
            ctx: &mut ViewCtx,
            mut element: Mut<'_, Self::Element>,
            _app_state: &mut State,
        ) {
            let spec = current_spec_arc(ctx);
            let track = spec.palette.accent_container;
            let indicator = spec.palette.accent;
            let reduced = spec.reduced_motion;
            if prev.progress != self.progress {
                WavyLinearProgressWidget::set_progress(&mut element, self.progress);
            }
            if *prev_track != track || *prev_indicator != indicator {
                WavyLinearProgressWidget::set_colors(&mut element, track, indicator);
                *prev_track = track;
                *prev_indicator = indicator;
            }
            if *prev_reduced != reduced {
                WavyLinearProgressWidget::set_reduced_motion(&mut element, reduced);
                *prev_reduced = reduced;
            }
        }

        fn teardown(
            &self,
            _view_state: &mut Self::ViewState,
            _ctx: &mut ViewCtx,
            _element: Mut<'_, Self::Element>,
        ) {
        }

        fn message(
            &self,
            _view_state: &mut Self::ViewState,
            _message: &mut MessageCtx,
            _element: Mut<'_, Self::Element>,
            _app_state: &mut State,
        ) -> MessageResult<Action> {
            MessageResult::Nop
        }
    }

    /// Material 3 Expressive wavy linear progress indicator (`Some(0.0..=1.0)` determinate, `None` indeterminate).
    pub fn wavy_linear_progress(progress: Option<f64>) -> UiWavyLinearProgress {
        UiWavyLinearProgress { progress }
    }
}
