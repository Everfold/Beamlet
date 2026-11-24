//! Compile-time structural overflow-safety traits, zero-cost axis-locked layout wrappers,
//! bounded type erasure, and auditable escape hatches.
//!
//! # Design overview
//!
//! * **Level 1 (`BoundedX`)**: Proves at compile time that a view fits arbitrarily narrow widths
//!   without overflowing horizontally (by wrapping, scrolling, clipping, or shrinking).
//! * **Level 2 (`BoundedY` and [`Bounded`])**: Extends compile-time safety to the vertical axis and
//!   tracks axis constraints across type-state builders such as [`ScrollView`](super::ScrollView) and
//!   [`ConstrainedBoxView`](super::ConstrainedBoxView).
//!
//! Upstream `xilem_masonry::view::Flex` stores its axis at runtime (`Axis`), and `SizedBox` allows
//! mutating fixed extents after construction. Brocado provides [`column`], [`row`], and
//! [`container`] as zero-cost wrappers delegating directly to those upstream Masonry widgets while
//! keeping axis & sizing invariants visible to Rust's type checker.
//!
//! # Examples
//!
//! Valid dual-axis bounded composition using [`bounded_scaffold`](crate::components::bounded_scaffold),
//! [`scroll_vertical`](crate::foundation::scroll_vertical), [`column`], [`wrap`], [`adaptive_pair`],
//! and bounded modals:
//!
//! ```rust,no_run
//! use brocado::WidgetView;
//! use brocado::components::{
//!     CardVariant, bounded_bottom_sheet, bounded_card, bounded_dialog, bounded_scaffold,
//!     primary_button, ui_label,
//! };
//! use brocado::foundation::{
//!     BoundedViewExt as _, adaptive_pair, clip_box, column, constrained_box, container,
//!     scroll_vertical, wrap,
//! };
//! use brocado::masonry::layout::AsUnit;
//! use brocado::masonry::properties::Padding;
//! use brocado::style::Style as _;
//! use brocado::tokens::TypeRole;
//!
//! fn safe_screen() -> impl WidgetView<()> + brocado::foundation::Bounded {
//!     let card_body = column((
//!         ui_label(TypeRole::TitleMedium, "Safe Section"),
//!         adaptive_pair(
//!             ui_label(TypeRole::BodyMedium, "Adaptive label wraps cleanly"),
//!             primary_button("Action", |()| {}),
//!         ),
//!         wrap((
//!             primary_button("One", |()| {}),
//!             primary_button("Two", |()| {}),
//!         )),
//!     ));
//!
//!     let page = scroll_vertical(
//!         container(column((
//!             constrained_box(bounded_card(CardVariant::Elevated, card_body)).max_width(480.0),
//!             clip_box(ui_label(TypeRole::BodySmall, "Clipped chrome caption")),
//!         )))
//!         .padding(Padding::all(12.px())),
//!     );
//!
//!     bounded_scaffold(page)
//!         .dialog(Some(bounded_dialog(
//!             ui_label(TypeRole::BodyMedium, "Modal content wraps"),
//!             |()| {},
//!         )))
//!         .sheet(Some(bounded_bottom_sheet(
//!             ui_label(TypeRole::BodyMedium, "Sheet content wraps"),
//!             |()| {},
//!         )))
//!         .build()
//!         .assert_bounded()
//! }
//! ```
//!
//! Horizontally scrolling a [`row`] of compact [`BoundedY`] items produces a [`Bounded`] view:
//!
//! ```rust,no_run
//! use brocado::components::primary_button;
//! use brocado::foundation::{bounded, row, scroll_horizontal};
//!
//! let strip = bounded(scroll_horizontal(row((
//!     primary_button("A", |()| {}),
//!     primary_button("B", |()| {}),
//! ))));
//! # let _ = strip;
//! ```
//!
//! Two-axis [`scroll_view`](crate::foundation::scroll_view) absorbs overflow even for raw `flex_row`:
//!
//! ```rust,no_run
//! use brocado::components::primary_button;
//! use brocado::foundation::{bounded, scroll_view};
//! use brocado::view::flex_row;
//!
//! let table = bounded(scroll_view(flex_row((
//!     primary_button("Col 1", |()| {}),
//!     primary_button("Col 2", |()| {}),
//! ))));
//! # let _ = table;
//! ```
//!
//! Type-erased [`BoundedXAnyView`] / [`BoundedAnyView`] preserve bounds across `Vec` collections:
//!
//! ```rust,no_run
//! use brocado::components::{primary_button, ui_label};
//! use brocado::foundation::{
//!     BoundedAnyView, BoundedViewExt as _, BoundedXAnyView, bounded, column, scroll_vertical,
//! };
//! use brocado::tokens::TypeRole;
//!
//! let items: Vec<BoundedXAnyView<()>> = vec![
//!     ui_label(TypeRole::BodyMedium, "Row 1").boxed_bounded_x(),
//!     primary_button("Save", |()| {}).boxed_bounded_x(),
//! ];
//! let boxed_page: BoundedAnyView<()> = scroll_vertical(column(items)).boxed_bounded();
//! let _ = bounded(boxed_page);
//! ```
//!
//! Auditable escape hatches opt manually verified custom widgets into compile-time bounds:
//!
//! ```rust,no_run
//! use brocado::WidgetView;
//! use brocado::components::ui_label;
//! use brocado::foundation::{
//!     Bounded, BoundedX, assume_bounded, assume_bounded_x, bounded, bounded_x,
//! };
//! use brocado::tokens::TypeRole;
//! use brocado::view::flex_row;
//!
//! fn custom_x() -> impl WidgetView<()> + BoundedX {
//!     bounded_x(assume_bounded_x(flex_row(ui_label(
//!         TypeRole::BodyMedium,
//!         "Custom width-safe row",
//!     ))))
//! }
//!
//! fn custom_both() -> impl WidgetView<()> + Bounded {
//!     bounded(assume_bounded(flex_row(ui_label(
//!         TypeRole::BodyMedium,
//!         "Fixed verified canvas",
//!     ))))
//! }
//! # let _ = (custom_x, custom_both);
//! ```
//!
//! ## Compile-time rejections (`compile_fail`)
//!
//! Raw `flex_row` fails `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::bounded_x;
//! use brocado::tokens::TypeRole;
//! use brocado::view::flex_row;
//!
//! let _ = bounded_x(flex_row::<(), (), _>(ui_label(TypeRole::BodyMedium, "Unwrapped row")));
//! ```
//!
//! Raw `flex_col` erases its axis type and therefore does not satisfy `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::bounded_x;
//! use brocado::tokens::TypeRole;
//! use brocado::view::flex_col;
//!
//! let _ = bounded_x(flex_col::<(), (), _>(ui_label(TypeRole::BodyMedium, "Upstream untyped Flex")));
//! ```
//!
//! Fixed-width `sized_box` is rejected by `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::bounded_x;
//! use brocado::masonry::layout::AsUnit;
//! use brocado::tokens::TypeRole;
//! use brocado::view::sized_box;
//!
//! let _ = bounded_x(sized_box::<(), (), _>(ui_label(TypeRole::BodyMedium, "Fixed")).fixed_width(400.px()));
//! ```
//!
//! Adding `.width(...)` (`Prop<Dimensions, ...>`) on [`container`] drops `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::{bounded_x, container};
//! use brocado::masonry::layout::AsUnit;
//! use brocado::style::Style as _;
//! use brocado::tokens::TypeRole;
//!
//! let _ = bounded_x(container::<(), (), _>(ui_label(TypeRole::BodyMedium, "Fixed")).width(400.px()));
//! ```
//!
//! Vertical-only scrolling (`scroll_vertical` / `.constrain_horizontal(true)`) propagates width
//! constraints inward, so wrapping an unbounded `flex_row` fails `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::{bounded_x, scroll_vertical};
//! use brocado::tokens::TypeRole;
//! use brocado::view::flex_row;
//!
//! let _ = bounded_x(scroll_vertical(flex_row::<(), (), _>(ui_label(TypeRole::BodyMedium, "Wide"))));
//! ```
//!
//! An unscrolled [`column`] is only horizontally bounded (`BoundedX`), not `Bounded`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::{bounded, column};
//! use brocado::tokens::TypeRole;
//!
//! let _ = bounded(column::<(), (), _>(ui_label(TypeRole::BodyMedium, "Tall content")));
//! ```
//!
//! Setting a positive `.min_width(...)` on [`constrained_box`] transitions `MinX` to `HasMinAxis`
//! and drops `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::{bounded_x, constrained_box};
//! use brocado::tokens::TypeRole;
//!
//! let _ = bounded_x(constrained_box::<(), (), _>(ui_label(TypeRole::BodyMedium, "Floored")).min_width(320.0));
//! ```
//!
//! Setting `.min_height(...)` on [`constrained_box`] transitions `MinY` to `HasMinAxis` and drops
//! [`Bounded`]:
//!
//! ```compile_fail,E0277
//! use brocado::components::ui_label;
//! use brocado::foundation::{bounded, column, constrained_box, scroll_vertical};
//! use brocado::tokens::TypeRole;
//!
//! let page = scroll_vertical(column::<(), (), _>(ui_label(TypeRole::BodyMedium, "Body")));
//! let _ = bounded(constrained_box(page).min_height(400.0));
//! ```
//!
//! Unverified `Box<AnyWidgetView>` (via `.boxed()`) cannot masquerade as `BoundedX`:
//!
//! ```compile_fail,E0277
//! use brocado::WidgetView as _;
//! use brocado::components::ui_label;
//! use brocado::foundation::bounded_x;
//! use brocado::tokens::TypeRole;
//!
//! let _ = bounded_x(ui_label::<(), ()>(TypeRole::BodyMedium, "Unchecked dynamic view").boxed());
//! ```

