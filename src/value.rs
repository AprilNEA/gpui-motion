use std::{cell::RefCell, marker::PhantomData, rc::Rc, time::Instant};

use gpui::{App, Window};

use crate::{Animatable, ChannelTransitions, MAX_CHANNELS, MotionState, Transition, reduce_motion};

pub struct MotionValue<T: Animatable> {
    inner: Rc<RefCell<Inner>>,
    value_type: PhantomData<T>,
}

struct Inner {
    motion: MotionState,
    transition: Transition,
}

impl<T: Animatable> MotionValue<T> {
    pub fn new(initial: T, transition: Transition) -> Self {
        assert!(T::CHANNELS <= MAX_CHANNELS);
        let mut channels = [0.0; MAX_CHANNELS];
        initial.write(&mut channels[..T::CHANNELS]);
        Self {
            inner: Rc::new(RefCell::new(Inner {
                motion: MotionState::new(&channels[..T::CHANNELS], &channels[..T::CHANNELS]),
                transition,
            })),
            value_type: PhantomData,
        }
    }

    pub fn set_target(&self, target: T) {
        let channels = channels(&target);
        self.inner
            .borrow_mut()
            .motion
            .retarget_if_needed(&channels[..T::CHANNELS]);
    }

    pub fn get(&self, window: &Window, cx: &App) -> T {
        let mut inner = self.inner.borrow_mut();
        if reduce_motion(cx) {
            inner.motion.snap();
        } else {
            let transition = inner.transition;
            inner
                .motion
                .tick(Instant::now(), ChannelTransitions::Uniform(&transition));
        }
        if !inner.motion.settled() {
            window.request_animation_frame();
        }
        T::read(inner.motion.current())
    }

    pub fn settled(&self) -> bool {
        self.inner.borrow().motion.settled()
    }

    pub fn get_velocity(&self) -> T {
        T::read(self.inner.borrow().motion.velocity())
    }

    pub fn set_target_with_velocity(&self, target: T, velocity: T) {
        let target = channels(&target);
        let velocity = channels(&velocity);
        let mut inner = self.inner.borrow_mut();
        inner.motion.retarget_if_needed(&target[..T::CHANNELS]);
        inner.motion.set_velocity(&velocity[..T::CHANNELS]);
    }

    pub fn flick(&self, velocity: T) {
        let velocity = channels(&velocity);
        self.inner
            .borrow_mut()
            .motion
            .set_velocity(&velocity[..T::CHANNELS]);
    }

    pub fn jump(&self, value: T) {
        let value = channels(&value);
        let mut inner = self.inner.borrow_mut();
        inner.motion.retarget_if_needed(&value[..T::CHANNELS]);
        inner.motion.snap();
    }

    pub fn map<U: Animatable>(&self, f: impl Fn(T) -> U + 'static) -> MappedValue<T, U> {
        MappedValue {
            source: self.clone(),
            transform: Box::new(f),
        }
    }
}

impl<T: Animatable> Clone for MotionValue<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            value_type: PhantomData,
        }
    }
}

pub struct MappedValue<T: Animatable, U: Animatable> {
    source: MotionValue<T>,
    transform: Box<dyn Fn(T) -> U + 'static>,
}

impl<T: Animatable, U: Animatable> MappedValue<T, U> {
    pub fn get(&self, window: &Window, cx: &App) -> U {
        (self.transform)(self.source.get(window, cx))
    }
}

fn channels<T: Animatable>(value: &T) -> [f32; MAX_CHANNELS] {
    let mut channels = [0.0; MAX_CHANNELS];
    value.write(&mut channels[..T::CHANNELS]);
    channels
}
