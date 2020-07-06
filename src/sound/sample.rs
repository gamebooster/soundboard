// Initial version from Rodio and Cpal APACHE LICENSE 2.0

use std::mem;

/// Format that each sample has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// The value 0 corresponds to 0.
    I16,
    /// The boundaries are (-1.0, 1.0).
    F32,
}

/// Trait for containers that contain PCM data.
pub unsafe trait Sample: Copy + Clone {
    /// The `SampleFormat` corresponding to this data type.
    const FORMAT: SampleFormat;

    /// Turns the sample into its equivalent as a floating-point.
    fn to_f32(&self) -> f32;
    /// Converts this sample into a standard i16 sample.
    fn to_i16(&self) -> i16;

    /// Converts any sample type to this one by calling `to_i16` or `to_f32`.
    fn from<S>(sample: &S) -> Self
    where
        S: Sample;
}

unsafe impl Sample for i16 {
    const FORMAT: SampleFormat = SampleFormat::I16;

    #[inline]
    fn to_f32(&self) -> f32 {
        if *self < 0 {
            *self as f32 / -(::std::i16::MIN as f32)
        } else {
            *self as f32 / ::std::i16::MAX as f32
        }
    }

    #[inline]
    fn to_i16(&self) -> i16 {
        *self
    }

    #[inline]
    fn from<S>(sample: &S) -> Self
    where
        S: Sample,
    {
        sample.to_i16()
    }
}

unsafe impl Sample for f32 {
    const FORMAT: SampleFormat = SampleFormat::F32;

    #[inline]
    fn to_f32(&self) -> f32 {
        *self
    }

    #[inline]
    fn to_i16(&self) -> i16 {
        if *self >= 0.0 {
            (*self * ::std::i16::MAX as f32) as i16
        } else {
            (-*self * ::std::i16::MIN as f32) as i16
        }
    }

    #[inline]
    fn from<S>(sample: &S) -> Self
    where
        S: Sample,
    {
        sample.to_f32()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::Sample;

    #[test]
    fn i16_to_i16() {
        assert_eq!(0i16.to_i16(), 0);
        assert_eq!((-467i16).to_i16(), -467);
        assert_eq!(32767i16.to_i16(), 32767);
        assert_eq!((-32768i16).to_i16(), -32768);
    }

    #[test]
    fn i16_to_f32() {
        assert_eq!(0i16.to_f32(), 0.0);
        assert_eq!((-16384i16).to_f32(), -0.5);
        assert_eq!(32767i16.to_f32(), 1.0);
        assert_eq!((-32768i16).to_f32(), -1.0);
    }

    #[test]
    fn f32_to_i16() {
        assert_eq!(0.0f32.to_i16(), 0);
        assert_eq!((-0.5f32).to_i16(), ::std::i16::MIN / 2);
        assert_eq!(1.0f32.to_i16(), ::std::i16::MAX);
        assert_eq!((-1.0f32).to_i16(), ::std::i16::MIN);
    }

    #[test]
    fn f32_to_f32() {
        assert_eq!(0.1f32.to_f32(), 0.1);
        assert_eq!((-0.7f32).to_f32(), -0.7);
        assert_eq!(1.0f32.to_f32(), 1.0);
    }
}
