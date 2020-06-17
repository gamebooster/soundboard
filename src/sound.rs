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

static DEFAULT_BACKENDS: [miniaudio::Backend; 5] = [
    miniaudio::Backend::Wasapi,
    miniaudio::Backend::DSound,
    miniaudio::Backend::CoreAudio,
    miniaudio::Backend::PulseAudio,
    miniaudio::Backend::Alsa,
];

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
    let context = Context::new(&DEFAULT_BACKENDS, None).expect("failed to create context");

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

pub fn init_sound(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    input_device_identifier: Option<String>,
    output_device_identifier: Option<String>,
    loop_device_identifier: String,
) -> Result<()> {
    let context = Context::new(&DEFAULT_BACKENDS, None).expect("failed to create context");

    let mut ms_input_device = None;
    let mut ms_output_device = None;
    let mut ms_loop_device = None;

    context
        .with_devices(|playback_devices, capture_devices| {
            for (_, device) in playback_devices.iter().enumerate() {
                println!("\t {}: {}", loop_device_identifier, device.name());
                if device.name() == loop_device_identifier {
                    ms_loop_device = Some(device.id().clone());
                }
                if output_device_identifier.is_some()
                    && device.name() == output_device_identifier.as_ref().unwrap()
                {
                    ms_output_device = Some(device.id().clone());
                }
            }

            if input_device_identifier.is_none() {
                return;
            };
            for (_, device) in capture_devices.iter().enumerate() {
                println!(
                    "\t{}: {}",
                    input_device_identifier.as_ref().unwrap(),
                    device.name()
                );
                if device.name() == input_device_identifier.as_ref().unwrap() {
                    ms_input_device = Some(device.id().clone());
                }
            }
        })
        .expect("failed to create context");

    if ms_loop_device.is_none() {
        error!("could not find loop device in miniaudio");
        return Ok(());
    }

    let ms_loop_device_clone = ms_loop_device.clone();
    std::thread::spawn(move || {
        play_thread(
            receiver,
            sender,
            ms_loop_device_clone.unwrap(),
            ms_output_device,
        );
    });

    std::thread::spawn(move || -> Result<()> {
        sound_thread(ms_input_device, ms_loop_device.unwrap())
    });

    Ok(())
}

type StartedTime = std::time::Instant;

struct DeviceWrapper {
    pub ms_device: miniaudio::Device,
    pub finished: Arc<std::sync::atomic::AtomicBool>,
}

type SoundMap =
    HashMap<config::SoundConfig, (Vec<DeviceWrapper>, StartedTime, Option<TotalDuration>)>;

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
    device_id: Option<miniaudio::DeviceId>,
    sound_config: config::SoundConfig,
    volume: f32,
    sinks: &mut SoundMap,
) -> Result<()> {
    info!("Playing sound config: {:?} on device", sound_config,);

    let local_path = download::get_local_path_from_sound_config(&sound_config)?;

    let file = std::fs::File::open(&local_path)?;

    let mut decoder = minimp3::Decoder::new(file);
    let mut mp3_sample_rate = None;
    let mut mp3_channels = None;

    match decoder.next_frame() {
        Ok(minimp3::Frame {
            data: _,
            sample_rate,
            channels,
            ..
        }) => {
            mp3_sample_rate = Some(sample_rate);
            mp3_channels = Some(channels);
        }
        Err(minimp3::Error::Eof) => {}
        Err(e) => panic!("{:?}", e),
    }

    let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Playback);
    device_config.playback_mut().set_device_id(device_id);
    device_config.set_sample_rate(mp3_sample_rate.unwrap() as u32);
    device_config
        .playback_mut()
        .set_channels(mp3_channels.unwrap() as u32);
    device_config
        .playback_mut()
        .set_format(miniaudio::Format::S16);

    let file = std::fs::File::open(&local_path)?;
    let decoder = Arc::new(std::sync::Mutex::new(minimp3::Decoder::new(file)));
    let mut sound_buffer: Vec<i16> = Vec::new();

    let finished_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished_flag_clone = Arc::clone(&finished_flag);

    device_config.set_data_callback(move |_device, output, _input| {
        while sound_buffer.len() < output.sample_count() {
            match decoder.lock().unwrap().next_frame() {
                Ok(minimp3::Frame { data, .. }) => {
                    sound_buffer.extend(data);
                }
                Err(minimp3::Error::Eof) => {
                    finished_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                Err(e) => {
                    error!("{:?}", e);
                    finished_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);

                    break;
                }
            }
        }
        let max_length = {
            if output.sample_count() < sound_buffer.len() {
                output.sample_count()
            } else {
                sound_buffer.len()
            }
        };
        let output_buffer: Vec<_> = sound_buffer.drain(..max_length).collect();
        output.as_samples_mut()[..output_buffer.len()].copy_from_slice(&output_buffer[..]);
    });

    let finished_flag_clone = Arc::clone(&finished_flag);
    device_config.set_stop_callback(move |_device| {
        println!("Device Stopped.");
        finished_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let device =
        miniaudio::Device::new(None, &device_config).expect("failed to open playback device");
    device
        .set_master_volume(volume)
        .expect("failed to set volume");
    device.start().expect("failed to start device");

    let total_duration = None
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

    let wrapper = DeviceWrapper {
        ms_device: device,
        finished: finished_flag,
    };

    match sinks.entry(sound_config) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let entry = entry.get_mut();
            entry.0.push(wrapper);
            entry.1 = std::time::Instant::now();
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert((vec![wrapper], std::time::Instant::now(), total_duration));
        }
    }
    Ok(())
}

fn play_thread(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    loop_device: miniaudio::DeviceId,
    output_device: Option<miniaudio::DeviceId>,
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
                            if let Err(err) = sink.ms_device.set_master_volume(volume) {
                                error!("could not set master volume {}", err);
                            }
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

        sinks.retain(|_, (local_sinks, _, _)| {
            local_sinks
                .iter()
                .any(|s| !s.finished.load(std::sync::atomic::Ordering::Relaxed))
        });
    }
}

fn sound_thread(
    input_device: Option<miniaudio::DeviceId>,
    loop_device: miniaudio::DeviceId,
) -> Result<()> {
    let context = Context::new(&DEFAULT_BACKENDS, None).expect("failed to create context");
    let loop_info = match context.get_device_info(
        miniaudio::DeviceType::Playback,
        &loop_device,
        ShareMode::Shared,
    ) {
        Ok(loop_info) => loop_info,
        Err(err) => {
            error!("failed to get device info: {}", err);
            return Err(anyhow!("failed to get device info: {}", err));
        }
    };

    let mut device_config = miniaudio::DeviceConfig::new(DeviceType::Duplex);
    device_config
        .capture_mut()
        .set_format(loop_info.formats()[0]);
    device_config
        .capture_mut()
        .set_channels(loop_info.max_channels());
    if input_device.is_some() {
        device_config.capture_mut().set_device_id(input_device);
    }
    device_config.set_sample_rate(loop_info.max_sample_rate());
    device_config
        .playback_mut()
        .set_device_id(Some(loop_device));

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
