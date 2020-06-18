// Initial version from Rodio APACHE LICENSE 2.0

//! Sources of sound and various filters.

use std::time::Duration;

use super::sample::Sample;

///
pub trait Source: Iterator
where
    Self::Item: Sample,
{
    /// Returns the number of samples before the current frame ends. `None` means "infinite" or
    /// "until the sound ends".
    /// Should never return 0 unless there's no more data.
    ///
    /// After the engine has finished reading the specified number of samples, it will check
    /// whether the value of `channels()` and/or `sample_rate()` have changed.
    fn current_frame_len(&self) -> Option<usize>;

    /// Returns the number of channels. Channels are always interleaved.
    fn channels(&self) -> u16;

    /// Returns the rate at which the source should be played. In number of samples per second.
    fn sample_rate(&self) -> u32;

    /// Returns the total duration of this source, if known.
    ///
    /// `None` indicates at the same time "infinite" or "unknown".
    fn total_duration(&self) -> Option<Duration>;
}

impl<S> Source for Box<dyn Source<Item = S>>
where
    S: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        (**self).current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        (**self).channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        (**self).total_duration()
    }
}

impl<S> Source for Box<dyn Source<Item = S> + Send>
where
    S: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        (**self).current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        (**self).channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        (**self).total_duration()
    }
}

impl<S> Source for Box<dyn Source<Item = S> + Send + Sync>
where
    S: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        (**self).current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        (**self).channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        (**self).total_duration()
    }
}
