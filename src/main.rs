#![allow(unused_imports)]
extern crate clap;
#[cfg(feature = "autoloop")]
extern crate ctrlc;
extern crate log;
extern crate strum;
extern crate strum_macros;

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use std::panic;

#[cfg(feature = "gui")]
extern crate iced;
#[cfg(feature = "gui")]
use iced::Application;
#[cfg(feature = "gui")]
use iced::Settings;
#[cfg(feature = "gui")]
mod gui;

#[cfg(feature = "terminal-ui")]
mod tui;

use std::process;

#[cfg(feature = "http")]
mod http_server;

#[cfg(feature = "telegram")]
mod telegram;

#[cfg(feature = "autoloop")]
mod pulseauto;

mod config;
mod download;
mod hotkey;
mod sound;
mod utils;

fn main() {
    macro_rules! FATAL_ERROR_MESSAGE {
    () => {
      r"
soundboard encountered an fatal error:
  Description:
    {}
  Location:
    {}
  Additional:
    If unexpected please file a bug report at https://github.com/gamebooster/soundboard/issues"
    };
  };
    panic::set_hook(Box::new(|panic_info| {
        let mut location_info = String::new();
        if let Some(location) = panic_info.location() {
            location_info += &format!(
                "panic occurred in file '{}' at line {}",
                location.file(),
                location.line(),
            );
        } else {
            location_info += "panic occurred but can't get location information...";
        }
        if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            error!(FATAL_ERROR_MESSAGE!(), payload, location_info);
        } else if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
            error!(FATAL_ERROR_MESSAGE!(), payload, location_info);
        } else {
            error!(
                FATAL_ERROR_MESSAGE!(),
                "No description location: {}", location_info
            );
        }

        std::process::exit(1);
    }));

    if let Err(err) = try_main() {
        error!(FATAL_ERROR_MESSAGE!(), err, "No location");
        std::process::exit(1);
    }

    tui::quit_terminal_ui().ok();
    info!("Auf Wiedersehen!");
}

