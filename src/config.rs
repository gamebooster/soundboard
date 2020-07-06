extern crate anyhow;
extern crate clap;
extern crate hotkey_soundboard;
extern crate log;
extern crate serde;
extern crate toml;

use anyhow::{anyhow, Context, Result};
use clap::{crate_authors, crate_version, App, Arg};
use hotkey_soundboard::keys;
use hotkey_soundboard::modifiers;
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

use once_cell::sync::Lazy;

type GlobalConfig = Lazy<parking_lot::RwLock<std::sync::Arc<MainConfig>>>;

static GLOBAL_CONFIG: GlobalConfig = Lazy::new(|| {
    let config = load_and_merge_config().expect("failed to load and merge config");
    parking_lot::RwLock::new(std::sync::Arc::new(config))
});

#[derive(Debug, Deserialize, Default, Clone, Serialize)]
pub struct MainConfig {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub loopback_device: Option<String>,
    pub stop_hotkey: Option<String>,
    pub http_socket_addr: Option<String>,

    pub http_server: Option<bool>,
    pub telegram: Option<bool>,
    pub no_gui: Option<bool>,
    pub auto_loop_device: Option<bool>,
    pub print_possible_devices: Option<bool>,

    #[serde(skip_serializing, skip_deserializing)]
    pub soundboards: Vec<SoundboardConfig>,
}

fn load_and_merge_config() -> Result<MainConfig> {
    let mut config = load_and_parse_config()?;
    let arguments = parse_arguments();

    merge_option_with_args_and_env(&mut config.input_device, &arguments, "input-device");
    merge_option_with_args_and_env(&mut config.output_device, &arguments, "output-device");
    merge_option_with_args_and_env(&mut config.loopback_device, &arguments, "loopback-device");
    merge_option_with_args_and_env(&mut config.stop_hotkey, &arguments, "stop-hotkey");
    merge_option_with_args_and_env(&mut config.http_socket_addr, &arguments, "http-socket-addr");

    merge_flag_with_args_and_env(&mut config.auto_loop_device, &arguments, "auto-loop-device");
    merge_flag_with_args_and_env(&mut config.http_server, &arguments, "http-server");
    merge_flag_with_args_and_env(&mut config.telegram, &arguments, "telegram");
    merge_flag_with_args_and_env(&mut config.no_gui, &arguments, "no-gui");
    merge_flag_with_args_and_env(
        &mut config.print_possible_devices,
        &arguments,
        "print-possible-devices",
    );
    Ok(config)
}

impl MainConfig {
    pub fn read() -> std::sync::Arc<MainConfig> {
        GLOBAL_CONFIG.read().clone()
    }

