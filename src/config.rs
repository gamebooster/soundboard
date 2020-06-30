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
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use super::sound;
use super::utils;

#[derive(Debug, Deserialize, Default, Clone, Serialize)]
pub struct MainConfig {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub loopback_device: Option<String>,
    pub stop_hotkey: Option<String>,
    pub http_server: Option<bool>,
    pub telegram: Option<bool>,
    pub no_gui: Option<bool>,
    pub auto_loop_device: Option<bool>,
    #[serde(skip_serializing, skip_deserializing)]
    pub soundboards: Vec<SoundboardConfig>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct SoundboardConfig {
    pub name: String,
    pub hotkey: Option<String>,
    pub position: Option<usize>,
    #[serde(rename = "sound")]
    pub sounds: Option<Vec<SoundConfig>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub last_hash: u64,
}

impl PartialEq for SoundboardConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.hotkey == other.hotkey
            && self.position == other.position
            && self.sounds == other.sounds
    }
}
impl Eq for SoundboardConfig {}

impl Hash for SoundboardConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.hotkey.hash(state);
        self.position.hash(state);
        self.sounds.hash(state);
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct SoundConfig {
    pub name: String,
    pub path: String,
    pub hotkey: Option<String>,
    #[serde(rename = "header")]
    pub headers: Option<Vec<HeaderConfig>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub full_path: String,
}

impl PartialEq for SoundConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.hotkey == other.hotkey
            && self.path == other.path
            && self.headers == other.headers
    }
}
impl Eq for SoundConfig {}

impl Hash for SoundConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.path.hash(state);
        self.hotkey.hash(state);
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
#[repr(u32)]
pub enum Modifier {
    ALT = hotkey::modifiers::ALT,
    CTRL = hotkey::modifiers::CONTROL,
    SHIFT = hotkey::modifiers::SHIFT,
    SUPER = hotkey::modifiers::SUPER,
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[allow(non_camel_case_types)]
#[derive(
    Debug, Deserialize, Copy, Clone, Serialize, strum_macros::EnumString, PartialEq, Hash, Eq,
)]
#[repr(u32)]
pub enum Key {
    BACKSPACE = hotkey::keys::BACKSPACE,
    TAB = hotkey::keys::TAB,
    ENTER = hotkey::keys::ENTER,
    CAPS_LOCK = hotkey::keys::CAPS_LOCK,
    ESCAPE = hotkey::keys::ESCAPE,
    SPACEBAR = hotkey::keys::SPACEBAR,
    PAGE_UP = hotkey::keys::PAGE_UP,
    PAGE_DOWN = hotkey::keys::PAGE_DOWN,
    END = hotkey::keys::END,
    HOME = hotkey::keys::HOME,
    ARROW_LEFT = hotkey::keys::ARROW_LEFT,
    ARROW_RIGHT = hotkey::keys::ARROW_RIGHT,
    ARROW_UP = hotkey::keys::ARROW_UP,
    ARROW_DOWN = hotkey::keys::ARROW_DOWN,
    PRINT_SCREEN = hotkey::keys::PRINT_SCREEN,
    INSERT = hotkey::keys::INSERT,
    DELETE = hotkey::keys::DELETE,
    #[serde(rename = "0")]
    KEY_0 = hotkey::keys::KEY_0,
    #[serde(rename = "1")]
    KEY_1 = hotkey::keys::KEY_1,
    #[serde(rename = "2")]
    KEY_2 = hotkey::keys::KEY_2,
    #[serde(rename = "3")]
    KEY_3 = hotkey::keys::KEY_3,
    #[serde(rename = "4")]
    KEY_4 = hotkey::keys::KEY_4,
    #[serde(rename = "5")]
    KEY_5 = hotkey::keys::KEY_5,
    #[serde(rename = "6")]
    KEY_6 = hotkey::keys::KEY_6,
    #[serde(rename = "7")]
    KEY_7 = hotkey::keys::KEY_7,
    #[serde(rename = "8")]
    KEY_8 = hotkey::keys::KEY_8,
    #[serde(rename = "9")]
    KEY_9 = hotkey::keys::KEY_9,
    A = hotkey::keys::A,
    B = hotkey::keys::B,
    C = hotkey::keys::C,
    D = hotkey::keys::D,
    E = hotkey::keys::E,
    F = hotkey::keys::F,
    G = hotkey::keys::G,
    H = hotkey::keys::H,
    I = hotkey::keys::I,
    J = hotkey::keys::J,
    K = hotkey::keys::K,
    L = hotkey::keys::L,
    M = hotkey::keys::M,
    N = hotkey::keys::N,
    O = hotkey::keys::O,
    P = hotkey::keys::P,
    Q = hotkey::keys::Q,
    R = hotkey::keys::R,
    S = hotkey::keys::S,
    T = hotkey::keys::T,
    V = hotkey::keys::V,
    X = hotkey::keys::X,
    Y = hotkey::keys::Y,
    Z = hotkey::keys::Z,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
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

use once_cell::sync::Lazy;
static REGEX_HOTKEY_PATTERN: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(
        r"^(?i)(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(?:(CTRL|SHIFT|ALT|SUPER)-){0,1}(\w+)$",
    ).unwrap()
});

