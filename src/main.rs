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

use warp::Filter;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_module("soundboard", log::LevelFilter::Info)
        .filter_module("warp", log::LevelFilter::Info)
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

    if arguments.is_present("http-server") {
        let gui_sender_clone = gui_sender.clone();
        let gui_receiver_clone = gui_receiver.clone();
        let config_file_clone = config_file.clone();
        std::thread::spawn(move || {
            http_server_routine(config_file_clone, gui_sender_clone, gui_receiver_clone);
        });
    }

    let config_file_clone = config_file.clone();
    if arguments.is_present("no-gui") {
        no_gui_routine(config_file_clone, gui_sender)?;

        std::thread::park();
        return Ok(());
    }

    let mut settings = Settings::with_flags((gui_sender, gui_receiver, config_file));
    settings.window.size = (500, 350);
    gui::Soundboard::run(settings);
    Ok(())
}

#[tokio::main]
async fn http_server_routine(
    config_file: config::MainConfig,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    gui_receiver: crossbeam_channel::Receiver<sound::Message>,
) {
    let config_file_clone = config_file.clone();
    let soundboards_route = warp::path!("soundboards").map(move || {
        let mut soundboards = Vec::new();
        for soundboard in config_file_clone.soundboards.as_ref().unwrap() {
            soundboards.push(&soundboard.name);
        }
        warp::reply::json(&soundboards)
    });

    let config_file_clone = config_file.clone();
    let soundboards_sounds_route =
        warp::path!("soundboards" / String / "sounds").map(move |soundboard_name: String| {
            let maybe_soundboard = config_file_clone
                .soundboards
                .as_ref()
                .unwrap()
                .iter()
                .find(|s| s.name.as_ref().unwrap() == &soundboard_name);
            if let Some(soundboard) = maybe_soundboard {
                let mut sounds = Vec::new();
                for sound in soundboard.sounds.as_ref().unwrap() {
                    sounds.push((sound.name.clone(), sound.path.clone()));
                }
                warp::reply::with_status(warp::reply::json(&sounds), warp::http::StatusCode::OK)
            } else {
                warp::reply::with_status(
                    warp::reply::json(&"no soundboard found with this name"),
                    warp::http::StatusCode::NOT_FOUND,
                )
            }
        });

    let gui_sender_clone = gui_sender.clone();
    let sounds_play_route = warp::path!("sounds" / "play" / String).map(move |path: String| {
        gui_sender_clone
            .send(sound::Message::PlaySound(
                config::SoundConfig {
                    path: path.clone(),
                    ..config::SoundConfig::default()
                },
                sound::SoundDevices::Both,
            ))
            .unwrap();
        format!("PlaySound {}", &path)
    });

    let gui_sender_clone = gui_sender.clone();
    let sounds_stop_route = warp::path!("sounds" / "stop" / String).map(move |path: String| {
        gui_sender_clone
            .send(sound::Message::StopSound(config::SoundConfig {
                path: path.clone(),
                ..config::SoundConfig::default()
            }))
            .unwrap();
        format!("StopSound {}", &path)
    });

    let gui_sender_clone = gui_sender.clone();
    let sounds_stop_all_route = warp::path!("sounds" / "stop").map(move || {
        gui_sender_clone.send(sound::Message::StopAll).unwrap();
        format!("StopAllSound")
    });

    let gui_sender_clone = gui_sender.clone();
    let sounds_active_route = warp::path!("sounds" / "active").map(move || {
        gui_sender_clone
            .send(sound::Message::PlayStatus(Vec::new()))
            .unwrap();
        match gui_receiver.recv() {
            Ok(sound::Message::PlayStatus(sounds)) => {
                return warp::reply::with_status(
                    warp::reply::json(&sounds),
                    warp::http::StatusCode::OK,
                );
            }
            _ => warp::reply::with_status(
                warp::reply::json(&"unknown error"),
                warp::http::StatusCode::from_u16(500).unwrap(),
            ),
        }
    });

    let help_api = warp::path::end()
        .map(|| "This is the Soundboard API. Try calling /api/soundboards or /api/sounds/active");

    let routes = (warp::path("api").and(
        soundboards_route
            .or(sounds_play_route)
            .or(soundboards_sounds_route)
            .or(sounds_stop_route)
            .or(sounds_stop_all_route)
            .or(sounds_active_route)
            .or(help_api),
    ))
    .or(warp::get().and(warp::fs::dir("web")));

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

fn no_gui_routine(
    config_file: config::MainConfig,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
) -> Result<()> {
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
    for sound in config_file.soundboards.unwrap()[0]
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
                sound.clone(),
                sound::SoundDevices::Both,
            )) {
                error!("failed to play sound {}", err);
            };
        })?;
    }

    Ok(())
}
