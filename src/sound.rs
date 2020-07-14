use anyhow::{anyhow, Result};
use log::{error, info, trace, warn};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

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
    sound_receiver: crossbeam_channel::Receiver<Message>,
    sound_sender: crossbeam_channel::Sender<Message>,
    gui_sender: crossbeam_channel::Sender<Message>,
    input_device_identifier: Option<String>,
    output_device_identifier: Option<String>,
    loop_device_identifier: String,
) -> ! {
    let mut context_config = miniaudio::ContextConfig::default();
    context_config
        .pulse_mut()
        .set_application_name("soundboard")
        .expect("failed to set pulse app name");
    let context = Context::new(&DEFAULT_BACKENDS, Some(&context_config))
        .expect("could not create audio context");
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

    if input_device_identifier.is_some() && ms_input_device.is_none() {
        panic!(
            "Could not find input device identifier \"{}\"",
            input_device_identifier.unwrap()
        );
    }

    if output_device_identifier.is_some() && ms_output_device.is_none() {
        panic!(
            "Could not find output device identifier \"{}\"",
            output_device_identifier.unwrap()
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

    let loop_back_device = {
        if !config::MainConfig::read()
            .no_duplex_device
            .unwrap_or_default()
        {
            let ms_loop_device_clone = ms_loop_device.clone();
            Some(
                create_duplex_device(&context, ms_input_device, ms_loop_device_clone.unwrap())
                    .expect("create duplex device failed"),
            )
        } else {
            None
        }
    };

    run_sound_message_loop(
        context,
        sound_receiver,
        sound_sender,
        gui_sender,
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
    pub start: Option<f32>,
    pub end: Option<f32>,
}

impl From<config::SoundConfig> for SoundKey {
    fn from(sound_config: config::SoundConfig) -> Self {
        SoundKey {
            path: sound_config.path,
            headers: sound_config.headers,
            name: sound_config.name,
            hotkey: sound_config.hotkey,
            start: sound_config.start,
            end: sound_config.end,
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
type SoundMap = HashMap<SoundKey, (SoundStatus, StartedTime, Option<TotalDuration>)>;

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

#[derive(Debug, PartialEq, Eq, serde::Deserialize, Copy, Clone, serde::Serialize)]
pub enum SoundStatus {
    Downloading,
    Playing,
}

#[derive(Debug, PartialEq)]
pub enum Message {
    PlaySound(config::SoundConfig, SoundDevices),
    StopSound(config::SoundConfig),
    StopAll,
    SetVolume(f32),
    PlayStatus(
        Vec<(
            SoundStatus,
            config::SoundConfig,
            PlayDuration,
            Option<TotalDuration>,
        )>,
        f32,
    ),
    _PlaySoundDownloaded(config::SoundConfig, SoundDevices, std::path::PathBuf),
}

fn insert_sink_with_config(
    path: &std::path::Path,
    device: Option<miniaudio::DeviceIdAndName>,
    sink: &mut SinkDecoder,
    sound_config: config::SoundConfig,
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

    if let Some(start) = sound_config.start {
        if start < 0.0 {
            return Err(anyhow!("error: start timestamp is negative {}", start));
        }
    }

    if let Some(end) = sound_config.end {
        if end < 0.0 {
            return Err(anyhow!("error: end timestamp is negative {}", end));
        }
    }

    let reader = std::io::BufReader::with_capacity(1000 * 50, std::fs::File::open(path)?);
    let mut decoder = Decoder::new(reader)?;
    let mut reader = std::io::BufReader::with_capacity(1000 * 50, std::fs::File::open(path)?);
    let total_duration = decoder.total_duration_mut(&mut reader);
    let total_duration = match (total_duration, sound_config.start, sound_config.end) {
        (Some(total_duration), Some(start), None) => {
            if let Some(duration) = total_duration.checked_sub(Duration::from_secs_f32(start)) {
                Some(duration)
            } else {
                return Err(anyhow!(
                    "error: total_duration - supplied start timestamp is negative"
                ));
            }
        }
        (Some(total_duration), None, None) => Some(total_duration),
        (None, None, Some(end)) => Some(Duration::from_secs_f32(end)),
        (Some(total_duration), None, Some(end)) => {
            let end = Duration::from_secs_f32(end);
            if total_duration < end {
                Some(total_duration)
            } else {
                Some(end)
            }
        }
        (total_duration, Some(start), Some(mut end)) => {
            if let Some(total_duration) = total_duration {
                if total_duration.as_secs_f32() < end {
                    end = total_duration.as_secs_f32();
                }
            }
            if end - start < 0.0 {
                return Err(anyhow!(
                    "error: end - start duration is negative start: {} end: {}",
                    start,
                    end
                ));
            }
            Some(Duration::from_secs_f32(end - start))
        }
        (None, _, None) => None,
    };
    sink.play(
        sound_config.clone().into(),
        decoder,
        sound_config.start,
        sound_config.end,
    )?;

    match sinks.entry(sound_config.into()) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let entry = entry.get_mut();
            entry.0 = SoundStatus::Playing;
            entry.1 = std::time::Instant::now();
            entry.2 = total_duration;
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert((
                SoundStatus::Playing,
                std::time::Instant::now(),
                total_duration,
            ));
        }
    }
    Ok(())
}

type SinkDecoder = Sink<SoundKey, Decoder<std::io::BufReader<std::fs::File>>>;

fn run_sound_message_loop(
    context: Context,
    sound_receiver: crossbeam_channel::Receiver<Message>,
    sound_sender: crossbeam_channel::Sender<Message>,
    gui_sender: crossbeam_channel::Sender<Message>,
    loop_device: miniaudio::DeviceIdAndName,
    output_device: Option<miniaudio::DeviceIdAndName>,
    loopback_device: Option<miniaudio::Device>,
) -> ! {
    let mut volume: f32 = 1.0;
    let mut sinks: SoundMap = HashMap::new();

    let output_device_id = {
        if let Some(device) = output_device.clone() {
            Some(device.id().clone())
        } else {
            None
        }
    };

    let mut output_sink =
        SinkDecoder::new(&context, output_device_id).expect("failed to create output sink");
    output_sink.start().expect("failed to start output_sink");

    let mut loopback_sink = SinkDecoder::new(&context, Some(loop_device.id().clone()))
        .expect("failed to create output sink");
    loopback_sink
        .start()
        .expect("failed to start loopback_sink");

    loop {
        match sound_receiver.recv() {
            Ok(message) => match message {
                Message::PlaySound(sound_config, sound_devices) => {
                    let maybe_path = {
                        let result = download::local_path_for_sound_config_exists(&sound_config);
                        if let Err(err) = result {
                            error!("local_path_for_sound_config_exists error {}", err);
                            continue;
                        }
                        result.unwrap()
                    };

                    if config::MainConfig::read()
                        .disable_simultaneous_playback
                        .unwrap_or_default()
                    {
                        gui_sender
                            .send(Message::StopAll)
                            .expect("error stopping all playing sounds");
                    }

                    if let Some(path) = maybe_path {
                        gui_sender
                            .send(Message::_PlaySoundDownloaded(
                                sound_config,
                                sound_devices,
                                path,
                            ))
                            .expect("sound channel send error");
                    } else {
                        match sinks.entry(sound_config.clone().into()) {
                            std::collections::hash_map::Entry::Occupied(_) => {
                                panic!("sink should not be occupied");
                            }
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                entry.insert((
                                    SoundStatus::Downloading,
                                    std::time::Instant::now(),
                                    None,
                                ));
                            }
                        }
                        let gui_sender_clone = gui_sender.clone();
                        std::thread::spawn(
                            move || match download::get_local_path_from_sound_config(&sound_config)
                            {
                                Ok(path) => {
                                    gui_sender_clone
                                        .send(Message::_PlaySoundDownloaded(
                                            sound_config,
                                            sound_devices,
                                            path,
                                        ))
                                        .expect("sound channel send error");
                                }
                                Err(err) => {
                                    gui_sender_clone
                                        .send(Message::StopSound(sound_config))
                                        .expect("sound channel error");
                                    error!("get_local_path_from_sound_config failed: {:#}", err)
                                }
                            },
                        );
                    }
                }
                Message::_PlaySoundDownloaded(sound_config, sound_devices, path) => {
                    if sound_devices == SoundDevices::Both || sound_devices == SoundDevices::Output
                    {
                        match insert_sink_with_config(
                            &path,
                            output_device.clone(),
                            &mut output_sink,
                            sound_config.clone(),
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
                            &path,
                            Some(loop_device.clone()),
                            &mut loopback_sink,
                            sound_config,
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
                    if let Some((_, _, _)) = sinks.remove(&sound_handle.clone().into()) {
                        output_sink.remove(&sound_handle.clone().into());
                        loopback_sink.remove(&sound_handle.into());
                    };
                }
                Message::StopAll => {
                    for (key, _) in sinks.drain() {
                        output_sink.remove(&key);
                        loopback_sink.remove(&key);
                    }
                }
                Message::SetVolume(volume_new) => {
                    volume = volume_new;
                    output_sink
                        .set_volume(volume)
                        .expect("failed to set volume");
                    loopback_sink
                        .set_volume(volume)
                        .expect("failed to set volume");
                }
                Message::PlayStatus(_, _) => {
                    let mut sounds = Vec::new();
                    for (id, (status, instant, total_duration)) in sinks.iter() {
                        sounds.push((
                            *status,
                            config::SoundConfig {
                                name: id.name.clone(),
                                path: id.path.clone(),
                                headers: id.headers.clone(),
                                hotkey: id.hotkey.clone(),
                                full_path: String::new(),
                                start: id.start,
                                end: id.end,
                            },
                            instant.elapsed(),
                            *total_duration,
                        ));
                    }
                    sound_sender
                        .send(Message::PlayStatus(sounds, volume))
                        .expect("sound channel error");
                }
            },
            Err(err) => {
                error!("message receive error {}", err);
            }
        };
        sinks.retain(|key, (status, _, _)| {
            *status == SoundStatus::Downloading
                || output_sink.is_playing(&key)
                || loopback_sink.is_playing(&key)
        });
        if loopback_sink.stopped() {
            loopback_sink
                .start()
                .expect("failed to start loopback_sink again");
        }
        if output_sink.stopped() {
            output_sink
                .start()
                .expect("failed to start output_sink again");
        }
        if let Some(loopback_device) = loopback_device.as_ref() {
            if !loopback_device.is_started() {
                loopback_device
                    .start()
                    .expect("failed to start loopback device again");
            }
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
