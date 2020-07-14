// Initial version from Rodio APACHE LICENSE 2.0
//! Decodes samples from an audio file.

use std::error::Error;
use std::fmt;
use std::io::{Read, Seek};
use std::time::Duration;

use super::source::Source;

#[cfg(feature = "flac")]
mod flac;
#[cfg(feature = "mp3")]
mod mp3;
#[cfg(feature = "opus")]
mod opus;
#[cfg(feature = "vorbis")]
mod vorbis;
#[cfg(feature = "wav")]
mod wav;
#[cfg(feature = "xm")]
mod xm;

/// Source of audio samples from decoding a file.
///
/// Supports MP3, WAV, Vorbis and Flac.
#[cfg(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm",
    feature = "opus"
))]
pub struct Decoder<R>(DecoderImpl<R>)
where
    R: Read + Seek;

#[cfg(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm",
    feature = "opus"
))]
enum DecoderImpl<R>
where
    R: Read + Seek,
{
    #[cfg(feature = "wav")]
    Wav(wav::WavDecoder<R>),
    #[cfg(feature = "vorbis")]
    Vorbis(vorbis::VorbisDecoder<R>),
    #[cfg(feature = "opus")]
    Opus(opus::OpusDecoder<R>),
    #[cfg(feature = "flac")]
    Flac(flac::FlacDecoder<R>),
    #[cfg(feature = "mp3")]
    Mp3(mp3::Mp3Decoder<R>),
    #[cfg(feature = "xm")]
    XM(xm::XMDecoder<R>),
}

impl<R> Decoder<R>
where
    R: Read + Seek + Send + 'static,
{
    /// Builds a new decoder.
    ///
    /// Attempts to automatically detect the format of the source of data.
    #[allow(unused_variables)]
    pub fn new(data: R) -> Result<Decoder<R>, DecoderError> {
        #[cfg(feature = "mp3")]
        let data = match mp3::Mp3Decoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::Mp3(decoder)));
            }
        };

        #[cfg(feature = "wav")]
        let data = match wav::WavDecoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::Wav(decoder)));
            }
        };

        #[cfg(feature = "flac")]
        let data = match flac::FlacDecoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::Flac(decoder)));
            }
        };

        #[cfg(feature = "vorbis")]
        let data = match vorbis::VorbisDecoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::Vorbis(decoder)));
            }
        };

        #[cfg(feature = "opus")]
        let data = match opus::OpusDecoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::Opus(decoder)));
            }
        };

        #[cfg(feature = "xm")]
        let data = match xm::XMDecoder::new(data) {
            Err(data) => data,
            Ok(decoder) => {
                return Ok(Decoder(DecoderImpl::XM(decoder)));
            }
        };

        Err(DecoderError::UnrecognizedFormat)
    }

    pub fn total_duration_mut<T>(&mut self, reader: &mut T) -> Option<Duration>
    where
        T: std::io::Read + std::io::Seek,
    {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref mut source) => source.total_duration(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref mut source) => source.total_duration_mut(reader),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref mut source) => source.total_duration_mut(reader),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref mut source) => source.total_duration(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref mut source) => source.total_duration_mut(reader),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref mut source) => source.total_duration(),
        }
    }
}

#[cfg(not(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm",
    feature = "opus"
)))]
impl<R> Iterator for Decoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        None
    }
}

#[cfg(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm",
    feature = "opus"
))]
impl<R> Iterator for Decoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref mut source) => source.next(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref mut source) => source.next(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref mut source) => source.next(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref mut source) => source.next(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref mut source) => source.next(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref mut source) => source.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref source) => source.size_hint(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref source) => source.size_hint(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref source) => source.size_hint(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref source) => source.size_hint(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref source) => source.size_hint(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref source) => source.size_hint(),
        }
    }
}

#[cfg(not(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm",
    feauture = "opus"
)))]
impl<R> Source for Decoder<R>
where
    R: Read + Seek,
{
    fn current_frame_len(&self) -> Option<usize> {
        Some(0)
    }
    fn channels(&self) -> u16 {
        0
    }
    fn sample_rate(&self) -> u32 {
        1
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::default())
    }
}

#[cfg(any(
    feature = "wav",
    feature = "flac",
    feature = "vorbis",
    feature = "mp3",
    feature = "xm"
))]
impl<R> Source for Decoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref source) => source.current_frame_len(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref source) => source.current_frame_len(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref source) => source.current_frame_len(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref source) => source.current_frame_len(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref source) => source.current_frame_len(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref source) => source.current_frame_len(),
        }
    }

    #[inline]
    fn channels(&self) -> u16 {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref source) => source.channels(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref source) => source.channels(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref source) => source.channels(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref source) => source.channels(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref source) => source.channels(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref source) => source.channels(),
        }
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref source) => source.sample_rate(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref source) => source.sample_rate(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref source) => source.sample_rate(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref source) => source.sample_rate(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref source) => source.sample_rate(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref source) => source.sample_rate(),
        }
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        match self.0 {
            #[cfg(feature = "wav")]
            DecoderImpl::Wav(ref source) => source.total_duration(),
            #[cfg(feature = "vorbis")]
            DecoderImpl::Vorbis(ref source) => source.total_duration(),
            #[cfg(feature = "opus")]
            DecoderImpl::Opus(ref source) => source.total_duration(),
            #[cfg(feature = "flac")]
            DecoderImpl::Flac(ref source) => source.total_duration(),
            #[cfg(feature = "mp3")]
            DecoderImpl::Mp3(ref source) => source.total_duration(),
            #[cfg(feature = "xm")]
            DecoderImpl::XM(ref source) => source.total_duration(),
        }
    }
}

/// Error that can happen when creating a decoder.
#[derive(Debug, Clone)]
pub enum DecoderError {
    /// The format of the data has not been recognized.
    UnrecognizedFormat,
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecoderError::UnrecognizedFormat => write!(f, "Unrecognized format"),
        }
    }
}

impl Error for DecoderError {
    fn description(&self) -> &str {
        match self {
            DecoderError::UnrecognizedFormat => "Unrecognized format",
        }
    }
}
