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

pub type SoundboardId = Ulid;
type SoundboardMap = HashMap<SoundboardId, Soundboard>;
pub type SoundId = Ulid;
type SoundMap = HashMap<SoundId, Sound>;
type GlobalSoundboardMap = Lazy<parking_lot::RwLock<std::sync::Arc<SoundboardMap>>>;

static GLOBAL_APP_CONFIG: GlobalAppConfig = Lazy::new(|| {
    let app_config = load_and_merge_app_config().expect("failed to load and merge app config");
    parking_lot::RwLock::new(std::sync::Arc::new(app_config))
});

static GLOBAL_SOUNDBOARD_MAP: GlobalSoundboardMap = Lazy::new(|| {
    let soundboards = load_and_parse_soundboards().expect("failed to load soundboards");
    parking_lot::RwLock::new(std::sync::Arc::new(soundboards))
});

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

pub fn get_app_config() -> std::sync::Arc<AppConfig> {
    GLOBAL_APP_CONFIG.read().clone()
}

pub fn load_app_config_from_disk() -> Result<()> {
    *GLOBAL_APP_CONFIG.write() = std::sync::Arc::new(load_and_merge_app_config()?);
    Ok(())
}

pub fn save_app_config_to_disk(config: &AppConfig) -> Result<()> {
    save_app_config(config)?;
    *GLOBAL_APP_CONFIG.write() = std::sync::Arc::new(config.clone());
    Ok(())
}

pub fn get_soundboards() -> std::sync::Arc<SoundboardMap> {
    GLOBAL_SOUNDBOARD_MAP.read().clone()
}

// pub fn get_soundboards_sorted_by_position() -> Vec<Soundboard> {
//     let soundboards = Vec::new();
//     for soundboard in get_soundboards().values() {
//         soundboards.push(soundboard.clone());
//     }
//     soundboards.sort_by(|a, b| match (a.get_position(), b.get_position()) {
//         (None, Some(_)) => std::cmp::Ordering::Greater,
//         (Some(_), None) => std::cmp::Ordering::Less,
//         (a, b) => a.cmp(b),
//     });
//     soundboards
// }

pub fn load_soundboards_from_disk() -> Result<()> {
    *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(load_and_parse_soundboards()?);
    Ok(())
}

pub fn get_sound(soundboard_id: Ulid, sound_id: Ulid) -> Option<Sound> {
    if let Some(soundboard) = GLOBAL_SOUNDBOARD_MAP.read().clone().get(&soundboard_id) {
        if let Some(sound) = soundboard.sounds.get(&sound_id) {
            return Some(sound.clone());
        }
    }

    None
}

