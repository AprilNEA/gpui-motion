use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

use crate::{Animatable, MAX_CHANNELS, MotionState, Transition};

type SettleCallback = Box<dyn Fn(&mut Window, &mut App) + 'static>;

pub trait MotionExt: Element + Sized {
    fn with_motion<V: Animatable>(
        self,
        id: impl Into<ElementId>,
        target: V,
        transition: Transition,
        f: impl Fn(Self, V) -> Self + 'static,
    ) -> MotionElement<Self, V> {
        assert!(V::CHANNELS <= MAX_CHANNELS);
        MotionElement {
            id: id.into(),
            element: Some(self),
            target,
            transition,
            animator: Box::new(f),
            initial: None,
            on_settle: None,
        }
    }
}

impl<E: Element> MotionExt for E {}

pub struct MotionElement<E, V> {
    id: ElementId,
    element: Option<E>,
    target: V,
    transition: Transition,
    animator: Box<dyn Fn(E, V) -> E + 'static>,
    initial: Option<V>,
    on_settle: Option<SettleCallback>,
}

impl<E, V> MotionElement<E, V> {
    pub fn initial(mut self, initial: V) -> Self {
        self.initial = Some(initial);
        self
    }

    pub fn on_settle(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_settle = Some(Box::new(callback));
        self
    }
}

impl<E: Element, V: Animatable> IntoElement for MotionElement<E, V> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct MotionElementState {
    motion: MotionState,
    settle_notified: bool,
}

impl<E: Element, V: Animatable> Element for MotionElement<E, V> {
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
        let mut target = [0.0; MAX_CHANNELS];
        self.target.write(&mut target[..V::CHANNELS]);

        window.with_element_state::<MotionElementState, _>(
            global_id.expect("motion elements always have an id"),
            |state, window| {
                let mut state = state.unwrap_or_else(|| {
                    let mut initial = [0.0; MAX_CHANNELS];
                    self.initial
                        .as_ref()
                        .unwrap_or(&self.target)
                        .write(&mut initial[..V::CHANNELS]);
                    MotionElementState {
                        motion: MotionState::new(&initial[..V::CHANNELS], &target[..V::CHANNELS]),
                        settle_notified: false,
                    }
                });

                if state.motion.retarget_if_needed(&target[..V::CHANNELS]) {
                    state.settle_notified = false;
                }

                if cx.reduce_motion() {
                    state.motion.snap();
                } else {
                    state.motion.tick(Instant::now(), &self.transition);
                }

                if !state.motion.settled() {
                    window.request_animation_frame();
                } else if !state.settle_notified {
                    if let Some(callback) = &self.on_settle {
                        callback(window, cx);
                    }
                    state.settle_notified = true;
                }

                let value = V::read(state.motion.current());
                let element = self.element.take().expect("request_layout is called once");
                let mut element = (self.animator)(element, value).into_any_element();
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
