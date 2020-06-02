use clap::{crate_authors, crate_version, App, Arg};
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use hotkey;
use iced::{
    button, executor, Align, Application, Button, Column, Command, Element, Settings, Subscription,
    Text,
};
use rodio;
use std::path::PathBuf;
use std::io::BufReader;

fn print_possible_devices() {
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

fn sound_thread(input_device_index: usize, output_device_index: usize) {
    let host = cpal::default_host();

    let devices: Vec<_> = host
        .devices()
        .expect("No available sound devices")
        .collect();
    let input_device = devices
        .get(input_device_index)
        .expect("invalid input device specified");
    let output_device = devices
        .get(output_device_index)
        .expect("invalid output device specified");
    let output_device_default = host        //Needs additional flag instead of default device
        .default_output_device()
        .expect("no default device available");
    println!("  Using Devices: ");
    println!(
        "  {}. \"{}\"",
        input_device_index,
        input_device.name().unwrap()
    );
    // Input configs
    if let Ok(conf) = input_device.default_input_format() {
        println!("    Default input stream format:\n      {:?}", conf);
    }

    println!(
        "  {}. \"{}\"",
        output_device_index,
        output_device.name().unwrap()
    );
    // Output configs
    if let Ok(conf) = output_device.default_output_format() {
        println!("    Default output stream format:\n      {:?}", conf);
    }

    let sink = rodio::Sink::new(&output_device);
    let sounds_only_sink = rodio::Sink::new(&output_device_default);

    std::thread::spawn(move || {
        let mut hk = hotkey::Listener::new();
        hk.register_hotkey(hotkey::modifiers::CONTROL, 'P' as u32, move || {
            let mut file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            file_path.push("resources/nicht-so-tief-rudiger.mp3");
            let file_path_string = file_path.to_str().unwrap();
            let file = std::fs::File::open(&file_path).unwrap();
            let file2 = std::fs::File::open(&file_path).unwrap();       
            sink.append(rodio::Decoder::new(BufReader::new(file)).unwrap());
            sounds_only_sink.append(rodio::Decoder::new(BufReader::new(file2)).unwrap());
            println!("Playing sound: {}", file_path_string);
        })
        .unwrap();

        hk.listen();
    });

    let sink2 = rodio::Sink::new(&output_device);

    let host = cpal::default_host();
    let event_loop = host.event_loop();

    let input_format = input_device.default_input_format().unwrap();

    // // Build streams.
    println!(
        "Attempting to build input stream with `{:?}`.",
        input_format
    );
    let input_stream_id = event_loop
        .build_input_stream(&input_device, &input_format)
        .unwrap();
    println!("Successfully built input stream.");

    event_loop
        .play_stream(input_stream_id.clone())
        .expect("fail stream1");

    event_loop.run(move |id, result| {
        let data = match result {
            Ok(data) => data,
            Err(err) => {
                eprintln!("an error occurred on stream {:?}: {}", id, err);
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
                sink2.append(buffer);
            }
            _ => panic!("we're expecting f32 data"),
        }
    });
}

pub fn main() {
    let matches = App::new("soundboard")
        .version(crate_version!())
        .author(crate_authors!())
        .about("play sounds over your microphone")
        .arg(
            Arg::with_name("config-file")
                .short('c')
                .long("config")
                .value_name("FILE")
                .default_value("soundboard.toml")
                .about("sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("input-device")
                .short('i')
                .long("input-device")
                .about("Sets the input device to use")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output-device")
                .short('o')
                .long("output-device")
                .about("Sets the output device to use")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .takes_value(true)
                .about("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("print-possible-devices")
                .long("print-possible-devices")
                .about("Print possible devices"),
        )
        .arg(Arg::with_name("no-gui").long("no-gui").about("Disable GUI"))
        .get_matches();

    if matches.is_present("print-possible-devices") {
        print_possible_devices();
        return;
    }

    let input_device_index: usize = matches
        .value_of("input-device")
        .expect("No input device specified")
        .parse()
        .expect("No number specified");
    let output_device_index: usize = matches
        .value_of("output-device")
        .expect("No ouput device specified")
        .parse()
        .expect("No number specified");

    let handle = std::thread::spawn(move || {
        sound_thread(input_device_index, output_device_index);
    });

    if matches.is_present("no-gui") {
        handle.join().expect("sound_thread join failed");
        return;
    }

    let mut settings = Settings::default();
    settings.window.size = (275, 150);
    Soundboard::run(settings);
}

#[derive(Default)]
struct Soundboard {
    play_sound_button: button::State,
    status_text: String,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    PlaySound,
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Soundboard, Command<Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        String::from("soundboard")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PlaySound => {
                self.status_text = "Start playing sound...".to_string();
            }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        Column::new()
            .padding(20)
            .align_items(Align::Center)
            .push(Text::new(self.title()).size(32))
            .push(
                Button::new(&mut self.play_sound_button, Text::new("Play sound"))
                    .on_press(Message::PlaySound),
            )
            .padding(10)
            .push(Text::new(&self.status_text).size(10))
            .into()
    }
}