use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use masonry::kurbo::Axis;
use masonry::properties::{
    Background, BorderColor, BorderWidth, BoxShadow, ContentColor, CornerRadius, Gap, LineBreaking,
    Padding,
};
use masonry::widgets;
use xilem_core::one_of::OneOf2;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::view::{
    CrossAxisAlignment, Divider, Flex, FlexItem, FlexSequence, FlexSpacer, MainAxisAlignment, Prop,
    SizedBox, Spinner, ZStack, ZStackItem, flex_col, flex_row, sized_box,
};
use xilem_masonry::{AnyWidgetView, Pod, ViewCtx, WidgetView};

use crate::components::button::{UiButton, UiCustomButton};
use crate::components::card::SurfaceBox;
use crate::components::chip::UiChip;
use crate::components::dialog::{BasicDialog, UiDialog};
use crate::components::icon_button::{UiCustomIconButton, UiIconButton};
use crate::components::list::UiListItem;
use crate::components::navigation::UiModalDrawer;
use crate::components::progress::{UiDeterminateCircular, UiLinearProgress};
use crate::components::range_slider::RangeSlider;
use crate::components::rich_text::UiRichProse;
use crate::components::search_bar::UiSearchBar;
use crate::components::selection::{UiCheckboxView, UiRadio, UiSliderView, UiSwitchView};
use crate::components::sheet::UiSheet;
use crate::components::split::UiSplit;
use crate::components::text_field::{UiTextArea, UiTextField};
use crate::components::toast::UiToastHost;
use crate::components::typography::{UiLabel, UiProse};
use crate::foundation::adaptive_pair::AdaptivePairView;
use crate::foundation::class_view::WithClass;
use crate::foundation::clip_box::ClipBoxView;
use crate::foundation::constrained_box::ConstrainedBoxView;
use crate::foundation::focus_frame::FocusFrameView;
use crate::foundation::overlay::{
    AnchoredPopupView, ContextMenuView, EscapeCatcherView, HoverTooltipView, PopoverAnchorView,
    PopoverPlacementView,
};
use crate::foundation::ripple::RippleLayerView;
use crate::foundation::scroll_view::{ScrollBarStyle, ScrollView};
use crate::foundation::transition::UiTransition;
use crate::foundation::wrap::WrapView;
use crate::style_engine::scope::{MemoizeWithTheme, ThemeScope, WithTheme};

