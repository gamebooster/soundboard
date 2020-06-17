use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use cpal::{Device, Devices, Host};

use log::{error, info, trace, warn};
use ringbuf::RingBuffer;
use rodio::source::UniformSourceIterator;
use rodio::{Sink, Source};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::JoinHandle;

use super::config;
use super::download;
use super::utils;

use miniaudio::{Context, DeviceId, DeviceType, ShareMode};

pub fn print_device_info(context: &Context, device_type: DeviceType, device_id: &DeviceId) {
    // This can fail, so we have to check the result.
    let info = match context.get_device_info(device_type, device_id, ShareMode::Shared) {
        Ok(info) => info,
        Err(err) => {
            eprintln!("\t\tfailed to get device info: {}", err);
            return;
        }
    };

    println!(
        "\t\tSample Rate: {}-{}Hz",
        info.min_sample_rate(),
        info.max_sample_rate()
    );

    println!(
        "\t\tChannels: {}-{}",
        info.min_channels(),
        info.max_channels()
    );

    println!("\t\tFormats: {:?}", info.formats());
}

pub fn print_possible_devices() {
    let context = Context::new(&[], None).expect("failed to create context");

    context
        .with_devices(|playback_devices, capture_devices| {
            println!("Playback Devices:");
            for (idx, device) in playback_devices.iter().enumerate() {
                println!("\t{}: {}", idx, device.name());
                print_device_info(&context, DeviceType::Playback, device.id());
            }

            println!("Capture Devices:");
            for (idx, device) in capture_devices.iter().enumerate() {
                println!("\t{}: {}", idx, device.name());
                print_device_info(&context, DeviceType::Capture, device.id());
            }
        })
        .expect("failed to get devices");
}

pub trait FindDevice {
    fn into_device(self) -> Result<Device>;
}

impl FindDevice for String {
    fn into_device(self) -> Result<Device> {
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;

        devices
            .find(|device: &Device| device.name().unwrap() == self)
            .ok_or_else(|| anyhow!("No device from name {}", self))
    }
}

// fn get_default_input_device() -> Result<Device> {
//     let host: Host = cpal::default_host();
//     host.default_input_device()
//         .ok_or_else(|| anyhow!("no default input device"))
// }

fn get_default_output_device() -> Result<Device> {
    let host: Host = cpal::default_host();
    host.default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))
}

pub fn init_sound(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    input_device_identifier: Option<String>,
    output_device_identifier: Option<String>,
    loop_device_identifier: String,
) -> Result<()> {
    let mut output_device = get_default_output_device()?;
    if output_device_identifier.is_some() {
        output_device = output_device_identifier.unwrap().into_device()?;
    }

    let loop_device = loop_device_identifier.clone().into_device()?;

    info!("Output: \"{}\"", output_device.name().unwrap());
    info!("Loopback: \"{}\"", loop_device.name().unwrap());

    let shared_loop_device = Arc::new(loop_device);
    let shared_output_device = Arc::new(output_device);

    std::thread::spawn(move || {
        play_thread(receiver, sender, shared_loop_device, shared_output_device);
    });

    std::thread::spawn(move || -> Result<()> {
        sound_thread(input_device_identifier, loop_device_identifier)
    });

    Ok(())
}

type StartedTime = std::time::Instant;
type SoundMap = HashMap<config::SoundConfig, (Vec<Sink>, StartedTime, Option<TotalDuration>)>;

#[derive(PartialEq)]
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
    PlayStatus(Vec<(config::SoundConfig, PlayDuration, Option<TotalDuration>)>),
}

