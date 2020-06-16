use anyhow::{anyhow, Context, Result};
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

const LATENCY_MS: f32 = 150.0;

use super::config;
use super::download;
use super::utils;

pub fn print_possible_devices() {
    let host = cpal::default_host();

    let devices = host.devices().expect("No available sound devices");

    println!("  Devices: ");
    for (device_index, device) in devices.enumerate() {
        println!("  {}. \"{}\"", device_index, device.name().unwrap());

        // Input configs
        if let Ok(conf) = device.default_input_format() {
            println!("    Default input stream format:\n      {:?}", conf);
        }

        // Output configs
        if let Ok(conf) = device.default_output_format() {
            println!("    Default output stream format:\n      {:?}", conf);
        }
    }
}

pub trait FindDevice {
    fn into_device(self) -> Result<Device>;
}

impl FindDevice for usize {
    fn into_device(self) -> Result<Device> {
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;

        devices
            .nth(self)
            .ok_or_else(|| anyhow!("No device device from index"))
    }
}

impl FindDevice for String {
    fn into_device(self) -> Result<Device> {
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;

        devices
            .find(|device: &Device| device.name().unwrap() == self)
            .ok_or_else(|| anyhow!("No device device from name"))
    }
}

fn get_default_input_device() -> Result<Device> {
    let host: Host = cpal::default_host();
    host.default_input_device()
        .ok_or_else(|| anyhow!("no default input device"))
}

fn get_default_output_device() -> Result<Device> {
    let host: Host = cpal::default_host();
    host.default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))
}

pub fn init_sound<T: FindDevice>(
    receiver: crossbeam_channel::Receiver<Message>,
    sender: crossbeam_channel::Sender<Message>,
    input_device_identifier: Option<T>,
    output_device_identifier: Option<T>,
    loop_device_identifier: T,
) -> Result<()> {
    let mut input_device = get_default_input_device()?;
    if input_device_identifier.is_some() {
        input_device = input_device_identifier.unwrap().into_device()?;
    }
    let mut output_device = get_default_output_device()?;
    if output_device_identifier.is_some() {
        output_device = output_device_identifier.unwrap().into_device()?;
    }

    let loop_device = loop_device_identifier.into_device()?;

    info!("Input:  \"{}\"", input_device.name().unwrap());
    info!("Output: \"{}\"", output_device.name().unwrap());
    info!("Loopback: \"{}\"", loop_device.name().unwrap());

    // Input configs
    if let Ok(conf) = input_device.default_input_format() {
        println!("Default input stream format:\n      {:?}", conf);
    }

    let shared_loop_device = Arc::new(loop_device);
    let shared_output_device = Arc::new(output_device);
    let shared_input_device = Arc::new(input_device);

    let shared_loop_device_clone = shared_loop_device.clone();

    std::thread::spawn(move || {
        play_thread(
            receiver,
            sender,
            shared_loop_device_clone,
            shared_output_device,
        );
    });

    std::thread::spawn(move || -> Result<()> {
        sound_thread(shared_input_device, shared_loop_device)
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

fn sound_thread(input_device: Arc<Device>, loop_device: Arc<Device>) -> Result<()> {
    let host = cpal::default_host();
    let event_loop = host.event_loop();

    let input_format = input_device.default_input_format().unwrap();

    // // Build streams.
    info!(
        "Attempting to build input stream with `{:?}`.",
        input_format
    );
    let input_stream_id = event_loop
        .build_input_stream(&*input_device, &input_format)
        .unwrap();
    info!("Successfully built input stream.");

    let loop_format = loop_device.default_output_format().unwrap();

    let loop_stream_id = event_loop
        .build_output_stream(&*loop_device, &loop_format)
        .unwrap();

    let latency_frames = (LATENCY_MS / 1_000.0) * input_format.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * input_format.channels as usize;

    // The buffer to share samples
    let ring = RingBuffer::new(latency_samples * 2);
    let (mut producer, mut consumer) = ring.split();

    // Fill the samples with 0.0 equal to the length of the delay.
    for _ in 0..latency_samples {
        // The ring buffer has twice as much space as necessary to add latency here,
        // so this should never fail
        producer.push(0.0).unwrap();
    }
    event_loop.play_stream(loop_stream_id.clone())?;
    event_loop.play_stream(input_stream_id.clone())?;

    event_loop.run(move |id, result| {
        let data = match result {
            Ok(data) => data,
            Err(err) => {
                error!("an error occurred on stream {:?}: {}", id, err);
                return;
            }
        };

        match data {
            cpal::StreamData::Input {
                buffer: cpal::UnknownTypeInputBuffer::F32(buffer),
            } => {
                assert_eq!(id, input_stream_id);
                let mut output_fell_behind = false;
                let mut new_buffer = Vec::new();
                for &sample in buffer.iter() {
                    new_buffer.push(sample);
                }

                let buffer = rodio::buffer::SamplesBuffer::new(
                    input_format.channels,
                    input_format.sample_rate.0,
                    new_buffer,
                );

                let converter = UniformSourceIterator::new(
                    buffer,
                    loop_format.channels,
                    loop_format.sample_rate.0,
                );

                for sample in converter {
                    if producer.push(sample).is_err() {
                        output_fell_behind = true;
                    }
                }
                if output_fell_behind {
                    eprintln!("output stream fell behind: try increasing latency");
                }
            }
            cpal::StreamData::Output {
                buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer),
            } => {
                assert_eq!(id, loop_stream_id);
                let mut input_fell_behind = None;

                for sample in buffer.iter_mut() {
                    *sample = match consumer.pop() {
                        Some(s) => s,
                        None => {
                            input_fell_behind = Some(0);
                            0.0
                        }
                    };
                }
                if input_fell_behind.is_some() {
                    eprintln!("input stream fell behind: try increasing latency");
                }
            }
            _ => panic!("we're expecting f32 data"),
        }
    });
}
