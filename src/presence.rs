use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement, Pixels, Window, div,
};

use crate::{
    Animatable, ChannelTransitions, MAX_CHANNELS, MotionState, Spring, Transition, reduce_motion,
};

type ChildRender<V> = Box<dyn Fn(V) -> AnyElement + 'static>;
type ExitCallback = Box<dyn Fn(&ElementId, &mut Window, &mut App) + 'static>;

pub fn presence<V: Animatable, E: IntoElement>(
    id: impl Into<ElementId>,
    visible: bool,
    enter: V,
    exit: V,
    transition: Transition,
    render: impl Fn(V) -> E + 'static,
) -> impl IntoElement {
    let group = presence_group(id)
        .enter(enter)
        .exit(exit)
        .transition(transition);
    if visible {
        group.child("presence-child", move |value| {
            render(value).into_any_element()
        })
    } else {
        group
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceMode {
    #[default]
    Sync,
    Wait,
}

pub struct PresenceGroup<V: Animatable> {
    id: ElementId,
    mode: PresenceMode,
    transition: Transition,
    enter: Option<V>,
    exit: Option<V>,
    children: Vec<(ElementId, ChildRender<V>)>,
    on_exit_complete: Option<ExitCallback>,
}

pub fn presence_group<V: Animatable>(id: impl Into<ElementId>) -> PresenceGroup<V> {
    assert!(V::CHANNELS <= MAX_CHANNELS);
    PresenceGroup {
        id: id.into(),
        mode: PresenceMode::Sync,
        transition: Spring::default().into(),
        enter: None,
        exit: None,
        children: Vec::new(),
        on_exit_complete: None,
    }
}

impl<V: Animatable> PresenceGroup<V> {
    pub fn mode(mut self, mode: PresenceMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn transition(mut self, transition: impl Into<Transition>) -> Self {
        self.transition = transition.into();
        self
    }

    pub fn enter(mut self, value: V) -> Self {
        self.enter = Some(value);
        self
    }

    pub fn exit(mut self, value: V) -> Self {
        self.exit = Some(value);
        self
    }

    pub fn child(
        mut self,
        key: impl Into<ElementId>,
        render: impl Fn(V) -> AnyElement + 'static,
    ) -> Self {
        self.children.push((key.into(), Box::new(render)));
        self
    }

    pub fn on_exit_complete(
        mut self,
        callback: impl Fn(&ElementId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_exit_complete = Some(Box::new(callback));
        self
    }
}

impl<V: Animatable> IntoElement for PresenceGroup<V> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pending,
    Entering,
    Present,
    Exiting,
}

struct ChildState<V: Animatable> {
    motion: MotionState,
    phase: Phase,
    render: ChildRender<V>,
}

struct PresenceGroupState<V: Animatable> {
    children: HashMap<ElementId, ChildState<V>>,
    order: Vec<ElementId>,
}

impl<V: Animatable> Default for PresenceGroupState<V> {
    fn default() -> Self {
        Self {
            children: HashMap::new(),
            order: Vec::new(),
        }
    }
}

impl<V: Animatable> Element for PresenceGroup<V> {
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
        let enter = channels(
            self.enter
                .as_ref()
                .expect("PresenceGroup::enter must be configured"),
        );
        let exit = channels(
            self.exit
                .as_ref()
                .expect("PresenceGroup::exit must be configured"),
        );
        let declarations = std::mem::take(&mut self.children);
        let current_order = declarations
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        let current_keys = current_order.iter().cloned().collect::<HashSet<_>>();

        window.with_element_state::<PresenceGroupState<V>, _>(
            global_id.expect("presence groups always have an id"),
            |state, window| {
                let mut state = state.unwrap_or_default();

                let mut never_mounted = Vec::new();
                for (key, child) in &mut state.children {
                    if !current_keys.contains(key) {
                        if child.phase == Phase::Pending {
                            never_mounted.push(key.clone());
                        } else if child.phase != Phase::Exiting {
                            child.phase = Phase::Exiting;
                            child.motion.retarget_if_needed(&exit[..V::CHANNELS]);
                        }
                    }
                }
                for key in never_mounted {
                    state.children.remove(&key);
                }

                let waiting = self.mode == PresenceMode::Wait
                    && state
                        .children
                        .values()
                        .any(|child| child.phase == Phase::Exiting);
                for (key, render) in declarations {
                    if let Some(child) = state.children.get_mut(&key) {
                        child.render = render;
                        if child.phase == Phase::Exiting {
                            child.phase = Phase::Entering;
                            child.motion.retarget_if_needed(&enter[..V::CHANNELS]);
                        } else if child.phase != Phase::Pending
                            && child.motion.retarget_if_needed(&enter[..V::CHANNELS])
                        {
                            child.phase = Phase::Entering;
                        }
                    } else {
                        let phase = if waiting {
                            Phase::Pending
                        } else {
                            Phase::Entering
                        };
                        state.children.insert(
                            key,
                            ChildState {
                                motion: MotionState::new(
                                    &exit[..V::CHANNELS],
                                    &enter[..V::CHANNELS],
                                ),
                                phase,
                                render,
                            },
                        );
                    }
                }

                let exiting_order = state
                    .order
                    .iter()
                    .filter(|key| {
                        !current_keys.contains(*key)
                            && state
                                .children
                                .get(*key)
                                .is_some_and(|child| child.phase == Phase::Exiting)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                state.order = current_order;
                state.order.extend(exiting_order);

                let mut completed = Vec::new();
                for (key, child) in &mut state.children {
                    if child.phase == Phase::Pending {
                        continue;
                    }
                    if reduce_motion(cx) {
                        child.motion.snap();
                    } else {
                        child.motion.tick(
                            Instant::now(),
                            ChannelTransitions::Uniform(&self.transition),
                        );
                    }
                    if child.motion.settled() {
                        match child.phase {
                            Phase::Entering => child.phase = Phase::Present,
                            Phase::Exiting => completed.push(key.clone()),
                            Phase::Pending | Phase::Present => {}
                        }
                    } else {
                        window.request_animation_frame();
                    }
                }

                for key in &completed {
                    state.children.remove(key);
                    if let Some(callback) = &self.on_exit_complete {
                        callback(key, window, cx);
                    }
                }
                state.order.retain(|key| state.children.contains_key(key));

                let has_exiting = state
                    .children
                    .values()
                    .any(|child| child.phase == Phase::Exiting);
                if self.mode == PresenceMode::Wait && !has_exiting {
                    for child in state.children.values_mut() {
                        if child.phase == Phase::Pending {
                            child.phase = Phase::Entering;
                            child.motion.retarget_if_needed(&enter[..V::CHANNELS]);
                            window.request_animation_frame();
                        }
                    }
                }

                let rendered = state
                    .order
                    .iter()
                    .filter_map(|key| state.children.get(key))
                    .filter(|child| child.phase != Phase::Pending)
                    .map(|child| (child.render)(V::read(child.motion.current())))
                    .collect::<Vec<_>>();
                let mut element = div().children(rendered).into_any_element();
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

fn channels<V: Animatable>(value: &V) -> [f32; MAX_CHANNELS] {
    let mut channels = [0.0; MAX_CHANNELS];
    value.write(&mut channels[..V::CHANNELS]);
    channels
}