pub fn find_sound(sound_id: Ulid) -> Option<Sound> {
    for soundboard in GLOBAL_SOUNDBOARD_MAP.read().values() {
        if let Some(sound) = soundboard.get_sounds().get(&sound_id) {
            return Some(sound.clone());
        }
    }

    None
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Soundboard {
    config: SoundboardConfig,

    id: SoundboardId,
    path: PathBuf,
    last_hash: u64,
    hotkey: Option<hotkey::Hotkey>,
    sounds: SoundMap,
}

impl Soundboard {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            config: SoundboardConfig::new(name),
            hotkey: None,
            sounds: SoundMap::default(),
            id: Ulid::new(),
            last_hash: 0,
            path: PathBuf::from(path),
        }
    }

    fn from_config(soundboard_path: &Path, config: SoundboardConfig) -> Result<Self> {
        let mut sound_map = SoundMap::default();
        if let Some(sound_configs) = config.sounds.as_ref() {
            for sound_config in sound_configs.iter() {
                let new_sound = Sound::from_config(sound_config.clone())?;
                sound_map.insert(new_sound.id, new_sound);
            }
        }
        let hotkey = {
            if let Some(hotkey) = config.hotkey.as_ref() {
                Some(hotkey::parse_hotkey(&hotkey)?)
            } else {
                None
            }
        };
        Ok(Self {
            last_hash: utils::calculate_hash(&config),
            config,
            hotkey,
            sounds: sound_map,
            path: PathBuf::from(soundboard_path),
            id: Ulid::new(),
        })
    }

    pub fn save_to_disk(&mut self) -> Result<()> {
        let mut soundboard_map: SoundboardMap = (**GLOBAL_SOUNDBOARD_MAP.read()).clone();

        if let Some(val) = soundboard_map.get_mut(&self.id) {
            if self.last_hash == 0 {
                panic!("should never be 0 for old soundboard");
            }
            save_soundboard_config(self.path.as_path(), &self.config, Some(self.last_hash))?;
            *val = self.clone();
            *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(soundboard_map);
        } else {
            if self.last_hash != 0 {
                panic!("should never be not 0 for new soundboard");
            }
            save_soundboard_config(self.path.as_path(), &self.config, None)?;
            soundboard_map.insert(self.id, self.clone());
            *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(soundboard_map);
        }
        self.last_hash = utils::calculate_hash(&self.config);
        Ok(())
    }

    pub fn insert_sound(&mut self, sound: Sound) -> Option<Sound> {
        self.sounds.insert(sound.id, sound)
    }

    pub fn get_id(&self) -> &Ulid {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.config.name
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }

    pub fn get_sounds_path(&self) -> Result<PathBuf> {
        get_soundboard_sound_directory(self.get_path())
    }

    pub fn get_position(&self) -> &Option<usize> {
        &self.config.position
    }

    pub fn get_hotkey(&self) -> &Option<hotkey::Hotkey> {
        &self.hotkey
    }

    pub fn get_sounds(&self) -> &SoundMap {
        &self.sounds
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, Eq, PartialEq, Hash, Default)]
struct SoundboardConfig {
    pub name: String,
    pub hotkey: Option<String>,
    pub position: Option<usize>,
    pub disabled: Option<bool>,
    #[serde(rename = "sound")]
    pub sounds: Option<Vec<SoundConfig>>,
}

impl SoundboardConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Sound {
    config: SoundConfig,

    hotkey: Option<hotkey::Hotkey>,
    id: ulid::Ulid,
}

impl Sound {
    pub fn new(name: &str, source: Source) -> Result<Self> {
        Ok(Self {
            config: SoundConfig::new(name, source),
            hotkey: None,
            id: Ulid::new(),
        })
    }

    fn from_config(config: SoundConfig) -> Result<Self> {
        let hotkey = {
            if let Some(hotkey) = config.hotkey.as_ref() {
                Some(hotkey::parse_hotkey(&hotkey)?)
            } else {
                None
            }
        };
        Ok(Self {
            config,
            hotkey,
            id: Ulid::new(),
        })
    }

    pub fn get_id(&self) -> &Ulid {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.config.name
    }

    pub fn set_name(&mut self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(anyhow!("sound: name is empty"));
        }
        self.config.name = name.to_string();
        Ok(())
    }

    pub fn get_source(&self) -> &Source {
        &self.config.source
    }

    pub fn set_source(&mut self, source: &Source) {
        self.config.source = source.clone();
    }

    pub fn get_hotkey(&self) -> &Option<hotkey::Hotkey> {
        &self.hotkey
    }

    pub fn set_hotkey(&mut self, hotkey: Option<hotkey::Hotkey>) {
        self.hotkey = hotkey.clone();
        if let Some(hotkey) = hotkey {
            self.config.hotkey = Some(hotkey.to_string());
        } else {
            self.config.hotkey = None;
        }
    }

    pub fn get_start(&self) -> Option<f32> {
        self.config.start
    }

    pub fn set_start(&mut self, start: Option<f32>) -> Result<()> {
        if let Some(start) = start {
            if start < 0.0 {
                return Err(anyhow!("start should be positive"));
            }
        }
        self.config.start = start;
        Ok(())
    }

    pub fn get_end(&self) -> Option<f32> {
        self.config.end
    }

    pub fn set_end(&mut self, end: Option<f32>) -> Result<()> {
        if let Some(end) = end {
            if end < 0.0 {
                return Err(anyhow!("end should be positive"));
            }
        }
        self.config.end = end;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum Source {
    #[serde(rename = "local")]
    Local { path: PathBuf },
    #[serde(rename = "http")]
    Http {
        url: String,
        headers: Option<Vec<HeaderConfig>>,
    },
    #[serde(rename = "youtube")]
    Youtube { id: String },
    #[serde(rename = "tts")]
    TTS { ssml: String, lang: String },
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct SoundConfig {
    pub name: String,
    pub source: Source,
    pub hotkey: Option<String>,
    pub start: Option<f32>,
    pub end: Option<f32>,
}

impl SoundConfig {
    pub fn new(name: &str, source: Source) -> Self {
        Self {
            name: name.to_string(),
            source,
            hotkey: None,
            start: None,
            end: None,
        }
    }
}

impl PartialEq for SoundConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.hotkey == other.hotkey
            && self.source == other.source
            && ((self.start.unwrap_or_default() * 10.0) as usize)
                == ((other.start.unwrap_or_default() * 10.0) as usize)
            && ((self.end.unwrap_or_default() * 10.0) as usize)
                == ((other.end.unwrap_or_default() * 10.0) as usize)
    }
}
impl Eq for SoundConfig {}

