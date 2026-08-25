use std::time::Instant;

use crate::{
    ChannelTransitions, EasingSeq, Inertia, KeyframesTiming, MAX_CHANNELS, MAX_KEYFRAMES, Repeat,
    RepeatCount, RepeatKind, Spring, Transition, TransitionKind, Tween,
};

/// Fixed-capacity state for independently animated scalar channels.
pub struct MotionState {
    x: [f32; MAX_CHANNELS],
    v: [f32; MAX_CHANNELS],
    target: [f32; MAX_CHANNELS],
    keyframes: [[f32; MAX_CHANNELS]; MAX_KEYFRAMES],
    keyframes_len: usize,
    len: usize,
    settled: bool,
    last_tick: Instant,
    revision: u64,
    tracks: [Track; MAX_CHANNELS],
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
        x[..len].copy_from_slice(initial);
        target_channels[..len].copy_from_slice(target);

        Self {
            x,
            v: [0.0; MAX_CHANNELS],
            target: target_channels,
            keyframes: [[0.0; MAX_CHANNELS]; MAX_KEYFRAMES],
            keyframes_len: 0,
            len,
            settled: initial == target,
            last_tick: Instant::now(),
            revision: 0,
            tracks: [Track::new(); MAX_CHANNELS],
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
        if self.keyframes_len == 0 && self.target[..self.len] == *new_target {
            return false;
        }

        self.target[..self.len].copy_from_slice(new_target);
        self.keyframes_len = 0;
        self.restart();
        true
    }

    /// Changes a multi-keyframe path only when one of its values changed.
    ///
    /// The current value replaces the first keyframe when playback begins, so
    /// retargeting never causes a visible jump.
    ///
    /// # Panics
    ///
    /// Panics unless there are `1..=MAX_KEYFRAMES` equally sized frames.
    pub fn retarget_keyframes_if_needed(&mut self, frames: &[&[f32]]) -> bool {
        assert!((1..=MAX_KEYFRAMES).contains(&frames.len()));
        for frame in frames {
            assert_eq!(frame.len(), self.len);
        }

        let unchanged = self.keyframes_len == frames.len()
            && frames
                .iter()
                .enumerate()
                .all(|(frame_index, frame)| self.keyframes[frame_index][..self.len] == **frame);
        if unchanged {
            return false;
        }

        for (frame_index, frame) in frames.iter().enumerate() {
            self.keyframes[frame_index][..self.len].copy_from_slice(frame);
        }
        self.keyframes_len = frames.len();
        self.target[..self.len].copy_from_slice(frames[frames.len() - 1]);
        self.restart();
        true
    }

    /// Injects per-channel velocity, for example from a released gesture.
    ///
    /// # Panics
    ///
    /// Panics when `velocity` does not match the state's channel count.
    pub fn set_velocity(&mut self, velocity: &[f32]) {
        assert_eq!(velocity.len(), self.len);
        self.v[..self.len].copy_from_slice(velocity);
        self.restart();
    }

    /// Advances every channel to `now` using uniform or segmented transitions.
    ///
    /// # Panics
    ///
    /// Panics when segmented transition lengths do not sum to the channel count.
    pub fn tick(&mut self, now: Instant, transitions: ChannelTransitions) {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;
        let assigned = self.assign_transitions(transitions);

        if self.settled {
            let repeats_forever = assigned[..self.len].iter().any(|transition| {
                transition.is_some_and(|transition| {
                    matches!(
                        transition.kind,
                        TransitionKind::Tween(_) | TransitionKind::Keyframes(_)
                    ) && transition
                        .repeat
                        .is_some_and(|repeat| repeat.count == RepeatCount::Forever)
                })
            });
            if !repeats_forever {
                return;
            }
            self.settled = false;
        }

        for (channel, transition) in assigned[..self.len].iter().enumerate() {
            let transition = transition.expect("every channel has a transition");
            let track = self.tracks[channel];
            let mut track = if track.revision != self.revision
                || !track
                    .transition
                    .is_some_and(|current| transition_eq(current, transition))
            {
                self.initialize_track(channel, transition)
            } else {
                track
            };

            if !track.settled {
                self.advance_track(channel, transition, elapsed, &mut track);
            }
            self.tracks[channel] = track;
        }

        self.settled = self.tracks[..self.len].iter().all(|track| track.settled);
    }

