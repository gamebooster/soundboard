use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use rodio;
use cpal::{Device, Devices};

use std::io::BufReader;
use std::path::PathBuf;
//use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
//use std::fs::File;
use std::thread::JoinHandle;
use log::{info, trace, warn, error};
use std::sync::Arc;


/*
struct StreamStruct{
    output_device : Refrodio::Device,

}

fn playFile(filepath : Path){
    let file_path_string = file_path.to_str().unwrap();
    let file = std::fs::File::open(&file_path).unwrap();
    let file2 = std::fs::File::open(&file_path).unwrap();
    sink.append(rodio::Decoder::new(BufReader::new(file)).unwrap());
    sounds_only_sink.append(rodio::Decoder::new(BufReader::new(file2)).unwrap());
    println!("Playing sound: {}", file_path_string);

}
*/

pub fn init_sound(
    rx : Receiver<PathBuf>,
    input_device_index: Option<usize>,
    output_device_index: Option<usize>,
    loop_device_index: usize,
){


    let host = cpal::default_host();

    let mut devices: Devices = host
        .devices()
        .expect("No available sound devices");

    let mut input_device = host
        .default_input_device()
        .expect("No default input device");
    if input_device_index.is_some() {
        input_device = devices
            .nth(input_device_index.unwrap())
            .expect("invalid input device specified");
    }
    let mut output_device = host
        .default_output_device()
        .expect("No default output device");
    if output_device_index.is_some() {
        output_device = devices
            .nth(output_device_index.unwrap())
            .expect("invalid input device specified");
    }
    let loop_device = devices
        .nth(loop_device_index)
        .expect("invalid loop device specified");

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

    std::thread::spawn(move || {
        sound_thread(shared_input_device, shared_loop_device);
    });
}

pub fn play_thread(rx: Receiver<PathBuf>, loop_device : Arc<Device>, output_device: Arc<Device>){

    loop{

        let receive = rx.recv();

        trace!("Received filepath");

        match receive {
            Ok(file_path) => {

                let loop_sink = rodio::Sink::new(&*loop_device);
                let sound_only_sink = rodio::Sink::new(&*output_device);

                
                let file = std::fs::File::open(&file_path).unwrap();
                let file2 = std::fs::File::open(&file_path).unwrap();   

                loop_sink.append(rodio::Decoder::new(BufReader::new(file)).unwrap());
                sound_only_sink.append(rodio::Decoder::new(BufReader::new(file2)).unwrap());

                loop_sink.detach();
                sound_only_sink.detach();

            },
            Err(_err) => {}
        };
    }

}
//devices : Vec<Device>, 
pub fn sound_thread(input_device : Arc<Device>, loop_device : Arc<Device>){
    
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

    event_loop
        .play_stream(input_stream_id.clone())
        .expect("Fail loopStream");

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
