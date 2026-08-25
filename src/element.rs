use std::{cell::Cell, rc::Rc, time::Instant};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window,
};

use crate::{
    Animatable, ChannelTransitions, Inertia, KeyframesTiming, MAX_CHANNELS, MAX_KEYFRAMES,
    MotionState, Spring, Transition, Tween,
};

type SettleCallback = Box<dyn Fn(&mut Window, &mut App) + 'static>;

pub trait IntoTransitions<V: Animatable> {
    fn into_transitions(self) -> Vec<(usize, Transition)>;
}

macro_rules! impl_uniform_transition {
    ($($transition:ty),+ $(,)?) => {
        $(
            impl<V: Animatable> IntoTransitions<V> for $transition {
                fn into_transitions(self) -> Vec<(usize, Transition)> {
                    vec![(V::CHANNELS, self.into())]
                }
            }
        )+
    };
}

impl_uniform_transition!(Transition, Spring, Tween, KeyframesTiming, Inertia);

macro_rules! impl_tuple_transitions {
    ($(($($value:ident: $transition:ident),+)),+ $(,)?) => {
        $(
            impl<$($value, $transition),+> IntoTransitions<($($value,)+)> for ($($transition,)+)
            where
                $($value: Animatable, $transition: Into<Transition>,)+
            {
                #[allow(non_snake_case, reason = "tuple macro binds type-parameter-shaped values")]
                fn into_transitions(self) -> Vec<(usize, Transition)> {
                    let ($($transition,)+) = self;
                    vec![$(($value::CHANNELS, $transition.into()),)+]
                }
            }
        )+
    };
}

impl_tuple_transitions!(
    (A: TA, B: TB),
    (A: TA, B: TB, C: TC),
    (A: TA, B: TB, C: TC, D: TD),
    (A: TA, B: TB, C: TC, D: TD, E: TE),
    (A: TA, B: TB, C: TC, D: TD, E: TE, F: TF),
);

pub trait IntoMotionTarget<V: Animatable> {
    fn into_motion_target(self) -> (Vec<V>, bool);
}

impl<V: Animatable> IntoMotionTarget<V> for V {
    fn into_motion_target(self) -> (Vec<V>, bool) {
        (vec![self], false)
    }
}

impl<V: Animatable, const N: usize> IntoMotionTarget<V> for [V; N] {
    fn into_motion_target(self) -> (Vec<V>, bool) {
        assert!(N > 0 && N <= MAX_KEYFRAMES);
        (Vec::from(self), true)
    }
}

impl<V: Animatable> IntoMotionTarget<V> for Vec<V> {
    fn into_motion_target(self) -> (Vec<V>, bool) {
        assert!(!self.is_empty() && self.len() <= MAX_KEYFRAMES);
        (self, true)
    }
}

pub trait MotionExt: Element + Sized {
    fn with_motion<V: Animatable>(
        self,
        id: impl Into<ElementId>,
        target: impl IntoMotionTarget<V>,
        transition: impl IntoTransitions<V>,
        f: impl Fn(Self, V) -> Self + 'static,
    ) -> MotionElement<Self, V> {
        assert!(V::CHANNELS <= MAX_CHANNELS);
        let (targets, keyframes) = target.into_motion_target();
        let transitions = transition.into_transitions();
        assert_eq!(
            transitions
                .iter()
                .map(|(channels, _)| channels)
                .sum::<usize>(),
            V::CHANNELS,
            "segmented transition lengths must equal the animated value's channel count"
        );
        MotionElement {
            id: id.into(),
            element: Some(self),
            targets,
            keyframes,
            transitions,
            animator: Box::new(f),
            initial: None,
            while_hover: None,
            while_press: None,
            on_settle: None,
        }
    }
}

impl<E: Element> MotionExt for E {}

pub struct MotionElement<E, V> {
    id: ElementId,
    element: Option<E>,
    targets: Vec<V>,
    keyframes: bool,
    transitions: Vec<(usize, Transition)>,
    animator: Box<dyn Fn(E, V) -> E + 'static>,
    initial: Option<V>,
    while_hover: Option<V>,
    while_press: Option<V>,
    on_settle: Option<SettleCallback>,
}

impl<E, V> MotionElement<E, V> {
    pub fn initial(mut self, initial: V) -> Self {
        self.initial = Some(initial);
        self
    }

    pub fn while_hover(mut self, target: V) -> Self {
        self.while_hover = Some(target);
        self
    }

