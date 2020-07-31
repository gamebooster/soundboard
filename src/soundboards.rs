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

pub use helpers::get_soundboards_path;

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

// pub fn save_soundboards_to_disk() -> Result<()> {
//     for soundboard in get_soundboards().values() {
//         soundboard.clone().save_to_disk();
//     }
//     Ok(())
// }

pub fn update_soundboards(mut soundboard: Soundboard) -> Result<()> {
    soundboard.save_to_disk()?;
    let mut cloned_map = (**GLOBAL_SOUNDBOARD_MAP.read()).clone();
    cloned_map.insert(soundboard.id, soundboard);
    *GLOBAL_SOUNDBOARD_MAP.write() = std::sync::Arc::new(cloned_map);
    Ok(())
}

/// Returns the soundboard with the specified id
pub fn get_soundboard(
    id: Ulid,
) -> Option<owning_ref::OwningRef<std::sync::Arc<SoundboardMap>, Soundboard>> {
    if let Ok(owning) = owning_ref::OwningRef::new(get_soundboards()).try_map(|f| {
        f.get(&id)
            .ok_or_else(|| anyhow!("no soundboard with specified id"))
    }) {
        Some(owning)
    } else {
        None
    }
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
    last_hash: Option<u64>,
}

impl Soundboard {
    pub fn new(name: &str) -> Result<Self> {
        let mut path = helpers::get_soundboards_path().unwrap();
        path.push(name);
        path = path.with_extension("json");

        if path.exists() {
            return Err(anyhow!("soundboard path already exists {}", name));
        }

        Ok(Self {
            name: name.to_owned(),
            hotkey: None,
            position: None,
            sounds: SoundMap::default(),
            sound_positions: Vec::new(),
            id: Ulid::new(),
            path,
            last_hash: None,
        })
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
            last_hash: Some(hash),
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
    pub fn save_to_disk(&mut self) -> Result<()> {
        let mut config = SoundboardConfig::new(self.name.as_str());

        if let Some(hotkey) = self.get_hotkey() {
            config.hotkey = Some(hotkey.to_string());
        }

        config.position = *self.get_position();
        config.sounds = Some(
            self.iter()
                .map(|s| SoundConfig::from(s))
                .collect::<Vec<SoundConfig>>(),
        );

        save_soundboard_config(self.path.as_path(), &config, self.last_hash)?;

        self.path.set_extension("json");

        self.last_hash = Some(utils::calculate_hash(&config));
        Ok(())
    }

    pub fn remove_sound(&mut self, sound: &Sound) -> Result<()> {
        self.sounds
            .remove(&sound.id)
            .ok_or_else(|| anyhow!("no sound found with specified id"))?;
        if let Some(position) = self
            .sound_positions
            .iter()
            .position(|id| id == sound.get_id())
        {
            self.sound_positions.remove(position);
        } else {
            panic!("inconsistent soundboard state");
        }
        Ok(())
    }

    /// Add sound to soundboard
    pub fn add_sound(&mut self, sound: Sound) -> Result<()> {
        if let Source::Local { path } = sound.get_source() {
            let path = PathBuf::from(path);
            if path.is_absolute() && !path.exists() {
                return Err(anyhow!(
                    "local sound file does not exist: {}",
                    path.display()
                ));
            } else {
                let mut sound_path = self.get_sounds_path()?;
                sound_path.push(path);
                if !sound_path.exists() {
                    return Err(anyhow!(
                        "local sound file does not exist: {}",
                        sound_path.display()
                    ));
                }
            }
        }
        self.sound_positions.push(sound.id);
        self.sounds.insert(sound.id, sound);
        Ok(())
    }

    pub fn copy_sound_from_another_soundboard(
        &mut self,
        soundboard: &Soundboard,
        sound: &Sound,
    ) -> Result<SoundId> {
        let new_sound = Sound::new(sound.get_name(), sound.get_source().clone())?;
        let new_sound_id = *new_sound.get_id();
        if let Source::Local { path } = sound.get_source() {
            let mut old_path = soundboard.get_sounds_path().unwrap();
            old_path.push(&path);
            self.add_sound_with_file_path(new_sound, &old_path, true)?;
        } else {
            self.add_sound(new_sound)?;
        }
        Ok(new_sound_id)
    }

    pub fn add_sound_with_reader<R: ?Sized>(
        &mut self,
        sound: Sound,
        reader: &mut R,
        overwrite: bool,
    ) -> Result<()>
    where
        R: std::io::Read,
    {
        if let Source::Local { path } = sound.get_source() {
            let mut sound_path = self.get_sounds_path()?;
            sound_path.push(path);
            if sound_path.exists() && !overwrite {
                return Err(anyhow!(
                    "local sound file already exists: {}",
                    sound_path.display()
                ));
            }
            let new_sounds_path = get_soundboard_sound_directory(self.get_path())?;

            if !new_sounds_path.exists() {
                std::fs::create_dir(new_sounds_path)?;
            }
            let mut file = std::fs::File::create(&sound_path)?;
            std::io::copy(reader, &mut file)
                .with_context(|| format!("cant copy file to {}", &sound_path.display()))?;
            self.add_sound(sound)
        } else {
            Err(anyhow!("not a local source sound"))
        }
    }

    pub fn add_sound_with_file_path(
        &mut self,
        sound: Sound,
        source_path: &Path,
        overwrite: bool,
    ) -> Result<()> {
        let mut file = std::fs::File::open(source_path)?;
        self.add_sound_with_reader(sound, &mut file, overwrite)
    }

    pub fn change_sound_position(&mut self, target: SoundId, after: SoundId) -> Result<()> {
        let target_position = match self.sound_positions.iter().position(|s| s == &target) {
            Some(pos) => pos,
            None => return Err(anyhow!("unknown target sound")),
        };
        let after_position = match self.sound_positions.iter().position(|s| s == &after) {
            Some(pos) => pos,
            None => return Err(anyhow!("unknown after sound")),
        };

        self.sound_positions.remove(target_position);
        self.sound_positions.insert(after_position + 1, target);
        Ok(())
    }

    pub fn get_id(&self) -> &Ulid {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
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

    pub fn set_position(&mut self, position: Option<usize>) {
        self.position = position;
    }

    pub fn get_hotkey(&self) -> &Option<hotkey::Hotkey> {
        &self.hotkey
    }

    pub fn set_hotkey(&mut self, hotkey: Option<hotkey::Hotkey>) {
        self.hotkey = hotkey;
    }

    pub fn get_hotkey_string_or_none(&self) -> Option<String> {
        if let Some(hotkey) = self.get_hotkey() {
            Some(hotkey.to_string())
        } else {
            None
        }
    }

    pub fn get_sounds(&self) -> &SoundMap {
        &self.sounds
    }

    pub fn get_sounds_mut(&mut self) -> &mut SoundMap {
        &mut self.sounds
    }

    pub fn get_sound_positions(&self) -> &SoundPositions {
        &self.sound_positions
    }

    pub fn iter(&self) -> SoundIterator {
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

    pub fn set_source(&mut self, source: Source) -> Result<()> {
        if let Source::Local { path: _ } = source {
            return Err(anyhow!("sound: source cant change to local"));
        }
        self.config.source = source;
        Ok(())
    }

    pub fn get_hotkey(&self) -> &Option<hotkey::Hotkey> {
        &self.hotkey
    }

    pub fn get_hotkey_string_or_none(&self) -> Option<String> {
        if let Some(hotkey) = self.get_hotkey() {
            Some(hotkey.to_string())
        } else {
            None
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(rename = "sound")]
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f32>,
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum Source {
    #[serde(rename = "local")]
    Local { path: String },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<Vec<HeaderConfig>>,
    },
    #[serde(rename = "youtube")]
    Youtube { id: String },
    #[serde(rename = "tts")]
    TTS { ssml: String, lang: String },
    #[serde(rename = "spotify")]
    Spotify { id: String },
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

    pub fn from(sound: &Sound) -> Self {
        let hotkey = {
            if let Some(hotkey) = sound.get_hotkey() {
                Some(hotkey.to_string())
            } else {
                None
            }
        };
        Self {
            name: sound.get_name().to_string(),
            source: sound.get_source().clone(),
            hotkey,
            start: sound.get_start(),
            end: sound.get_end(),
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
        let extension = path.extension().unwrap_or_default().to_string_lossy();

        if extension == "toml" || extension == "json" {
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
    let file_str = fs::read_to_string(&soundboard_path)?;
    let soundboard_config: SoundboardConfig;

    if PathBuf::from(soundboard_path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        == "toml"
    {
        soundboard_config = toml::from_str(&file_str)?;
    } else {
        soundboard_config = serde_json::from_str(&file_str)?;
    }
    Ok(soundboard_config)
}

fn check_soundboard_config_mutated_on_disk(path: &Path, last_hash: u64) -> Result<bool> {
    let file_str = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read_to_string {}", path.display()))?;

    let soundboard_config: SoundboardConfig;
    if PathBuf::from(path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        == "toml"
    {
        soundboard_config = toml::from_str(&file_str)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
    } else {
        soundboard_config = serde_json::from_str(&file_str)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
    }

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

    let soundboard_json_path = PathBuf::from(soundboard_config_path).with_extension("json");

    let pretty_json_string =
        serde_json::to_string_pretty(&config).context("failed to serialize soundboard config")?;
    fs::write(&soundboard_json_path, pretty_json_string)
        .with_context(|| format!("Failed to write {}", &soundboard_config_path.display()))?;

    if soundboard_config_path.exists()
        && soundboard_config_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            == "toml"
    {
        fs::rename(
            soundboard_config_path,
            PathBuf::from(soundboard_config_path).with_extension("toml_disabled_oldformat"),
        )
        .unwrap();
    }

    // let pretty_string =
    //     toml::to_string_pretty(&config).context("failed to serialize soundboard config")?;
    // fs::write(&soundboard_config_path, pretty_string)
    //     .with_context(|| format!("Failed to write {}", &soundboard_config_path.display()))?;

    info!(
        "Saved config file at {}",
        soundboard_json_path.to_str().unwrap()
    );
    Ok(())
}
