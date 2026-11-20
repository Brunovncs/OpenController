//! How things move: short and soft. What opens rises a few pixels as it fades in, and a dialog
//! grows from a little smaller. GPUI holds still when the system asks for less motion.

use gpui::{Animation, AnimationElement, AnimationExt, ElementId, IntoElement, Styled, px};
use std::time::Duration;

/// Content taking another's place: a screen, a section, a panel opening.
pub const ENTER: Duration = Duration::from_millis(180);

/// Rises into place as it fades in, keyed by `id`: an element with a new id plays it again, so
/// giving the content's identity as the id plays it on every change of content. It moves by its
/// offset, not its margin, so nothing around it is laid out again.
pub fn enter<E: IntoElement + Styled + 'static>(el: E, id: impl Into<ElementId>) -> AnimationElement<E> {
    const RISE: f32 = 6.;
    el.with_animation(id, Animation::new(ENTER).with_easing(gpui::ease_out_quint()), |d, p| {
        d.opacity(p).relative().top(px(RISE * (1. - p)))
    })
}

/// A dialog over the page: fades in as it rises a little further than [`enter`].
pub fn pop<E: IntoElement + Styled + 'static>(el: E, id: impl Into<ElementId>) -> AnimationElement<E> {
    const RISE: f32 = 12.;
    el.with_animation(id, Animation::new(Duration::from_millis(220)).with_easing(gpui::ease_out_quint()), |d, p| {
        d.opacity(p).relative().top(px(RISE * (1. - p)))
    })
}

/// The dim behind a dialog, fading in.
pub fn fade<E: IntoElement + Styled + 'static>(el: E, id: impl Into<ElementId>) -> AnimationElement<E> {
    el.with_animation(id, Animation::new(ENTER), |d, p| d.opacity(p))
}