    /// Immediately moves to the final target and clears all velocity.
    pub fn snap(&mut self) {
        self.x[..self.len].copy_from_slice(&self.target[..self.len]);
        self.v[..self.len].fill(0.0);
        for track in &mut self.tracks[..self.len] {
            track.settled = true;
        }
        self.settled = true;
    }

    /// Returns whether every channel track has completed.
    pub fn settled(&self) -> bool {
        self.settled
    }

    /// Returns the current scalar channels.
    pub fn current(&self) -> &[f32] {
        &self.x[..self.len]
    }

    /// Returns the current per-channel velocity in units per second.
    pub fn velocity(&self) -> &[f32] {
        &self.v[..self.len]
    }

    fn restart(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.settled = false;
    }

    fn assign_transitions(
        &self,
        transitions: ChannelTransitions<'_>,
    ) -> [Option<Transition>; MAX_CHANNELS] {
        let mut assigned = [None; MAX_CHANNELS];
        match transitions {
            ChannelTransitions::Uniform(transition) => {
                assigned[..self.len].fill(Some(*transition));
            }
            ChannelTransitions::Segmented(segments) => {
                let mut offset = 0;
                for (count, transition) in segments {
                    assert!(
                        offset + count <= self.len,
                        "segmented transitions exceed the channel count"
                    );
                    assigned[offset..offset + count].fill(Some(*transition));
                    offset += count;
                }
                assert_eq!(
                    offset, self.len,
                    "segmented transition lengths must equal the channel count"
                );
            }
        }
        assigned
    }

    fn initialize_track(&mut self, channel: usize, transition: Transition) -> Track {
        let origin = self.x[channel];
        let velocity = self.v[channel];
        let generator = match transition.kind {
            TransitionKind::Spring(spring) => Generator::Spring(SpringGenerator::new(
                origin,
                self.target[channel],
                velocity,
                spring,
            )),
            TransitionKind::Tween(tween) => Generator::Tween(TweenGenerator {
                origin,
                target: self.target[channel],
                tween,
            }),
            TransitionKind::Keyframes(timing) => {
                Generator::Keyframes(KeyframesGenerator { origin, timing })
            }
            TransitionKind::Inertia(inertia) => {
                let generator = InertiaGenerator::new(origin, velocity, inertia);
                self.target[channel] = generator.target;
                Generator::Inertia(generator)
            }
        };
        let settled = !self.generator_has_motion(channel, generator, transition.repeat);
        if settled {
            self.v[channel] = 0.0;
        }

        Track {
            transition: Some(transition),
            revision: self.revision,
            elapsed: 0.0,
            start: origin,
            generator,
            settled,
        }
    }

    fn generator_has_motion(
        &self,
        channel: usize,
        generator: Generator,
        repeat: Option<Repeat>,
    ) -> bool {
        let repeats_forever = repeat.is_some_and(|repeat| repeat.count == RepeatCount::Forever);
        match generator {
            Generator::None => false,
            Generator::Spring(generator) => {
                generator.origin != generator.target || generator.velocity != 0.0
            }
            Generator::Tween(generator) => repeats_forever || generator.origin != generator.target,
            Generator::Keyframes(generator) => {
                repeats_forever || self.keyframe_path_has_motion(channel, generator.origin)
            }
            Generator::Inertia(generator) => {
                generator.amplitude != 0.0
                    || nearest_violated_boundary(generator.target, generator.inertia).is_some()
            }
        }
    }

    fn keyframe_path_has_motion(&self, channel: usize, origin: f32) -> bool {
        match self.keyframes_len {
            0 => self.target[channel] != origin,
            1 => self.keyframes[0][channel] != origin,
            count => self.keyframes[1..count]
                .iter()
                .any(|frame| frame[channel] != origin),
        }
    }

