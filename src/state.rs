use std::time::Instant;

use crate::{MAX_CHANNELS, Spring, Tween};

const SPRING_DT: f32 = 1.0 / 240.0;
const MAX_SPRING_FRAME_DT: f32 = 1.0 / 30.0;

/// Animation strategy used to advance a [`MotionState`].
#[derive(Clone, Copy)]
pub enum Transition {
    /// A velocity-preserving spring transition.
    Spring(Spring),
    /// A duration-based tween transition.
    Tween(Tween),
}

impl From<Spring> for Transition {
    fn from(spring: Spring) -> Self {
        Self::Spring(spring)
    }
}

impl From<Tween> for Transition {
    fn from(tween: Tween) -> Self {
        Self::Tween(tween)
    }
}

/// Fixed-capacity animation state shared by spring and tween transitions.
pub struct MotionState {
    x: [f32; MAX_CHANNELS],
    v: [f32; MAX_CHANNELS],
    target: [f32; MAX_CHANNELS],
    tween_from: [f32; MAX_CHANNELS],
    len: usize,
    settled: bool,
    last_tick: Instant,
    tween_elapsed: f32,
    accum: f32,
}

impl MotionState {
    /// Creates motion state for equally sized initial and target channel slices.
    ///
    /// # Panics
    ///
    /// Panics when the slices have different lengths or exceed [`MAX_CHANNELS`].
    pub fn new(initial: &[f32], target: &[f32]) -> Self {
        assert_eq!(initial.len(), target.len());
        assert!(initial.len() <= MAX_CHANNELS);

        let len = initial.len();
        let mut x = [0.0; MAX_CHANNELS];
        let mut target_channels = [0.0; MAX_CHANNELS];
        let mut tween_from = [0.0; MAX_CHANNELS];
        x[..len].copy_from_slice(initial);
        target_channels[..len].copy_from_slice(target);
        tween_from[..len].copy_from_slice(initial);

        Self {
            x,
            v: [0.0; MAX_CHANNELS],
            target: target_channels,
            tween_from,
            len,
            settled: initial == target,
            last_tick: Instant::now(),
            tween_elapsed: 0.0,
            accum: 0.0,
        }
    }

    /// Changes the target while preserving the current value and velocity.
    ///
    /// Returns `true` when the target changed.
    ///
    /// # Panics
    ///
    /// Panics when `new_target` does not match the state's channel count.
    pub fn retarget_if_needed(&mut self, new_target: &[f32]) -> bool {
        assert_eq!(new_target.len(), self.len);
        if self.target[..self.len] == *new_target {
            return false;
        }

        self.target[..self.len].copy_from_slice(new_target);
        self.tween_from[..self.len].copy_from_slice(&self.x[..self.len]);
        self.tween_elapsed = 0.0;
        self.settled = false;
        true
    }

    /// Advances the animation to `now` using `transition`.
    pub fn tick(&mut self, now: Instant, transition: &Transition) {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        if self.settled {
            return;
        }

        match transition {
            Transition::Spring(spring) => self.tick_spring(elapsed, *spring),
            Transition::Tween(tween) => self.tick_tween(elapsed, *tween),
        }
    }

    /// Immediately moves to the target and clears velocity.
    pub fn snap(&mut self) {
        self.x[..self.len].copy_from_slice(&self.target[..self.len]);
        self.v[..self.len].fill(0.0);
        self.settled = true;
        self.accum = 0.0;
    }

    /// Returns whether the animation has reached its target.
    pub fn settled(&self) -> bool {
        self.settled
    }

    /// Returns the current scalar channels.
    pub fn current(&self) -> &[f32] {
        &self.x[..self.len]
    }

    /// Returns the current per-channel velocity.
    pub fn velocity(&self) -> &[f32] {
        &self.v[..self.len]
    }

