#![allow(unused_imports)]

extern crate clap;
extern crate cpal;
extern crate iced;
extern crate log;

use log::{error, info, trace, warn};
use anyhow::{anyhow, Context, Result};

use ::hotkey as hotkeyExt;
use clap::{crate_authors, crate_version, App, Arg};
use cpal::traits::{DeviceTrait, HostTrait};
use iced::{
    button, executor, Align, Application, Button, Column, Command, Container, Element, Length, Row,
    Settings, Subscription, Text,
};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

mod config;
mod gui;
mod hotkey;
mod sound;
mod download;

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

    let (tx, rx): (Sender<sound::Message>, Receiver<sound::Message>) = mpsc::channel();

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

        
        //-----------------
        //Text for Message Passing to sound thread, remove later 
        
        let tx_clone2 = tx_clone.clone();
        hk.register_hotkey(hotkeyExt::modifiers::CONTROL, 'M' as u32, move || {
            
            let result = tx_clone2.send(sound::Message::StopAll);
        })
        .unwrap();
        
        // -----------------

        for sound in config_file.sounds.unwrap_or_default() {
            if !sound.hotkey_key.is_some() {
                continue;
            }
            let modifier = sound.hotkey_modifier.clone().unwrap_or_default();
            let tx_clone = tx_clone.clone();
            let _result = hk
                .register_hotkey(
                    modifier
                        .iter()
                        .fold(0, |acc, x| acc | (*x as u32)) as u32,
                    sound.hotkey_key.unwrap() as u32,
                    move || {
                        let tx_clone = tx_clone.clone();
                        let _result = sound::send_playsound(tx_clone, &sound.path);
                    },
                ).map_err(|_s| anyhow!("register key"));
        }
        hk.listen();
        Ok(())
    });

    if arguments.is_present("no-gui") {
        let _result = hotkey_thread.join().expect("sound thread join failed");
        return Ok(());
    }

    let config_file = config::load_and_parse_config(arguments.value_of("config-file").unwrap())?;
    let tx_clone = tx;
    let mut settings = Settings::with_flags((tx_clone, config_file));
    settings.window.size = (500, 350);
    gui::Soundboard::run(settings);
    Ok(())
}
