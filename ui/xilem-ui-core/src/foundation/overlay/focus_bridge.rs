//! Temporary upstream Masonry focus-management bridge for modal dialogs.
//!
//! Current upstream Masonry exposes `set_focus` / `focus_target_id` only on `EventCtx` (not on
//! `MutateCtx` / `UpdateCtx` / Xilem `ViewCtx`), and lacks a built-in focus-scope layer
//! primitive. This module isolates the thread-local modal focusable collector and `Tab` ring
//! stepping so they can be removed in one place once upstream Masonry introduces native Focus
//! Scope / Layer Focus support.

use std::cell::RefCell;

use masonry::core::WidgetId;

thread_local! {
    static MODAL_FOCUS_COLLECTOR: RefCell<Option<Vec<WidgetId>>> = const { RefCell::new(None) };
}

/// Records `id` into the active modal dialog's focus ring when built inside
/// [`collect_modal_focusables`].
pub(crate) fn record_modal_focusable(id: WidgetId) {
    MODAL_FOCUS_COLLECTOR.with(|cell| {
        if let Some(list) = cell.borrow_mut().as_mut() {
            list.push(id);
        }
    });
}

/// Captures focusable descendant [`WidgetId`]s created while building a modal dialog's content.
pub(crate) fn collect_modal_focusables<R>(f: impl FnOnce() -> R) -> (R, Vec<WidgetId>) {
    struct Reset(Option<Vec<WidgetId>>);
    impl Drop for Reset {
        fn drop(&mut self) {
            let prev = self.0.take();
            MODAL_FOCUS_COLLECTOR.with(|cell| {
                *cell.borrow_mut() = prev;
            });
        }
    }

    let prev = MODAL_FOCUS_COLLECTOR.with(|cell| cell.borrow_mut().replace(Vec::new()));
    let _guard = Reset(prev);
    let out = f();
    let collected = MODAL_FOCUS_COLLECTOR
        .with(|cell| cell.borrow_mut().take())
        .unwrap_or_default();
    (out, collected)
}

/// Computes the next focus target inside a trapped modal `focus_ring` on `Tab` (`shift = false`)
/// or `Shift+Tab` (`shift = true`).
#[must_use]
pub(crate) fn next_focus_target(
    focus_ring: &[WidgetId],
    current: Option<WidgetId>,
    shift: bool,
) -> Option<WidgetId> {
    if focus_ring.is_empty() {
        return None;
    }
    let cur_pos = current.and_then(|id| focus_ring.iter().position(|ring_id| *ring_id == id));
    let next_idx = match (cur_pos, shift) {
        (Some(idx), false) => (idx + 1) % focus_ring.len(),
        (Some(0), true) => focus_ring.len() - 1,
        (Some(idx), true) => idx - 1,
        (None, false) => 0,
        (None, true) => focus_ring.len() - 1,
    };
    Some(focus_ring[next_idx])
}

#[cfg(test)]
mod tests {
    use masonry::core::NewWidget;
    use masonry::widgets::Label;

    use super::*;

    fn test_widget_id() -> WidgetId {
        NewWidget::new(Label::new("")).id()
    }

    #[test]
    fn next_focus_target_handles_empty_forward_and_backward_wrapping() {
        assert_eq!(next_focus_target(&[], None, false), None);

        let a = test_widget_id();
        let b = test_widget_id();
        let c = test_widget_id();
        let ring = [a, b, c];

        assert_eq!(next_focus_target(&ring, None, false), Some(a));
        assert_eq!(next_focus_target(&ring, None, true), Some(c));
        assert_eq!(next_focus_target(&ring, Some(a), false), Some(b));
        assert_eq!(next_focus_target(&ring, Some(c), false), Some(a));
        assert_eq!(next_focus_target(&ring, Some(a), true), Some(c));
        assert_eq!(next_focus_target(&ring, Some(b), true), Some(a));

        let unknown = test_widget_id();
        assert_eq!(next_focus_target(&ring, Some(unknown), false), Some(a));
        assert_eq!(next_focus_target(&ring, Some(unknown), true), Some(c));
    }

    #[test]
    fn collect_modal_focusables_restores_outer_scope_on_nested_calls() {
        let a = test_widget_id();
        let b = test_widget_id();
        let c = test_widget_id();

        let ((), outer) = collect_modal_focusables(|| {
            record_modal_focusable(a);
            let ((), inner) = collect_modal_focusables(|| {
                record_modal_focusable(b);
            });
            assert_eq!(inner, vec![b]);
            record_modal_focusable(c);
        });
        assert_eq!(outer, vec![a, c]);
    }
}
