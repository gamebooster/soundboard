extern crate anyhow;
extern crate hotkey;
extern crate log;
extern crate serde;
extern crate toml;

use anyhow::Result;
use log::{error, info, trace, warn};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub input_device: Option<usize>,
    pub output_device: Option<usize>,
    pub loopback_device: Option<usize>,
    pub sounds: Option<Vec<SoundConfig>>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum Modifiers {
    ALT = hotkey::modifiers::ALT as isize,
    CTRL= hotkey::modifiers::CONTROL as isize,
    SHIFT= hotkey::modifiers::SHIFT as isize,
    SUPER= hotkey::modifiers::SUPER as isize,
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
    INSERT = hotkey::keys::INSERT  as isize,
    DELETE = hotkey::keys::DELETE  as isize,
    KEY_1 = '1'  as isize,
    KEY_2 = '2'   as isize,
    KEY_3 = '3'  as isize,
    KEY_4 = '4'  as isize,
    KEY_5 = '5' as isize,
    KEY_6 = '6'  as isize,
    KEY_7 = '7'  as isize,
    KEY_8 = '8'  as isize,
    KEY_9 = '9'  as isize,
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

#[derive(Debug, Deserialize)]
pub struct SoundConfig {
    pub name: String,
    pub path: String,
    pub hotkey_modifier: Vec<Modifiers>,
    pub hotkey_key: Key,
}

pub fn parse_config(path: &Path) -> Result<Config> {
    let toml_str = fs::read_to_string(path)?;
    let toml_config = toml::from_str(&toml_str)?;
    info!("Loaded config file from {}", path.display());
    Ok(toml_config)
}
