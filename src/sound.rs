use anyhow::{anyhow, Result};
use log::{error, info, trace, warn};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::JoinHandle;

use super::config;
use super::download;
use super::utils;

mod decoder;
mod sample;
mod sink;
mod source;

use decoder::Decoder;
use miniaudio::{Context, DeviceId, DeviceType, ShareMode};
use sink::Sink;
use source::Source;

static DEFAULT_BACKENDS: [miniaudio::Backend; 5] = [
    miniaudio::Backend::Wasapi,
    miniaudio::Backend::DSound,
    miniaudio::Backend::CoreAudio,
    miniaudio::Backend::PulseAudio,
    miniaudio::Backend::Alsa,
];

fn print_device_info(context: &Context, device_type: DeviceType, device_id: &DeviceId) {
    // This can fail, so we have to check the result.
    let info = match context.get_device_info(device_type, device_id, ShareMode::Shared) {
        Ok(info) => info,
        Err(err) => {
            error!("\t\tfailed to get device info: {}", err);
            return;
        }
    };

    info!(
        "\t\t\tSample Rate: {}-{}Hz",
        info.min_sample_rate(),
        info.max_sample_rate()
    );

    info!(
        "\t\t\tChannels: {}-{}",
        info.min_channels(),
        info.max_channels()
    );

    info!("\t\t\tFormats: {:?}", info.formats());
}

pub fn print_possible_devices_and_exit() {
    let context = Context::new(&DEFAULT_BACKENDS, None).expect("could not create audio context");
    print_possible_devices(&context, true);
}

fn print_possible_devices(context: &Context, full: bool) {
    info!("Audio Backend: {:?}", context.backend());

    context
        .with_devices(|playback_devices, capture_devices| {
            info!("\tOutput Devices:");
            for (idx, device) in playback_devices.iter().enumerate() {
                info!("\t\t{}: {}", idx, device.name());
                if full {
                    print_device_info(&context, DeviceType::Playback, device.id());
                }
            }

            info!("\tInput Devices:");
            for (idx, device) in capture_devices.iter().enumerate() {
                info!("\t\t{}: {}", idx, device.name());
                if full {
                    print_device_info(&context, DeviceType::Capture, device.id());
                }
            }
        })
        .expect("failed to get devices");
}

pub fn run_sound_loop(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    input_device_identifier: Option<String>,
    output_device_identifier: Option<String>,
    loop_device_identifier: String,
) -> ! {
    let context = Context::new(&DEFAULT_BACKENDS, None).expect("could not create audio context");
    let mut ms_input_device = None;
    let mut ms_output_device = None;
    let mut ms_loop_device = None;

    info!("Possible Devices: ");
    print_possible_devices(&context, false);

    context
        .with_devices(|playback_devices, capture_devices| {
            for (_, device) in playback_devices.iter().enumerate() {
                if device.name() == loop_device_identifier {
                    ms_loop_device = Some(device.clone());
                }
                if output_device_identifier.is_some()
                    && device.name() == output_device_identifier.as_ref().unwrap()
                {
                    ms_output_device = Some(device.clone());
                }
            }

            if input_device_identifier.is_none() {
                return;
            };
            for (_, device) in capture_devices.iter().enumerate() {
                if device.name() == input_device_identifier.as_ref().unwrap() {
                    ms_input_device = Some(device.clone());
                }
            }
        })
        .expect("failed to create context");

    if ms_loop_device.is_none() {
        panic!(
            "Could not find loop device identifier \"{}\"",
            loop_device_identifier
        );
    }

    if let Some(input_device) = ms_input_device.as_ref() {
        info!("Input device: \"{}\"", input_device.name());
    } else {
        info!("Input device: default input device");
    }
    if let Some(output_device) = ms_output_device.as_ref() {
        info!("Output device: \"{}\"", output_device.name());
    } else {
        info!("Output device: default output device");
    }
    info!(
        "Loop device: \"{}\"",
        ms_loop_device.as_ref().unwrap().name()
    );

    let ms_loop_device_clone = ms_loop_device.clone();
    let loop_back_device =
        create_duplex_device(&context, ms_input_device, ms_loop_device_clone.unwrap())
            .expect("create duplex device failed");

    run_sound_message_loop(
        context,
        receiver,
        sender,
        ms_loop_device.unwrap(),
        ms_output_device,
        loop_back_device,
    );
}

