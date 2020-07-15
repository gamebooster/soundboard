// Initial version from Rodio APACHE LICENSE 2.0
use log::{error, info, trace, warn};
use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;
use std::vec;

use crate::sound::source::Source;

use lewton::inside_ogg::OggStreamReader;

/// Decoder for an OGG file that contains Vorbis sound format.
pub struct VorbisDecoder<R>
where
    R: Read + Seek,
{
    stream_reader: OggStreamReader<R>,
    current_data: vec::IntoIter<i16>,
}

impl<R> VorbisDecoder<R>
where
    R: Read + Seek,
{
    /// Attempts to decode the data as ogg/vorbis.
    pub fn new(mut data: R) -> Result<VorbisDecoder<R>, R> {
        if !is_vorbis(data.by_ref()) {
            return Err(data);
        }

        let mut stream_reader = OggStreamReader::new(data).unwrap();

        let mut data = match stream_reader.read_dec_packet_itl().ok().and_then(|v| v) {
            Some(d) => d,
            None => Vec::new(),
        };

        // The first packet is always empty, therefore
        // we need to read the second frame to get some data
        if let Some(mut d) = stream_reader.read_dec_packet_itl().ok().and_then(|v| v) {
            data.append(&mut d)
        }

        Ok(VorbisDecoder {
            stream_reader,
            current_data: data.into_iter(),
        })
    }
    pub fn total_duration_mut<T>(&self, reader: &mut T) -> Option<Duration>
    where
        T: std::io::Read + std::io::Seek,
    {
        use ogg_metadata::AudioMetadata;
        match ogg_metadata::read_format(reader) {
            Ok(vec) => {
                if let ogg_metadata::OggFormat::Vorbis(vorbis_metadata) = &vec[0] {
                    return Some(vorbis_metadata.get_duration().unwrap());
                }
            }
            Err(err) => {
                trace!("Could not read ogg info {}", err);
            }
        }
        None
    }
}

impl<R> Source for VorbisDecoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_data.len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.stream_reader.ident_hdr.audio_channels as u16
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.stream_reader.ident_hdr.audio_sample_rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl<R> Iterator for VorbisDecoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if let Some(sample) = self.current_data.next() {
            if self.current_data.len() == 0 {
                if let Some(data) = self
                    .stream_reader
                    .read_dec_packet_itl()
                    .ok()
                    .and_then(|v| v)
                {
                    self.current_data = data.into_iter();
                }
            }
            Some(sample)
        } else {
            if let Some(data) = self
                .stream_reader
                .read_dec_packet_itl()
                .ok()
                .and_then(|v| v)
            {
                self.current_data = data.into_iter();
            }
            self.current_data.next()
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.current_data.size_hint().0, None)
    }
}

/// Returns true if the stream contains Vorbis data, then resets it to where it was.
fn is_vorbis<R>(mut data: R) -> bool
where
    R: Read + Seek,
{
    let stream_pos = data.seek(SeekFrom::Current(0)).unwrap();

    if OggStreamReader::new(data.by_ref()).is_err() {
        data.seek(SeekFrom::Start(stream_pos)).unwrap();
        return false;
    }

    data.seek(SeekFrom::Start(stream_pos)).unwrap();
    true
}