    fn advance_track(
        &mut self,
        channel: usize,
        transition: Transition,
        elapsed: f32,
        track: &mut Track,
    ) {
        track.elapsed += elapsed;
        let delay = transition.delay.max(0.0);
        if track.elapsed < delay {
            self.x[channel] = track.start;
            self.v[channel] = 0.0;
            return;
        }
        let active_elapsed = track.elapsed - delay;
        let previous = self.x[channel];

        match track.generator {
            Generator::None => {}
            Generator::Spring(generator) => {
                let (value, velocity, done) = generator.sample(active_elapsed);
                self.x[channel] = value;
                self.v[channel] = velocity;
                track.settled = done;
            }
            Generator::Tween(generator) => {
                let (value, done) = generator.sample(active_elapsed, transition.repeat);
                self.x[channel] = value;
                self.update_discrete_velocity(channel, previous, elapsed, done);
                track.settled = done;
            }
            Generator::Keyframes(generator) => {
                let (value, done) =
                    self.sample_keyframes(channel, generator, active_elapsed, transition.repeat);
                self.x[channel] = value;
                self.update_discrete_velocity(channel, previous, elapsed, done);
                track.settled = done;
            }
            Generator::Inertia(mut generator) => {
                let (value, velocity, done, boundary) = generator.sample(active_elapsed);
                self.x[channel] = value;
                self.v[channel] = velocity;
                if let Some(boundary) = boundary {
                    self.target[channel] = boundary;
                }
                track.generator = Generator::Inertia(generator);
                track.settled = done;
            }
        }
    }

    fn update_discrete_velocity(
        &mut self,
        channel: usize,
        previous: f32,
        elapsed: f32,
        done: bool,
    ) {
        if done {
            self.v[channel] = 0.0;
        } else if elapsed > 0.0 {
            self.v[channel] = (self.x[channel] - previous) / elapsed;
        }
    }

    fn sample_keyframes(
        &self,
        channel: usize,
        generator: KeyframesGenerator,
        elapsed: f32,
        repeat: Option<Repeat>,
    ) -> (f32, bool) {
        let phase = playback_phase(elapsed, generator.timing.duration, repeat);
        match phase {
            PlaybackPhase::Running { cycle, progress } => (
                self.keyframe_value(channel, generator, cycle, progress, repeat),
                false,
            ),
            PlaybackPhase::Gap { cycle } => (
                self.keyframe_value(channel, generator, cycle, 1.0, repeat),
                false,
            ),
            PlaybackPhase::Complete { cycle } => (
                self.keyframe_value(channel, generator, cycle, 1.0, repeat),
                true,
            ),
        }
    }

    fn keyframe_value(
        &self,
        channel: usize,
        generator: KeyframesGenerator,
        cycle: u64,
        progress: f32,
        repeat: Option<Repeat>,
    ) -> f32 {
        let kind = repeat.map_or(RepeatKind::Loop, |repeat| repeat.kind);
        let odd = cycle % 2 == 1;
        match kind {
            RepeatKind::Loop => self.interpolate_keyframes(channel, generator, progress, false),
            RepeatKind::Reverse if odd => {
                self.interpolate_keyframes(channel, generator, 1.0 - progress, false)
            }
            RepeatKind::Mirror if odd => {
                self.interpolate_keyframes(channel, generator, progress, true)
            }
            RepeatKind::Reverse | RepeatKind::Mirror => {
                self.interpolate_keyframes(channel, generator, progress, false)
            }
        }
    }

    fn interpolate_keyframes(
        &self,
        channel: usize,
        generator: KeyframesGenerator,
        progress: f32,
        mirrored: bool,
    ) -> f32 {
        let count = self.effective_keyframe_count();
        let timing = generator.timing;
        let valid_times = valid_keyframe_times(timing, count);
        let time_at = |index: usize| {
            if valid_times {
                timing.times.expect("validated keyframe times").0[index]
            } else {
                index as f32 / (count - 1) as f32
            }
        };

        if progress <= time_at(0) {
            return self.keyframe_at(channel, generator.origin, 0, count, mirrored);
        }
        if progress >= time_at(count - 1) {
            return self.keyframe_at(channel, generator.origin, count - 1, count, mirrored);
        }

        let segment = (0..count - 1)
            .find(|index| progress <= time_at(index + 1))
            .unwrap_or(count - 2);
        let start_time = time_at(segment);
        let end_time = time_at(segment + 1);
        let segment_progress = if end_time > start_time {
            (progress - start_time) / (end_time - start_time)
        } else {
            1.0
        };
        let easing = match timing.easings {
            EasingSeq::Single(easing) => easing,
            EasingSeq::PerSegment(easings, len) if len == count - 1 => easings[segment],
            EasingSeq::PerSegment(_, _) => crate::tween::easing::ease_in_out,
        };
        let eased = easing(segment_progress.clamp(0.0, 1.0));
        let from = self.keyframe_at(channel, generator.origin, segment, count, mirrored);
        let to = self.keyframe_at(channel, generator.origin, segment + 1, count, mirrored);
        from + (to - from) * eased
    }

