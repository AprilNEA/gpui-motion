use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window,
};

use crate::{
    Animatable, ChannelTransitions, Inertia, KeyframesTiming, MAX_CHANNELS, MAX_KEYFRAMES,
    MotionState, Spring, Transition, Tween, Variants, When, reduce_motion,
};

use crate::scope::{ScopeFrame, SettledRegistry, current_scope, pop_scope, push_scope};

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
            variants: None,
            active_variant: None,
            stagger_children: None,
            delay_children: None,
            when: None,
        }
    }

    fn with_variants<V: Animatable>(
        self,
        id: impl Into<ElementId>,
        variants: Variants<V>,
        active: Option<&str>,
        transition: impl IntoTransitions<V>,
        f: impl Fn(Self, V) -> Self + 'static,
    ) -> MotionElement<Self, V> {
        assert!(V::CHANNELS <= MAX_CHANNELS);
        let first = variants
            .first()
            .expect("with_variants requires at least one variant");
        let variants = variants.into_entries();
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
            targets: vec![first],
            keyframes: false,
            transitions,
            animator: Box::new(f),
            initial: None,
            while_hover: None,
            while_press: None,
            on_settle: None,
            variants: Some(variants),
            active_variant: active.map(str::to_owned),
            stagger_children: None,
            delay_children: None,
            when: None,
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
    variants: Option<Vec<(&'static str, V)>>,
    active_variant: Option<String>,
    stagger_children: Option<f32>,
    delay_children: Option<f32>,
    when: Option<When>,
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

    pub fn stagger_children(mut self, seconds: f32) -> Self {
        self.stagger_children = Some(seconds);
        self
    }

    pub fn delay_children(mut self, seconds: f32) -> Self {
        self.delay_children = Some(seconds);
        self
    }

    pub fn when(mut self, when: When) -> Self {
        self.when = Some(when);
        self
    }

    fn is_scope_root(&self) -> bool {
        self.active_variant.is_some()
            || self.stagger_children.is_some()
            || self.delay_children.is_some()
            || self.when.is_some()
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
    variant_name: Option<String>,
    variant_target: Option<[f32; MAX_CHANNELS]>,
    transition_delay: f32,
    scope_settled: Rc<RefCell<SettledRegistry>>,
}

pub struct MotionLayoutState {
    element: AnyElement,
    gestures: Rc<Cell<GestureState>>,
    scope: Option<ScopeFrame>,
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
        let outer_scope = current_scope(cx);
        let descendant_index = self
            .variants
            .as_ref()
            .and(outer_scope.as_ref())
            .map(ScopeFrame::claim);
        let inherited_active = outer_scope.as_ref().and_then(|scope| scope.active.clone());
        let resolved_active = self
            .active_variant
            .clone()
            .or_else(|| inherited_active.clone());
        let scope_active = self.active_variant.clone().or(inherited_active);
        let resolved_variant = resolved_active.as_deref().and_then(|name| {
            self.variants
                .as_ref()?
                .iter()
                .find_map(|(entry_name, value)| (*entry_name == name).then(|| value.clone()))
        });

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
                    let initial_target = if self.variants.is_some() {
                        &initial[..V::CHANNELS]
                    } else {
                        declared_target
                    };
                    MotionElementState {
                        motion: MotionState::new(&initial[..V::CHANNELS], initial_target),
                        gestures: Rc::default(),
                        settle_notified: false,
                        variant_name: None,
                        variant_target: None,
                        transition_delay: 0.0,
                        scope_settled: Rc::default(),
                    }
                });

                let scope_root = self.is_scope_root();
                let when = self.when.unwrap_or_default();
                let root_variant_ready = !scope_root
                    || state
                        .scope_settled
                        .borrow_mut()
                        .begin_frame(scope_active.as_deref(), when);
                let reduce_motion = reduce_motion(cx);
                let gestures = state.gestures.get();
                let gesture_target = if gestures.pressed {
                    self.while_press.as_ref().or(self.while_hover.as_ref())
                } else if gestures.hovered {
                    self.while_hover.as_ref()
                } else {
                    None
                };
                let mut pending_retarget = false;
                let mut retargeted = false;
                if let Some(target) = gesture_target {
                    let mut channels = [0.0; MAX_CHANNELS];
                    target.write(&mut channels[..V::CHANNELS]);
                    retargeted = state.motion.retarget_if_needed(&channels[..V::CHANNELS]);
                    if retargeted {
                        state.transition_delay = 0.0;
                    }
                } else if self.variants.is_some() {
                    if let (Some(name), Some(target)) =
                        (resolved_active.as_deref(), resolved_variant.as_ref())
                    {
                        let mut channels = [0.0; MAX_CHANNELS];
                        target.write(&mut channels[..V::CHANNELS]);
                        let variant_changed = state.variant_name.as_deref() != Some(name)
                            || state.variant_target.as_ref().is_none_or(|previous| {
                                previous[..V::CHANNELS] != channels[..V::CHANNELS]
                            });
                        let outer_ready = outer_scope.as_ref().is_none_or(|scope| {
                            scope.when != When::BeforeChildren || scope.root_settled
                        });
                        let own_ready = when != When::AfterChildren || root_variant_ready;
                        if variant_changed && !reduce_motion && (!outer_ready || !own_ready) {
                            pending_retarget = true;
                        } else {
                            retargeted = state.motion.retarget_if_needed(&channels[..V::CHANNELS]);
                            if retargeted {
                                state.transition_delay = if variant_changed {
                                    descendant_index
                                        .zip(outer_scope.as_ref())
                                        .map_or(0.0, |(index, scope)| scope.delay(index))
                                } else {
                                    0.0
                                };
                            }
                            state.variant_name = Some(name.to_owned());
                            state.variant_target = Some(channels);
                        }
                    }
                } else if self.keyframes {
                    let frame_refs = frames
                        .iter()
                        .map(|frame| &frame[..V::CHANNELS])
                        .collect::<Vec<_>>();
                    retargeted = state.motion.retarget_keyframes_if_needed(&frame_refs);
                    if retargeted {
                        state.transition_delay = 0.0;
                    }
                } else {
                    retargeted = state.motion.retarget_if_needed(declared_target);
                    if retargeted {
                        state.transition_delay = 0.0;
                    }
                }
                if retargeted {
                    state.settle_notified = false;
                }

                if reduce_motion {
                    state.motion.snap();
                } else {
                    let delayed_transitions = (state.transition_delay != 0.0).then(|| {
                        self.transitions
                            .iter()
                            .map(|(channels, transition)| {
                                let mut transition = *transition;
                                transition.delay += state.transition_delay;
                                (*channels, transition)
                            })
                            .collect::<Vec<_>>()
                    });
                    let transitions = delayed_transitions.as_deref().unwrap_or(&self.transitions);
                    let transitions = if transitions.len() == 1 {
                        ChannelTransitions::Uniform(&transitions[0].1)
                    } else {
                        ChannelTransitions::Segmented(transitions)
                    };
                    state.motion.tick(Instant::now(), transitions);
                }

                if !state.motion.settled() || pending_retarget {
                    window.request_animation_frame();
                } else if !state.settle_notified {
                    if let Some(callback) = &self.on_settle {
                        callback(window, cx);
                    }
                    state.settle_notified = true;
                }

                if let (Some(scope), Some(index)) = (&outer_scope, descendant_index) {
                    scope.register(index, state.motion.settled() && !pending_retarget);
                }

                let scope = scope_root.then(|| ScopeFrame {
                    active: scope_active.clone(),
                    stagger_children: self.stagger_children.unwrap_or(0.0),
                    delay_children: self.delay_children.unwrap_or(0.0),
                    when,
                    root_settled: state.motion.settled() && !pending_retarget,
                    settled: Rc::clone(&state.scope_settled),
                });
                let value = V::read(state.motion.current());
                let element = self.element.take().expect("request_layout is called once");
                let mut element = (self.animator)(element, value).into_any_element();
                if let Some(scope) = &scope {
                    push_scope(cx, scope.clone());
                }
                let layout_id = element.request_layout(window, cx);
                if scope.is_some() {
                    pop_scope(cx);
                }
                let layout_state = MotionLayoutState {
                    element,
                    gestures: Rc::clone(&state.gestures),
                    scope,
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
        if let Some(scope) = &state.scope {
            push_scope(cx, scope.clone());
        }
        state.element.prepaint(window, cx);
        if state.scope.is_some() {
            pop_scope(cx);
        }
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
        if let Some(scope) = &state.scope {
            push_scope(cx, scope.clone());
        }
        state.element.paint(window, cx);
        if state.scope.is_some() {
            pop_scope(cx);
        }
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
