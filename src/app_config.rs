use super::hotkey;
use super::sound;
use super::utils;
use anyhow::{anyhow, Context, Result};
use clap::{crate_authors, crate_description, crate_version, App, Arg};
use log::{error, info, trace, warn};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::collections::hash_map::HashMap;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use ulid::Ulid;

mod helpers;

use helpers::*;

type GlobalAppConfig = Lazy<parking_lot::RwLock<std::sync::Arc<AppConfig>>>;

static GLOBAL_APP_CONFIG: GlobalAppConfig = Lazy::new(|| {
    let app_config = load_and_merge_app_config().expect("failed to load and merge app config");
    parking_lot::RwLock::new(std::sync::Arc::new(app_config))
});

/// AppConfig
///
/// to preserve changes you need to call save_app_config_to_disk
#[derive(Debug, Deserialize, Default, Clone, Serialize)]
pub struct AppConfig {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub loopback_device: Option<String>,
    pub stop_hotkey: Option<String>,
    pub http_socket_addr: Option<String>,
    pub telegram_token: Option<String>,
    pub spotify_user: Option<String>,
    pub spotify_pass: Option<String>,

    pub print_possible_devices: Option<bool>,

    #[serde(default = "option_true")]
    pub http_server: Option<bool>,
    #[serde(default = "option_true")]
    pub tui: Option<bool>,

    pub gui: Option<bool>,
    pub auto_loop_device: Option<bool>,
    #[serde(default = "option_true")]
    pub stream_input_to_loop: Option<bool>,
    #[serde(default = "option_true")]
    pub simultaneous_playback: Option<bool>,
    /// use embed web ui html,js,css resources
    #[serde(default = "option_true")]
    pub embed_web: Option<bool>,
}

fn option_true() -> Option<bool> {
    Some(true)
}

/// Returns the global app config
///
/// Lazily initialized and merged from command line args, enviroment args and if existing a config file
pub fn get_app_config() -> std::sync::Arc<AppConfig> {
    GLOBAL_APP_CONFIG.read().clone()
}

/// Reload the app config from a possibly changed config file
pub fn reload_app_config_from_disk() -> Result<()> {
    *GLOBAL_APP_CONFIG.write() = std::sync::Arc::new(load_and_merge_app_config()?);
    Ok(())
}

fn save_app_config_to_disk(config: &AppConfig) -> Result<()> {
    save_app_config(config)?;
    *GLOBAL_APP_CONFIG.write() = std::sync::Arc::new(config.clone());
    Ok(())
}

/// Merges the file config with command line args and enviroment args
fn load_and_merge_app_config() -> Result<AppConfig> {
    let mut config = load_and_parse_app_config()?;
    let arguments = parse_arguments();

    merge_option_with_args_and_env(&mut config.input_device, &arguments, "input-device");
    merge_option_with_args_and_env(&mut config.output_device, &arguments, "output-device");
    merge_option_with_args_and_env(&mut config.loopback_device, &arguments, "loopback-device");
    merge_option_with_args_and_env(&mut config.stop_hotkey, &arguments, "stop-hotkey");
    merge_option_with_args_and_env(&mut config.http_socket_addr, &arguments, "http-socket-addr");
    merge_option_with_args_and_env(&mut config.telegram_token, &arguments, "telegram-token");
    merge_option_with_args_and_env(&mut config.spotify_user, &arguments, "spotify-user");
    merge_option_with_args_and_env(&mut config.spotify_pass, &arguments, "spotify-pass");

    merge_bool_option_with_args_and_env(
        &mut config.auto_loop_device,
        &arguments,
        "auto-loop-device",
    )?;
    merge_bool_option_with_args_and_env(&mut config.http_server, &arguments, "http-server")?;
    merge_bool_option_with_args_and_env(&mut config.tui, &arguments, "tui")?;
    merge_bool_option_with_args_and_env(&mut config.gui, &arguments, "gui")?;
    merge_bool_option_with_args_and_env(&mut config.embed_web, &arguments, "embed-web")?;
    merge_bool_option_with_args_and_env(
        &mut config.stream_input_to_loop,
        &arguments,
        "stream-input-to-loop",
    )?;
    merge_bool_option_with_args_and_env(
        &mut config.simultaneous_playback,
        &arguments,
        "simultaneous-playback",
    )?;

    merge_flag_with_args_and_env(
        &mut config.print_possible_devices,
        &arguments,
        "print-possible-devices",
    );

    Ok(config)
}

/// Finds the config file and try to parse it
fn load_and_parse_app_config() -> Result<AppConfig> {
    let config_path = get_config_file_path().context("Failed to get config file path")?;

    let toml_config: AppConfig = {
        if let Some(config_path) = config_path.as_ref() {
            let toml_str = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read_to_string {}", config_path.display()))?;
            toml::from_str(&toml_str)
                .with_context(|| format!("Failed to parse {}", config_path.display()))?
        } else {
            AppConfig::default()
        }
    };

    info!("Loaded config file from {:?}", config_path);
    Ok(toml_config)
}

fn save_app_config(config: &AppConfig) -> Result<()> {
    let config_path = get_config_file_path().context("Failed to get config file path")?;

    let config_path = {
        if let Some(config_path) = config_path {
            config_path
        } else {
            return Err(anyhow!("no existing config file path"));
        }
    };

    let pretty_string = toml::to_string_pretty(&config)?;
    fs::write(&config_path, pretty_string)?;
    info!("Saved config file at {:?}", config_path.display());
    Ok(())
}

fn parse_arguments() -> clap::ArgMatches {
    let matches = App::new("soundboard")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
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
            Arg::with_name("spotify-user")
                .long("spotify-user")
                .about("Sets the spotify user name to use spotify as source")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("spotify-pass")
                .long("spotify-pass")
                .about("Sets the spotify passowrd to use spotify as source")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("stop-hotkey")
                .long("stop-hotkey")
                .about("Sets the stop hotkey to stop all sounds")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("print-possible-devices")
                .short('P')
                .long("print-possible-devices")
                .about("Print possible devices"),
        )
        .arg(
            Arg::with_name("simultaneous-playback")
                .long("simultaneous-playback")
                .takes_value(true)
                .possible_values(&["true", "false"])
                .about("Enable/disable simultaneous-playback of sounds"),
        )
        .arg(
            Arg::with_name("stream-input-to-loop")
                .long("stream-input-to-loop")
                .takes_value(true)
                .possible_values(&["true", "false"])
                .about("Enable/disable to stream audio from input device to loopback device"),
        );

    #[cfg(feature = "autoloop")]
    let matches = matches.arg(
        Arg::with_name("auto-loop-device")
            .short('A')
            .long("auto-loop-device")
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable the automatic creation of a PulseAudio loopback device"),
    );

    #[cfg(feature = "gui")]
    let matches = matches.arg(
        Arg::with_name("gui")
            .long("gui")
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable the graphical user interface"),
    );

    #[cfg(feature = "textui")]
    let matches = matches.arg(
        Arg::with_name("tui")
            .long("tui")
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable the text user interface"),
    );
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("http-server")
            .long("http-server")
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable the http server api and web user interface"),
    );
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("http-socket-addr")
            .long("http-socket-addr")
            .about("Specify the socket addr for http server")
            .takes_value(true),
    );
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("embed-web")
            .long("embed-web")
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable the usage of the embed web ui resource files."),
    );
    #[cfg(feature = "telegram-bot")]
    let matches = matches.arg(
        Arg::with_name("telegram-token")
            .long("telegram-token")
            .about("Set the telegram token for the telegram bot")
            .takes_value(true),
    );
    matches.get_matches()
}
