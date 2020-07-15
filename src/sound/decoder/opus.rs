// Initial version from Rodio APACHE LICENSE 2.0
use log::{error, info, trace, warn};
use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;
use std::vec;

use crate::sound::source::Source;

use audiopus::coder::Decoder;
use ogg::reading::PacketReader;

use parking_lot::Mutex;
use std::sync::Arc;

/// Decoder for an OGG file that contains Opus sound format.
pub struct OpusDecoder<R>
where
    R: Read + Seek,
{
    packet_reader: PacketReader<R>,
    decoder: Arc<Mutex<Decoder>>,
    current_data: vec::IntoIter<i16>,
}

impl<R> OpusDecoder<R>
where
    R: Read + Seek,
{
    /// Attempts to decode the data as ogg/opus.
    pub fn new(mut data: R) -> Result<OpusDecoder<R>, R> {
        if !is_opus(data.by_ref()) {
            return Err(data);
        }

        let mut packet_reader = PacketReader::new(data);

        let mut decoded_data: Vec<i16> = vec![0; 4096];
        let mut decoder =
            Decoder::new(audiopus::SampleRate::Hz48000, audiopus::Channels::Mono).unwrap();
        loop {
            let input_data = match packet_reader.read_packet() {
                Ok(Some(d)) => d.data,
                Ok(None) => {
                    error!("unexpected end of file");
                    decoded_data.truncate(0);
                    break;
                }
                Err(_) => {
                    error!("unexpected read packet error");
                    decoded_data.truncate(0);
                    break;
                }
            };
            if let Ok(length) = decoder.decode(Some(&input_data), &mut decoded_data, false) {
                decoded_data.truncate(length);
                break;
            }
        }

        Ok(OpusDecoder {
            packet_reader,
            decoder: Arc::new(Mutex::new(decoder)),
            current_data: decoded_data.into_iter(),
        })
    }
    pub fn total_duration_mut<T>(&self, reader: &mut T) -> Option<Duration>
    where
        T: std::io::Read + std::io::Seek,
    {
        use ogg_metadata::AudioMetadata;
        match ogg_metadata::read_format(reader) {
            Ok(vec) => {
                if let ogg_metadata::OggFormat::Opus(opus_metadata) = &vec[0] {
                    return Some(opus_metadata.get_duration().unwrap());
                }
            }
            Err(err) => {
                trace!("Could not read ogg info {}", err);
            }
        }
        None
    }
}

impl<R> Source for OpusDecoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_data.len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        1
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        48000
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl<R> Iterator for OpusDecoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if let Some(sample) = self.current_data.next() {
            Some(sample)
        } else {
            let input_data = match self.packet_reader.read_packet_expected() {
                Ok(d) => d.data,
                Err(_) => return None,
            };

            let mut decoded_data: Vec<i16> = vec![0; 4096];
            match self
                .decoder
                .lock()
                .decode(Some(&input_data), &mut decoded_data, false)
            {
                Ok(length) => decoded_data.truncate(length),
                Err(err) => {
                    error!("opus decoder error {}", err);
                    return None;
                }
            }
            self.current_data = decoded_data.into_iter();
            self.current_data.next()
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.current_data.size_hint().0, None)
    }
}

/// Returns true if the stream contains Vorbis data, then resets it to where it was.
fn is_opus<R>(mut data: R) -> bool
where
    R: Read + Seek,
{
    let stream_pos = data.seek(SeekFrom::Current(0)).unwrap();

    use ogg_metadata::AudioMetadata;

    if let Ok(vec) = ogg_metadata::read_format(data.by_ref()) {
        if let ogg_metadata::OggFormat::Opus(_) = &vec[0] {
            data.seek(SeekFrom::Start(stream_pos)).unwrap();
            return true;
        }
    }

    data.seek(SeekFrom::Start(stream_pos)).unwrap();
    false
}
