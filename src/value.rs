use std::{cell::RefCell, marker::PhantomData, rc::Rc, time::Instant};

use gpui::{App, Window};

use crate::{Animatable, MAX_CHANNELS, MotionState, Transition};

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
        let mut channels = [0.0; MAX_CHANNELS];
        target.write(&mut channels[..T::CHANNELS]);
        self.inner
            .borrow_mut()
            .motion
            .retarget_if_needed(&channels[..T::CHANNELS]);
    }

    pub fn get(&self, window: &Window, cx: &App) -> T {
        let mut inner = self.inner.borrow_mut();
        if cx.reduce_motion() {
            inner.motion.snap();
        } else {
            let transition = inner.transition;
            inner.motion.tick(Instant::now(), &transition);
        }
        if !inner.motion.settled() {
            window.request_animation_frame();
        }
        T::read(inner.motion.current())
    }

    pub fn settled(&self) -> bool {
        self.inner.borrow().motion.settled()
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
