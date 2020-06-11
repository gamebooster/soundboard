use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use cpal::{Device, Devices, Host};
use crossbeam_channel;
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
            .ok_or(anyhow!("No device device from index"))
    }
}

impl FindDevice for String {
    fn into_device(self) -> Result<Device> {
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;

        devices
            .find(|device: &Device| device.name().unwrap() == self)
            .ok_or(anyhow!("No device device from name"))
    }
}

fn get_default_input_device() -> Result<Device> {
    let host: Host = cpal::default_host();
    host.default_input_device()
        .ok_or(anyhow!("no default input device"))
}

fn get_default_output_device() -> Result<Device> {
    let host: Host = cpal::default_host();
    host.default_output_device()
        .ok_or(anyhow!("no default output device"))
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

type SoundMap = HashMap<config::SoundConfig, Vec<Sink>>;

#[derive(PartialEq)]
pub enum SoundDevices {
    Loop,
    Output,
    Both,
}

#[derive(PartialEq)]
pub enum Message {
    PlaySound(config::SoundConfig, SoundDevices),
    StopSound(config::SoundConfig),
    StopAll,
    SetVolume(f32),
    PlayStatus(Vec<config::SoundConfig>),
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
    sink.append(rodio::Decoder::new(BufReader::new(file))?);

    if !sinks.contains_key(&sound_config) {
        sinks.insert(sound_config, vec![sink]);
    } else {
        let vec = sinks.get_mut(&sound_config).unwrap();
        vec.push(sink);
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
                    match sinks.remove(&sound_handle) {
                        Some(vec) => {
                            for sink in vec {
                                drop(sink);
                            }
                        }
                        None => (),
                    };
                }
                Message::StopAll => {
                    for (_, vec) in sinks.drain() {
                        for sink in vec {
                            drop(sink);
                        }
                    }
                }
                Message::SetVolume(volume_new) => {
                    volume = volume_new;
                    for (_, vec) in sinks.iter() {
                        for sink in vec {
                            sink.set_volume(volume);
                        }
                    }
                }
                Message::PlayStatus(_) => {
                    let mut sounds = Vec::new();
                    for (id, _) in sinks.iter() {
                        sounds.push(id.clone());
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

        sinks.retain(|_, local_sinks| {
            let playing = local_sinks.iter().any(|s| !s.empty());
            playing
        });
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
