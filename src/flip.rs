use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Point, Window, point, px,
};

use crate::{ChannelTransitions, MotionState, Spring, Transition};

/// Adds translation-only FLIP layout animation to an element.
///
/// The ID must remain stable across frames; list items should use their data key.
/// GPUI divs have no transform support, so size changes are not animated. Deferred-draw
/// subtrees are unsupported because GPUI snapshots do not capture third-party scopes.
pub trait FlipExt: Element + Sized {
    fn with_flip(self, id: impl Into<ElementId>) -> Flip<Self> {
        Flip {
            id: id.into(),
            element: Some(self),
            transition: Spring::stiff().into(),
        }
    }
}

impl<E: Element> FlipExt for E {}

/// An element wrapper that animates layout-origin changes with translation-only FLIP.
///
/// The element ID must remain stable across frames. Size animation and deferred-draw
/// subtrees are not supported.
pub struct Flip<E> {
    id: ElementId,
    element: Option<E>,
    transition: Transition,
}

impl<E> Flip<E> {
    pub fn transition(mut self, transition: impl Into<Transition>) -> Self {
        self.transition = transition.into();
        self
    }
}

impl<E: Element> IntoElement for Flip<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct FlipState {
    origin: Option<Point<Pixels>>,
    offset: MotionState,
}

impl Default for FlipState {
    fn default() -> Self {
        Self {
            origin: None,
            offset: MotionState::new(&[0.0, 0.0], &[0.0, 0.0]),
        }
    }
}

impl<E: Element> Element for Flip<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut element = self
            .element
            .take()
            .expect("request_layout is called once")
            .into_any_element();
        let layout_id = element.request_layout(window, cx);
        (layout_id, element)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let offset = window.with_element_state::<FlipState, _>(
            global_id.expect("FLIP elements always have an id"),
            |state, window| {
                let mut state = state.unwrap_or_default();

                if !cx.reduce_motion() {
                    if let Some(previous) = state.origin
                        && previous != bounds.origin
                    {
                        add_offset(
                            &mut state.offset,
                            [
                                f32::from(previous.x - bounds.origin.x),
                                f32::from(previous.y - bounds.origin.y),
                            ],
                        );
                    }
                    state.offset.tick(
                        Instant::now(),
                        ChannelTransitions::Uniform(&self.transition),
                    );
                } else {
                    state.offset = MotionState::new(&[0.0, 0.0], &[0.0, 0.0]);
                }
                state.origin = Some(bounds.origin);

                if !state.offset.settled() {
                    window.request_animation_frame();
                }
                let current = state.offset.current();
                ((point(px(current[0]), px(current[1]))), state)
            },
        );

        let _ = window.with_element_offset(offset, |window| element.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

fn add_offset(offset: &mut MotionState, delta: [f32; 2]) {
    let current = [
        offset.current()[0] + delta[0],
        offset.current()[1] + delta[1],
    ];
    let velocity = [offset.velocity()[0], offset.velocity()[1]];
    *offset = MotionState::new(&current, &[0.0, 0.0]);
    offset.set_velocity(&velocity);
}

#[cfg(test)]
mod tests {
    use super::add_offset;
    use crate::MotionState;

    #[test]
    fn layout_delta_accumulates_without_losing_velocity() {
        let mut offset = MotionState::new(&[12.0, -4.0], &[0.0, 0.0]);
        offset.set_velocity(&[-30.0, 18.0]);

        add_offset(&mut offset, [8.0, -6.0]);

        assert_eq!(offset.current(), &[20.0, -10.0]);
        assert_eq!(offset.velocity(), &[-30.0, 18.0]);
    }
}