// ---------------------------------------------------------------------------
// Marker traits
// ---------------------------------------------------------------------------

/// Marker trait asserting a view cannot cause unbounded horizontal overflow inside arbitrary
/// narrow viewports (e.g. via wrapping, clipping, scrolling, or adaptive stacking).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not proven horizontally overflow-safe (`BoundedX`)",
    note = "Wrap rows in `wrap(...)`, `scroll_view(...)`, `scroll_horizontal(...)`, `adaptive_pair(...)`, or `clip_box(...)`; use `column(...)` instead of raw `flex_col(...)`; or mark intentionally fixed content with `assume_bounded_x(...)`."
)]
pub trait BoundedX {}

/// Marker trait asserting a view cannot cause unbounded vertical overflow inside short viewports
/// (e.g. compact controls, horizontally arranged rows of `BoundedY` items, or vertical scrollers).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not proven vertically overflow-safe (`BoundedY`)",
    note = "Wrap tall columns, wrapping text, or flow grids in `scroll_view(...)` / `scroll_vertical(...)`, use `clip_box(...)`, or mark fixed chrome with `assume_bounded_y(...)`."
)]
pub trait BoundedY {}

/// Combined dual-axis overflow-safety marker (`BoundedX + BoundedY`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not proven overflow-safe on both axes (`Bounded = BoundedX + BoundedY`)",
    note = "Use `scroll_view(...)`, `scroll_vertical(column(...))`, `scroll_horizontal(row(...))`, or `assume_bounded(...)`."
)]
pub trait Bounded: BoundedX + BoundedY {}

impl<T: BoundedX + BoundedY + ?Sized> Bounded for T {}

/// Marker trait for `ViewSequence`s whose children are all [`BoundedX`].
#[diagnostic::on_unimplemented(
    message = "Every child in `{Self}` must implement `BoundedX`",
    note = "Replace raw `flex_row(...)`, fixed `sized_box(...)`, or `AnyWidgetView` with `BoundedX` views (`wrap`, `adaptive_pair`, `ui_label`, `BoundedXAnyView`, etc.)."
)]
pub trait BoundedXSeq {}

/// Marker trait for `ViewSequence`s whose children are all [`BoundedY`].
#[diagnostic::on_unimplemented(
    message = "Every child in `{Self}` must implement `BoundedY`",
    note = "Wrap tall children in `scroll_vertical(...)` or `scroll_view(...)` before placing them in this sequence."
)]
pub trait BoundedYSeq {}

impl<T: BoundedX> BoundedXSeq for T {}
impl<T: BoundedY> BoundedYSeq for T {}

