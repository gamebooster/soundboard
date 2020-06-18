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
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Deserialize, Default, Clone, Serialize)]
pub struct MainConfig {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub loopback_device: Option<String>,
    pub stop_hotkey: Option<String>,
    pub http_server: Option<bool>,
    pub no_gui: Option<bool>,
    #[serde(rename = "soundboard")]
    pub soundboards: Option<Vec<SoundboardConfig>>,
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq)]
pub struct SoundboardConfig {
    pub name: Option<String>,
    pub hotkey: Option<String>,
    pub path: Option<String>,
    #[serde(rename = "sound")]
    pub sounds: Option<Vec<SoundConfig>>,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct SoundConfig {
    pub name: String,
    pub path: String,
    pub hotkey: Option<String>,
    #[serde(rename = "header")]
    pub headers: Option<Vec<HeaderConfig>>,
}

impl PartialEq for SoundConfig {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.headers == other.headers
    }
}
impl Eq for SoundConfig {}

impl std::hash::Hash for SoundConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.headers.hash(state);
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Hash, Default, Eq)]
pub struct HeaderConfig {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Hash, Eq)]
pub struct Hotkey {
    pub modifier: Vec<Modifier>,
    pub key: Key,
}

impl Hotkey {
    pub fn modifier_as_flag(&self) -> u32 {
        self.modifier.iter().fold(0, |acc, x| acc | (*x as u32)) as u32
    }
}

#[derive(
    Debug, Deserialize, Copy, Clone, Serialize, strum_macros::EnumString, PartialEq, Hash, Eq,
)]
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
#[derive(
    Debug, Deserialize, Copy, Clone, Serialize, strum_macros::EnumString, PartialEq, Hash, Eq,
)]
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

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let modifier_string = self
            .modifier
            .clone()
            .into_iter()
            .fold(String::new(), |all, one| {
                if !all.is_empty() {
                    format!("{}-{}", all, one)
                } else {
                    one.to_string()
                }
            });
        let hotkey_string = {
            if !modifier_string.is_empty() {
                format!("{}-{}", modifier_string, self.key.to_string())
            } else {
                self.key.to_string()
            }
        };
        write!(f, "{}", hotkey_string)
    }
}

pub fn parse_hotkey(hotkey_string: &str) -> Result<Hotkey> {
    let re = regex::Regex::new(
        r"^(?i)(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(\w+)$",
    )?;
    let caps: regex::Captures = re
        .captures(hotkey_string)
        .ok_or_else(|| anyhow!("No valid hotkey match"))?;
    let mut modifier = Vec::new();
    let mut key: Option<Key> = None;
    for caps in caps.iter().skip(1) {
        if let Some(caps) = caps {
            let mut mat = caps.as_str().to_uppercase();
            if mat.parse::<usize>().is_ok() {
                mat = format!("KEY_{}", mat);
            }
            if let Ok(res) = Modifier::from_str(&mat) {
                modifier.push(res);
                continue;
            }
            if key.is_some() {
                return Err(anyhow!("hotkey has alread a key specified"));
            }
            if let Ok(res) = Key::from_str(&mat) {
                key = Some(res);
            }
        }
    }
    if key.is_none() {
        return Err(anyhow!("hotkey has no key specified"));
    }
    Ok(Hotkey {
        modifier,
        key: key.unwrap(),
    })
}

pub fn load_and_parse_config(name: &str) -> Result<MainConfig> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.push(name);
    let toml_str = fs::read_to_string(&path)?;
    let mut toml_config: MainConfig = toml::from_str(&toml_str)?;

    toml_config.soundboards = Some(toml_config.soundboards.unwrap_or_default());

    for soundboard in toml_config.soundboards.as_mut().unwrap() {
        if soundboard.path.is_none() {
            continue;
        }
        let mut path = std::env::current_exe()?;
        path.pop();
        path.push(soundboard.path.as_ref().unwrap());
        let soundboard_str = fs::read_to_string(&path)?;
        let soundboard_config: SoundboardConfig = toml::from_str(&soundboard_str)?;
        if soundboard_config.sounds.is_none() {
            return Err(anyhow!("expected sounds in {}", path.to_str().unwrap()));
        }
        let mut sounds = soundboard.sounds.clone().unwrap_or_default();
        sounds.append(&mut soundboard_config.sounds.unwrap());
        soundboard.sounds = Some(sounds);
    }

    let mut soundboards_path = std::env::current_exe()?;
    soundboards_path.pop();
    soundboards_path.push("soundboards");

    for entry in std::fs::read_dir(&soundboards_path)? {
        if entry.is_err() {
            continue;
        }
        let path = entry.unwrap().path();
        let extension: &str = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        if extension == "toml" {
            let toml_str = fs::read_to_string(&path)?;
            let mut soundboard_config: SoundboardConfig = toml::from_str(&toml_str)?;
            if soundboard_config.sounds.is_none() {
                return Err(anyhow!("expected sounds in {}", path.to_str().unwrap()));
            }
            let mut sounds = soundboard_config.sounds.unwrap();
            for sound in &mut sounds {
                let relative_path = Path::new(&sound.path);
                if relative_path.is_absolute() || sound.path.starts_with("http") {
                    continue;
                }
                let mut new_path = soundboards_path.clone();
                new_path.push(relative_path);
                sound.path = new_path.to_str().unwrap().to_string();
            }
            soundboard_config.sounds = Some(sounds);
            toml_config
                .soundboards
                .as_mut()
                .unwrap()
                .push(soundboard_config);
        }
    }

    if toml_config.soundboards.as_ref().unwrap().is_empty() {
        return Err(anyhow!(
            "could not find any soundboards in {:?} or {:?}",
            path,
            soundboards_path
        ));
    }

    info!("Loaded config file from {}", path.display());
    Ok(toml_config)
}

