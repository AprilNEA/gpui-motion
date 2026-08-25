use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use gpui::{Pixels, Point, point, px};

const VELOCITY_WINDOW: Duration = Duration::from_millis(30);
const SAMPLE_CAPACITY: usize = 8;

#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    position: Point<Pixels>,
}

pub struct DragTracker {
    samples: VecDeque<Sample>,
    position: Point<Pixels>,
    dragging: bool,
}

impl DragTracker {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(SAMPLE_CAPACITY),
            position: zero(),
            dragging: false,
        }
    }

    pub fn begin(&mut self, position: Point<Pixels>) {
        self.begin_at(position, Instant::now());
    }

    pub fn update(&mut self, position: Point<Pixels>) -> Point<Pixels> {
        self.update_at(position, Instant::now())
    }

    pub fn end(&mut self) -> Point<Pixels> {
        self.end_at(Instant::now())
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn begin_at(&mut self, position: Point<Pixels>, at: Instant) {
        self.samples.clear();
        self.position = position;
        self.dragging = true;
        self.push_sample(Sample { at, position });
    }

    fn update_at(&mut self, position: Point<Pixels>, at: Instant) -> Point<Pixels> {
        if !self.dragging {
            return zero();
        }
        let delta = point(position.x - self.position.x, position.y - self.position.y);
        self.position = position;
        self.push_sample(Sample { at, position });
        delta
    }

    fn end_at(&mut self, now: Instant) -> Point<Pixels> {
        if !self.dragging {
            return zero();
        }
        self.dragging = false;
        let Some(latest) = self.samples.back().copied() else {
            return zero();
        };
        if now.saturating_duration_since(latest.at) > VELOCITY_WINDOW {
            return zero();
        }
        let oldest = self
            .samples
            .iter()
            .find(|sample| now.saturating_duration_since(sample.at) <= VELOCITY_WINDOW)
            .copied()
            .unwrap_or(latest);
        let elapsed = latest.at.saturating_duration_since(oldest.at).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return zero();
        }
        point(
            px(f32::from(latest.position.x - oldest.position.x) / elapsed),
            px(f32::from(latest.position.y - oldest.position.y) / elapsed),
        )
    }

    fn push_sample(&mut self, sample: Sample) {
        if self.samples.len() == SAMPLE_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }
}

impl Default for DragTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn zero() -> Point<Pixels> {
    point(px(0.0), px(0.0))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui::{point, px};

    use super::DragTracker;

    #[test]
    fn release_velocity_uses_only_the_last_thirty_milliseconds() {
        let start = Instant::now();
        let mut tracker = DragTracker::new();
        tracker.begin_at(point(px(0.0), px(0.0)), start);
        tracker.update_at(point(px(20.0), px(10.0)), start + Duration::from_millis(10));
        tracker.update_at(point(px(80.0), px(40.0)), start + Duration::from_millis(40));

        let velocity = tracker.end_at(start + Duration::from_millis(40));

        assert!((f32::from(velocity.x) - 2_000.0).abs() < 0.1);
        assert!((f32::from(velocity.y) - 1_000.0).abs() < 0.1);
    }

    #[test]
    fn release_after_thirty_milliseconds_without_motion_is_zero() {
        let start = Instant::now();
        let mut tracker = DragTracker::new();
        tracker.begin_at(point(px(0.0), px(0.0)), start);
        tracker.update_at(point(px(10.0), px(4.0)), start + Duration::from_millis(5));

        let velocity = tracker.end_at(start + Duration::from_millis(36));

        assert_eq!(f32::from(velocity.x), 0.0);
        assert_eq!(f32::from(velocity.y), 0.0);
    }
}
