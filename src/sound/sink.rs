// Initial version from Rodio APACHE LICENSE 2.0

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use miniaudio::{Device, DeviceConfig, DeviceId, DeviceType, Frames, FramesMut};
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use super::sample::Sample;
use super::source::Source;
use std::collections::HashMap;
use std::collections::VecDeque;

/// Handle to an device that outputs sounds.
///
/// Dropping the `Sink` stops all sounds.

struct ConverterWrapper(pub miniaudio::DataConverter);

unsafe impl Sync for ConverterWrapper {}
unsafe impl Send for ConverterWrapper {}

type SourcesType<T, S> = std::sync::Arc<
    parking_lot::Mutex<
        HashMap<T, Vec<(S, VecDeque<i16>, Option<ConverterWrapper>, f32, f32, f32)>>,
    >,
>;

pub struct Sink<T, S>
where
    S: Source + Send + Sync + 'static,
    S::Item: Sample,
    S::Item: Send,
    T: std::cmp::Eq,
    T: std::hash::Hash,
{
    device: miniaudio::Device,
    stopped: Arc<AtomicBool>,
    sources: SourcesType<T, S>,
}

impl<T, S> Sink<T, S>
where
    S: Source + Send + Sync + 'static,
    S::Item: Sample,
    S::Item: Send,
    T: std::cmp::Eq,
    T: std::hash::Hash,
    T: Clone + Send + 'static,
    T: std::fmt::Debug,
{
    /// Builds a new `Sink`
    #[inline]
    pub fn new(
        context: &miniaudio::Context,
        device_id: Option<miniaudio::DeviceId>,
    ) -> Result<Self> {
        let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Playback);
        device_config.playback_mut().set_device_id(device_id);
        device_config
            .playback_mut()
            .set_format(miniaudio::Format::S16);

        let hash_map = SourcesType::<T, S>::default();
        let hash_map_clone = hash_map.clone();
        let stopped = Arc::new(AtomicBool::new(false));

        device_config.set_data_callback(move |device, output, _input| {
            let mut remove_keys = Vec::new();
            let mut unlocked = hash_map_clone.lock();

            for (key, sources) in unlocked.iter_mut() {
                for (source, buffer, resampler, start, end, current_duration) in sources {
                    if *start > 0.0 {
                        source.nth(
                            ((*start * source.sample_rate() as f32) * source.channels() as f32)
                                as usize,
                        );
                        *current_duration = *start;
                        *start = 0.0;
                    }
                    if current_duration >= end {
                        remove_keys.push(key.clone());
                        continue;
                    }

                    *current_duration += ((output.sample_count() / output.channels() as usize)
                        as f32)
                        / device.sample_rate() as f32;

                    if source.sample_rate() != device.sample_rate()
                        || source.channels() != output.channels() as u16
                    {
                        if resampler.is_none() {
                            let config = miniaudio::DataConverterConfig::new(
                                miniaudio::Format::S16,
                                miniaudio::Format::S16,
                                source.channels() as u32,
                                output.channels(),
                                source.sample_rate(),
                                device.sample_rate(),
                            );
                            *resampler = Some(ConverterWrapper(
                                miniaudio::DataConverter::new(&config).unwrap(),
                            ));
                        }
                        let mut old_samples: Vec<i16> = Vec::with_capacity(output.sample_count());
                        let mut filled_count = 0;
                        for _ in 0..output.sample_count() {
                            if let Some(item) = buffer.pop_front() {
                                old_samples.push(item);
                                continue;
                            }
                            let next = source.next();
                            if let Some(next) = next {
                                old_samples.push(next.to_i16());
                            } else {
                                filled_count = output.sample_count() - old_samples.len();
                                old_samples.resize(output.sample_count() as usize, 0);
                                break;
                            }
                        }
                        let mut new_samples_mut: Vec<i16> = vec![0; output.sample_count()];
                        let (_output_frame_count, input_frame_count) = resampler
                            .as_mut()
                            .unwrap()
                            .0
                            .process_pcm_frames(
                                &mut FramesMut::wrap(
                                    &mut new_samples_mut,
                                    output.format(),
                                    output.channels() as u32,
                                ),
                                &Frames::wrap(
                                    &old_samples,
                                    miniaudio::Format::S16,
                                    source.channels() as u32,
                                ),
                            )
                            .expect("resampling failed");
                        let mut iterator = new_samples_mut.iter();
                        for item in output.as_samples_mut::<i16>() {
                            if let Some(value) = iterator.next() {
                                *item = item.saturating_add(*value);
                            } else {
                                break;
                            }
                        }
                        for item in old_samples
                            .iter()
                            .skip((input_frame_count * source.channels() as u64) as usize)
                            .skip(filled_count)
                        {
                            buffer.push_back(*item);
                        }
                        if filled_count > 0 && buffer.is_empty() {
                            remove_keys.push(key.clone());
                        }
                    } else {
                        for item in output.as_samples_mut::<i16>() {
                            if let Some(value) = source.next() {
                                *item = item.saturating_add(value.to_i16());
                            } else {
                                remove_keys.push(key.clone());
                                break;
                            }
                        }
                    }
                }
            }
            for key in &remove_keys {
                let entry: &mut Vec<_> = unlocked.get_mut(key).unwrap();
                entry.remove(0);
                if entry.is_empty() {
                    unlocked.remove(key);
                }
            }
        });
        let stopped_clone = Arc::clone(&stopped);
        device_config.set_stop_callback(move |_device| {
            stopped_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let device = miniaudio::Device::new(Some(context.clone()), &device_config)
            .expect("failed to create miniaudio device");
        Ok(Sink {
            device,
            stopped,
            sources: hash_map,
        })
    }

    pub fn play(&mut self, key: T, source: S, start: Option<f32>, end: Option<f32>) -> Result<()> {
        let mut unlocked = self.sources.lock();
        let start_float = {
            let start = start.unwrap_or_default();
            if start < 0.0 {
                return Err(anyhow!("supplied start timestamp is negative {}", start));
            }
            start
        };
        let end_float = {
            if let Some(end_duration) = end {
                if end_duration < 0.0 {
                    return Err(anyhow!(
                        "supplied end timestamp is negative {}",
                        end_duration
                    ));
                }
                end_duration
            } else {
                f32::INFINITY
            }
        };
        match unlocked.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.push((source, VecDeque::new(), None, start_float, end_float, 0.0));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(vec![(
                    source,
                    VecDeque::new(),
                    None,
                    start_float,
                    end_float,
                    0.0,
                )]);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &T) {
        let mut unlocked = self.sources.lock();
        unlocked.remove(key);
    }

    pub fn is_playing(&mut self, key: &T) -> bool {
        let unlocked = self.sources.lock();
        unlocked.contains_key(&key)
    }

    /// Gets the volume of the sound.
    ///
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than 1.0 will
    /// multiply each sample by this value.
    #[inline]
    #[allow(dead_code)]
    pub fn volume(&self) -> Result<f32> {
        self.device
            .get_master_volume()
            .map_err(|err| anyhow!("Could not get volume {}", err))
    }

    /// Changes the volume of the sound.
    ///
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0` will
    /// multiply each sample by this value.
    #[inline]
    pub fn set_volume(&self, value: f32) -> Result<()> {
        self.device
            .set_master_volume(value)
            .map_err(|err| anyhow!("Could not set volume {}", err))
    }

    /// Starts the sink
    #[inline]
    pub fn start(&self) -> Result<()> {
        self.device
            .start()
            .map_err(|err| anyhow!("Could not start device {}", err))?;
        self.stopped
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Stops the sink
    #[inline]
    #[allow(dead_code)]
    pub fn stop(&self) -> Result<()> {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if self.device.is_started() {
            self.device
                .stop()
                .map_err(|err| anyhow!("Could not stop device {}", err))?;
        }
        Ok(())
    }

    /// Is the sink stopped
    #[inline]
    pub fn stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::Relaxed)
    }
}
