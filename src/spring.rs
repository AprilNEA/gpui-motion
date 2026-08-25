/// Parameters for a mass-1 damped spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Spring stiffness.
    pub stiffness: f32,
    /// Damping coefficient.
    pub damping: f32,
    /// Maximum displacement at which the spring is considered at rest.
    pub rest_delta: f32,
    /// Maximum speed at which the spring is considered at rest.
    pub rest_speed: f32,
}

impl Spring {
    /// Creates a spring with the given stiffness and damping.
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self {
            stiffness,
            damping,
            rest_delta: 0.01,
            rest_speed: 0.01,
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

    /// Sets the displacement and speed thresholds used to detect rest.
    pub fn rest(self, rest_delta: f32, rest_speed: f32) -> Self {
        Self {
            rest_delta,
            rest_speed,
            ..self
        }
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::new(170.0, 26.0)
    }
}