    fn tick_spring(&mut self, elapsed: f32, spring: Spring) {
        self.accum += elapsed.min(MAX_SPRING_FRAME_DT);
        let steps = (self.accum / SPRING_DT).floor() as usize;
        self.accum -= steps as f32 * SPRING_DT;

        for _ in 0..steps {
            for channel in 0..self.len {
                let acceleration = spring.stiffness * (self.target[channel] - self.x[channel])
                    - spring.damping * self.v[channel];
                self.v[channel] += acceleration * SPRING_DT;
                self.x[channel] += self.v[channel] * SPRING_DT;
            }
        }

        let at_rest = (0..self.len).all(|channel| {
            (self.target[channel] - self.x[channel]).abs() < spring.rest_delta
                && self.v[channel].abs() < spring.rest_speed
        });
        if at_rest {
            self.snap();
        }
    }

    fn tick_tween(&mut self, elapsed: f32, tween: Tween) {
        self.accum = 0.0;
        self.tween_elapsed += elapsed;

        if self.tween_elapsed >= tween.duration {
            self.snap();
            return;
        }

        let progress = (self.tween_elapsed / tween.duration).clamp(0.0, 1.0);
        let eased = (tween.easing)(progress);
        for channel in 0..self.len {
            let previous = self.x[channel];
            self.x[channel] = self.tween_from[channel]
                + (self.target[channel] - self.tween_from[channel]) * eased;
            if elapsed > 0.0 {
                self.v[channel] = (self.x[channel] - previous) / elapsed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{MotionState, Transition};
    use crate::{Spring, Tween, easing};

    #[test]
    fn spring_converges() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::Spring(Spring::default());
        let mut now = Instant::now();

        for _ in 0..600 {
            now += Duration::from_secs_f64(1.0 / 60.0);
            state.tick(now, &transition);
            if state.settled() {
                break;
            }
        }

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
    }

    #[test]
    fn retarget_preserves_velocity() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::Spring(Spring::default());
        let now = Instant::now() + Duration::from_millis(100);
        state.tick(now, &transition);
        let x_before = state.current()[0];
        let v_before = state.velocity()[0];

        assert!(state.retarget_if_needed(&[200.0]));

        assert_eq!(state.current()[0], x_before);
        assert_eq!(state.velocity()[0], v_before);
        assert_ne!(v_before, 0.0);
    }

    #[test]
    fn tween_completes_and_snaps() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::Tween(Tween::new(0.1));
        let now = Instant::now() + Duration::from_millis(100);

        state.tick(now, &transition);

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
        assert_eq!(state.velocity(), &[0.0]);
    }

    #[test]
    fn tween_retarget_no_jump() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::Tween(Tween::new(1.0).easing(easing::linear));
        let mut now = Instant::now() + Duration::from_millis(250);
        state.tick(now, &transition);
        let x_before = state.current()[0];

        assert!(state.retarget_if_needed(&[20.0]));
        assert_eq!(state.current()[0], x_before);

        now += Duration::from_millis(100);
        state.tick(now, &transition);
        let expected = x_before + (20.0 - x_before) * 0.1;
        assert!((state.current()[0] - expected).abs() < 0.0001);
    }

    #[test]
    fn dt_clamped() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::Spring(Spring::default());
        let now = Instant::now() + Duration::from_secs(1);

        state.tick(now, &transition);

        assert!(state.current()[0].is_finite());
        assert!(state.velocity()[0].is_finite());
        assert!((0.0..100.0).contains(&state.current()[0]));
    }

    #[test]
    fn mixed_transition_velocity_handoff() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let tween = Transition::Tween(Tween::new(1.0).easing(easing::linear));
        let spring = Transition::Spring(Spring::default());
        let mut now = Instant::now() + Duration::from_millis(100);
        state.tick(now, &tween);
        now += Duration::from_millis(100);
        state.tick(now, &tween);
        let tween_velocity = state.velocity()[0];

        now += Duration::from_secs_f64(1.0 / 60.0);
        state.tick(now, &spring);

        assert!(tween_velocity > 0.0);
        assert!(state.velocity()[0] > 0.0);
    }
}
