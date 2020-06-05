use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};

use cpal::{Device, Devices, Host};

use std::io::BufReader;
use std::path::PathBuf;
use std::collections::HashMap;
use std::str::FromStr;
//use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
//use std::fs::File;
use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use std::sync::Arc;
use std::thread::JoinHandle;

use ringbuf::RingBuffer;
use rodio::source::UniformSourceIterator;
<<<<<<< HEAD
use rodio::{Source, Sink};

use uuid::Uuid;


=======
use rodio::Source;
>>>>>>> e2193b293ed8325274301d5b8e75aa172e932fdd

const LATENCY_MS: f32 = 150.0;

use super::download;

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

pub type SoundHandle = String;

pub fn send_playsound(sender: Sender<Message>, sound_path: &str) -> Result<SoundHandle> {
    let path = {
        if sound_path.starts_with("http") {
            download::request_file(sound_path.to_string())?
        } else {
            let mut path = std::env::current_exe()?;
            path.pop();
            path.push("sounds");
            path.push(sound_path);
            path
        }
    };

    let buffer = &mut Uuid::encode_buffer();

    let my_uuid = Uuid::new_v4().to_simple().encode_lower(buffer);
    let uuid_string : SoundHandle = String::from_str(my_uuid)?;
    let uuid_clone = uuid_string.clone();

    info!("Playing sound: {}", sound_path);
    sender.send(Message::PlaySound(path, uuid_clone))?;
    Ok(uuid_string)
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
    rx: Receiver<Message>,
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
        play_thread(rx, shared_loop_device_clone, shared_output_device);
    });

    std::thread::spawn(move || -> Result<()> {
        sound_thread(shared_input_device, shared_loop_device)
    });

    Ok(())
}

pub enum Message{
    PlaySound(PathBuf, SoundHandle),
    StopSound(SoundHandle),
    StopAll,
    SetVolume(f32)
}

struct DoubleSink(Sink, Sink);

fn play_thread(rx: Receiver<Message>, loop_device: Arc<Device>, output_device: Arc<Device>) {

    let mut volume : f32 = 1.0;
    let mut sinks: HashMap<String, DoubleSink> = HashMap::new();

    loop {

        let receive = rx.recv();

        trace!("Received filepath");

        match receive {
            Ok(message) => {
                match message {
                    Message::PlaySound(file_path, uuid) => {
                        
                        let loop_sink = Sink::new(&*loop_device);
                        let sound_only_sink = rodio::Sink::new(&*output_device);

                        let file = match std::fs::File::open(&file_path) {
                            Ok(file) => file,
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        };
                        let file2 = match std::fs::File::open(&file_path) {
                            Ok(file) => file,
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        };

                        loop_sink.set_volume(volume);
                        sound_only_sink.set_volume(volume);
        
                        loop_sink.append(rodio::Decoder::new(BufReader::new(file)).unwrap());
                        sound_only_sink.append(rodio::Decoder::new(BufReader::new(file2)).unwrap());

                        sinks.insert(uuid, DoubleSink(loop_sink, sound_only_sink));
                    }
                    Message::StopSound(uuid) => {
                        match sinks.remove(&uuid) {
                            Some(double_sink) => {
                                drop(double_sink.0);
                                drop(double_sink.1);
                            },
                            None => ()
                        };
                    }
                    Message::StopAll => {
                        for (_, double_sink) in sinks.drain(){
                            drop(double_sink.0);
                            drop(double_sink.1);
                        }
                    }
                    Message::SetVolume(volume_new) => volume = volume_new,
                }
            }
            Err(_err) => {}
        };

        sinks.retain(|_, double_sinks| {
            !double_sinks.0.empty() && !double_sinks.1.empty()
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
                if let Some(_) = input_fell_behind {
                    eprintln!("input stream fell behind: try increasing latency");
                }
            }
            _ => panic!("we're expecting f32 data"),
        }
    });
}