pub fn parse_hotkey(hotkey_string: &str) -> Result<Hotkey> {
    let caps: regex::Captures = REGEX_HOTKEY_PATTERN
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

fn get_config_file_path() -> Result<Option<PathBuf>> {
    let mut relative_from_exe = std::env::current_exe()?;
    relative_from_exe.pop();
    relative_from_exe.push("soundboard.toml");
    if relative_from_exe.is_file() {
        return Ok(Some(relative_from_exe));
    }
    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("soundboard");
        config_path.push("soundboard.toml");
        if config_path.is_file() {
            return Ok(Some(config_path));
        }
    }
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".config");
        config_path.push("soundboard");
        config_path.push("soundboard.toml");
        if config_path.is_file() {
            return Ok(Some(config_path));
        }
    }
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".soundboard");
        config_path.push("soundboard.toml");
        if config_path.is_file() {
            return Ok(Some(config_path));
        }
    }
    Ok(None)
}

fn get_soundboards_path() -> Result<PathBuf> {
    let mut relative_from_exe = std::env::current_exe()?;
    relative_from_exe.pop();
    relative_from_exe.push("soundboards");
    if relative_from_exe.is_dir() {
        return Ok(relative_from_exe);
    }
    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("soundboard");
        config_path.push("soundboards");
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".config");
        config_path.push("soundboard");
        config_path.push("soundboards");
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".soundboard");
        config_path.push("soundboards");
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    Err(anyhow!("could not find soundboards directory"))
}

pub fn load_and_parse_config() -> Result<MainConfig> {
    let config_path = get_config_file_path()?;

    let mut toml_config: MainConfig = {
        if let Some(config_path) = config_path.as_ref() {
            let toml_str = fs::read_to_string(&config_path)?;
            toml::from_str(&toml_str)?
        } else {
            MainConfig::default()
        }
    };

    let soundboards_path = get_soundboards_path()?;

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
            let sb_config = load_soundboard_config(&path)?;
            toml_config.soundboards.push(sb_config);
        }
    }

    if toml_config.soundboards.is_empty() {
        return Err(anyhow!(
            "could not find any soundboards in {:?}",
            soundboards_path
        ));
    }

    toml_config
        .soundboards
        .sort_by(|a, b| match (a.position, b.position) {
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            _ => a.position.cmp(&b.position),
        });

    info!("Loaded config file from {:?}", config_path);
    info!("Loaded soundboards from {}", soundboards_path.display());
    Ok(toml_config)
}

fn resolve_sound_path(soundboard_path: &Path, sound_path: &str) -> Result<String> {
    let relative_path = Path::new(sound_path);
    if relative_path.is_absolute() || sound_path.starts_with("http") {
        return Ok(sound_path.to_string());
    }
    let mut new_path = get_soundboards_path()?;
    let stem: &str = soundboard_path
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    new_path.push(stem);
    new_path.push(relative_path);

    if !new_path.exists() || !new_path.is_file() {
        return Err(anyhow!(
            "expected sound file at {}",
            new_path.to_str().unwrap()
        ));
    }

    Ok(new_path.to_str().unwrap().to_string())
}

pub fn load_soundboard_config(soundboard_path: &Path) -> Result<SoundboardConfig> {
    let toml_str = fs::read_to_string(&soundboard_path)?;
    let mut soundboard_config: SoundboardConfig = toml::from_str(&toml_str)?;
    if soundboard_config.sounds.is_none() {
        return Err(anyhow!(
            "expected sounds in {}",
            soundboard_path.to_str().unwrap()
        ));
    }
    soundboard_config.last_hash = utils::calculate_hash(&soundboard_config);

    let mut sounds = soundboard_config.sounds.unwrap();
    for sound in &mut sounds {
        sound.full_path = resolve_sound_path(soundboard_path, &sound.path)?;
    }
    soundboard_config.sounds = Some(sounds);
    soundboard_config.path = soundboard_path
        .as_os_str()
        .to_os_string()
        .into_string()
        .unwrap();
    Ok(soundboard_config)
}

