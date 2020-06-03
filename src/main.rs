#![allow(unused_imports)]

extern crate log;

use ::hotkey as hotkeyExt;
use clap::{crate_authors, crate_version, App, Arg};
use cpal::traits::{DeviceTrait,  HostTrait};
use iced::{
    button, executor, Align, Application, Button, Column, Command, Element, Settings, Subscription,
    Text,
};

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use std::env;
use log::{info, trace, warn, error};

//use rodio;

mod config;
mod gui;
mod hotkey;
mod sound;

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

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    info!("Parsing arguments");

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
            Arg::with_name("loopback-device")
                .short('l')
                .long("loopback-device")
                .about("Sets the loopback device to use")
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
        return Ok(())
    }

    let mut path = env::current_exe()?;
    path.pop();
    path.push(matches.value_of("config-file").unwrap());
    let config_file = config::parse_config(path.as_path())?;
    //println!("{:#?}", config_file);

    let input_device_index: Option<usize> = {
        if matches.is_present("input-device") {
            Some(
                matches
                    .value_of("input-device")
                    .expect("No input device specified")
                    .parse()
                    .expect("No number specified"),
            )
        } else {
            None
        }
    };
    let output_device_index: Option<usize> = {
        if matches.is_present("output-device") {
            Some(
                matches
                    .value_of("output-device")
                    .expect("No ouput device specified")
                    .parse()
                    .expect("No number specified"),
            )
        } else {
            None
        }
    };
    let loop_device_index: usize = matches
        .value_of("loopback-device")
        .expect("No loopback device specified")
        .parse()
        .expect("No number specified");


    let (tx, rx) : (Sender<PathBuf>, Receiver<PathBuf>) = mpsc::channel();

    //Init Sound Module, pass Receiver to send File Paths to 
    std::thread::spawn(move || -> Result<()>{
      sound::init_sound(rx, input_device_index, output_device_index, loop_device_index)
    });

    let hotkey_thread = std::thread::spawn(move || {
        let mut hk = hotkeyExt::Listener::new();
        hk.register_hotkey(hotkeyExt::modifiers::CONTROL, 'P' as u32, move || {
            let mut file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            file_path.push("resources/nicht-so-tief-rudiger.mp3");
            let file_path_string = file_path.to_str().unwrap();
            info!("Playing sound: {}", file_path_string);
            tx.send(file_path).unwrap();
        })
        .unwrap();

        hk.listen();
    });

    if matches.is_present("no-gui") {
        hotkey_thread.join().expect("sound thread join failed");
        return Ok(());
    }

    let mut settings = Settings::default();
    settings.window.size = (275, 150);
    Soundboard::run(settings);
    Ok(())
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