impl BoundedXSeq for () {}
impl BoundedYSeq for () {}

impl<T: BoundedXSeq> BoundedXSeq for Option<T> {}
impl<T: BoundedYSeq> BoundedYSeq for Option<T> {}

impl<T: BoundedXSeq> BoundedXSeq for Vec<T> {}
impl<T: BoundedYSeq> BoundedYSeq for Vec<T> {}

impl<T: BoundedXSeq, const N: usize> BoundedXSeq for [T; N] {}
impl<T: BoundedYSeq, const N: usize> BoundedYSeq for [T; N] {}

impl<V: BoundedX, State, Action> BoundedXSeq for FlexItem<V, State, Action> {}
impl<V: BoundedY, State, Action> BoundedYSeq for FlexItem<V, State, Action> {}

// FlexSpacer can safely sit in vertical columns without introducing width overflow;
// fixed spacers enforce minimum height so FlexSpacer is deliberately NOT BoundedYSeq.
impl BoundedXSeq for FlexSpacer {}

impl<V: BoundedX, State, Action> BoundedXSeq for ZStackItem<V, State, Action> {}
impl<V: BoundedY, State, Action> BoundedYSeq for ZStackItem<V, State, Action> {}

macro_rules! impl_bounded_seq_tuples {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: BoundedXSeq),+> BoundedXSeq for ($($name,)+) {}
        impl<$($name: BoundedYSeq),+> BoundedYSeq for ($($name,)+) {}
    };
}

impl_bounded_seq_tuples!(T0);
impl_bounded_seq_tuples!(T0, T1);
impl_bounded_seq_tuples!(T0, T1, T2);
impl_bounded_seq_tuples!(T0, T1, T2, T3);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_bounded_seq_tuples!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);

// ---------------------------------------------------------------------------
// Type-state axis markers (consumed in Level 2 ScrollView & ConstrainedBox)
// ---------------------------------------------------------------------------

/// Type-state marker indicating an axis scrolls freely and absorbs child overflow on that axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScrollAxis;

/// Type-state marker indicating an axis constrains its child to the viewport and therefore
/// requires the child itself to be bounded on that axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FitAxis;

/// Type-state marker indicating a constraint wrapper imposes no positive minimum floor on an axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UnminAxis;

/// Type-state marker indicating a constraint wrapper enforces a positive minimum extent floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HasMinAxis;

// ---------------------------------------------------------------------------
// Axis-locked zero-cost layout views: column / row / container
// ---------------------------------------------------------------------------

/// Vertical `Flex` view locked to [`Axis::Vertical`] at the type level.
///
/// Implements [`BoundedX`] whenever every child in `Seq` implements [`BoundedX`], avoiding the
/// limitation where upstream `xilem_masonry::view::Flex` stores direction dynamically.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Wraps upstream xilem_masonry Flex which does not derive Debug"
)]
pub struct ColumnView<Seq, State, Action = ()> {
    inner: Flex<Seq, State, Action>,
}

/// Arranges `sequence` vertically; implements [`BoundedX`] when every child is [`BoundedX`].
pub fn column<State: 'static, Action, Seq: FlexSequence<State, Action>>(
    sequence: Seq,
) -> ColumnView<Seq, State, Action> {
    ColumnView {
        inner: flex_col(sequence),
    }
}

impl<Seq, State, Action> ColumnView<Seq, State, Action> {
    /// Sets the children's [`CrossAxisAlignment`].
    pub fn cross_axis_alignment(mut self, align: CrossAxisAlignment) -> Self {
        self.inner = self.inner.cross_axis_alignment(align);
        self
    }

    /// Sets the children's [`MainAxisAlignment`].
    pub fn main_axis_alignment(mut self, align: MainAxisAlignment) -> Self {
        self.inner = self.inner.main_axis_alignment(align);
        self
    }
}

impl<Seq, State, Action> ViewMarker for ColumnView<Seq, State, Action> {}

impl<State, Action, Seq> View<State, Action, ViewCtx> for ColumnView<Seq, State, Action>
where
    State: 'static,
    Action: 'static,
    Seq: FlexSequence<State, Action>,
{
    type Element = Pod<widgets::Flex>;
    type ViewState = <Flex<Seq, State, Action> as View<State, Action, ViewCtx>>::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.inner.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.inner
            .rebuild(&prev.inner, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.inner.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.inner.message(view_state, message, element, app_state)
    }
}

impl<Seq: BoundedXSeq, State, Action> BoundedX for ColumnView<Seq, State, Action> {}

/// Horizontal `Flex` view locked to [`Axis::Horizontal`] at the type level.
///
/// Implements [`BoundedY`] whenever every child is [`BoundedY`], and deliberately does **not**
/// implement [`BoundedX`] unless wrapped in [`scroll_view`](super::scroll_view) /
/// [`scroll_horizontal`](super::scroll_horizontal) / [`clip_box`](super::clip_box).
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Wraps upstream xilem_masonry Flex which does not derive Debug"
)]
pub struct RowView<Seq, State, Action = ()> {
    inner: Flex<Seq, State, Action>,
}

