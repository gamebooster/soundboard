// Initial version from Rodio APACHE LICENSE 2.0
use crate::sound::source::Source;
use libxm_soundboard::XMContext;
use log::{error, info, trace, warn};
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::time::Duration;

pub struct XMDecoder<R>
where
    R: Read + Seek,
{
    context: XMContext,
    current_frame_data: Box<[f32; 4096]>,
    current_frame_offset: usize,
    phantom: PhantomData<R>,
}

/// Returns true if the stream contains xm data, then resets it to where it was.
fn is_xm<R>(mut data: R) -> bool
where
    R: Read + Seek,
{
    let stream_pos = data.seek(SeekFrom::Current(0)).unwrap();

    let mut data_buffer = [0; 15];
    if data.read_exact(&mut data_buffer).is_err()
        || std::str::from_utf8(&data_buffer).unwrap_or_default() != "Extended Module"
    {
        data.seek(SeekFrom::Start(stream_pos)).unwrap();
        return false;
    }

    data.seek(SeekFrom::Start(stream_pos)).unwrap();
    true
}

impl<R> XMDecoder<R>
where
    R: Read + Seek,
{
    pub fn new(mut data: R) -> Result<Self, R> {
        if !is_xm(data.by_ref()) {
            return Err(data);
        }

        let mut data_buffer = Vec::new();
        data.read_to_end(&mut data_buffer).unwrap();
        let mut xm = XMContext::new(&data_buffer, 48000).unwrap();
        xm.set_max_loop_count(1);
        let mut buffer = [0.0; 4096];
        xm.generate_samples(&mut buffer);

        Ok(XMDecoder {
            context: xm,
            phantom: PhantomData,
            current_frame_data: Box::new(buffer),
            current_frame_offset: 0,
        })
    }
}

impl<R> Source for XMDecoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        2
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        48000
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        let speed = self.context.playing_speed().tempo as f64;
        let bpm = self.context.playing_speed().bpm as f64;
        let patterns = self.context.number_of_patterns();
        let kbps = (bpm * 2.0) / 5.0;
        let st: f64 = ((1.0 / kbps) * 1000.0) * speed;
        let mut t: f64 = 0.0;
        for pattern in 0..patterns - 1 {
            t += self.context.number_of_rows(pattern) as f64;
        }

        info!("duration: {:?}", Duration::from_millis((t * st) as u64));
        Some(Duration::from_millis((t * st) as u64))
    }
}

fn f32_to_i16(f: f32) -> i16 {
    // prefer to clip the input rather than be excessively loud.
    (f.max(-1.0).min(1.0) * i16::max_value() as f32) as i16
}

impl<R> Iterator for XMDecoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset == self.current_frame_data.len() {
            self.current_frame_offset = 0;
            if self.context.loop_count() == 0 {
                self.context.generate_samples(&mut *self.current_frame_data);
            } else {
                return None;
            }
        }

        let v = self.current_frame_data[self.current_frame_offset];
        self.current_frame_offset += 1;
        Some(f32_to_i16(v))
    }
}
