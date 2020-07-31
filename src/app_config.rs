#![allow(dead_code)]

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

fn option_true() -> Option<bool> {
    Some(true)
}

macro_rules! make_config {
    ( @ $name:ident { } -> ($($result:tt)*) ) => (
        /// AppConfig
        ///
        /// to preserve changes you need to call save_app_config_to_disk
        #[derive(Debug, serde::Serialize, serde::Deserialize, Default, Clone)]
        pub struct $name {
            $($result)*
        }
    );

    ( @ $name:ident { $param:ident : false, $($rest:tt)* } -> ($($result:tt)*) ) => (
        make_config!(@ $name { $($rest)* } -> (
            $($result)*
            #[serde(skip_serializing_if = "Option::is_none")]
            pub $param : Option<bool>,
        ));
    );

    ( @ $name:ident { $param:ident : true, $($rest:tt)* } -> ($($result:tt)*) ) => (
        make_config!(@ $name { $($rest)* } -> (
            $($result)*
            #[serde(skip_serializing_if = "Option::is_none")]
            #[serde(default = "option_true")]
            pub $param : Option<bool>,
        ));
    );

    ( @ $name:ident { $param:ident : String, $($rest:tt)* } -> ($($result:tt)*) ) => (
        make_config!(@ $name { $($rest)* } -> (
            $($result)*
            #[serde(skip_serializing_if = "Option::is_none")]
            pub $param : Option<String>,
        ));
    );

    ( @ $name:ident { $param:ident : String $default:literal, $($rest:tt)* } -> ($($result:tt)*) ) => (
        make_config!(@ $name { $($rest)* } -> (
            $($result)*
            #[serde(skip_serializing_if = "Option::is_none")]
            #[serde(default = $default)]
            pub $param : Option<String>,
        ));
    );

    ( $name:ident { $( $param:ident : $type:ident $($default:literal),*),* $(,)* } ) => (
        make_config!(@ $name { $($param : $type $($default),*,)* } -> ());
    );
}

fn default_stop_hotkey() -> Option<String> {
    Some("CTRL-ALT-E".to_owned())
}

fn default_http_socket_addr() -> Option<String> {
    Some("127.0.0.1:8080".to_owned())
}

make_config!(AppConfig {
    input_device : String,
    output_device : String,
    loopback_device: String,
    stop_hotkey: String "default_stop_hotkey",
    http_socket_addr: String "default_http_socket_addr",
    spotify_user: String,
    spotify_pass: String,

    print_possible_devices: false,

    telegram_token: String, // enables telegram bot if present
    http_server: true,
    tui: false,
    gui: false,

    stream_input_to_loop: true,
    simultaneous_playback: true,
    auto_loop_device: false,
    embed_web: true,
});

/// Returns the global app config
///
/// Lazily initialized and merged from command line args, enviroment args and if existing a config file
pub fn get_app_config() -> std::sync::Arc<AppConfig> {
    GLOBAL_APP_CONFIG.read().clone()
}

pub fn set_stream_input_to_loop_option(option: Option<bool>) {
    let mut config = (*get_app_config()).clone();
    config.stream_input_to_loop = option;
    *GLOBAL_APP_CONFIG.write() = std::sync::Arc::new(config);
}

