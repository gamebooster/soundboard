extern crate anyhow;
extern crate clap;
extern crate hotkey;
extern crate log;
extern crate serde;
extern crate toml;

use anyhow::{anyhow, Context, Result};
use clap::{crate_authors, crate_version, App, Arg};
use log::{error, info, trace, warn};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::fmt;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    pub input_device: Option<usize>,
    pub output_device: Option<usize>,
    pub loopback_device: Option<usize>,
    pub sounds: Option<Vec<SoundConfig>>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum Modifier {
    ALT = hotkey::modifiers::ALT as isize,
    CTRL = hotkey::modifiers::CONTROL as isize,
    SHIFT = hotkey::modifiers::SHIFT as isize,
    SUPER = hotkey::modifiers::SUPER as isize,
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Deserialize, Copy, Clone)]
pub enum Key {
    BACKSPACE = hotkey::keys::BACKSPACE as isize,
    TAB = hotkey::keys::TAB as isize,
    ENTER = hotkey::keys::ENTER as isize,
    CAPS_LOCK = hotkey::keys::CAPS_LOCK as isize,
    ESCAPE = hotkey::keys::ESCAPE as isize,
    SPACEBAR = hotkey::keys::SPACEBAR as isize,
    PAGE_UP = hotkey::keys::PAGE_UP as isize,
    PAGE_DOWN = hotkey::keys::PAGE_DOWN as isize,
    END = hotkey::keys::END as isize,
    HOME = hotkey::keys::HOME as isize,
    ARROW_LEFT = hotkey::keys::ARROW_LEFT as isize,
    ARROW_RIGHT = hotkey::keys::ARROW_RIGHT as isize,
    ARROW_UP = hotkey::keys::ARROW_UP as isize,
    ARROW_DOWN = hotkey::keys::ARROW_DOWN as isize,
    PRINT_SCREEN = hotkey::keys::PRINT_SCREEN as isize,
    INSERT = hotkey::keys::INSERT as isize,
    DELETE = hotkey::keys::DELETE as isize,
    KEY_1 = '1' as isize,
    KEY_2 = '2' as isize,
    KEY_3 = '3' as isize,
    KEY_4 = '4' as isize,
    KEY_5 = '5' as isize,
    KEY_6 = '6' as isize,
    KEY_7 = '7' as isize,
    KEY_8 = '8' as isize,
    KEY_9 = '9' as isize,
    A = 'A' as isize,
    B = 'B' as isize,
    C = 'C' as isize,
    D = 'D' as isize,
    E = 'E' as isize,
    F = 'F' as isize,
    G = 'G' as isize,
    H = 'H' as isize,
    I = 'I' as isize,
    J = 'J' as isize,
    K = 'K' as isize,
    L = 'L' as isize,
    M = 'M' as isize,
    N = 'N' as isize,
    O = 'O' as isize,
    P = 'P' as isize,
    Q = 'Q' as isize,
    R = 'R' as isize,
    S = 'S' as isize,
    T = 'T' as isize,
    V = 'V' as isize,
    X = 'X' as isize,
    Y = 'Y' as isize,
    Z = 'Z' as isize,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SoundConfig {
    pub name: String,
    pub path: String,
    pub hotkey_modifier: Option<Vec<Modifier>>,
    pub hotkey_key: Option<Key>,
}

pub fn load_and_parse_config(name: &str) -> Result<Config> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.push(name);
    let toml_str = fs::read_to_string(&path)?;
    let toml_config = toml::from_str(&toml_str)?;
    info!("Loaded config file from {}", path.display());
    Ok(toml_config)
}

pub fn parse_arguments() -> clap::ArgMatches {
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

    matches
}

pub fn parse_devices(
    config: &Config,
    arguments: &clap::ArgMatches,
) -> Result<(Option<usize>, Option<usize>, usize)> {
    let input_device_index: Option<usize> = {
        if arguments.is_present("input-device") {
            Some(
                arguments
                    .value_of("input-device")
                    .expect("No input device specified")
                    .parse()
                    .expect("No number specified"),
            )
        } else if config.input_device.is_some() {
            config.input_device
        } else {
            None
        }
    };
    let output_device_index: Option<usize> = {
        if arguments.is_present("output-device") {
            Some(
                arguments
                    .value_of("output-device")
                    .expect("No ouput device specified")
                    .parse()
                    .expect("No number specified"),
            )
        } else if config.output_device.is_some() {
            config.output_device
        } else {
            None
        }
    };

    let loop_device_index: usize = {
        if arguments.is_present("loopback-device") {
            arguments
                .value_of("loopback-device")
                .expect("No loopback device specified")
                .parse()
                .expect("No number specified")
        } else if config.loopback_device.is_some() {
            config.loopback_device.unwrap()
        } else {
            return Err(anyhow!(
                "No loopback device specified in config or on command line"
            ));
        }
    };

    Ok((input_device_index, output_device_index, loop_device_index))
}
