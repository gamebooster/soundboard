use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};

use cpal::{Host, Device, Devices};

use std::io::BufReader;
use std::path::PathBuf;
//use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
//use std::fs::File;
use std::thread::JoinHandle;
use log::{info, trace, warn, error};
use std::sync::Arc;
use anyhow::{Context, Result, anyhow};

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

pub fn send_playsound(sender: Sender<PathBuf>, sound_path: &std::path::Path) -> Result<()> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.push("sounds");
    path.push(sound_path);
    info!("Playing sound: {}", sound_path.display());
    sender.send(path)?;
    Ok(())
}

pub trait FindDevice {

    fn into_device(self) -> Result<Device>;
}

impl FindDevice for usize{

    fn into_device(self) -> Result<Device>{
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;
        
        devices.nth(self).ok_or(anyhow!("No device device from index")) 
    }
}

impl FindDevice for String{

    fn into_device(self) -> Result<Device>{
        let host = cpal::default_host();

        let mut devices: Devices = host.devices()?;

        devices.find(|device : &Device| device.name().unwrap() == self).ok_or(anyhow!("No device device from name")) 

    }
}

fn get_default_input_device() -> Result<Device> {
    let host : Host = cpal::default_host();
    host.default_input_device().ok_or(anyhow!("no default input device"))
}

fn get_default_output_device() -> Result<Device> {
    let host : Host = cpal::default_host();
    host.default_output_device().ok_or(anyhow!("no default output device"))
}


pub fn init_sound<T : FindDevice>(
    rx : Receiver<PathBuf>,
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

    std::thread::spawn(move || -> Result<()>{
        sound_thread(shared_input_device, shared_loop_device)
    });

    Ok(())
}

fn play_thread(rx: Receiver<PathBuf>, loop_device : Arc<Device>, output_device: Arc<Device>){

    loop{

        let receive = rx.recv();

        trace!("Received filepath");

        match receive {
            Ok(file_path) => {

                let loop_sink = rodio::Sink::new(&*loop_device);
                let sound_only_sink = rodio::Sink::new(&*output_device);

            
                let file = match std::fs::File::open(&file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        error!("{}", e);
                        continue
                    },
                };
                let file2 = match std::fs::File::open(&file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        error!("{}", e);
                        continue
                    },
                };

                loop_sink.append(rodio::Decoder::new(BufReader::new(file)).unwrap());
                sound_only_sink.append(rodio::Decoder::new(BufReader::new(file2)).unwrap());

                loop_sink.detach();
                sound_only_sink.detach();

            },
            Err(_err) => {}
        };
    }

}

fn sound_thread(input_device : Arc<Device>, loop_device : Arc<Device>) -> Result<()>{
    
    let loop_sink = rodio::Sink::new(&*loop_device);
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

    /*
    let loop_format = loop_device.default_output_format().unwrap();

    let loop_stream_id = event_loop
        .build_output_stream(&*loop_device, &loop_format)
        .unwrap();

    event_loop
        .play_stream(loop_stream_id.clone())?;
    */

    event_loop
        .play_stream(input_stream_id)?;

    

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
                let mut new_buffer = Vec::new();
                for sample in buffer.iter() {
                    let sample = cpal::Sample::to_f32(sample);
                    new_buffer.push(sample);
                }
                let buffer = rodio::buffer::SamplesBuffer::new(
                    input_format.channels,
                    input_format.sample_rate.0,
                    new_buffer,
                );
                loop_sink.append(buffer);
            }
            _ => panic!("we're expecting f32 data"),
        }
    });
}
