/// Parameters for velocity-projected inertia with optional boundary springs.
#[derive(Clone, Copy)]
pub struct Inertia {
    /// Multiplier used to project the supplied velocity.
    pub power: f32,
    /// Exponential decay time constant in seconds.
    pub time_constant: f32,
    /// Optional hook used to snap the projected target.
    pub modify_target: Option<fn(f32) -> f32>,
    /// Optional lower boundary.
    pub min: Option<f32>,
    /// Optional upper boundary.
    pub max: Option<f32>,
    /// Stiffness of the boundary spring.
    pub bounce_stiffness: f32,
    /// Damping of the boundary spring.
    pub bounce_damping: f32,
    /// Distance at which inertia is considered complete.
    pub rest_delta: f32,
}

impl Inertia {
    /// Creates inertia using Motion-compatible defaults.
    pub fn new() -> Self {
        Self {
            power: 0.8,
            time_constant: 0.325,
            modify_target: None,
            min: None,
            max: None,
            bounce_stiffness: 500.0,
            bounce_damping: 10.0,
            rest_delta: 0.5,
        }
    }

    /// Adds lower and upper boundary springs.
    pub fn bounds(self, min: f32, max: f32) -> Self {
        assert!(min <= max, "inertia minimum cannot exceed maximum");
        Self {
            min: Some(min),
            max: Some(max),
            ..self
        }
    }

    /// Sets a projected-target modifier such as grid snapping.
    pub fn modify_target(self, f: fn(f32) -> f32) -> Self {
        Self {
            modify_target: Some(f),
            ..self
        }
    }
}

impl Default for Inertia {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Inertia;

    #[test]
    fn defaults_match_motion_units() {
        let inertia = Inertia::default();

        assert_eq!(inertia.power, 0.8);
        assert_eq!(inertia.time_constant, 0.325);
        assert_eq!(inertia.bounce_stiffness, 500.0);
    }
}
