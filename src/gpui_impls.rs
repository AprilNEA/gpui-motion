use gpui::{Hsla, Pixels, Point, Rgba, Size};

use crate::Animatable;

impl Animatable for Pixels {
    const CHANNELS: usize = 1;

    fn write(&self, out: &mut [f32]) {
        out[0] = f32::from(*self);
    }

    fn read(src: &[f32]) -> Self {
        Self::from(src[0])
    }
}

impl Animatable for Rgba {
    const CHANNELS: usize = 4;

    fn write(&self, out: &mut [f32]) {
        out[..Self::CHANNELS].copy_from_slice(&[self.r, self.g, self.b, self.a]);
    }

    fn read(src: &[f32]) -> Self {
        Self {
            r: src[0],
            g: src[1],
            b: src[2],
            a: src[3],
        }
    }
}

impl Animatable for Hsla {
    const CHANNELS: usize = 4;

    fn write(&self, out: &mut [f32]) {
        Rgba::from(*self).write(out);
    }

    fn read(src: &[f32]) -> Self {
        Self::from(Rgba::read(src))
    }
}

impl Animatable for Point<Pixels> {
    const CHANNELS: usize = 2;

    fn write(&self, out: &mut [f32]) {
        out[0] = f32::from(self.x);
        out[1] = f32::from(self.y);
    }

    fn read(src: &[f32]) -> Self {
        Self {
            x: Pixels::from(src[0]),
            y: Pixels::from(src[1]),
        }
    }
}

impl Animatable for Size<Pixels> {
    const CHANNELS: usize = 2;

    fn write(&self, out: &mut [f32]) {
        out[0] = f32::from(self.width);
        out[1] = f32::from(self.height);
    }

    fn read(src: &[f32]) -> Self {
        Self {
            width: Pixels::from(src[0]),
            height: Pixels::from(src[1]),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{hsla, point, px, rgba, size};

    use super::*;

    fn round_trip<T: Animatable>(value: T) -> T {
        let mut channels = [0.0; crate::MAX_CHANNELS];
        value.write(&mut channels[..T::CHANNELS]);
        T::read(&channels[..T::CHANNELS])
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
    }

    #[test]
    fn pixels_round_trip() {
        assert_close(f32::from(round_trip(px(42.25))), 42.25);
    }

    #[test]
    fn rgba_round_trip() {
        let expected = rgba(0x7c5cffb3);
        let actual = round_trip(expected);
        assert_close(actual.r, expected.r);
        assert_close(actual.g, expected.g);
        assert_close(actual.b, expected.b);
        assert_close(actual.a, expected.a);
    }

    #[test]
    fn hsla_round_trip_through_rgba() {
        let expected = hsla(0.72, 0.65, 0.42, 0.75);
        let actual = round_trip(expected);
        assert_close(actual.h, expected.h);
        assert_close(actual.s, expected.s);
        assert_close(actual.l, expected.l);
        assert_close(actual.a, expected.a);
    }

    #[test]
    fn point_round_trip() {
        let expected = point(px(-13.5), px(81.25));
        let actual = round_trip(expected);
        assert_close(f32::from(actual.x), f32::from(expected.x));
        assert_close(f32::from(actual.y), f32::from(expected.y));
    }

    #[test]
    fn size_round_trip() {
        let expected = size(px(320.5), px(180.25));
        let actual = round_trip(expected);
        assert_close(f32::from(actual.width), f32::from(expected.width));
        assert_close(f32::from(actual.height), f32::from(expected.height));
    }
}
