use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Empty, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

use crate::{Animatable, MAX_CHANNELS, MotionState, Transition};

pub fn presence<V: Animatable, E: IntoElement>(
    id: impl Into<ElementId>,
    visible: bool,
    enter: V,
    exit: V,
    transition: Transition,
    render: impl Fn(V) -> E + 'static,
) -> impl IntoElement {
    assert!(V::CHANNELS <= MAX_CHANNELS);
    PresenceElement {
        id: id.into(),
        visible,
        enter,
        exit,
        transition,
        render,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Entering,
    Present,
    Exiting,
    Gone,
}

struct PresenceElementState {
    motion: MotionState,
    phase: Phase,
}

struct PresenceElement<V, F> {
    id: ElementId,
    visible: bool,
    enter: V,
    exit: V,
    transition: Transition,
    render: F,
}

impl<V: Animatable, E: IntoElement, F: Fn(V) -> E + 'static> IntoElement for PresenceElement<V, F> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<V: Animatable, E: IntoElement, F: Fn(V) -> E + 'static> Element for PresenceElement<V, F> {
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
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut enter = [0.0; MAX_CHANNELS];
        let mut exit = [0.0; MAX_CHANNELS];
        self.enter.write(&mut enter[..V::CHANNELS]);
        self.exit.write(&mut exit[..V::CHANNELS]);

        window.with_element_state::<PresenceElementState, _>(
            global_id.expect("presence elements always have an id"),
            |state, window| {
                let mut state = state.unwrap_or_else(|| {
                    let phase = if self.visible {
                        Phase::Entering
                    } else {
                        Phase::Gone
                    };
                    PresenceElementState {
                        motion: MotionState::new(
                            &exit[..V::CHANNELS],
                            if self.visible {
                                &enter[..V::CHANNELS]
                            } else {
                                &exit[..V::CHANNELS]
                            },
                        ),
                        phase,
                    }
                });

                if self.visible {
                    if matches!(state.phase, Phase::Exiting | Phase::Gone) {
                        state.phase = Phase::Entering;
                    }

                    if state.motion.retarget_if_needed(&enter[..V::CHANNELS])
                        && state.phase == Phase::Present
                    {
                        state.phase = Phase::Entering;
                    }
                } else if state.phase == Phase::Gone {
                    if state.motion.retarget_if_needed(&exit[..V::CHANNELS]) {
                        state.motion.snap();
                    }
                } else {
                    if matches!(state.phase, Phase::Entering | Phase::Present) {
                        state.phase = Phase::Exiting;
                    }
                    state.motion.retarget_if_needed(&exit[..V::CHANNELS]);
                }

                if state.phase != Phase::Gone {
                    if cx.reduce_motion() {
                        state.motion.snap();
                    } else {
                        state.motion.tick(Instant::now(), &self.transition);
                    }

                    if state.motion.settled() {
                        state.phase = match state.phase {
                            Phase::Entering => Phase::Present,
                            Phase::Exiting => Phase::Gone,
                            phase => phase,
                        };
                    } else {
                        window.request_animation_frame();
                    }
                }

                let mut element = if state.phase == Phase::Gone {
                    Empty.into_any_element()
                } else {
                    (self.render)(V::read(state.motion.current())).into_any_element()
                };
                let layout_id = element.request_layout(window, cx);
                ((layout_id, element), state)
            },
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
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