/// Arranges `sequence` horizontally; implements [`BoundedY`] when every child is [`BoundedY`].
pub fn row<State: 'static, Action, Seq: FlexSequence<State, Action>>(
    sequence: Seq,
) -> RowView<Seq, State, Action> {
    let _ = Axis::Horizontal;
    RowView {
        inner: flex_row(sequence),
    }
}

impl<Seq, State, Action> RowView<Seq, State, Action> {
    /// Sets the children's [`CrossAxisAlignment`].
    pub fn cross_axis_alignment(mut self, align: CrossAxisAlignment) -> Self {
        self.inner = self.inner.cross_axis_alignment(align);
        self
    }

    /// Sets the children's [`MainAxisAlignment`].
    pub fn main_axis_alignment(mut self, align: MainAxisAlignment) -> Self {
        self.inner = self.inner.main_axis_alignment(align);
        self
    }
}

impl<Seq, State, Action> ViewMarker for RowView<Seq, State, Action> {}

impl<State, Action, Seq> View<State, Action, ViewCtx> for RowView<Seq, State, Action>
where
    State: 'static,
    Action: 'static,
    Seq: FlexSequence<State, Action>,
{
    type Element = Pod<widgets::Flex>;
    type ViewState = <Flex<Seq, State, Action> as View<State, Action, ViewCtx>>::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.inner.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.inner
            .rebuild(&prev.inner, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.inner.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.inner.message(view_state, message, element, app_state)
    }
}

impl<Seq: BoundedYSeq, State, Action> BoundedY for RowView<Seq, State, Action> {}

/// Overflow-safe style container delegating to `widgets::SizedBox` without exposing `.fixed_width`
/// or `.fixed_height`.
///
/// Propagates [`BoundedX`] and [`BoundedY`] from `V`. Calling `.padding()`, `.background()`,
/// `.border()`, `.corner_radius()`, `.box_shadow()`, or `.ui_class()` preserves those bounds,
/// whereas `.dims()`/`.width()`/`.height()` wraps in `Prop<Dimensions, ...>` and drops them.
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Wraps upstream xilem_masonry SizedBox which does not derive Debug"
)]
pub struct ContainerView<V, State, Action = ()> {
    inner: SizedBox<V, State, Action>,
}

/// Wraps `child` in an auto-sized `SizedBox` container preserving [`BoundedX`] and [`BoundedY`].
pub fn container<State: 'static, Action: 'static, V: WidgetView<State, Action>>(
    child: V,
) -> ContainerView<V, State, Action> {
    ContainerView {
        inner: sized_box(child),
    }
}

impl<V, State, Action> ViewMarker for ContainerView<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for ContainerView<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = Pod<widgets::SizedBox>;
    type ViewState = <SizedBox<V, State, Action> as View<State, Action, ViewCtx>>::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.inner.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.inner
            .rebuild(&prev.inner, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.inner.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.inner.message(view_state, message, element, app_state)
    }
}

impl<V: BoundedX, State, Action> BoundedX for ContainerView<V, State, Action> {}
impl<V: BoundedY, State, Action> BoundedY for ContainerView<V, State, Action> {}

// ---------------------------------------------------------------------------
// Bounded type erasure wrappers
// ---------------------------------------------------------------------------

/// Type-erased [`WidgetView`] whose inner view was statically verified to implement [`BoundedX`].
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Holds Box<AnyWidgetView<State, Action>>"
)]
pub struct BoundedXAnyView<State: 'static, Action: 'static = ()> {
    inner: Box<AnyWidgetView<State, Action>>,
}

impl<State: 'static, Action: 'static> BoundedXAnyView<State, Action> {
    /// Boxes any concrete view that satisfies [`BoundedX`].
    pub fn new<V: WidgetView<State, Action> + BoundedX>(view: V) -> Self {
        Self {
            inner: Box::new(view),
        }
    }

    /// Unwraps the inner boxed `AnyWidgetView`.
    #[must_use]
    pub fn into_inner(self) -> Box<AnyWidgetView<State, Action>> {
        self.inner
    }
}

impl<State: 'static, Action: 'static> ViewMarker for BoundedXAnyView<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for BoundedXAnyView<State, Action>
{
    type Element = Pod<widgets::Passthrough>;
    type ViewState = <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.inner.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.inner
            .rebuild(&prev.inner, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.inner.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.inner.message(view_state, message, element, app_state)
    }
}

impl<State: 'static, Action: 'static> BoundedX for BoundedXAnyView<State, Action> {}

/// Type-erased [`WidgetView`] whose inner view was statically verified to implement both
/// [`BoundedX`] and [`BoundedY`] ([`Bounded`]).
#[must_use = "View values do nothing unless provided to Brocado."]
#[expect(
    missing_debug_implementations,
    reason = "Holds Box<AnyWidgetView<State, Action>>"
)]
pub struct BoundedAnyView<State: 'static, Action: 'static = ()> {
    inner: Box<AnyWidgetView<State, Action>>,
}

