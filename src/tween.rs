/// Function used to map normalized tween progress to eased progress.
pub type Easing = fn(f32) -> f32;

/// Duration and easing parameters for a tween.
#[derive(Clone, Copy)]
pub struct Tween {
    /// Duration in seconds. Must be greater than zero.
    pub duration: f32,
    /// Easing function applied to normalized progress.
    pub easing: Easing,
}

impl Tween {
    /// Creates a tween using [`easing::ease_in_out`].
    pub fn new(duration: f32) -> Self {
        assert!(duration > 0.0, "tween duration must be greater than zero");
        Self {
            duration,
            easing: easing::ease_in_out,
        }
    }

    /// Sets the easing function.
    pub fn easing(self, easing: Easing) -> Self {
        Self { easing, ..self }
    }
}

/// Common easing functions.
pub mod easing {
    /// Returns unmodified linear progress.
    pub fn linear(t: f32) -> f32 {
        t
    }

    /// Applies smoothstep easing.
    pub fn ease_in_out(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Applies cubic ease-out easing.
    pub fn ease_out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }
}