#[derive(Debug, Clone, Default)]
struct SoundKey {
    pub name: String,
    pub path: String,
    pub hotkey: Option<String>,
    pub headers: Option<Vec<config::HeaderConfig>>,
}

impl From<config::SoundConfig> for SoundKey {
    fn from(sound_config: config::SoundConfig) -> Self {
        SoundKey {
            path: sound_config.path,
            headers: sound_config.headers,
            name: sound_config.name,
            hotkey: sound_config.hotkey,
        }
    }
}

impl PartialEq for SoundKey {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.headers == other.headers
    }
}
impl Eq for SoundKey {}

impl std::hash::Hash for SoundKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.headers.hash(state);
    }
}

type StartedTime = std::time::Instant;
type SoundMap = HashMap<SoundKey, (Vec<Sink>, StartedTime, Option<TotalDuration>)>;

#[derive(
    Debug,
    serde::Deserialize,
    Copy,
    Clone,
    serde::Serialize,
    strum_macros::EnumString,
    PartialEq,
    Hash,
    Eq,
)]
pub enum SoundDevices {
    Loop,
    Output,
    Both,
}

type PlayDuration = std::time::Duration;
type TotalDuration = std::time::Duration;

#[derive(PartialEq)]
pub enum Message {
    PlaySound(config::SoundConfig, SoundDevices),
    StopSound(config::SoundConfig),
    StopAll,
    SetVolume(f32),
    PlayStatus(
        Vec<(config::SoundConfig, PlayDuration, Option<TotalDuration>)>,
        f32,
    ),
}

fn insert_sink_with_config(
    context: &Context,
    device: Option<miniaudio::DeviceIdAndName>,
    sound_config: config::SoundConfig,
    volume: f32,
    sinks: &mut SoundMap,
) -> Result<()> {
    let device_name = {
        if let Some(device) = device.as_ref() {
            device.name().to_string()
        } else {
            "default output".to_string()
        }
    };
    info!(
        "Playing sound config: {:?} on device: {}",
        sound_config, device_name
    );

    let local_path = download::get_local_path_from_sound_config(&sound_config)?;

    let file = std::fs::File::open(&local_path)?;
    let mut decoder = Decoder::new(file)?;
    let mut file = std::fs::File::open(&local_path)?;
    let total_duration = decoder.total_duration_mut(&mut file);
    let device_id = {
        if let Some(device) = device {
            Some(device.id().clone())
        } else {
            None
        }
    };
    let sink = Sink::new(context, decoder, device_id)?;
    sink.set_volume(volume)?;
    sink.start()?;

    match sinks.entry(sound_config.into()) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let entry = entry.get_mut();
            entry.0.push(sink);
            entry.1 = std::time::Instant::now();
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert((vec![sink], std::time::Instant::now(), total_duration));
        }
    }
    Ok(())
}

