extern crate toml;
extern crate serde;
extern crate anyhow;
extern crate log;

use serde::{Deserialize};
use std::path::Path;
use std::fs;
use anyhow::Result;
use log::{info, trace, warn, error};

#[derive(Debug, Deserialize)]
pub struct Config {
    input_device: Option<usize>,
    output_device: Option<usize>,
    loopback_device: Option<usize>,
    sounds: Option<Vec<SoundConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct SoundConfig {
    name: String,
    path: String,
    hotkey: String
}

pub fn parse_config(path : &Path) -> Result<Config> {
    let toml_str = fs::read_to_string(path)?;
    let toml_config = toml::from_str(&toml_str)?;
    info!("Loaded config file from ");
    Ok(toml_config)
}