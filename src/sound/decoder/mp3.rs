// Initial version from Rodio APACHE LICENSE 2.0
use crate::sound::source::Source;
use log::{error, info, trace, warn};
use minimp3::{Decoder, Frame};
use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::time::Duration;

pub struct Mp3Decoder<R>
where
    R: Read + Seek,
{
    decoder: Decoder<R>,
    current_frame: Frame,
    current_frame_offset: usize,
}

fn is_mp3<R>(mut data: R) -> bool
where
    R: Read + Seek,
{
    let stream_pos = data.seek(SeekFrom::Current(0)).unwrap();

    let mut decoder = Decoder::new(data);
    if decoder.next_frame().is_err() {
        decoder
            .reader_mut()
            .seek(SeekFrom::Start(stream_pos))
            .unwrap();
        return false;
    }

    decoder
        .reader_mut()
        .seek(SeekFrom::Start(stream_pos))
        .unwrap();
    true
}

impl<R> Mp3Decoder<R>
where
    R: Read + Seek,
{
    pub fn new(mut data: R) -> Result<Self, R> {
        if !is_mp3(data.by_ref()) {
            return Err(data);
        }
        let mut decoder = Decoder::new(data);
        let current_frame = decoder.next_frame().unwrap();

        Ok(Mp3Decoder {
            decoder,
            current_frame,
            current_frame_offset: 0,
        })
    }

    pub fn total_duration_mut<T>(&self, reader: &mut T) -> Option<Duration>
    where
        T: std::io::Read,
    {
        let duration = mp3_duration::from_read(reader);
        match duration {
            Ok(duration) => Some(duration),
            Err(err) => {
                if err.at_duration > std::time::Duration::from_nanos(0) {
                    trace!("mp3-duration: used error duration");
                    Some(err.at_duration)
                } else {
                    fn get_duration<R: std::io::Read>(reader: R) -> Option<std::time::Duration> {
                        let mut decoder = Decoder::new(reader);
                        let mut length_in_seconds = 0.0;
                        loop {
                            match decoder.next_frame() {
                                Ok(Frame {
                                    data,
                                    sample_rate,
                                    channels,
                                    ..
                                }) => {
                                    length_in_seconds +=
                                        (data.len() / channels) as f32 / sample_rate as f32;
                                }
                                Err(minimp3::Error::Eof) => {
                                    return Some(std::time::Duration::from_secs_f32(
                                        length_in_seconds,
                                    ))
                                }
                                Err(_) => return None,
                            }
                        }
                    };
                    trace!("mp3-duration: used decoder duration");
                    get_duration(reader)
                }
            }
        }
    }
}

impl<R> Source for Mp3Decoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_frame.data.len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.current_frame.channels as _
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.current_frame.sample_rate as _
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl<R> Iterator for Mp3Decoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset == self.current_frame.data.len() {
            match self.decoder.next_frame() {
                Ok(frame) => self.current_frame = frame,
                _ => return None,
            }
            self.current_frame_offset = 0;
        }

        let v = self.current_frame.data[self.current_frame_offset];
        self.current_frame_offset += 1;

        Some(v)
    }
}