impl<State: 'static, Action: 'static> BoundedAnyView<State, Action> {
    /// Boxes any concrete view that satisfies [`Bounded`] (`BoundedX + BoundedY`).
    pub fn new<V: WidgetView<State, Action> + Bounded>(view: V) -> Self {
        Self {
            inner: Box::new(view),
        }
    }

    /// Weakens a dual-axis bounded boxed view into a horizontal-only [`BoundedXAnyView`].
    pub fn into_bounded_x(self) -> BoundedXAnyView<State, Action> {
        BoundedXAnyView { inner: self.inner }
    }

    /// Unwraps the inner boxed `AnyWidgetView`.
    #[must_use]
    pub fn into_inner(self) -> Box<AnyWidgetView<State, Action>> {
        self.inner
    }
}

impl<State: 'static, Action: 'static> ViewMarker for BoundedAnyView<State, Action> {}

impl<State: 'static, Action: 'static> View<State, Action, ViewCtx>
    for BoundedAnyView<State, Action>
{
    type Element = Pod<widgets::Passthrough>;
    type ViewState = <Box<AnyWidgetView<State, Action>> as View<State, Action, ViewCtx>>::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.inner.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.inner
            .rebuild(&prev.inner, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.inner.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.inner.message(view_state, message, element, app_state)
    }
}

impl<State: 'static, Action: 'static> BoundedX for BoundedAnyView<State, Action> {}
impl<State: 'static, Action: 'static> BoundedY for BoundedAnyView<State, Action> {}

// ---------------------------------------------------------------------------
// Explicit auditable escape hatches
// ---------------------------------------------------------------------------

/// Auditable zero-cost wrapper asserting [`BoundedX`] for a child view whose horizontal safety was
/// verified manually (e.g. bespoke FFI/custom widget or legacy `Box<AnyWidgetView>`).
///
/// Vertical [`BoundedY`] status is still inherited transparently from `V`.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct UncheckedBoundedX<V, State, Action = ()> {
    child: V,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Marks `child` as [`BoundedX`] without changing the underlying Masonry widget tree.