    fn effective_keyframe_count(&self) -> usize {
        match self.keyframes_len {
            0 | 1 => 2,
            count => count,
        }
    }

    fn keyframe_at(
        &self,
        channel: usize,
        origin: f32,
        index: usize,
        count: usize,
        mirrored: bool,
    ) -> f32 {
        let index = if mirrored { count - 1 - index } else { index };
        if index == 0 {
            origin
        } else if self.keyframes_len == 0 {
            self.target[channel]
        } else if self.keyframes_len == 1 {
            self.keyframes[0][channel]
        } else {
            self.keyframes[index][channel]
        }
    }
}

#[derive(Clone, Copy)]
struct Track {
    transition: Option<Transition>,
    revision: u64,
    elapsed: f32,
    start: f32,
    generator: Generator,
    settled: bool,
}

impl Track {
    const fn new() -> Self {
        Self {
            transition: None,
            revision: u64::MAX,
            elapsed: 0.0,
            start: 0.0,
            generator: Generator::None,
            settled: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Generator {
    None,
    Spring(SpringGenerator),
    Tween(TweenGenerator),
    Keyframes(KeyframesGenerator),
    Inertia(InertiaGenerator),
}

#[derive(Clone, Copy)]
struct SpringGenerator {
    origin: f32,
    target: f32,
    velocity: f32,
    spring: Spring,
    rest_delta: f32,
    rest_speed: f32,
}

impl SpringGenerator {
    fn new(origin: f32, target: f32, velocity: f32, spring: Spring) -> Self {
        let short_travel = (target - origin).abs() < 5.0;
        Self {
            origin,
            target,
            velocity: if spring.is_duration_based() {
                0.0
            } else {
                velocity
            },
            spring,
            rest_delta: spring
                .rest_delta
                .unwrap_or(if short_travel { 0.005 } else { 0.5 }),
            rest_speed: spring
                .rest_speed
                .unwrap_or(if short_travel { 0.01 } else { 2.0 }),
        }
    }

    fn sample(self, elapsed: f32) -> (f32, f32, bool) {
        let mass = f64::from(self.spring.mass);
        let stiffness = f64::from(self.spring.stiffness);
        let damping = f64::from(self.spring.damping);
        let natural = (stiffness / mass).sqrt();
        let ratio = damping / (2.0 * (stiffness * mass).sqrt());
        let time = f64::from(elapsed.max(0.0));
        let displacement = f64::from(self.origin - self.target);
        let initial_velocity = f64::from(self.velocity);

        let (position, velocity) = if ratio < 1.0 - 1.0e-7 {
            let damped = natural * (1.0 - ratio * ratio).sqrt();
            let envelope = (-ratio * natural * time).exp();
            let cosine = (damped * time).cos();
            let sine = (damped * time).sin();
            let sine_coefficient = (initial_velocity + ratio * natural * displacement) / damped;
            let offset = envelope * (displacement * cosine + sine_coefficient * sine);
            let speed = envelope
                * (-ratio * natural * (displacement * cosine + sine_coefficient * sine)
                    + damped * (-displacement * sine + sine_coefficient * cosine));
            (f64::from(self.target) + offset, speed)
        } else if ratio <= 1.0 + 1.0e-7 {
            let coefficient = initial_velocity + natural * displacement;
            let envelope = (-natural * time).exp();
            let offset = envelope * (displacement + coefficient * time);
            let speed = envelope * (initial_velocity - natural * coefficient * time);
            (f64::from(self.target) + offset, speed)
        } else {
            let damped = natural * (ratio * ratio - 1.0).sqrt();
            let argument = (damped * time).min(300.0);
            let envelope = (-ratio * natural * time).exp();
            let sine_coefficient = (initial_velocity + ratio * natural * displacement) / damped;
            let term = displacement * argument.cosh() + sine_coefficient * argument.sinh();
            let offset = envelope * term;
            let speed = envelope
                * (-ratio * natural * term
                    + damped
                        * (displacement * argument.sinh() + sine_coefficient * argument.cosh()));
            (f64::from(self.target) + offset, speed)
        };

        let position = position as f32;
        let velocity = velocity as f32;
        let done =
            velocity.abs() <= self.rest_speed && (self.target - position).abs() <= self.rest_delta;
        if done {
            (self.target, 0.0, true)
        } else {
            (position, velocity, false)
        }
    }
}

#[derive(Clone, Copy)]
struct TweenGenerator {
    origin: f32,
    target: f32,
    tween: Tween,
}

impl TweenGenerator {
    fn sample(self, elapsed: f32, repeat: Option<Repeat>) -> (f32, bool) {
        let phase = playback_phase(elapsed, self.tween.duration, repeat);
        match phase {
            PlaybackPhase::Running { cycle, progress } => {
                (self.value(cycle, progress, repeat), false)
            }
            PlaybackPhase::Gap { cycle } => (self.value(cycle, 1.0, repeat), false),
            PlaybackPhase::Complete { cycle } => (self.value(cycle, 1.0, repeat), true),
        }
    }

    fn value(self, cycle: u64, progress: f32, repeat: Option<Repeat>) -> f32 {
        let kind = repeat.map_or(RepeatKind::Loop, |repeat| repeat.kind);
        let odd = cycle % 2 == 1;
        let eased = match kind {
            RepeatKind::Reverse if odd => (self.tween.easing)(1.0 - progress),
            RepeatKind::Mirror if odd => 1.0 - (self.tween.easing)(progress),
            RepeatKind::Loop | RepeatKind::Reverse | RepeatKind::Mirror => {
                (self.tween.easing)(progress)
            }
        };
        self.origin + (self.target - self.origin) * eased
    }
}

#[derive(Clone, Copy)]
struct KeyframesGenerator {
    origin: f32,
    timing: KeyframesTiming,
}

#[derive(Clone, Copy)]
struct InertiaGenerator {
    amplitude: f32,
    target: f32,
    inertia: Inertia,
    boundary_time: f32,
    bounce: Option<SpringGenerator>,
}

impl InertiaGenerator {
    fn new(origin: f32, velocity: f32, inertia: Inertia) -> Self {
        let ideal = origin + inertia.power * velocity;
        let target = inertia.modify_target.map_or(ideal, |modify| modify(ideal));
        Self {
            amplitude: target - origin,
            target,
            inertia,
            boundary_time: 0.0,
            bounce: None,
        }
    }

    fn sample(&mut self, elapsed: f32) -> (f32, f32, bool, Option<f32>) {
        if let Some(bounce) = self.bounce {
            let (value, velocity, done) = bounce.sample(elapsed - self.boundary_time);
            return (value, velocity, done, None);
        }

        let time_constant = self.inertia.time_constant.max(f32::MIN_POSITIVE);
        let envelope = (-elapsed / time_constant).exp();
        let delta = -self.amplitude * envelope;
        let value = self.target + delta;
        let velocity = self.amplitude / time_constant * envelope;

        if let Some(boundary) = nearest_violated_boundary(value, self.inertia) {
            let spring = Spring::new(self.inertia.bounce_stiffness, self.inertia.bounce_damping)
                .rest(self.inertia.rest_delta, 2.0);
            self.boundary_time = elapsed;
            self.bounce = Some(SpringGenerator::new(value, boundary, velocity, spring));
            self.target = boundary;
            return (value, velocity, false, Some(boundary));
        }

        if delta.abs() <= self.inertia.rest_delta {
            (self.target, 0.0, true, None)
        } else {
            (value, velocity, false, None)
        }
    }
}

#[derive(Clone, Copy)]
enum PlaybackPhase {
    Running { cycle: u64, progress: f32 },
    Gap { cycle: u64 },
    Complete { cycle: u64 },
}

fn playback_phase(elapsed: f32, duration: f32, repeat: Option<Repeat>) -> PlaybackPhase {
    let duration = duration.max(f32::MIN_POSITIVE);
    let elapsed = elapsed.max(0.0);
    let Some(repeat) = repeat else {
        return if elapsed >= duration {
            PlaybackPhase::Complete { cycle: 0 }
        } else {
            PlaybackPhase::Running {
                cycle: 0,
                progress: elapsed / duration,
            }
        };
    };
    let repeat_delay = repeat.delay.max(0.0);

    if let RepeatCount::Times(count) = repeat.count {
        let end = duration * (count as f32 + 1.0) + repeat_delay * count as f32;
        if elapsed >= end {
            return PlaybackPhase::Complete {
                cycle: u64::from(count),
            };
        }
    }
    if elapsed < duration {
        return PlaybackPhase::Running {
            cycle: 0,
            progress: elapsed / duration,
        };
    }

    let remainder = elapsed - duration;
    let span = repeat_delay + duration;
    let slot = (remainder / span).floor() as u64;
    let offset = remainder - slot as f32 * span;
    if offset < repeat_delay {
        PlaybackPhase::Gap { cycle: slot }
    } else {
        PlaybackPhase::Running {
            cycle: slot + 1,
            progress: (offset - repeat_delay) / duration,
        }
    }
}

fn valid_keyframe_times(timing: KeyframesTiming, count: usize) -> bool {
    let Some((times, len)) = timing.times else {
        return false;
    };
    len == count
        && times[..len].iter().all(|time| (0.0..=1.0).contains(time))
        && times[..len].windows(2).all(|pair| pair[0] <= pair[1])
}

fn nearest_violated_boundary(value: f32, inertia: Inertia) -> Option<f32> {
    let out_of_bounds =
        inertia.min.is_some_and(|min| value < min) || inertia.max.is_some_and(|max| value > max);
    if !out_of_bounds {
        return None;
    }

    match (inertia.min, inertia.max) {
        (Some(min), Some(max)) => {
            if (value - min).abs() <= (value - max).abs() {
                Some(min)
            } else {
                Some(max)
            }
        }
        (Some(min), None) => Some(min),
        (None, Some(max)) => Some(max),
        (None, None) => None,
    }
}

fn transition_eq(left: Transition, right: Transition) -> bool {
    left.delay == right.delay
        && repeat_eq(left.repeat, right.repeat)
        && match (left.kind, right.kind) {
            (TransitionKind::Spring(left), TransitionKind::Spring(right)) => left == right,
            (TransitionKind::Tween(left), TransitionKind::Tween(right)) => {
                left.duration == right.duration && std::ptr::fn_addr_eq(left.easing, right.easing)
            }
            (TransitionKind::Keyframes(left), TransitionKind::Keyframes(right)) => {
                keyframes_timing_eq(left, right)
            }
            (TransitionKind::Inertia(left), TransitionKind::Inertia(right)) => {
                inertia_eq(left, right)
            }
            _ => false,
        }
}

fn repeat_eq(left: Option<Repeat>, right: Option<Repeat>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.count == right.count && left.kind == right.kind && left.delay == right.delay
        }
        (None, None) => true,
        _ => false,
    }
}

fn keyframes_timing_eq(left: KeyframesTiming, right: KeyframesTiming) -> bool {
    left.duration == right.duration
        && left.times == right.times
        && match (left.easings, right.easings) {
            (EasingSeq::Single(left), EasingSeq::Single(right)) => {
                std::ptr::fn_addr_eq(left, right)
            }
            (EasingSeq::PerSegment(left, left_len), EasingSeq::PerSegment(right, right_len))
                if left_len == right_len =>
            {
                left[..left_len]
                    .iter()
                    .zip(&right[..right_len])
                    .all(|(left, right)| std::ptr::fn_addr_eq(*left, *right))
            }
            _ => false,
        }
}

fn inertia_eq(left: Inertia, right: Inertia) -> bool {
    left.power == right.power
        && left.time_constant == right.time_constant
        && optional_fn_eq(left.modify_target, right.modify_target)
        && left.min == right.min
        && left.max == right.max
        && left.bounce_stiffness == right.bounce_stiffness
        && left.bounce_damping == right.bounce_damping
        && left.rest_delta == right.rest_delta
}

fn optional_fn_eq(left: Option<fn(f32) -> f32>, right: Option<fn(f32) -> f32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::SpringGenerator;
    use crate::Spring;

    #[test]
    fn adaptive_rest_thresholds() {
        let short = SpringGenerator::new(0.0, 3.0, 0.0, Spring::default());
        let long = SpringGenerator::new(0.0, 300.0, 0.0, Spring::default());

        assert_eq!((short.rest_delta, short.rest_speed), (0.005, 0.01));
        assert_eq!((long.rest_delta, long.rest_speed), (0.5, 2.0));
    }
}
