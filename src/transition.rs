use crate::{Inertia, Spring, Tween, tween::Easing};

/// Maximum number of values in a keyframe path.
pub const MAX_KEYFRAMES: usize = 8;

/// Timing and playback configuration for an animation generator.
#[derive(Clone, Copy)]
pub struct Transition {
    /// Generator used by this transition.
    pub kind: TransitionKind,
    /// Initial delay in seconds.
    pub delay: f32,
    /// Optional repetition for tweens and keyframes.
    pub repeat: Option<Repeat>,
}

/// Animation generator selected by a transition.
#[derive(Clone, Copy)]
pub enum TransitionKind {
    /// Analytical spring animation.
    Spring(Spring),
    /// Duration-based two-value interpolation.
    Tween(Tween),
    /// Duration-based multi-keyframe interpolation.
    Keyframes(KeyframesTiming),
    /// Velocity-projected decay with optional boundaries.
    Inertia(Inertia),
}

/// Repetition configuration for tween and keyframe transitions.
#[derive(Clone, Copy)]
pub struct Repeat {
    /// Number of additional plays.
    pub count: RepeatCount,
    /// Direction behavior for additional plays.
    pub kind: RepeatKind,
    /// Pause between plays in seconds.
    pub delay: f32,
}

/// Number of additional transition plays.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RepeatCount {
    /// Repeat the transition this many times after its initial play.
    Times(u32),
    /// Repeat without settling.
    Forever,
}

/// Direction behavior for repeated transitions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RepeatKind {
    /// Restart the same forward generator.
    Loop,
    /// Play the same generated values backward on alternating iterations.
    Reverse,
    /// Generate from reversed values on alternating iterations.
    Mirror,
}

impl Transition {
    /// Sets an initial delay in seconds.
    pub fn delay(self, seconds: f32) -> Self {
        Self {
            delay: seconds,
            ..self
        }
    }

    /// Sets explicit repetition configuration.
    pub fn repeat(self, repeat: Repeat) -> Self {
        Self {
            repeat: Some(repeat),
            ..self
        }
    }

    /// Repeats a tween or keyframe transition a finite number of times.
    pub fn repeat_times(self, count: u32, kind: RepeatKind) -> Self {
        self.repeat(Repeat {
            count: RepeatCount::Times(count),
            kind,
            delay: 0.0,
        })
    }

    /// Repeats a tween or keyframe transition forever.
    pub fn repeat_forever(self, kind: RepeatKind) -> Self {
        self.repeat(Repeat {
            count: RepeatCount::Forever,
            kind,
            delay: 0.0,
        })
    }
}

impl From<Spring> for Transition {
    fn from(spring: Spring) -> Self {
        Self {
            kind: TransitionKind::Spring(spring),
            delay: 0.0,
            repeat: None,
        }
    }
}

impl From<Tween> for Transition {
    fn from(tween: Tween) -> Self {
        Self {
            kind: TransitionKind::Tween(tween),
            delay: 0.0,
            repeat: None,
        }
    }
}

impl From<KeyframesTiming> for Transition {
    fn from(keyframes: KeyframesTiming) -> Self {
        Self {
            kind: TransitionKind::Keyframes(keyframes),
            delay: 0.0,
            repeat: None,
        }
    }
}

impl From<Inertia> for Transition {
    fn from(inertia: Inertia) -> Self {
        Self {
            kind: TransitionKind::Inertia(inertia),
            delay: 0.0,
            repeat: None,
        }
    }
}

/// Timing and easing configuration for a keyframe path.
#[derive(Clone, Copy)]
pub struct KeyframesTiming {
    /// Total duration in seconds.
    pub duration: f32,
    /// Optional normalized keyframe offsets and their populated length.
    pub times: Option<([f32; MAX_KEYFRAMES], usize)>,
    /// Easing shared by all segments or supplied per segment.
    pub easings: EasingSeq,
}

/// Easing assignment for keyframe segments.
#[derive(Clone, Copy)]
pub enum EasingSeq {
    /// Reuse one easing function for every segment.
    Single(Easing),
    /// Use one easing function per segment and record the populated length.
    PerSegment([Easing; MAX_KEYFRAMES - 1], usize),
}

impl KeyframesTiming {
    /// Creates uniformly spaced keyframe timing using smoothstep easing.
    pub fn new(duration: f32) -> Self {
        assert!(
            duration > 0.0,
            "keyframe duration must be greater than zero"
        );
        Self {
            duration,
            times: None,
            easings: EasingSeq::Single(crate::tween::easing::ease_in_out),
        }
    }