#[allow(dead_code)]
pub fn save_config(config: &MainConfig, name: &str) -> Result<()> {
    let config_path = PathBuf::from_str(name)?;

    let pretty_string = toml::to_string_pretty(&config)?;
    fs::write(&config_path, pretty_string)?;
    info!("Saved config file at {}", &config_path.display());
    Ok(())
}

fn check_soundboard_config_mutated_on_disk(
    old_path: &Path,
    new_config: &SoundboardConfig,
) -> Result<bool> {
    let toml_str = fs::read_to_string(&old_path)?;
    let soundboard_config: SoundboardConfig = toml::from_str(&toml_str)?;

    let old_config_hash = utils::calculate_hash(&soundboard_config);

    if old_config_hash == new_config.last_hash {
        return Ok(false);
    }

    Ok(true)
}

pub fn save_soundboard_config(config: &mut SoundboardConfig) -> Result<()> {
    let soundboard_config_path = PathBuf::from_str(&config.path)?;

    if config.sounds.is_none() {
        return Err(anyhow!("save_soundboard: expected sounds",));
    }

    if check_soundboard_config_mutated_on_disk(&soundboard_config_path, config)? {
        return Err(anyhow!(
            "save_soundboard: soundboard config file mutated on disk",
        ));
    }

    for sound in config.sounds.as_ref().unwrap() {
        resolve_sound_path(&soundboard_config_path, &sound.path)?;
    }

    let pretty_string = toml::to_string_pretty(&config)?;
    fs::write(&soundboard_config_path, pretty_string)?;
    config.last_hash = utils::calculate_hash(&config);
    info!("Saved config file at {}", &config.path);
    Ok(())
}

pub fn is_flag_set(args: &clap::ArgMatches, config_option: &Option<bool>, name: &str) -> bool {
    if args.is_present(name)
        || config_option.unwrap_or_default()
        || std::env::var("SB".to_owned() + &name.to_ascii_uppercase().replace("-", "_")).is_ok()
    {
        return true;
    }

    false
}

pub fn parse_arguments() -> clap::ArgMatches {
    let matches = App::new("soundboard")
        .version(crate_version!())
        .author(crate_authors!())
        .about("play sounds over your microphone")
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
        );
    #[cfg(feature = "autoloop")]
    let matches = matches.arg(
        Arg::with_name("auto-loop-device")
            .long("auto-loop-device")
            .about("Automatically create PulseAudio Loopback Device"),
    );
    #[cfg(feature = "gui")]
    let matches = matches.arg(Arg::with_name("no-gui").long("no-gui").about("Disable GUI"));
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("http-server")
            .long("http-server")
            .about("Enable http server API and web app"),
    );
    matches.get_matches()
}

pub fn parse_devices(
    config: &MainConfig,
    arguments: &clap::ArgMatches,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let input_device_id: Option<String> = {
        if arguments.is_present("input-device") {
            Some(arguments.value_of("input-device").unwrap().to_string())
        } else if std::env::var("SB_INPUT_DEVICE").is_ok() {
            std::env::var("SB_INPUT_DEVICE").ok()
        } else if config.input_device.is_some() {
            config.input_device.clone()
        } else {
            None
        }
    };
    let output_device_id: Option<String> = {
        if arguments.is_present("output-device") {
            Some(arguments.value_of("output-device").unwrap().to_string())
        } else if std::env::var("SB_OUTPUT_DEVICE").is_ok() {
            std::env::var("SB_OUTPUT_DEVICE").ok()
        } else if config.output_device.is_some() {
            config.output_device.clone()
        } else {
            None
        }
    };

    let loop_device_id: Option<String> = {
        if arguments.is_present("loopback-device") {
            Some(arguments.value_of("loopback-device").unwrap().to_string())
        } else if std::env::var("SB_LOOPBACK_DEVICE").is_ok() {
            std::env::var("SB_LOOPBACK_DEVICE").ok()
        } else if config.loopback_device.is_some() {
            config.loopback_device.clone()
        } else {
            None //return Err(anyhow!(
                 //"No loopback device specified in config loopbackdevice or in env SB_LOOPBACK_DEVICE var on on command line --loopback-device"
                 //));
        }
    };

    Ok((input_device_id, output_device_id, loop_device_id))
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