pub fn assume_bounded_x<State, Action, V>(child: V) -> UncheckedBoundedX<V, State, Action> {
    UncheckedBoundedX {
        child,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for UncheckedBoundedX<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UncheckedBoundedX<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = V::Element;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.child.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.child
            .rebuild(&prev.child, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.child.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.child.message(view_state, message, element, app_state)
    }
}

impl<V, State, Action> BoundedX for UncheckedBoundedX<V, State, Action> {}
impl<V: BoundedY, State, Action> BoundedY for UncheckedBoundedX<V, State, Action> {}

/// Auditable zero-cost wrapper asserting [`BoundedY`] for a child view whose vertical fit was
/// verified manually. Horizontal [`BoundedX`] status is still inherited transparently from `V`.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct UncheckedBoundedY<V, State, Action = ()> {
    child: V,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Marks `child` as [`BoundedY`] without changing the underlying Masonry widget tree.
pub fn assume_bounded_y<State, Action, V>(child: V) -> UncheckedBoundedY<V, State, Action> {
    UncheckedBoundedY {
        child,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for UncheckedBoundedY<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UncheckedBoundedY<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = V::Element;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.child.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.child
            .rebuild(&prev.child, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.child.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.child.message(view_state, message, element, app_state)
    }
}

impl<V: BoundedX, State, Action> BoundedX for UncheckedBoundedY<V, State, Action> {}
impl<V, State, Action> BoundedY for UncheckedBoundedY<V, State, Action> {}

/// Auditable zero-cost wrapper asserting both [`BoundedX`] and [`BoundedY`] ([`Bounded`]) for a
/// manually verified child view.
#[must_use = "View values do nothing unless provided to Brocado."]
#[derive(Debug)]
pub struct UncheckedBounded<V, State, Action = ()> {
    child: V,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Marks `child` as [`Bounded`] (`BoundedX + BoundedY`) without changing the Masonry widget tree.
pub fn assume_bounded<State, Action, V>(child: V) -> UncheckedBounded<V, State, Action> {
    UncheckedBounded {
        child,
        phantom: PhantomData,
    }
}

impl<V, State, Action> ViewMarker for UncheckedBounded<V, State, Action> {}

impl<V, State, Action> View<State, Action, ViewCtx> for UncheckedBounded<V, State, Action>
where
    State: 'static,
    Action: 'static,
    V: WidgetView<State, Action>,
{
    type Element = V::Element;
    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut State) -> (Self::Element, Self::ViewState) {
        self.child.build(ctx, app_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) {
        self.child
            .rebuild(&prev.child, view_state, ctx, element, app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        self.child.teardown(view_state, ctx, element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        element: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        self.child.message(view_state, message, element, app_state)
    }
}

impl<V, State, Action> BoundedX for UncheckedBounded<V, State, Action> {}
impl<V, State, Action> BoundedY for UncheckedBounded<V, State, Action> {}

// ---------------------------------------------------------------------------
// Assertion functions and extension trait
// ---------------------------------------------------------------------------

/// Identity guard requiring `view` to satisfy [`BoundedX`] at compile time.
#[inline]
pub fn bounded_x<V: BoundedX>(view: V) -> V {
    view
}

/// Identity guard requiring `view` to satisfy [`BoundedY`] at compile time.
#[inline]
pub fn bounded_y<V: BoundedY>(view: V) -> V {
    view
}

/// Identity guard requiring `view` to satisfy both [`BoundedX`] and [`BoundedY`] at compile time.
#[inline]
pub fn bounded<V: Bounded>(view: V) -> V {
    view
}

/// Convenience extension trait for boxing or asserting overflow safety on any [`WidgetView`].
pub trait BoundedViewExt<State: 'static, Action: 'static>:
    WidgetView<State, Action> + Sized
{
    /// Erases `self` into [`BoundedXAnyView`], proving [`BoundedX`] at the call site.
    fn boxed_bounded_x(self) -> BoundedXAnyView<State, Action>
    where
        Self: BoundedX,
    {
        BoundedXAnyView::new(self)
    }

    /// Erases `self` into [`BoundedAnyView`], proving [`Bounded`] (`BoundedX + BoundedY`).
    fn boxed_bounded(self) -> BoundedAnyView<State, Action>
    where
        Self: Bounded,
    {
        BoundedAnyView::new(self)
    }

    /// Asserts at compile time that `self` implements [`BoundedX`].
    fn assert_bounded_x(self) -> Self
    where
        Self: BoundedX,
    {
        self
    }

    /// Asserts at compile time that `self` implements [`BoundedY`].
    fn assert_bounded_y(self) -> Self
    where
        Self: BoundedY,
    {
        self
    }

    /// Asserts at compile time that `self` implements [`Bounded`] (`BoundedX + BoundedY`).
    fn assert_bounded(self) -> Self
    where
        Self: Bounded,
    {
        self
    }
}

impl<State: 'static, Action: 'static, V: WidgetView<State, Action> + Sized>
    BoundedViewExt<State, Action> for V
{
}

// ---------------------------------------------------------------------------
// Trusted trait implementations
// ---------------------------------------------------------------------------

// Smart pointers and static combinators (Box<AnyWidgetView> is excluded since AnyWidgetView: !BoundedX)
impl<V: BoundedX + ?Sized> BoundedX for Box<V> {}
impl<V: BoundedY + ?Sized> BoundedY for Box<V> {}

impl<V: BoundedX + ?Sized> BoundedX for Arc<V> {}
impl<V: BoundedY + ?Sized> BoundedY for Arc<V> {}

impl<V: BoundedX + ?Sized> BoundedX for Rc<V> {}
impl<V: BoundedY + ?Sized> BoundedY for Rc<V> {}

impl<A: BoundedX, B: BoundedX> BoundedX for OneOf2<A, B> {}
impl<A: BoundedY, B: BoundedY> BoundedY for OneOf2<A, B> {}

// Upstream xilem_masonry containers & leaf primitives
impl<Seq: BoundedXSeq> BoundedX for ZStack<Seq> {}
impl<Seq: BoundedYSeq> BoundedY for ZStack<Seq> {}

impl BoundedX for Spinner {}
impl BoundedY for Spinner {}

impl<State: 'static, Action, V: BoundedX> BoundedX for Divider<State, Action, V> {}
impl<State: 'static, Action, V: BoundedY> BoundedY for Divider<State, Action, V> {}

// Visual Prop<P, V> wrappers that preserve layout bounds.
// `Prop<Dimensions, V, ...>` is deliberately OMITTED because fixed dims can overflow.
macro_rules! impl_bounded_preserving_prop {
    ($($prop:ty),+ $(,)?) => {
        $(
            impl<V: BoundedX, State, Action> BoundedX for Prop<$prop, V, State, Action> {}
            impl<V: BoundedY, State, Action> BoundedY for Prop<$prop, V, State, Action> {}
        )+
    };
}

impl_bounded_preserving_prop!(
    Padding,
    Background,
    BorderColor,
    BorderWidth,
    CornerRadius,
    BoxShadow,
    ContentColor,
    Gap,
    ScrollBarStyle,
);

// LineBreaking::Overflow can disable horizontal wrapping, so only vertical safety propagates.
impl<V: BoundedY, State, Action> BoundedY for Prop<LineBreaking, V, State, Action> {}

// Brocado foundation wrappers
impl<V, State, Action> BoundedX for ClipBoxView<V, State, Action> {}
impl<V: BoundedY, State, Action> BoundedY for ClipBoxView<V, State, Action> {}

// ScrollView dual-axis type-state bounds:
// * A scrolling axis (`ScrollAxis`) absorbs child overflow unconditionally.
// * A fitted axis (`FitAxis`) delegates sizing to the child and therefore preserves bounds
//   only when the child itself is bounded on that axis.
impl<V, State, Action, CY> BoundedX for ScrollView<V, State, Action, ScrollAxis, CY> {}
impl<V: BoundedX, State, Action, CY> BoundedX for ScrollView<V, State, Action, FitAxis, CY> {}
impl<V, State, Action, CX> BoundedY for ScrollView<V, State, Action, CX, ScrollAxis> {}
impl<V: BoundedY, State, Action, CX> BoundedY for ScrollView<V, State, Action, CX, FitAxis> {}

// ConstrainedBoxView dual-axis type-state bounds:
// * Only unfloored axes (`UnminAxis`) preserve child bounds; adding `.min_width` / `.min_height`
//   transitions the axis to `HasMinAxis`, which can push out narrow/short parents unless
//   wrapped in `scroll_view(...)` or `clip_box(...)`.
impl<V: BoundedX, State, Action, MinY> BoundedX
    for ConstrainedBoxView<V, State, Action, UnminAxis, MinY>
{
}
impl<V: BoundedY, State, Action, MinX> BoundedY
    for ConstrainedBoxView<V, State, Action, MinX, UnminAxis>
{
}

impl<Seq: BoundedXSeq, State, Action> BoundedX for WrapView<Seq, State, Action> {}

impl<L: BoundedX, T: BoundedX, State, Action> BoundedX for AdaptivePairView<L, T, State, Action> {}

macro_rules! impl_passthrough_bounded {
    ($($ty:ident),+ $(,)?) => {
        $(
            impl<V: BoundedX, State, Action> BoundedX for $ty<V, State, Action> {}
            impl<V: BoundedY, State, Action> BoundedY for $ty<V, State, Action> {}
        )+
    };
}

impl_passthrough_bounded!(
    WithClass,
    FocusFrameView,
    RippleLayerView,
    UiTransition,
    EscapeCatcherView,
    HoverTooltipView,
    AnchoredPopupView,
    ContextMenuView,
    PopoverAnchorView,
    PopoverPlacementView,
    SurfaceBox,
    UiCustomButton,
    UiCustomIconButton,
);

impl<State, Action, V: BoundedX> BoundedX for ThemeScope<State, Action, V> {}
impl<State, Action, V: BoundedY> BoundedY for ThemeScope<State, Action, V> {}

impl<State, Action, F, V: BoundedX> BoundedX for WithTheme<State, Action, F, V> {}
impl<State, Action, F, V: BoundedY> BoundedY for WithTheme<State, Action, F, V> {}

impl<State, Action, Data, F, V: BoundedX> BoundedX for MemoizeWithTheme<State, Action, Data, F, V> {}
impl<State, Action, Data, F, V: BoundedY> BoundedY for MemoizeWithTheme<State, Action, Data, F, V> {}

// Brocado multi-line wrapping text / lists: horizontally safe, vertically expandable.
impl<State, Action> BoundedX for UiLabel<State, Action> {}
impl<State, Action> BoundedX for UiProse<State, Action> {}
impl<State, Action> BoundedX for UiRichProse<State, Action> {}
impl<State, Action> BoundedX for UiListItem<State, Action> {}

// Brocado compact controls & internally-scrolling fields: bounded on both axes.
macro_rules! impl_leaf_bounded_both {
    ($($ty:ident),+ $(,)?) => {
        $(
            impl<State, Action> BoundedX for $ty<State, Action> {}
            impl<State, Action> BoundedY for $ty<State, Action> {}
        )+
    };
}

impl_leaf_bounded_both!(
    UiButton,
    UiIconButton,
    UiChip,
    UiCheckboxView,
    UiSwitchView,
    UiRadio,
    UiSliderView,
    RangeSlider,
    UiLinearProgress,
    UiDeterminateCircular,
    UiTextField,
    UiTextArea,
    UiSearchBar,
    UiToastHost,
    BasicDialog,
);

// Split panes distribute allocated space across both panes on their split axis.
impl<A: BoundedX, B: BoundedX, State, Action> BoundedX for UiSplit<A, B, State, Action> {}
impl<A: BoundedY, B: BoundedY, State, Action> BoundedY for UiSplit<A, B, State, Action> {}

// Modal shells host content in an internal vertical ScrollView clamped to viewport height,
// so they always satisfy BoundedY and satisfy BoundedX iff their inner content does.
impl<V: BoundedX, State, Action> BoundedX for UiDialog<V, State, Action> {}
impl<V, State, Action> BoundedY for UiDialog<V, State, Action> {}

impl<V: BoundedX, State, Action> BoundedX for UiSheet<V, State, Action> {}
impl<V, State, Action> BoundedY for UiSheet<V, State, Action> {}

impl<V: BoundedX, State, Action> BoundedX for UiModalDrawer<V, State, Action> {}
impl<V, State, Action> BoundedY for UiModalDrawer<V, State, Action> {}