impl Hash for SoundConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.source.hash(state);
        self.hotkey.hash(state);
        ((self.start.unwrap_or_default() * 10.0) as usize).hash(state);
        ((self.end.unwrap_or_default() * 10.0) as usize).hash(state);
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Hash, Default, Eq)]
pub struct HeaderConfig {
    pub name: String,
    pub value: String,
}

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

fn load_and_parse_soundboards() -> Result<SoundboardMap> {
    let soundboards_path = get_soundboards_path()?;
    let mut soundboard_map = SoundboardMap::default();

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
            let soundboard: Soundboard = Soundboard::from_config(&path, sb_config)?;
            soundboard_map.insert(soundboard.id, soundboard);
        }
    }

    if soundboard_map.is_empty() {
        return Err(anyhow!(
            "could not find any soundboards in {:?}",
            soundboards_path
        ));
    }

    info!("Loaded soundboards from {}", soundboards_path.display());
    Ok(soundboard_map)
}

fn load_soundboard_config(soundboard_path: &Path) -> Result<SoundboardConfig> {
    let toml_str = fs::read_to_string(&soundboard_path)?;
    let soundboard_config: SoundboardConfig = toml::from_str(&toml_str)?;
    Ok(soundboard_config)
}

fn save_app_config(config: &AppConfig) -> Result<()> {
    let config_path = get_soundboards_path()?;

    let pretty_string = toml::to_string_pretty(&config)?;
    fs::write(&config_path, pretty_string)?;
    info!("Saved config file at {}", &config_path.display());
    Ok(())
}

fn check_soundboard_config_mutated_on_disk(path: &Path, last_hash: u64) -> Result<bool> {
    let toml_str = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read_to_string {}", path.display()))?;
    let soundboard_config: SoundboardConfig =
        toml::from_str(&toml_str).with_context(|| format!("Failed to parse {}", path.display()))?;

    let old_config_hash = utils::calculate_hash(&soundboard_config);

    if old_config_hash == last_hash {
        return Ok(false);
    }

    Ok(true)
}

fn save_soundboard_config(
    soundboard_config_path: &Path,
    config: &SoundboardConfig,
    last_hash: Option<u64>,
) -> Result<()> {
    if soundboard_config_path.parent().is_none()
        || !soundboard_config_path.parent().unwrap().exists()
    {
        return Err(anyhow!(
            "save_soundboard: invalid path  {}",
            soundboard_config_path.display()
        ));
    }

    if last_hash.is_some()
        && check_soundboard_config_mutated_on_disk(&soundboard_config_path, last_hash.unwrap())?
    {
        return Err(anyhow!(
            "save_soundboard: soundboard config file mutated on disk",
        ));
    } else if last_hash.is_none() && soundboard_config_path.exists() {
        return Err(anyhow!(
            "save_soundboard: soundboard config file already exists on disk",
        ));
    }

    let pretty_string =
        toml::to_string_pretty(&config).context("failed to serialize soundboard config")?;
    fs::write(&soundboard_config_path, pretty_string)
        .with_context(|| format!("Failed to write {}", &soundboard_config_path.display()))?;

    info!(
        "Saved config file at {}",
        soundboard_config_path.to_str().unwrap()
    );
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