#[allow(dead_code)]
pub fn save_config(config: &MainConfig, name: &str) -> Result<()> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.push(name);

    let pretty_string = toml::to_string_pretty(&config)?;
    fs::write(&path, pretty_string)?;
    info!("Saved config file at {}", &path.display());
    Ok(())
}

pub fn parse_arguments() -> clap::ArgMatches {
    App::new("soundboard")
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
        .arg(
            Arg::with_name("http-server")
                .long("http-server")
                .about("Enable http server API and web app"),
        )
        .get_matches()
}

pub fn parse_devices(
    config: &MainConfig,
    arguments: &clap::ArgMatches,
) -> Result<(Option<String>, Option<String>, String)> {
    let input_device_index: Option<String> = {
        if arguments.is_present("input-device") {
            Some(arguments.value_of("input-device").unwrap().to_string())
        } else if config.input_device.is_some() {
            config.input_device.clone()
        } else {
            None
        }
    };
    let output_device_index: Option<String> = {
        if arguments.is_present("output-device") {
            Some(arguments.value_of("output-device").unwrap().to_string())
        } else if config.output_device.is_some() {
            config.output_device.clone()
        } else {
            None
        }
    };

    let loop_device_index: String = {
        if arguments.is_present("loopback-device") {
            arguments.value_of("loopback-device").unwrap().to_string()
        } else if config.loopback_device.is_some() {
            config.loopback_device.as_ref().unwrap().clone()
        } else {
            return Err(anyhow!(
                "No loopback device specified in config or on command line"
            ));
        }
    };

    Ok((input_device_index, output_device_index, loop_device_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_parse() {
        assert_eq!(
            parse_hotkey("CTRL-P").unwrap(),
            Hotkey {
                modifier: vec![Modifier::CTRL],
                key: Key::P
            }
        );
        assert_eq!(
            parse_hotkey("CTRL-SHIFT-P").unwrap(),
            Hotkey {
                modifier: vec![Modifier::CTRL, Modifier::SHIFT],
                key: Key::P
            }
        );
        assert_eq!(
            parse_hotkey("S").unwrap(),
            Hotkey {
                modifier: vec![],
                key: Key::S
            }
        );
        assert_eq!(
            parse_hotkey("ALT-BACKSPACE").unwrap(),
            Hotkey {
                modifier: vec![Modifier::ALT],
                key: Key::BACKSPACE
            }
        );
        assert_eq!(
            parse_hotkey("SHIFT-SUPER-A").unwrap(),
            Hotkey {
                modifier: vec![Modifier::SHIFT, Modifier::SUPER],
                key: Key::A
            }
        );
        assert_eq!(
            parse_hotkey("SUPER-ARROW_RIGHT").unwrap(),
            Hotkey {
                modifier: vec![Modifier::SUPER],
                key: Key::ARROW_RIGHT
            }
        );
        assert_eq!(
            parse_hotkey("SUPER-CTRL-SHIFT-ALT-9").unwrap(),
            Hotkey {
                modifier: vec![
                    Modifier::SUPER,
                    Modifier::CTRL,
                    Modifier::SHIFT,
                    Modifier::ALT
                ],
                key: Key::KEY_9
            }
        );
        assert_eq!(
            parse_hotkey("super-ctrl-SHIFT-alt-ARROW_Up").unwrap(),
            Hotkey {
                modifier: vec![
                    Modifier::SUPER,
                    Modifier::CTRL,
                    Modifier::SHIFT,
                    Modifier::ALT
                ],
                key: Key::ARROW_UP
            }
        );

        assert_eq!(
            parse_hotkey("5").unwrap(),
            Hotkey {
                modifier: vec![],
                key: Key::KEY_5
            }
        );

        assert_eq!(
            parse_hotkey("KEY_5").unwrap(),
            Hotkey {
                modifier: vec![],
                key: Key::KEY_5
            }
        );

        assert_eq!(
            parse_hotkey("5-5").unwrap_err().to_string(),
            "No valid hotkey match"
        );

        assert_eq!(
            parse_hotkey("CTRL-").unwrap_err().to_string(),
            "No valid hotkey match"
        );

        assert_eq!(
            parse_hotkey("").unwrap_err().to_string(),
            "No valid hotkey match"
        );
    }
}
