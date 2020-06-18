// Initial version from Rodio and Cpal APACHE LICENSE 2.0

use std::mem;

pub trait Sample: CpalSample {
    /// Linear interpolation between two samples.
    ///
    /// The result should be equal to
    /// `first * numerator / denominator + second * (1 - numerator / denominator)`.
    fn lerp(first: Self, second: Self, numerator: u32, denominator: u32) -> Self;
    /// Multiplies the value of this sample by the given amount.
    fn amplify(self, value: f32) -> Self;

    /// Calls `saturating_add` on the sample.
    fn saturating_add(self, other: Self) -> Self;

    /// Returns the value corresponding to the absence of sound.
    fn zero_value() -> Self;
}

impl Sample for u16 {
    #[inline]
    fn lerp(first: u16, second: u16, numerator: u32, denominator: u32) -> u16 {
        (first as u32 + (second as u32 - first as u32) * numerator / denominator) as u16
    }

    #[inline]
    fn amplify(self, value: f32) -> u16 {
        self.to_i16().amplify(value).to_u16()
    }

    #[inline]
    fn saturating_add(self, other: u16) -> u16 {
        self.saturating_add(other)
    }

    #[inline]
    fn zero_value() -> u16 {
        32768
    }
}

impl Sample for i16 {
    #[inline]
    fn lerp(first: i16, second: i16, numerator: u32, denominator: u32) -> i16 {
        (first as i32 + (second as i32 - first as i32) * numerator as i32 / denominator as i32)
            as i16
    }

    #[inline]
    fn amplify(self, value: f32) -> i16 {
        ((self as f32) * value) as i16
    }

    #[inline]
    fn saturating_add(self, other: i16) -> i16 {
        self.saturating_add(other)
    }

    #[inline]
    fn zero_value() -> i16 {
        0
    }
}

impl Sample for f32 {
    #[inline]
    fn lerp(first: f32, second: f32, numerator: u32, denominator: u32) -> f32 {
        first + (second - first) * numerator as f32 / denominator as f32
    }

    #[inline]
    fn amplify(self, value: f32) -> f32 {
        self * value
    }

    #[inline]
    fn saturating_add(self, other: f32) -> f32 {
        self + other
    }

    #[inline]
    fn zero_value() -> f32 {
        0.0
    }
}

/// Format that each sample has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// The value 0 corresponds to 0.
    I16,
    /// The value 0 corresponds to 32768.
    U16,
    /// The boundaries are (-1.0, 1.0).
    F32,
}

/// Trait for containers that contain PCM data.
pub unsafe trait CpalSample: Copy + Clone {
    /// The `SampleFormat` corresponding to this data type.
    const FORMAT: SampleFormat;

    /// Turns the sample into its equivalent as a floating-point.
    fn to_f32(&self) -> f32;
    /// Converts this sample into a standard i16 sample.
    fn to_i16(&self) -> i16;
    /// Converts this sample into a standard u16 sample.
    fn to_u16(&self) -> u16;

    /// Converts any sample type to this one by calling `to_i16`, `to_u16` or `to_f32`.
    fn from<S>(sample: &S) -> Self
    where
        S: CpalSample;
}

unsafe impl CpalSample for u16 {
    const FORMAT: SampleFormat = SampleFormat::U16;

    #[inline]
    fn to_f32(&self) -> f32 {
        self.to_i16().to_f32()
    }

    #[inline]
    fn to_i16(&self) -> i16 {
        if *self >= 32768 {
            (*self - 32768) as i16
        } else {
            (*self as i16) - 32767 - 1
        }
    }

    #[inline]
    fn to_u16(&self) -> u16 {
        *self
    }

    #[inline]
    fn from<S>(sample: &S) -> Self
    where
        S: CpalSample,
    {
        sample.to_u16()
    }
}

unsafe impl CpalSample for i16 {
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
    fn to_u16(&self) -> u16 {
        if *self < 0 {
            (*self - ::std::i16::MIN) as u16
        } else {
            (*self as u16) + 32768
        }
    }

    #[inline]
    fn from<S>(sample: &S) -> Self
    where
        S: CpalSample,
    {
        sample.to_i16()
    }
}

unsafe impl CpalSample for f32 {
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
    fn to_u16(&self) -> u16 {
        (((*self + 1.0) * 0.5) * ::std::u16::MAX as f32).round() as u16
    }

    #[inline]
    fn from<S>(sample: &S) -> Self
    where
        S: CpalSample,
    {
        sample.to_f32()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::CpalSample;

    #[test]
    fn i16_to_i16() {
        assert_eq!(0i16.to_i16(), 0);
        assert_eq!((-467i16).to_i16(), -467);
        assert_eq!(32767i16.to_i16(), 32767);
        assert_eq!((-32768i16).to_i16(), -32768);
    }

    #[test]
    fn i16_to_u16() {
        assert_eq!(0i16.to_u16(), 32768);
        assert_eq!((-16384i16).to_u16(), 16384);
        assert_eq!(32767i16.to_u16(), 65535);
        assert_eq!((-32768i16).to_u16(), 0);
    }

    #[test]
    fn i16_to_f32() {
        assert_eq!(0i16.to_f32(), 0.0);
        assert_eq!((-16384i16).to_f32(), -0.5);
        assert_eq!(32767i16.to_f32(), 1.0);
        assert_eq!((-32768i16).to_f32(), -1.0);
    }

    #[test]
    fn u16_to_i16() {
        assert_eq!(32768u16.to_i16(), 0);
        assert_eq!(16384u16.to_i16(), -16384);
        assert_eq!(65535u16.to_i16(), 32767);
        assert_eq!(0u16.to_i16(), -32768);
    }

    #[test]
    fn u16_to_u16() {
        assert_eq!(0u16.to_u16(), 0);
        assert_eq!(467u16.to_u16(), 467);
        assert_eq!(32767u16.to_u16(), 32767);
        assert_eq!(65535u16.to_u16(), 65535);
    }

    #[test]
    fn u16_to_f32() {
        assert_eq!(0u16.to_f32(), -1.0);
        assert_eq!(32768u16.to_f32(), 0.0);
        assert_eq!(65535u16.to_f32(), 1.0);
    }

    #[test]
    fn f32_to_i16() {
        assert_eq!(0.0f32.to_i16(), 0);
        assert_eq!((-0.5f32).to_i16(), ::std::i16::MIN / 2);
        assert_eq!(1.0f32.to_i16(), ::std::i16::MAX);
        assert_eq!((-1.0f32).to_i16(), ::std::i16::MIN);
    }

    #[test]
    fn f32_to_u16() {
        assert_eq!((-1.0f32).to_u16(), 0);
        assert_eq!(0.0f32.to_u16(), 32768);
        assert_eq!(1.0f32.to_u16(), 65535);
    }

    #[test]
    fn f32_to_f32() {
        assert_eq!(0.1f32.to_f32(), 0.1);
        assert_eq!((-0.7f32).to_f32(), -0.7);
        assert_eq!(1.0f32.to_f32(), 1.0);
    }
}
