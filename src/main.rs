#![allow(unused_imports)]

extern crate clap;
extern crate cpal;
extern crate iced;
extern crate log;

extern crate strum;
#[macro_use]
extern crate strum_macros;

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};

use ::hotkey as hotkeyExt;
use clap::{crate_authors, crate_version, App, Arg};
use cpal::traits::{DeviceTrait, HostTrait};
use crossbeam_channel;
use iced::{
    button, executor, Align, Application, Button, Column, Command, Container, Element, Length, Row,
    Settings, Subscription, Text,
};
use std::env;
use std::path::{Path, PathBuf};

mod config;
mod download;
mod gui;
mod hotkey;
mod sound;
mod utils;

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
    // println!("{:#?}", config_file);

    let (sound_sender, gui_receiver): (
        crossbeam_channel::Sender<sound::Message>,
        crossbeam_channel::Receiver<sound::Message>,
    ) = crossbeam_channel::unbounded();

    let (gui_sender, sound_receiver): (
        crossbeam_channel::Sender<sound::Message>,
        crossbeam_channel::Receiver<sound::Message>,
    ) = crossbeam_channel::unbounded();

    let (input_device_index, output_device_index, loop_device_index) =
        config::parse_devices(&config_file, &arguments)?;

    let sound_receiver_clone = sound_receiver.clone();
    let sound_sender_clone = sound_sender.clone();
    std::thread::spawn(move || -> Result<()> {
        sound::init_sound(
            sound_receiver_clone,
            sound_sender_clone,
            input_device_index,
            output_device_index,
            loop_device_index,
        )
    });

    if arguments.is_present("no-gui") {
        let mut hotkey_manager = hotkey::HotkeyManager::new();

        let stop_hotkey = {
            if config_file.stop_hotkey.is_some() {
                config::parse_hotkey(&config_file.stop_hotkey.as_ref().unwrap())?
            } else {
                config::Hotkey {
                    modifier: vec![config::Modifier::CTRL],
                    key: config::Key::S,
                }
            }
        };
        let gui_sender_clone = gui_sender.clone();
        hotkey_manager
            .register(stop_hotkey, move || {
                let _result = gui_sender_clone.send(sound::Message::StopAll);
            })
            .map_err(|_s| anyhow!("register key"))?;

        let gui_sender_clone = gui_sender.clone();
        // only register hotkeys for first soundboard in no-gui-mode
        for sound in config_file.soundboards[0]
            .sounds
            .clone()
            .unwrap_or_default()
        {
            if sound.hotkey.is_none() {
                continue;
            }
            let hotkey = config::parse_hotkey(&sound.hotkey.as_ref().unwrap())?;
            let tx_clone = gui_sender_clone.clone();
            let _result = hotkey_manager.register(hotkey, move || {
                if let Err(err) = tx_clone.send(sound::Message::PlaySound(
                    sound.path.clone(),
                    sound::SoundDevices::Both,
                )) {
                    error!("failed to play sound {}", err);
                };
            })?;
        }

        std::thread::park();
        return Ok(());
    }

    let config_file = config::load_and_parse_config(arguments.value_of("config-file").unwrap())?;
    let mut settings = Settings::with_flags((gui_sender, gui_receiver, config_file));
    settings.window.size = (500, 350);
    gui::Soundboard::run(settings);
    Ok(())
}
