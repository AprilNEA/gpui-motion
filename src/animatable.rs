/// Maximum number of scalar channels in an animated value.
pub const MAX_CHANNELS: usize = 8;

/// A value that can be flattened into independently animated scalar channels.
pub trait Animatable: Clone + 'static {
    /// Number of scalar channels used by this value.
    const CHANNELS: usize;

    /// Writes this value's channels into `out`.
    fn write(&self, out: &mut [f32]);

    /// Reconstructs a value from its scalar channels.
    fn read(src: &[f32]) -> Self;
}

impl Animatable for f32 {
    const CHANNELS: usize = 1;

    fn write(&self, out: &mut [f32]) {
        out[0] = *self;
    }

    fn read(src: &[f32]) -> Self {
        src[0]
    }
}

macro_rules! impl_animatable_tuple {
    ($(($($name:ident : $index:tt),+)),+ $(,)?) => {
        $(
            impl<$($name: Animatable),+> Animatable for ($($name,)+) {
                const CHANNELS: usize = 0 $(+ $name::CHANNELS)+;

                fn write(&self, out: &mut [f32]) {
                    assert!(Self::CHANNELS <= MAX_CHANNELS);
                    assert!(out.len() >= Self::CHANNELS);

                    let mut offset = 0;
                    $(
                        let end = offset + $name::CHANNELS;
                        self.$index.write(&mut out[offset..end]);
                        offset = end;
                    )+
                    debug_assert_eq!(offset, Self::CHANNELS);
                }

                fn read(src: &[f32]) -> Self {
                    assert!(Self::CHANNELS <= MAX_CHANNELS);
                    assert!(src.len() >= Self::CHANNELS);

                    let mut offset = 0;
                    let value = (
                        $(
                            {
                                let end = offset + $name::CHANNELS;
                                let value = $name::read(&src[offset..end]);
                                offset = end;
                                value
                            },
                        )+
                    );
                    debug_assert_eq!(offset, Self::CHANNELS);
                    value
                }
            }
        )+
    };
}

impl_animatable_tuple!(
    (A: 0, B: 1),
    (A: 0, B: 1, C: 2),
    (A: 0, B: 1, C: 2, D: 3),
    (A: 0, B: 1, C: 2, D: 3, E: 4),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5),
);

#[cfg(test)]
mod tests {
    use super::Animatable;

    #[test]
    fn tuple_roundtrip() {
        let original = (1.0, (2.0, 3.0), (4.0, 5.0, 6.0));
        let mut channels = [0.0; 6];

        original.write(&mut channels);
        let roundtrip = <(f32, (f32, f32), (f32, f32, f32))>::read(&channels);

        assert_eq!(channels, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(roundtrip, original);
    }
}
