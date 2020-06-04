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
                        let _result = sound::send_playsound(tx_clone, Path::new(&sound.path));
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
    settings.window.size = (450, 325);
    gui::Soundboard::run(settings);
    Ok(())
}