fn run_sound_message_loop(
    context: Context,
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    loop_device: miniaudio::DeviceIdAndName,
    output_device: Option<miniaudio::DeviceIdAndName>,
    loopback_device: miniaudio::Device,
) -> ! {
    let mut volume: f32 = 1.0;
    let mut sinks: SoundMap = HashMap::new();

    loop {
        match receiver.recv() {
            Ok(message) => match message {
                Message::PlaySound(sound_config, sound_devices) => {
                    if sound_devices == SoundDevices::Both || sound_devices == SoundDevices::Output
                    {
                        match insert_sink_with_config(
                            &context,
                            output_device.clone(),
                            sound_config.clone(),
                            volume,
                            &mut sinks,
                        ) {
                            Ok(path) => path,
                            Err(err) => {
                                error!("failed to insert sound at output sink {}", err);
                                continue;
                            }
                        };
                    }
                    if sound_devices == SoundDevices::Both || sound_devices == SoundDevices::Loop {
                        match insert_sink_with_config(
                            &context,
                            Some(loop_device.clone()),
                            sound_config,
                            volume,
                            &mut sinks,
                        ) {
                            Ok(path) => path,
                            Err(err) => {
                                error!("failed to insert sound at loop sink {}", err);
                                continue;
                            }
                        };
                    }
                }
                Message::StopSound(sound_handle) => {
                    if let Some((vec, _, _)) = sinks.remove(&sound_handle.into()) {
                        for sink in vec {
                            drop(sink);
                        }
                    };
                }
                Message::StopAll => {
                    for (_, tuple) in sinks.drain() {
                        for sink in tuple.0 {
                            drop(sink);
                        }
                    }
                }
                Message::SetVolume(volume_new) => {
                    volume = volume_new;
                    for (_, tuple) in sinks.iter_mut() {
                        for sink in &mut tuple.0 {
                            if let Err(err) = sink.set_volume(volume) {
                                error!("could not set master volume {}", err);
                            }
                        }
                    }
                }
                Message::PlayStatus(_, _) => {
                    let mut sounds = Vec::new();
                    for (id, (_, instant, total_duration)) in sinks.iter() {
                        sounds.push((
                            config::SoundConfig {
                                name: id.name.clone(),
                                path: id.path.clone(),
                                headers: id.headers.clone(),
                                hotkey: id.hotkey.clone(),
                                full_path: String::new(),
                            },
                            instant.elapsed(),
                            *total_duration,
                        ));
                    }
                    sender
                        .send(Message::PlayStatus(sounds, volume))
                        .expect("sound channel error");
                }
            },
            Err(err) => {
                error!("message receive error {}", err);
            }
        };

        sinks.retain(|_, (local_sinks, _, _)| local_sinks.iter().any(|s| !s.stopped()));
        if !loopback_device.is_started() {
            loopback_device
                .start()
                .expect("failed to start loopback device again");
        }
    }
}

fn create_duplex_device(
    context: &Context,
    input_device: Option<miniaudio::DeviceIdAndName>,
    loop_device: miniaudio::DeviceIdAndName,
) -> Result<miniaudio::Device> {
    let loop_info = match context.get_device_info(
        miniaudio::DeviceType::Playback,
        loop_device.id(),
        ShareMode::Shared,
    ) {
        Ok(loop_info) => loop_info,
        Err(err) => {
            error!("failed to get device info: {}", err);
            return Err(anyhow!("failed to get device info: {}", err));
        }
    };

    let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Duplex);
    let format = loop_info.formats()[0];
    info!("duplex: format {:?}", format);
    device_config.capture_mut().set_format(format);
    let channels = loop_info.max_channels();
    info!("duplex: channels {}", channels);
    device_config.capture_mut().set_channels(channels);
    if let Some(input_device) = input_device {
        device_config
            .capture_mut()
            .set_device_id(Some(input_device.id().clone()));
    }

    let sample_rate = {
        let default_sample_rate = 48000;

        if loop_info.min_sample_rate() <= default_sample_rate
            && loop_info.max_sample_rate() >= default_sample_rate
        {
            default_sample_rate
        } else {
            loop_info.min_sample_rate()
        }
    };
    info!("duplex: sample_rate {}", sample_rate);
    device_config.set_sample_rate(sample_rate);
    device_config
        .playback_mut()
        .set_device_id(Some(loop_device.id().clone()));

    device_config.set_data_callback(move |_device, output, input| {
        output.as_bytes_mut().copy_from_slice(input.as_bytes());
    });

    device_config.set_stop_callback(|_device| {
        error!("Loopback device stopped!!!");
    });

    let device = miniaudio::Device::new(Some(context.clone()), &device_config)
        .expect("failed to open playback device");
    device.start().expect("failed to start device");

    Ok(device)
}