    pub fn while_press(mut self, target: V) -> Self {
        self.while_press = Some(target);
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct GestureState {
    hovered: bool,
    pressed: bool,
}

struct MotionElementState {
    motion: MotionState,
    gestures: Rc<Cell<GestureState>>,
    settle_notified: bool,
}

pub struct MotionLayoutState {
    element: AnyElement,
    gestures: Rc<Cell<GestureState>>,
}

impl<E: Element, V: Animatable> Element for MotionElement<E, V> {
    type RequestLayoutState = MotionLayoutState;
    type PrepaintState = Option<Hitbox>;

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
        let frames = self
            .targets
            .iter()
            .map(|target| {
                let mut channels = [0.0; MAX_CHANNELS];
                target.write(&mut channels[..V::CHANNELS]);
                channels
            })
            .collect::<Vec<_>>();
        let declared_target = &frames[frames.len() - 1][..V::CHANNELS];

        window.with_element_state::<MotionElementState, _>(
            global_id.expect("motion elements always have an id"),
            |state, window| {
                let mut state = state.unwrap_or_else(|| {
                    let mut initial = [0.0; MAX_CHANNELS];
                    self.initial
                        .as_ref()
                        .unwrap_or(&self.targets[self.targets.len() - 1])
                        .write(&mut initial[..V::CHANNELS]);
                    MotionElementState {
                        motion: MotionState::new(&initial[..V::CHANNELS], declared_target),
                        gestures: Rc::default(),
                        settle_notified: false,
                    }
                });

                let gestures = state.gestures.get();
                let gesture_target = if gestures.pressed {
                    self.while_press.as_ref().or(self.while_hover.as_ref())
                } else if gestures.hovered {
                    self.while_hover.as_ref()
                } else {
                    None
                };
                let retargeted = if let Some(target) = gesture_target {
                    let mut channels = [0.0; MAX_CHANNELS];
                    target.write(&mut channels[..V::CHANNELS]);
                    state.motion.retarget_if_needed(&channels[..V::CHANNELS])
                } else if self.keyframes {
                    let frame_refs = frames
                        .iter()
                        .map(|frame| &frame[..V::CHANNELS])
                        .collect::<Vec<_>>();
                    state.motion.retarget_keyframes_if_needed(&frame_refs)
                } else {
                    state.motion.retarget_if_needed(declared_target)
                };
                if retargeted {
                    state.settle_notified = false;
                }

                if cx.reduce_motion() {
                    state.motion.snap();
                } else {
                    let transitions = if self.transitions.len() == 1 {
                        ChannelTransitions::Uniform(&self.transitions[0].1)
                    } else {
                        ChannelTransitions::Segmented(&self.transitions)
                    };
                    state.motion.tick(Instant::now(), transitions);
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
                let layout_state = MotionLayoutState {
                    element,
                    gestures: Rc::clone(&state.gestures),
                };
                ((layout_id, layout_state), state)
            },
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        state.element.prepaint(window, cx);
        (self.while_hover.is_some() || self.while_press.is_some())
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.element.paint(window, cx);
        let Some(hitbox) = hitbox else {
            return;
        };

        let hovered = hitbox.is_hovered(window);
        update_gestures(
            &state.gestures,
            |gestures| GestureState {
                hovered,
                ..gestures
            },
            window,
        );

        let hitbox_for_move = hitbox.clone();
        let gestures_for_move = Rc::clone(&state.gestures);
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _cx| {
            if phase == DispatchPhase::Bubble {
                let hovered = hitbox_for_move.is_hovered(window)
                    && hitbox_for_move.bounds.contains(&event.position);
                update_gestures(
                    &gestures_for_move,
                    |gestures| GestureState {
                        hovered,
                        ..gestures
                    },
                    window,
                );
            }
        });

        let hitbox_for_down = hitbox.clone();
        let gestures_for_down = Rc::clone(&state.gestures);
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && hitbox_for_down.is_hovered(window)
                && hitbox_for_down.bounds.contains(&event.position)
            {
                update_gestures(
                    &gestures_for_down,
                    |_| GestureState {
                        hovered: true,
                        pressed: true,
                    },
                    window,
                );
            }
        });

        let hitbox_for_up = hitbox.clone();
        let gestures_for_up = Rc::clone(&state.gestures);
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                let hovered = hitbox_for_up.is_hovered(window)
                    && hitbox_for_up.bounds.contains(&event.position);
                update_gestures(
                    &gestures_for_up,
                    |_| GestureState {
                        hovered,
                        pressed: false,
                    },
                    window,
                );
            }
        });
    }
}

fn update_gestures(
    state: &Cell<GestureState>,
    update: impl FnOnce(GestureState) -> GestureState,
    window: &mut Window,
) {
    let previous = state.get();
    let next = update(previous);
    if previous != next {
        state.set(next);
        window.refresh();
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Pixels, Rgba};

    use super::{IntoMotionTarget, IntoTransitions};
    use crate::{Spring, TransitionKind, Tween};

    #[test]
    fn tuple_transitions_follow_animatable_channel_boundaries() {
        let transitions = <(Spring, Tween) as IntoTransitions<(Pixels, Rgba)>>::into_transitions((
            Spring::wobbly(),
            Tween::new(0.2),
        ));

        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].0, 1);
        assert_eq!(transitions[1].0, 4);
        assert!(matches!(transitions[0].1.kind, TransitionKind::Spring(_)));
        assert!(matches!(transitions[1].1.kind, TransitionKind::Tween(_)));
    }

    #[test]
    fn arrays_are_keyframe_targets() {
        let (frames, keyframes) = [1.0_f32, 2.0, 3.0].into_motion_target();

        assert_eq!(frames, vec![1.0, 2.0, 3.0]);
        assert!(keyframes);
    }
}