    pub fn reload_from_disk() -> Result<()> {
        *GLOBAL_CONFIG.write() = std::sync::Arc::new(load_and_merge_config()?);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_soundboard(mut soundboard: SoundboardConfig) -> Result<()> {
        save_soundboard_config(&mut soundboard, true)?;
        let mut config: MainConfig = (*MainConfig::read()).clone();
        let mut writer = GLOBAL_CONFIG.write();
        config.soundboards.push(soundboard);
        *writer = std::sync::Arc::new(config);
        Ok(())
    }

    pub fn change_soundboard(index: usize, mut soundboard: SoundboardConfig) -> Result<()> {
        if MainConfig::read().soundboards.get(index).is_none() {
            return Err(anyhow!("invalid soundboard index"));
        }
        save_soundboard_config(&mut soundboard, false)?;
        let mut config: MainConfig = (*MainConfig::read()).clone();
        let mut writer = GLOBAL_CONFIG.write();
        config.soundboards[index] = soundboard;
        *writer = std::sync::Arc::new(config);
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct SoundboardConfig {
    pub name: String,
    pub hotkey: Option<String>,
    pub position: Option<usize>,
    pub disabled: Option<bool>,
    #[serde(rename = "sound")]
    pub sounds: Option<Vec<SoundConfig>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub path: String,
    #[serde(skip_serializing, skip_deserializing)]
    last_hash: u64,
}

impl PartialEq for SoundboardConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.hotkey == other.hotkey
            && self.position == other.position
            && self.sounds == other.sounds
            && self.disabled == other.disabled
    }
}
impl Eq for SoundboardConfig {}

impl Hash for SoundboardConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.hotkey.hash(state);
        self.position.hash(state);
        self.sounds.hash(state);
        self.disabled.hash(state);
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
    ALT = modifiers::ALT,
    CTRL = modifiers::CONTROL,
    SHIFT = modifiers::SHIFT,
    SUPER = modifiers::SUPER,
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
    BACKSPACE = keys::BACKSPACE,
    TAB = keys::TAB,
    ENTER = keys::ENTER,
    CAPS_LOCK = keys::CAPS_LOCK,
    ESCAPE = keys::ESCAPE,
    SPACEBAR = keys::SPACEBAR,
    PAGE_UP = keys::PAGE_UP,
    PAGE_DOWN = keys::PAGE_DOWN,
    END = keys::END,
    HOME = keys::HOME,
    ARROW_LEFT = keys::ARROW_LEFT,
    ARROW_RIGHT = keys::ARROW_RIGHT,
    ARROW_UP = keys::ARROW_UP,
    ARROW_DOWN = keys::ARROW_DOWN,
    PRINT_SCREEN = keys::PRINT_SCREEN,
    INSERT = keys::INSERT,
    DELETE = keys::DELETE,
    #[serde(rename = "0")]
    KEY_0 = keys::KEY_0,
    #[serde(rename = "1")]
    KEY_1 = keys::KEY_1,
    #[serde(rename = "2")]
    KEY_2 = keys::KEY_2,
    #[serde(rename = "3")]
    KEY_3 = keys::KEY_3,
    #[serde(rename = "4")]
    KEY_4 = keys::KEY_4,
    #[serde(rename = "5")]
    KEY_5 = keys::KEY_5,
    #[serde(rename = "6")]
    KEY_6 = keys::KEY_6,
    #[serde(rename = "7")]
    KEY_7 = keys::KEY_7,
    #[serde(rename = "8")]
    KEY_8 = keys::KEY_8,
    #[serde(rename = "9")]
    KEY_9 = keys::KEY_9,
    A = keys::A,
    B = keys::B,
    C = keys::C,
    D = keys::D,
    E = keys::E,
    F = keys::F,
    G = keys::G,
    H = keys::H,
    I = keys::I,
    J = keys::J,
    K = keys::K,
    L = keys::L,
    M = keys::M,
    N = keys::N,
    O = keys::O,
    P = keys::P,
    Q = keys::Q,
    R = keys::R,
    S = keys::S,
    T = keys::T,
    V = keys::V,
    X = keys::X,
    Y = keys::Y,
    Z = keys::Z,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let modifier_string: String = self.modifier.iter().fold(String::new(), |all, one| {
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

pub fn get_soundboards_path() -> Result<PathBuf> {
    let mut relative_from_exe = std::env::current_exe()?;
    relative_from_exe.pop();
    relative_from_exe.push("soundboards");
    if relative_from_exe.is_dir() {
        return Ok(relative_from_exe);
    }
    let mut config_dir_path1 = "$XDG_CONFIG_HOME/soundboard/soundboards/".to_owned();
    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("soundboard");
        config_path.push("soundboards");
        config_dir_path1 = config_path.to_str().unwrap().to_owned();
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    let mut config_dir_path2 = "$HOME/.config/soundboard/soundboards/".to_owned();
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".config");
        config_path.push("soundboard");
        config_path.push("soundboards");
        config_dir_path2 = config_path.to_str().unwrap().to_owned();
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    let mut home_dir_path = "$HOME/.soundboard/soundboards/".to_owned();
    if let Some(mut config_path) = dirs::home_dir() {
        config_path.push(".soundboard");
        config_path.push("soundboards");
        home_dir_path = config_path.to_str().unwrap().to_owned();
        if config_path.is_dir() {
            return Ok(config_path);
        }
    }
    Err(anyhow!(
        r"could not find soundboards directory at one of the following locations:
            relative_from_exe: {}
            config_dir_path1: {}
            config_dir_path2: {}
            home_dir_path: {}",
        relative_from_exe.display(),
        config_dir_path1,
        config_dir_path2,
        home_dir_path
    ))
}

fn load_and_parse_config() -> Result<MainConfig> {
    let config_path = get_config_file_path().context("Failed to get config file path")?;

    let mut toml_config: MainConfig = {
        if let Some(config_path) = config_path.as_ref() {
            let toml_str = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read_to_string {}", config_path.display()))?;
            toml::from_str(&toml_str)
                .with_context(|| format!("Failed to parse {}", config_path.display()))?
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
            let sb_config = load_soundboard_config(&path)
                .with_context(|| format!("Failed to load soundboard {}", path.display()))?;
            if sb_config.disabled.unwrap_or_default() {
                continue;
            }
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

pub fn get_soundboard_sound_directory(soundboard_path: &Path) -> Result<PathBuf> {
    let mut new_path = get_soundboards_path()?;
    let stem: &str = soundboard_path
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    new_path.push(stem);
    Ok(new_path)
}

fn resolve_sound_path(soundboard_path: &Path, sound_path: &str) -> Result<String> {
    let relative_path = Path::new(sound_path);
    if sound_path.starts_with("http") {
        return Ok(sound_path.to_string());
    }
    if relative_path.is_absolute() {
        if !relative_path.exists() || !relative_path.is_file() {
            return Err(anyhow!("expected sound file at {}", sound_path));
        }
        return Ok(sound_path.to_string());
    }
    let mut new_path = get_soundboard_sound_directory(soundboard_path)?;
    new_path.push(relative_path);

    if !new_path.exists() || !new_path.is_file() {
        return Err(anyhow!(
            "expected sound file at {}",
            new_path.to_str().unwrap()
        ));
    }

    Ok(new_path.to_str().unwrap().to_string())
}

fn load_soundboard_config(soundboard_path: &Path) -> Result<SoundboardConfig> {
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
        if sound.name.is_empty() {
            return Err(anyhow!("save_soundboard: sound name empty {}", sound.path));
        }
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
fn save_config(config: &MainConfig, name: &str) -> Result<()> {
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
    let toml_str = fs::read_to_string(&old_path)
        .with_context(|| format!("Failed to read_to_string {}", old_path.display()))?;
    let soundboard_config: SoundboardConfig = toml::from_str(&toml_str)
        .with_context(|| format!("Failed to parse {}", old_path.display()))?;

    let old_config_hash = utils::calculate_hash(&soundboard_config);

    if old_config_hash == new_config.last_hash {
        return Ok(false);
    }

    Ok(true)
}

fn save_soundboard_config(config: &mut SoundboardConfig, new: bool) -> Result<()> {
    let soundboard_config_path = PathBuf::from_str(&config.path)
        .with_context(|| format!("Failed to parse path {}", &config.path))?;

    if soundboard_config_path.parent().is_none()
        || !soundboard_config_path.parent().unwrap().exists()
    {
        return Err(anyhow!(
            "save_soundboard: invalid path  {}",
            soundboard_config_path.display()
        ));
    }

    if config.name.is_empty() {
        return Err(anyhow!("save_soundboard: invalid name  {}", &config.name));
    }

    if config.sounds.is_none() {
        return Err(anyhow!("save_soundboard: expected sounds"));
    }

    if !new && check_soundboard_config_mutated_on_disk(&soundboard_config_path, config)? {
        return Err(anyhow!(
            "save_soundboard: soundboard config file mutated on disk",
        ));
    } else if new && soundboard_config_path.exists() {
        return Err(anyhow!(
            "save_soundboard: soundboard config file already exists on disk",
        ));
    }

    for sound in config.sounds.as_ref().unwrap() {
        resolve_sound_path(&soundboard_config_path, &sound.path)?;
        if sound.name.is_empty() {
            return Err(anyhow!("save_soundboard: sound name empty {}", sound.path));
        }
    }

    let pretty_string =
        toml::to_string_pretty(&config).context("failed to serialize soundboard config")?;
    fs::write(&soundboard_config_path, pretty_string)
        .with_context(|| format!("Failed to write {}", &soundboard_config_path.display()))?;
    config.last_hash = utils::calculate_hash(&config);
    info!("Saved config file at {}", &config.path);
    Ok(())
}

fn get_env_name_from_cli_name(name: &str) -> String {
    "SB_".to_owned() + &name.to_ascii_uppercase().replace("-", "_")
}

fn merge_option_with_args_and_env<T: From<String>>(
    config_option: &mut Option<T>,
    args: &clap::ArgMatches,
    name: &str,
) {
    if args.is_present(name) {
        *config_option = Some(args.value_of(name).unwrap().to_owned().into())
    } else if let Ok(value) = std::env::var(get_env_name_from_cli_name(name)) {
        *config_option = Some(value.into());
    }
}

fn merge_flag_with_args_and_env(
    config_option: &mut Option<bool>,
    args: &clap::ArgMatches,
    name: &str,
) {
    if args.is_present(name) || std::env::var(get_env_name_from_cli_name(name)).is_ok() {
        *config_option = Some(true);
    }
}

fn parse_arguments() -> clap::ArgMatches {
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
    #[cfg(feature = "http")]
    let matches = matches.arg(
        Arg::with_name("http-socket-addr")
            .long("http-socket-addr")
            .about("Specify the socket addr for http server")
            .takes_value(true),
    );
    matches.get_matches()
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