fn insert_sink_with_config(
    device: &Device,
    volume: f32,
    sound_config: config::SoundConfig,
    sinks: &mut SoundMap,
) -> Result<()> {
    info!(
        "Playing sound config: {:?} on device: {}",
        sound_config,
        device.name().unwrap()
    );

    let local_path = download::get_local_path_from_sound_config(&sound_config)?;

    let file = std::fs::File::open(&local_path)?;

    let sink = Sink::new(device);
    sink.set_volume(volume);
    let decoder = rodio::Decoder::new(BufReader::new(file))?;
    let total_duration = decoder
        .total_duration()
        .or_else(|| -> Option<std::time::Duration> {
            let duration = mp3_duration::from_path(&local_path);
            if let Ok(dur) = duration {
                return Some(dur);
            } else {
                trace!("Could not read mp3 tag {:?}", duration.err());
            }
            None
        })
        .or_else(|| -> Option<std::time::Duration> {
            use ogg_metadata::AudioMetadata;
            let file = std::fs::File::open(&local_path);
            if file.is_err() {
                return None;
            }
            match ogg_metadata::read_format(&file.unwrap()) {
                Ok(vec) => match &vec[0] {
                    ogg_metadata::OggFormat::Vorbis(vorbis_metadata) => {
                        return Some(vorbis_metadata.get_duration().unwrap());
                    }
                    ogg_metadata::OggFormat::Opus(opus_metadata) => {
                        return Some(opus_metadata.get_duration().unwrap());
                    }
                    _ => {}
                },
                Err(err) => {
                    trace!("Could not read ogg info {}", err);
                }
            }
            None
        });
    sink.append(decoder);

    match sinks.entry(sound_config) {
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

fn play_thread(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    loop_device: Arc<Device>,
    output_device: Arc<Device>,
) {
    let mut volume: f32 = 1.0;
    let mut sinks: SoundMap = HashMap::new();

    loop {
        let receive = receiver.recv();

        match receive {
            Ok(message) => match message {
                Message::PlaySound(sound_config, sound_devices) => {
                    if sound_devices == SoundDevices::Both || sound_devices == SoundDevices::Output
                    {
                        match insert_sink_with_config(
                            &*output_device,
                            volume,
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
                            &*loop_device,
                            volume,
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
                    if let Some((vec, _, _)) = sinks.remove(&sound_handle) {
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
                            sink.set_volume(volume);
                        }
                    }
                }
                Message::PlayStatus(_) => {
                    let mut sounds = Vec::new();
                    for (id, (_, instant, total_duration)) in sinks.iter() {
                        sounds.push((id.clone(), instant.elapsed(), *total_duration));
                    }
                    sender
                        .send(Message::PlayStatus(sounds))
                        .expect("sound channel error");
                }
            },
            Err(err) => {
                error!("message receive error {}", err);
            }
        };

        sinks.retain(|_, (local_sinks, _, _)| local_sinks.iter().any(|s| !s.empty()));
    }
}

fn sound_thread(input_device: Option<String>, loop_device: String) -> Result<()> {
    let context = Context::new(&[], None).expect("failed to create context");

    let mut ms_input_device = None;
    let mut ms_loop_device = None;

    context
        .with_devices(|playback_devices, capture_devices| {
            for (_, device) in playback_devices.iter().enumerate() {
                println!("\t{}: {}", loop_device, device.name());
                if device.name() == loop_device {
                    ms_loop_device = Some(device.id().clone());
                }
            }

            if input_device.is_none() {
                return;
            };
            for (_, device) in capture_devices.iter().enumerate() {
                println!("\t{}: {}", input_device.as_ref().unwrap(), device.name());
                if device.name() == input_device.as_ref().unwrap() {
                    ms_input_device = Some(device.id().clone());
                }
            }
        })
        .expect("failed to get devices");

    if ms_loop_device.is_none() {
        error!("could not find loop device in miniaudio");
        return Ok(());
    }

    let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Duplex);
    device_config
        .capture_mut()
        .set_format(miniaudio::Format::F32);
    device_config.capture_mut().set_channels(2);
    if ms_input_device.is_some() {
        device_config.capture_mut().set_device_id(ms_input_device);
    }
    device_config.set_sample_rate(48000);

    device_config.playback_mut().set_channels(2);
    device_config
        .playback_mut()
        .set_format(miniaudio::Format::F32);
    device_config.playback_mut().set_device_id(ms_loop_device);

    device_config.set_data_callback(move |_device, output, input| {
        output.as_bytes_mut().copy_from_slice(input.as_bytes());
    });

    device_config.set_stop_callback(|_device| {
        println!("Device Stopped.");
    });

    let device =
        miniaudio::Device::new(None, &device_config).expect("failed to open playback device");
    device.start().expect("failed to start device");

    println!("Device Backend: {:?}", device.context().backend());

    std::thread::park();
    Ok(())
}
