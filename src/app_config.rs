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

    pub no_http_server: Option<bool>,
    pub telegram: Option<bool>,
    pub terminal_ui: Option<bool>,
    pub no_native_gui: Option<bool>,
    pub auto_loop_device: Option<bool>,
    pub no_duplex_device: Option<bool>,
    pub print_possible_devices: Option<bool>,
    pub disable_simultaneous_playback: Option<bool>,
    pub no_embed_web: Option<bool>,
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

    merge_flag_with_args_and_env(&mut config.auto_loop_device, &arguments, "auto-loop-device");
    merge_flag_with_args_and_env(&mut config.no_http_server, &arguments, "no-http-server");
    merge_flag_with_args_and_env(&mut config.telegram, &arguments, "telegram");
    merge_flag_with_args_and_env(&mut config.terminal_ui, &arguments, "terminal-ui");
    merge_flag_with_args_and_env(&mut config.no_native_gui, &arguments, "no-native-gui");
    merge_flag_with_args_and_env(&mut config.no_embed_web, &arguments, "no-embed-web");
    merge_flag_with_args_and_env(&mut config.no_duplex_device, &arguments, "no-duplex-device");
    merge_flag_with_args_and_env(
        &mut config.print_possible_devices,
        &arguments,
        "print-possible-devices",
    );
    merge_flag_with_args_and_env(
        &mut config.disable_simultaneous_playback,
        &arguments,
        "disable-simultaneous-playback",
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
            Arg::with_name("print-possible-devices")
                .short('P')
                .long("print-possible-devices")
                .about("Print possible devices"),
        )
        .arg(
            Arg::with_name("no-embed-web")
                .long("no-embed-web")
                .about("Do not use embed web ui files"),
        );

    #[cfg(feature = "autoloop")]
    let matches = matches.arg(
        Arg::with_name("auto-loop-device")
            .short('A')
            .long("auto-loop-device")
            .about("Automatically create PulseAudio Loopback Device"),
    );

    #[cfg(feature = "gui")]
    let matches = matches.arg(
        Arg::with_name("no-native-gui")
            .long("no-native-gui")
            .about("Disable native gui"),
    );

    #[cfg(feature = "terminal-ui")]
    let matches = matches.arg(
        Arg::with_name("terminal-ui")
            .long("terminal-ui")
            .about("Enable terminal-ui"),
    );
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("no-http-server")
            .long("no-http-server")
            .about("Disable http server api and web ui"),
    );
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("http-socket-addr")
            .long("http-socket-addr")
            .about("Specify the socket addr for http server")
            .takes_value(true),
    );
    matches.get_matches()
}
