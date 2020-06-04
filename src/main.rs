#![allow(unused_imports)]

extern crate clap;
extern crate cpal;
extern crate iced;
extern crate log;

use ::hotkey as hotkeyExt;
use anyhow::{anyhow, Context, Result};
use clap::{crate_authors, crate_version, App, Arg};
use cpal::traits::{DeviceTrait, HostTrait};
use iced::{
    button, executor, Align, Application, Button, Column, Command, Container, Element, Length, Row,
    Settings, Subscription, Text,
};
use log::{error, info, trace, warn};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

mod config;
mod gui;
mod hotkey;
mod sound;
mod style;

fn send_playsound(sender: Sender<PathBuf>, sound_path: &Path) -> Result<()> {
    let mut path = env::current_exe()?;
    path.pop();
    path.push("sounds");
    path.push(sound_path);
    info!("Playing sound: {}", sound_path.display());
    sender.send(path)?;
    Ok(())
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_module("soundboard", log::LevelFilter::Info)
        .init();
    info!("Parsing arguments");
    let arguments = config::parse_arguments();

    if arguments.is_present("print-possible-devices") {
        sound::print_possible_devices();
        return Ok(());
    }

    let config_file = config::load_and_parse_config(arguments.value_of("config-file").unwrap())?;
    println!("{:#?}", config_file);

    let (tx, rx): (Sender<PathBuf>, Receiver<PathBuf>) = mpsc::channel();

    let (input_device_index, output_device_index, loop_device_index) =
        config::parse_devices(&config_file, &arguments)?;

    std::thread::spawn(move || -> Result<()> {
        sound::init_sound(
            rx,
            input_device_index,
            output_device_index,
            loop_device_index,
        )
    });

    let tx_clone = tx.clone();
    let hotkey_thread = std::thread::spawn(move || -> Result<()> {
        let mut hk = hotkeyExt::Listener::new();
        for sound in config_file.sounds.unwrap_or(Vec::new()) {
            let tx_clone = tx_clone.clone();
            let _result = hk
                .register_hotkey(
                    sound
                        .hotkey_modifier
                        .iter()
                        .fold(0, |acc, x| acc | (*x as u32)) as u32,
                    sound.hotkey_key as u32,
                    move || {
                        let tx_clone = tx_clone.clone();
                        let _result = send_playsound(tx_clone, Path::new(&sound.path));
                    },
                )
                .or_else(|_s| Err(anyhow!("register key")));
        }
        hk.listen();
        Ok(())
    });

    if arguments.is_present("no-gui") {
        let _result = hotkey_thread.join().expect("sound thread join failed");
        return Ok(());
    }

    let config_file = config::load_and_parse_config(arguments.value_of("config-file").unwrap())?;
    let tx_clone = tx.clone();
    let mut settings = Settings::with_flags((tx_clone, config_file));
    settings.window.size = (400, 150);
    Soundboard::run(settings);
    Ok(())
}

#[derive(Debug, Clone)]
struct SoundButton {
    state: button::State,
    name: String,
    path: String,
    hotkey: String,
}

#[derive(Debug)]
struct Soundboard {
    buttons: Vec<SoundButton>,
    status_text: String,
    config: config::Config,
    sender: Sender<PathBuf>,
    increment_button: button::State,
}

#[derive(Debug, Clone)]
enum Message {
    PlaySound(String),
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = (Sender<PathBuf>, config::Config);

    fn new(flags: Self::Flags) -> (Soundboard, Command<Message>) {
        let mut soundboard = Soundboard {
            buttons: Vec::<SoundButton>::new(),
            status_text: String::new(),
            config: flags.1,
            sender: flags.0,
            increment_button: button::State::new(),
        };
        soundboard.buttons = soundboard.config.sounds.as_ref().unwrap().into_iter().fold(
            Vec::<SoundButton>::new(),
            |mut buttons, sound| {
                buttons.push(SoundButton {
                    state: button::State::new(),
                    path: sound.path.clone(),
                    name: sound.name.clone(),
                    hotkey: format!(
                        "{}-{}",
                        sound.hotkey_modifier.clone().into_iter().fold(
                            String::new(),
                            |all, one| {
                                if all.len() > 0 {
                                    format!("{}-{}", all, one)
                                } else {
                                    one.to_string()
                                }
                            }
                        ),
                        sound.hotkey_key.to_string()
                    ),
                });
                buttons
            },
        );
        (soundboard, Command::none())
    }

    fn title(&self) -> String {
        String::from("soundboard")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PlaySound(sound_path) => {
                let _result = send_playsound(self.sender.clone(), Path::new(&sound_path));
                self.status_text = "Start playing sound...".to_string();
            }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        let column = self.buttons.iter_mut().fold(
            Column::new()
                .padding(10)
                .spacing(5)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_items(Align::Center),
            |column, button| {
                let row_contents = Row::new()
                    .padding(10)
                    .spacing(20)
                    .align_items(Align::Center)
                    .push(Text::new(button.name.clone()))
                    .push(Text::new(button.hotkey.clone()))
                    .push(
                        Button::new(&mut button.state, Text::new("Play"))
                            .on_press(Message::PlaySound(button.path.clone()))
                            .style(style::Button::Constructive),
                    );
                column.push(Container::new(row_contents).style(style::Container::Entry))
            },
        );
        let container = Container::new(column)
            .padding(10)
            .style(style::Container::Background)
            .width(Length::Fill)
            .height(Length::Fill);
        container.into()
    }
}
