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
    const PRECISION: f32 = 0.000_000_1;

    /// Returns unmodified linear progress.
    pub fn linear(t: f32) -> f32 {
        t
    }

    /// Applies Motion's standard ease-in curve.
    pub fn ease_in(t: f32) -> f32 {
        cubic_bezier(t, 0.42, 0.0, 1.0, 1.0)
    }

    /// Applies Motion's standard ease-out curve.
    pub fn ease_out(t: f32) -> f32 {
        cubic_bezier(t, 0.0, 0.0, 0.58, 1.0)
    }

    /// Applies smoothstep easing.
    pub fn ease_in_out(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Applies cubic ease-out easing.
    pub fn ease_out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }

    /// Applies a slight overshoot before returning to the target.
    pub fn back_out(t: f32) -> f32 {
        cubic_bezier(t, 0.33, 1.53, 0.69, 0.99)
    }

    /// Pulls backward before accelerating toward the target.
    pub fn anticipate(mut t: f32) -> f32 {
        if t >= 1.0 {
            return 1.0;
        }

        t *= 2.0;
        if t < 1.0 {
            0.5 * (1.0 - back_out(1.0 - t))
        } else {
            0.5 * (2.0 - 2.0_f32.powf(-10.0 * (t - 1.0)))
        }
    }

    /// Applies circular ease-in.
    pub fn circ_in(t: f32) -> f32 {
        1.0 - (1.0 - t * t).max(0.0).sqrt()
    }

    /// Applies circular ease-out.
    pub fn circ_out(t: f32) -> f32 {
        (1.0 - (t - 1.0).powi(2)).max(0.0).sqrt()
    }

    fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }

        let mut low = 0.0;
        let mut high = 1.0;
        let mut curve_t = t;
        for _ in 0..12 {
            let x = cubic(curve_t, x1, x2);
            let error = x - t;
            if error.abs() <= PRECISION {
                break;
            }
            if error > 0.0 {
                high = curve_t;
            } else {
                low = curve_t;
            }
            curve_t = (low + high) * 0.5;
        }
        cubic(curve_t, y1, y2)
    }

    fn cubic(t: f32, p1: f32, p2: f32) -> f32 {
        let inverse = 1.0 - t;
        3.0 * inverse * inverse * t * p1 + 3.0 * inverse * t * t * p2 + t * t * t
    }
}

#[cfg(test)]
mod tests {
    use super::easing;

    #[test]
    fn back_out_overshoots() {
        assert!(easing::back_out(0.8) > 1.0);
        assert_eq!(easing::back_out(1.0), 1.0);
    }

    #[test]
    fn anticipate_withdraws_first() {
        assert!(easing::anticipate(0.25) < 0.0);
        assert_eq!(easing::anticipate(1.0), 1.0);
    }
}
