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

pub type SoundboardId = Ulid;
type SoundboardMap = indexmap::IndexMap<SoundboardId, Soundboard>;
pub type SoundId = Ulid;
type SoundMap = HashMap<SoundId, Sound>;
type GlobalSoundboardMap = Lazy<parking_lot::RwLock<std::sync::Arc<SoundboardMap>>>;

static GLOBAL_SOUNDBOARD_MAP: GlobalSoundboardMap = Lazy::new(|| {
    let soundboards = load_and_parse_soundboards().expect("failed to load soundboards");
    parking_lot::RwLock::new(std::sync::Arc::new(soundboards))
});

/// Returns all soundboards
///
/// Lazily initialized
pub fn get_soundboards() -> std::sync::Arc<SoundboardMap> {
    GLOBAL_SOUNDBOARD_MAP.read().clone()
}

/// Reloads all soundboards from disk
///
/// Expensive
pub fn reload_soundboards_from_disk() -> Result<()> {
    *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(load_and_parse_soundboards()?);
    Ok(())
}

/// Returns the soundboard with the specified id
pub fn get_soundboard(
    id: Ulid,
) -> Result<owning_ref::OwningRef<std::sync::Arc<SoundboardMap>, Soundboard>> {
    owning_ref::OwningRef::new(get_soundboards()).try_map(|f| {
        f.get(&id)
            .ok_or_else(|| anyhow!("no soundboard with specified id"))
    })
}

/// Returns the sound with specified id for the specified soundboard
pub fn get_sound(soundboard_id: Ulid, sound_id: Ulid) -> Option<Sound> {
    if let Some(soundboard) = GLOBAL_SOUNDBOARD_MAP.read().clone().get(&soundboard_id) {
        if let Some(sound) = soundboard.sounds.get(&sound_id) {
            return Some(sound.clone());
        }
    }

    None
}

/// Iterates through all soundboards and checks for the specified sound_id
pub fn find_sound(sound_id: Ulid) -> Option<Sound> {
    for soundboard in GLOBAL_SOUNDBOARD_MAP.read().values() {
        if let Some(sound) = soundboard.get_sounds().get(&sound_id) {
            return Some(sound.clone());
        }
    }

    None
}

type SoundPositions = Vec<SoundId>;

/// Soundboard
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Soundboard {
    name: String,
    hotkey: Option<hotkey::Hotkey>,
    position: Option<usize>,
    sounds: SoundMap,
    sound_positions: SoundPositions,

    id: SoundboardId,
    path: PathBuf,
    last_hash: u64,
}

impl Soundboard {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            name: name.to_owned(),
            hotkey: None,
            position: None,
            sounds: SoundMap::default(),
            sound_positions: Vec::new(),
            id: Ulid::new(),
            path: PathBuf::from(path),
            last_hash: 0,
        }
    }

    fn from_config(soundboard_path: &Path, config: SoundboardConfig) -> Result<Self> {
        let mut sound_map = SoundMap::default();
        let hash = utils::calculate_hash(&config);
        let mut sound_positions = Vec::new();
        if let Some(sound_configs) = config.sounds {
            for sound_config in sound_configs {
                let new_sound = Sound::from_config(sound_config)?;
                sound_positions.push(new_sound.id);
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
            last_hash: hash,
            name: config.name,
            position: config.position,
            hotkey,
            sounds: sound_map,
            sound_positions,
            path: PathBuf::from(soundboard_path),
            id: Ulid::new(),
        })
    }

    /// Save soundboard to disk
    ///
    /// fails if soundboard on disk was modified from our last load from disk
    // pub fn save_to_disk(&mut self) -> Result<()> {
    //     let mut soundboard_map: SoundboardMap = (**GLOBAL_SOUNDBOARD_MAP.read()).clone();
    //     let mut config = SoundboardConfig::new(self.name.as_str());

    //     if let Some(val) = soundboard_map.get_mut(&self.id) {
    //         if self.last_hash == 0 {
    //             panic!("should never be 0 for old soundboard");
    //         }
    //         save_soundboard_config(self.path.as_path(), &config, Some(self.last_hash))?;
    //         *val = self.clone();
    //         *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(soundboard_map);
    //     } else {
    //         if self.last_hash != 0 {
    //             panic!("should never be not 0 for new soundboard");
    //         }
    //         save_soundboard_config(self.path.as_path(), &config, None)?;
    //         soundboard_map.insert(self.id, self.clone());
    //         *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(soundboard_map);
    //     }
    //     self.last_hash = utils::calculate_hash(&config);
    //     Ok(())
    // }

    /// Add sound to soundboard
    pub fn insert_sound(&mut self, sound: Sound) -> Option<Sound> {
        self.sound_positions.push(sound.id);
        self.sounds.insert(sound.id, sound)
    }

    pub fn get_id(&self) -> &Ulid {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }

    /// Returns the local sounds directory for the soundboard
    ///
    /// name: soundboard file name without .toml
    pub fn get_sounds_path(&self) -> Result<PathBuf> {
        get_soundboard_sound_directory(self.get_path())
    }

    pub fn get_position(&self) -> &Option<usize> {
        &self.position
    }

    pub fn get_hotkey(&self) -> &Option<hotkey::Hotkey> {
        &self.hotkey
    }

    pub fn get_sounds(&self) -> &SoundMap {
        &self.sounds
    }

    pub fn get_sound_positions(&self) -> &SoundPositions {
        &self.sound_positions
    }

    pub fn iter<'a>(&self) -> SoundIterator {
        SoundIterator::new(&self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SoundIterator<'a> {
    curr: usize,
    soundboard: &'a Soundboard,
}

impl<'a> SoundIterator<'a> {
    pub fn new(soundboard: &'a Soundboard) -> Self {
        Self {
            curr: 0,
            soundboard,
        }
    }
}

impl<'a> Iterator for SoundIterator<'a> {
    type Item = &'a Sound;

    fn next(&mut self) -> Option<&'a Sound> {
        if let Some(id) = self.soundboard.get_sound_positions().get(self.curr) {
            self.curr += 1;
            self.soundboard.get_sounds().get(id)
        } else {
            None
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

#[derive(Debug, Deserialize, Clone, Serialize)]
struct SoundConfig {
    pub name: String,
    pub source: Source,
    pub hotkey: Option<String>,
    pub start: Option<f32>,
    pub end: Option<f32>,
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

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Hash, Default, Eq)]
pub struct HeaderConfig {
    pub name: String,
    pub value: String,
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

fn soundboard_position_sorter(a: &Option<usize>, b: &Option<usize>) -> std::cmp::Ordering {
    match (a, b) {
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (a, b) => a.cmp(b),
    }
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

    soundboard_map
        .sort_by(|_, a, _, b| soundboard_position_sorter(a.get_position(), b.get_position()));

    info!("Loaded soundboards from {}", soundboards_path.display());
    Ok(soundboard_map)
}

fn load_soundboard_config(soundboard_path: &Path) -> Result<SoundboardConfig> {
    let toml_str = fs::read_to_string(&soundboard_path)?;
    let soundboard_config: SoundboardConfig = toml::from_str(&toml_str)?;
    Ok(soundboard_config)
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