fn try_main() -> Result<()> {
    env_logger::builder()
        .filter_module("soundboard", log::LevelFilter::Trace)
        .filter_module("warp", log::LevelFilter::Info)
        .init();

    if config::MainConfig::read()
        .print_possible_devices
        .unwrap_or_default()
    {
        sound::print_possible_devices_and_exit();
        return Ok(());
    }

    let (sound_sender, gui_receiver): (
        crossbeam_channel::Sender<sound::Message>,
        crossbeam_channel::Receiver<sound::Message>,
    ) = crossbeam_channel::unbounded();

    let (gui_sender, sound_receiver): (
        crossbeam_channel::Sender<sound::Message>,
        crossbeam_channel::Receiver<sound::Message>,
    ) = crossbeam_channel::unbounded();

    #[cfg(feature = "autoloop")]
    let mut null_sink_module_id: Option<u32> = None;

    #[cfg(feature = "autoloop")]
    let mut loopback_module_id: Option<u32> = None;

    #[cfg(feature = "autoloop")]
    let mut loop_device_id = config::MainConfig::read().loopback_device.clone();

    #[cfg(feature = "autoloop")]
    {
        if config::MainConfig::read()
            .auto_loop_device
            .unwrap_or_default()
        {
            config::MainConfig::set_no_duplex_device_option(Some(true));
            let module_name = "module-null-sink";
            let module_args = "sink_name=SoundboardNullSink sink_properties=device.description=SoundboardNullSink";
            match pulseauto::load_module(module_name, module_args) {
                Ok(module_id) => {
                    loop_device_id = Some("SoundboardNullSink".to_owned());
                    null_sink_module_id = Some(module_id);
                }
                Err(error) => panic!("null_sink creation failed: {}", error),
            };

            info!("autoloop: created SoundboardNullSink pulse audio module");

            let module_name = "module-loopback";
            let module_args = "source=@DEFAULT_SOURCE@ sink=SoundboardNullSink latency_msec=20";
            match pulseauto::load_module(module_name, module_args) {
                Ok(module_id) => {
                    loopback_module_id = Some(module_id);
                }
                Err(error) => panic!("loopback creation failed: {}", error),
            };

            info!("autoloop: created SoundboardLoopback pulse audio module");
        }
    }
    #[cfg(not(feature = "autoloop"))]
    let loop_device_id = config::MainConfig::read().loopback_device.clone();

    let loop_device_id = loop_device_id.ok_or_else(|| {
        anyhow!(
            r"No loopback device specified in config file with loopback_device or
                                 in env with SB_LOOPBACK_DEVICE or
                                 in cmd arguments with --loopback-device"
        )
    })?;

    let gui_sender_clone = gui_sender.clone();
    let input_device_id_clone = config::MainConfig::read().input_device.clone();
    let output_device_id_clone = config::MainConfig::read().output_device.clone();
    let _sound_thread_handle = std::thread::spawn(move || {
        sound::run_sound_loop(
            sound_receiver,
            sound_sender,
            gui_sender_clone,
            input_device_id_clone,
            output_device_id_clone,
            loop_device_id,
        );
    });

    // test for sound thread successfull initialization
    if let Err(err) = gui_sender.send(sound::Message::PlayStatus(Vec::new(), 0.0)) {
        return Err(anyhow!(err));
    }
    if let Err(err) = gui_receiver.recv() {
        return Err(anyhow!(err));
    }

    #[cfg(feature = "http")]
    {
        if config::MainConfig::read().http_server.unwrap_or_default() {
            let gui_sender_clone = gui_sender.clone();
            let gui_receiver_clone = gui_receiver.clone();
            std::thread::spawn(move || {
                http_server::run(gui_sender_clone, gui_receiver_clone);
            });
        }
    }

    #[cfg(feature = "telegram")]
    {
        if config::MainConfig::read().telegram.unwrap_or_default() {
            let gui_sender_clone = gui_sender.clone();
            let gui_receiver_clone = gui_receiver.clone();
            std::thread::spawn(move || {
                telegram::run(gui_sender_clone, gui_receiver_clone);
            });
        }
    }

    #[cfg(feature = "autoloop")]
    ctrlc::set_handler(move || {
        if let Some(id) = null_sink_module_id {
            pulseauto::unload_module(id).expect("unload null sink failed");
        }
        info!("autoloop: unloaded SoundboardNullSink pulse audio module");

        if let Some(id) = loopback_module_id {
            pulseauto::unload_module(id).expect("unload loopback sink failed");
        }
        info!("autoloop: unloaded SoundboardLoopback pulse audio module");

        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");


    #[cfg(feature = "terminal-ui")]
    {
        tui::draw_terminal().ok();
    }

    #[cfg(feature = "gui")]
    {
        if config::MainConfig::read().no_gui.unwrap_or_default() {
            no_gui_routine(gui_sender)?;
            std::thread::park();
            return Ok(());
        }
        let mut settings = Settings::with_flags((gui_sender, gui_receiver));
        settings.window.size = (500, 350);
        gui::Soundboard::run(settings);
    }

    #[cfg(not(any(feature = "gui", feature = "terminal-ui")))]
    {
        no_gui_routine(gui_sender)?;
    }
    Ok(())
}

fn no_gui_routine(_gui_sender: crossbeam_channel::Sender<sound::Message>) -> Result<()> {
    use winit::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)
        .unwrap();
    // let mut hotkey_manager = hotkey::HotkeyManager::new();

    // let stop_hotkey = {
    //     if let Some(key) = config::MainConfig::read().stop_hotkey.as_ref() {
    //         config::parse_hotkey(&key)?
    //     } else {
    //         config::Hotkey {
    //             modifier: vec![config::Modifier::CTRL],
    //             key: config::Key::S,
    //         }
    //     }
    // };
    // let gui_sender_clone = gui_sender.clone();
    // hotkey_manager
    //     .register(stop_hotkey, move || {
    //         let _result = gui_sender_clone.send(sound::Message::StopAll);
    //     })
    //     .map_err(|_s| anyhow!("register key"))?;

    // let gui_sender_clone = gui_sender;
    // // only register hotkeys for first soundboard in no-gui-mode
    // for sound in config::MainConfig::read().soundboards[0]
    //     .sounds
    //     .clone()
    //     .unwrap_or_default()
    // {
    //     if sound.hotkey.is_none() {
    //         continue;
    //     }
    //     let hotkey = config::parse_hotkey(&sound.hotkey.as_ref().unwrap())?;
    //     let tx_clone = gui_sender_clone.clone();
    //     info!("register hotkey  {} for sound {}", &hotkey, sound.name);
    //     let _result = hotkey_manager.register(hotkey, move || {
    //         if let Err(err) = tx_clone.send(sound::Message::PlaySound(
    //             sound.clone(),
    //             sound::SoundDevices::Both,
    //         )) {
    //             error!("failed to play sound {}", err);
    //         };
    //     })?;
    // }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