/// Reload the app config from a possibly changed config file
fn reload_app_config_from_disk() -> Result<()> {
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
    let arguments = parse_arguments(&config);

    macro_rules! merge_option_with_args_and_env {
        ($name:ident) => {
            merge_option_with_args_and_env(
                &mut config.$name,
                &arguments,
                stringify!($name).replace("_", "-").as_str(),
            );
        };
    }

    merge_option_with_args_and_env!(input_device);
    merge_option_with_args_and_env!(output_device);
    merge_option_with_args_and_env!(loopback_device);
    merge_option_with_args_and_env!(stop_hotkey);
    merge_option_with_args_and_env!(http_socket_addr);
    merge_option_with_args_and_env!(telegram_token);
    merge_option_with_args_and_env!(spotify_user);
    merge_option_with_args_and_env!(spotify_pass);

    macro_rules! merge_bool_option_with_args_and_env {
        ($name:ident) => {
            merge_bool_option_with_args_and_env(
                &mut config.$name,
                &arguments,
                stringify!($name).replace("_", "-").as_str(),
            )?;
        };
    }

    merge_bool_option_with_args_and_env!(auto_loop_device);
    merge_bool_option_with_args_and_env!(http_server);
    merge_bool_option_with_args_and_env!(tui);
    merge_bool_option_with_args_and_env!(gui);
    merge_bool_option_with_args_and_env!(embed_web);
    merge_bool_option_with_args_and_env!(stream_input_to_loop);
    merge_bool_option_with_args_and_env!(simultaneous_playback);

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

fn parse_arguments(config: &AppConfig) -> clap::ArgMatches {
    let mut matches = App::new("soundboard")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!());

    macro_rules! add_arg {
        ($name:ident) => {
            let command_string = stringify!($name).replace("_", "-");
            let default_string = format!("{:?}", config.$name.clone().unwrap_or_default());
            let $name = Arg::with_name(&command_string)
                .long(&command_string)
                .takes_value(true)
                .default_value(&default_string);
        };
    }

    add_arg!(input_device);
    add_arg!(output_device);
    add_arg!(loopback_device);
    add_arg!(spotify_user);
    add_arg!(spotify_pass);
    add_arg!(stop_hotkey);
    add_arg!(print_possible_devices);
    add_arg!(simultaneous_playback);
    add_arg!(stream_input_to_loop);

    matches = matches.args(&[
        input_device
            .short('i')
            .about("Sets the input device to use"),
        output_device
            .short('o')
            .about("Sets the output device to use"),
        loopback_device
            .short('l')
            .about("Sets the loopback device to use"),
        spotify_user.about("Sets the spotify user name to use spotify as source"),
        spotify_pass.about("Sets the spotify passowrd to use spotify as source"),
        stop_hotkey.about("Sets the stop hotkey to stop all sounds"),
        print_possible_devices
            .short('P')
            .about("Print possible devices")
            .takes_value(false),
        simultaneous_playback
            .possible_values(&["true", "false"])
            .about("Enable/disable simultaneous-playback of sounds"),
        stream_input_to_loop
            .takes_value(true)
            .possible_values(&["true", "false"])
            .about("Enable/disable to stream audio from input device to loopback device"),
    ]);

    #[cfg(feature = "autoloop")]
    add_arg!(auto_loop_device);
    #[cfg(feature = "autoloop")]
    {
        matches = matches.arg(
            auto_loop_device
                .short('A')
                .possible_values(&["true", "false"])
                .about("Enable/disable the automatic creation of a PulseAudio loopback device"),
        );
    }

    #[cfg(feature = "gui")]
    add_arg!(gui);
    #[cfg(feature = "gui")]
    {
        matches = matches.arg(
            gui.possible_values(&["true", "false"])
                .about("Enable/disable the graphical user interface"),
        );
    }

    #[cfg(feature = "textui")]
    add_arg!(tui);
    #[cfg(feature = "textui")]
    {
        matches = matches.arg(
            tui.possible_values(&["true", "false"])
                .about("Enable/disable the text user interface"),
        );
    }
    #[cfg(feature = "http")]
    add_arg!(http_server);
    #[cfg(feature = "http")]
    add_arg!(http_socket_addr);
    #[cfg(feature = "http")]
    add_arg!(embed_web);
    #[cfg(feature = "http")]
    {
        matches = matches.args(&[
            http_server
                .possible_values(&["true", "false"])
                .about("Enable/disable the http server api and web user interface"),
            http_socket_addr.about("Specify the socket addr for http server"),
            embed_web
                .possible_values(&["true", "false"])
                .about("Enable/disable the usage of the embed web ui resource files."),
        ]);
    }
    #[cfg(feature = "telegram-bot")]
    add_arg!(telegram_token);
    #[cfg(feature = "telegram-bot")]
    {
        matches = matches.arg(
            telegram_token
                .about("Set the telegram token for the telegram bot")
                .takes_value(true),
        );
    }
    matches.get_matches()
}
