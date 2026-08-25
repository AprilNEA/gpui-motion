use std::f32::consts::TAU;

/// Parameters for a damped spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Spring stiffness.
    pub stiffness: f32,
    /// Damping coefficient.
    pub damping: f32,
    /// Spring mass.
    pub mass: f32,
    /// Maximum displacement at which the spring is considered at rest.
    pub rest_delta: Option<f32>,
    /// Maximum speed at which the spring is considered at rest.
    pub rest_speed: Option<f32>,
    duration_based: bool,
}

impl Spring {
    /// Creates a spring with the given stiffness and damping.
    pub fn new(stiffness: f32, damping: f32) -> Self {
        assert!(
            stiffness > 0.0,
            "spring stiffness must be greater than zero"
        );
        assert!(damping >= 0.0, "spring damping cannot be negative");
        Self {
            stiffness,
            damping,
            mass: 1.0,
            rest_delta: None,
            rest_speed: None,
            duration_based: false,
        }
    }

    /// Sets the spring mass.
    pub fn mass(self, mass: f32) -> Self {
        assert!(mass > 0.0, "spring mass must be greater than zero");
        Self { mass, ..self }
    }

    /// Sets the displacement and speed thresholds used to detect rest.
    pub fn rest(self, rest_delta: f32, rest_speed: f32) -> Self {
        Self {
            rest_delta: Some(rest_delta),
            rest_speed: Some(rest_speed),
            ..self
        }
    }

    /// Resolves a duration-based spring using the default bounce of `0.3`.
    pub fn from_duration(duration: f32) -> Self {
        Self::from_duration_bounce(duration, 0.3)
    }

    /// Resolves duration and bounce into physical spring parameters.
    pub fn from_duration_bounce(duration: f32, bounce: f32) -> Self {
        let duration = duration.clamp(0.01, 10.0);
        let damping_ratio = (1.0 - bounce).clamp(0.05, 1.0);
        let frequency = find_frequency(duration, damping_ratio);
        let mass = 1.0;
        let stiffness = frequency * frequency * mass;
        let damping = damping_ratio * 2.0 * (mass * stiffness).sqrt();

        if stiffness.is_finite() && damping.is_finite() {
            Self {
                stiffness,
                damping,
                mass,
                rest_delta: None,
                rest_speed: None,
                duration_based: true,
            }
        } else {
            Self {
                duration_based: true,
                ..Self::new(100.0, 10.0)
            }
        }
    }

    /// Resolves a visual duration and bounce into physical spring parameters.
    pub fn from_visual_duration(duration: f32, bounce: f32) -> Self {
        assert!(
            duration.is_finite() && duration > 0.0,
            "visual duration must be greater than zero"
        );
        let damping_ratio = (1.0 - bounce).clamp(0.05, 1.0);
        let root = TAU / (duration * 1.2);
        let stiffness = root * root;
        Self {
            stiffness,
            damping: 2.0 * damping_ratio * stiffness.sqrt(),
            mass: 1.0,
            rest_delta: None,
            rest_speed: None,
            duration_based: true,
        }
    }

    /// Returns a gentle spring preset.
    pub fn gentle() -> Self {
        Self::new(120.0, 14.0)
    }

    /// Returns a spring preset with visible overshoot.
    pub fn wobbly() -> Self {
        Self::new(180.0, 12.0)
    }

    /// Returns a stiff spring preset.
    pub fn stiff() -> Self {
        Self::new(310.0, 26.0)
    }

    /// Returns a slow spring preset.
    pub fn slow() -> Self {
        Self::new(280.0, 60.0)
    }

    pub(crate) fn is_duration_based(self) -> bool {
        self.duration_based
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::new(170.0, 26.0)
    }
}

fn find_frequency(duration: f32, damping_ratio: f32) -> f32 {
    const SAFE_MIN: f32 = 0.001;

    let envelope = |frequency: f32| {
        if damping_ratio < 1.0 {
            SAFE_MIN
                - (frequency * damping_ratio
                    / (frequency * (1.0 - damping_ratio * damping_ratio).sqrt()))
                    * (-frequency * damping_ratio * duration).exp()
        } else {
            -SAFE_MIN + (-frequency * duration).exp() * (frequency * duration + 1.0)
        }
    };
    let derivative = |frequency: f32| {
        if damping_ratio < 1.0 {
            let delta = frequency * damping_ratio * duration;
            let e = damping_ratio * damping_ratio * frequency * frequency * duration;
            let g = frequency * frequency * (1.0 - damping_ratio * damping_ratio).sqrt();
            let factor = if -envelope(frequency) + SAFE_MIN > 0.0 {
                -1.0
            } else {
                1.0
            };
            factor * (-e * (-delta).exp()) / g
        } else {
            (-frequency * duration).exp() * -frequency * duration * duration
        }
    };

    let mut result = 5.0 / duration;
    for _ in 0..11 {
        let slope = derivative(result);
        if !slope.is_finite() || slope.abs() <= f32::EPSILON {
            return f32::NAN;
        }
        result -= envelope(result) / slope;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::Spring;

    #[test]
    fn duration_resolution_produces_finite_physics() {
        let spring = Spring::from_duration_bounce(0.8, 0.3);

        assert!(spring.stiffness.is_finite() && spring.stiffness > 0.0);
        assert!(spring.damping.is_finite() && spring.damping > 0.0);
        assert!(spring.duration_based);
    }

    #[test]
    fn visual_duration_uses_requested_frequency() {
        let spring = Spring::from_visual_duration(0.5, 0.2);
        let expected_root = std::f32::consts::TAU / 0.6;

        assert!((spring.stiffness.sqrt() - expected_root).abs() < 0.0001);
    }
}