    /// Sets normalized keyframe offsets.
    pub fn times(self, times: &[f32]) -> Self {
        assert!(times.len() <= MAX_KEYFRAMES);
        let mut values = [0.0; MAX_KEYFRAMES];
        values[..times.len()].copy_from_slice(times);
        Self {
            times: Some((values, times.len())),
            ..self
        }
    }

    /// Sets one easing function for every keyframe segment.
    pub fn easing(self, easing: Easing) -> Self {
        Self {
            easings: EasingSeq::Single(easing),
            ..self
        }
    }

    /// Sets one easing function per keyframe segment.
    pub fn easings(self, easings: &[Easing]) -> Self {
        assert!(easings.len() < MAX_KEYFRAMES);
        let mut values: [Easing; MAX_KEYFRAMES - 1] =
            [crate::tween::easing::ease_in_out; MAX_KEYFRAMES - 1];
        values[..easings.len()].copy_from_slice(easings);
        Self {
            easings: EasingSeq::PerSegment(values, easings.len()),
            ..self
        }
    }
}

/// Per-channel transition assignment.
#[derive(Clone, Copy)]
pub enum ChannelTransitions<'a> {
    /// Applies one transition to every channel.
    Uniform(&'a Transition),
    /// Applies transitions to consecutive channel segments.
    Segmented(&'a [(usize, Transition)]),
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{ChannelTransitions, KeyframesTiming, RepeatKind, Transition, TransitionKind};
    use crate::{Inertia, MotionState, Spring, Tween, easing};

    fn begin(state: &mut MotionState, transition: &Transition) -> Instant {
        let now = Instant::now();
        state.tick(now, ChannelTransitions::Uniform(transition));
        now
    }

    fn advance(state: &mut MotionState, transition: &Transition, now: &mut Instant, seconds: f32) {
        *now += Duration::from_secs_f32(seconds);
        state.tick(*now, ChannelTransitions::Uniform(transition));
    }

    #[test]
    fn spring_converges() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::from(Spring::default());
        let mut now = begin(&mut state, &transition);

        for _ in 0..600 {
            advance(&mut state, &transition, &mut now, 1.0 / 60.0);
            if state.settled() {
                break;
            }
        }

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
        assert_eq!(state.velocity(), &[0.0]);
    }

    #[test]
    fn mass_changes_period() {
        fn first_velocity_turn(spring: Spring) -> f32 {
            let mut state = MotionState::new(&[0.0], &[1.0]);
            let transition = Transition::from(spring);
            let mut now = begin(&mut state, &transition);
            let mut previous_velocity = 0.0;
            for step in 1..2_000 {
                advance(&mut state, &transition, &mut now, 0.001);
                let velocity = state.velocity()[0];
                if previous_velocity > 0.0 && velocity <= 0.0 {
                    return step as f32 * 0.001;
                }
                previous_velocity = velocity;
            }
            panic!("spring did not reach its first turning point");
        }

        let light = first_velocity_turn(Spring::new(100.0, 0.0));
        let heavy = first_velocity_turn(Spring::new(100.0, 0.0).mass(4.0));

        assert!(
            (heavy / light - 2.0).abs() < 0.03,
            "light={light} heavy={heavy}"
        );
    }

    #[test]
    fn duration_spring_settles_near_duration() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::from(Spring::from_duration(0.8));
        let mut now = begin(&mut state, &transition);
        let mut elapsed = 0.0;

        while !state.settled() && elapsed < 2.0 {
            advance(&mut state, &transition, &mut now, 1.0 / 120.0);
            elapsed += 1.0 / 120.0;
        }

        assert!((0.56..=1.04).contains(&elapsed), "settled at {elapsed}s");
    }

    #[test]
    fn visual_duration_spring_settles() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::from(Spring::from_visual_duration(0.5, 0.2));
        let mut now = begin(&mut state, &transition);

        for _ in 0..240 {
            advance(&mut state, &transition, &mut now, 1.0 / 120.0);
            if state.settled() {
                break;
            }
        }

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
    }

    #[test]
    fn duration_spring_discards_velocity_on_retarget() {
        let transition = Transition::from(Spring::from_duration(0.8));
        let mut state = MotionState::new(&[0.0], &[0.0]);
        state.set_velocity(&[1_000.0]);
        assert!(state.retarget_if_needed(&[100.0]));

        state.tick(Instant::now(), ChannelTransitions::Uniform(&transition));

        assert!(state.velocity()[0].abs() < 10.0, "{:?}", state.velocity());
    }

    #[test]
    fn analytical_velocity_continuity() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::from(Spring::wobbly());
        let mut now = begin(&mut state, &transition);
        advance(&mut state, &transition, &mut now, 0.12);
        let before = state.velocity()[0];

        assert!(state.retarget_if_needed(&[200.0]));
        state.tick(now, ChannelTransitions::Uniform(&transition));

        assert!((state.velocity()[0] - before).abs() < 0.001);
    }

    #[test]
    fn tween_completes_and_snaps() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let transition = Transition::from(Tween::new(0.1));
        let mut now = begin(&mut state, &transition);

        advance(&mut state, &transition, &mut now, 0.1);

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
        assert_eq!(state.velocity(), &[0.0]);
    }

    #[test]
    fn tween_retarget_does_not_jump() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::from(Tween::new(1.0).easing(easing::linear));
        let mut now = begin(&mut state, &transition);
        advance(&mut state, &transition, &mut now, 0.25);
        let before = state.current()[0];

        assert!(state.retarget_if_needed(&[20.0]));
        assert_eq!(state.current()[0], before);
        advance(&mut state, &transition, &mut now, 0.1);

        let expected = before + (20.0 - before) * 0.1;
        assert!((state.current()[0] - expected).abs() < 0.001);
    }

    #[test]
    fn first_tick_uses_time_since_creation() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::from(Tween::new(1.0).easing(easing::linear));

        state.tick(
            Instant::now() + Duration::from_millis(250),
            ChannelTransitions::Uniform(&transition),
        );

        assert!((state.current()[0] - 2.5).abs() < 0.02);
    }

    #[test]
    fn tween_to_spring_preserves_velocity() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        let tween = Transition::from(Tween::new(1.0).easing(easing::linear));
        let spring = Transition::from(Spring::default());
        let mut now = begin(&mut state, &tween);
        advance(&mut state, &tween, &mut now, 0.1);
        let tween_velocity = state.velocity()[0];

        state.tick(now, ChannelTransitions::Uniform(&spring));

        assert!(tween_velocity > 0.0);
        assert!((state.velocity()[0] - tween_velocity).abs() < 0.01);
    }

    #[test]
    fn keyframes_hit_intermediate_values() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        assert!(state.retarget_keyframes_if_needed(&[&[0.0], &[50.0], &[100.0]]));
        let timing = KeyframesTiming::new(1.0)
            .times(&[0.0, 0.25, 1.0])
            .easings(&[easing::linear, easing::linear]);
        let transition = Transition::from(timing);
        let mut now = begin(&mut state, &transition);

        advance(&mut state, &transition, &mut now, 0.25);

        assert!((state.current()[0] - 50.0).abs() < 0.02);
    }

    #[test]
    fn keyframes_retarget_from_current() {
        let mut state = MotionState::new(&[0.0], &[100.0]);
        assert!(state.retarget_keyframes_if_needed(&[&[0.0], &[50.0], &[100.0]]));
        let transition = Transition::from(
            KeyframesTiming::new(1.0)
                .easing(easing::linear)
                .times(&[0.0, 0.5, 1.0]),
        );
        let mut now = begin(&mut state, &transition);
        advance(&mut state, &transition, &mut now, 0.4);
        let before = state.current()[0];

        assert!(state.retarget_keyframes_if_needed(&[&[-100.0], &[75.0], &[200.0]]));
        assert_eq!(state.current()[0], before);
        state.tick(now, ChannelTransitions::Uniform(&transition));

        assert_eq!(state.current()[0], before);
    }

    #[test]
    fn repeat_loop() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::from(Tween::new(1.0).easing(easing::linear))
            .repeat_times(1, RepeatKind::Loop);
        let mut now = begin(&mut state, &transition);

        advance(&mut state, &transition, &mut now, 1.25);

        assert!((state.current()[0] - 2.5).abs() < 0.01);
    }

    #[test]
    fn repeat_reverse() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::from(Tween::new(1.0).easing(easing::linear))
            .repeat_times(1, RepeatKind::Reverse);
        let mut now = begin(&mut state, &transition);

        advance(&mut state, &transition, &mut now, 1.25);

        assert!((state.current()[0] - 7.5).abs() < 0.01);
    }

    #[test]
    fn repeat_mirror_regenerates_easing() {
        let mut mirrored = MotionState::new(&[0.0], &[10.0]);
        let mut reversed = MotionState::new(&[0.0], &[10.0]);
        let mirror = Transition::from(Tween::new(1.0).easing(easing::ease_in))
            .repeat_times(1, RepeatKind::Mirror);
        let reverse = Transition::from(Tween::new(1.0).easing(easing::ease_in))
            .repeat_times(1, RepeatKind::Reverse);
        let mut mirror_now = begin(&mut mirrored, &mirror);
        let mut reverse_now = begin(&mut reversed, &reverse);

        advance(&mut mirrored, &mirror, &mut mirror_now, 1.25);
        advance(&mut reversed, &reverse, &mut reverse_now, 1.25);

        assert!(mirrored.current()[0] < 10.0);
        assert!(mirrored.current()[0] > reversed.current()[0]);
    }

    #[test]
    fn repeat_forever_never_settles() {
        let mut state = MotionState::new(&[0.0], &[0.0]);
        let transition = Transition::from(Tween::new(0.1)).repeat_forever(RepeatKind::Loop);

        state.tick(
            Instant::now() + Duration::from_secs(10),
            ChannelTransitions::Uniform(&transition),
        );

        assert!(!state.settled());
    }

    #[test]
    fn delay_holds_initial() {
        let mut state = MotionState::new(&[0.0], &[10.0]);
        let transition = Transition::from(Tween::new(1.0).easing(easing::linear)).delay(0.5);
        let mut now = begin(&mut state, &transition);

        advance(&mut state, &transition, &mut now, 0.3);
        assert_eq!(state.current(), &[0.0]);
        assert!(!state.settled());

        advance(&mut state, &transition, &mut now, 0.45);
        assert!((state.current()[0] - 2.5).abs() < 0.01);
    }

    fn snap_hundreds(value: f32) -> f32 {
        (value / 100.0).round() * 100.0
    }

    #[test]
    fn inertia_decays_to_projected_target() {
        let mut state = MotionState::new(&[0.0], &[0.0]);
        state.set_velocity(&[100.0]);
        let transition = Transition::from(Inertia::new().modify_target(snap_hundreds));
        let mut now = begin(&mut state, &transition);

        for _ in 0..240 {
            advance(&mut state, &transition, &mut now, 1.0 / 60.0);
            if state.settled() {
                break;
            }
        }

        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
    }

    #[test]
    fn inertia_bounces_at_boundary() {
        let mut state = MotionState::new(&[0.0], &[0.0]);
        state.set_velocity(&[500.0]);
        let transition = Transition::from(Inertia::new().bounds(-100.0, 100.0));
        let mut now = begin(&mut state, &transition);
        let mut crossed_boundary = false;

        for _ in 0..600 {
            advance(&mut state, &transition, &mut now, 1.0 / 120.0);
            crossed_boundary |= state.current()[0] > 100.0;
            if state.settled() {
                break;
            }
        }

        assert!(crossed_boundary);
        assert!(state.settled());
        assert_eq!(state.current(), &[100.0]);
    }

    #[test]
    fn segmented_channels_are_independent() {
        let mut state = MotionState::new(&[0.0, 0.0], &[10.0, 10.0]);
        let segments = [
            (1, Transition::from(Tween::new(0.1).easing(easing::linear))),
            (1, Transition::from(Tween::new(1.0).easing(easing::linear))),
        ];
        let mut now = Instant::now();
        state.tick(now, ChannelTransitions::Segmented(&segments));

        now += Duration::from_millis(200);
        state.tick(now, ChannelTransitions::Segmented(&segments));

        assert_eq!(state.current()[0], 10.0);
        assert!((state.current()[1] - 2.0).abs() < 0.01);
        assert!(!state.settled());
    }

    #[test]
    fn unchanged_segment_does_not_delay_settling() {
        let mut state = MotionState::new(&[0.0, 0.0], &[0.0, 10.0]);
        let segments = [
            (1, Transition::from(Tween::new(10.0))),
            (1, Transition::from(Tween::new(0.1))),
        ];
        let mut now = Instant::now();
        state.tick(now, ChannelTransitions::Segmented(&segments));

        now += Duration::from_millis(100);
        state.tick(now, ChannelTransitions::Segmented(&segments));

        assert!(state.settled());
        assert_eq!(state.current(), &[0.0, 10.0]);
    }

    #[test]
    fn spring_and_inertia_ignore_repeat() {
        let spring = Transition::from(Spring::default()).repeat_forever(RepeatKind::Reverse);
        let inertia = Transition::from(Inertia::default()).repeat_forever(RepeatKind::Mirror);

        assert!(matches!(spring.kind, TransitionKind::Spring(_)));
        assert!(matches!(inertia.kind, TransitionKind::Inertia(_)));
    }
}
