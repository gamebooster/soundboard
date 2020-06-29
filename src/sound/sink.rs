// Initial version from Rodio APACHE LICENSE 2.0

use anyhow::{anyhow, Result};
use miniaudio::{Device, DeviceConfig, DeviceId, DeviceType};
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use super::sample::CpalSample;
use super::sample::Sample;
use super::source::Source;

/// Handle to an device that outputs sounds.
///
/// Dropping the `Sink` stops all sounds.

pub struct Sink {
    device: miniaudio::Device,
    stopped: Arc<AtomicBool>,
}

impl Sink {
    /// Builds a new `Sink`
    #[inline]
    pub fn new<S>(source: S, device_id: Option<miniaudio::DeviceId>) -> Result<Sink>
    where
        S: Source + Send + Sync + 'static,
        S::Item: Sample,
        S::Item: Send,
    {
        let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Playback);
        device_config.playback_mut().set_device_id(device_id);
        device_config.set_sample_rate(source.sample_rate());
        device_config
            .playback_mut()
            .set_channels(source.channels() as u32);
        device_config
            .playback_mut()
            .set_format(miniaudio::Format::S16);

        let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped_clone = Arc::clone(&stopped);
        let source_arc = Arc::new(std::sync::Mutex::new(source));
        device_config.set_data_callback(move |_device, output, _input| {
            let stopped = stopped_clone.load(std::sync::atomic::Ordering::Relaxed);
            if stopped {
                return;
            }
            let mut unlocked_source = source_arc.lock().unwrap();
            for sample in output.as_samples_mut() {
                let next = unlocked_source.next();
                if let Some(next) = next {
                    *sample = next.to_i16();
                } else {
                    stopped_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
        });
        let stopped_clone = Arc::clone(&stopped);
        device_config.set_stop_callback(move |_device| {
            stopped_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let device = miniaudio::Device::new(
            Some(crate::sound::GLOBAL_AUDIO_CONTEXT.0.clone()),
            &device_config,
        )
        .expect("could not create device");
        Ok(Sink { device, stopped })
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
        let stopped = self.stopped.load(std::sync::atomic::Ordering::Relaxed);
        if stopped {
            self.stop().expect("could not stop device");
        }
        stopped
    }
}
