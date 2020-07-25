use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

pub(super) fn get_config_file_path() -> Result<Option<PathBuf>> {
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

pub(super) fn get_soundboards_path() -> Result<PathBuf> {
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

pub(super) fn get_soundboard_sound_directory(soundboard_path: &Path) -> Result<PathBuf> {
    let mut new_path = get_soundboards_path()?;
    let stem: &str = soundboard_path
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    new_path.push(stem);
    Ok(new_path)
}

pub(super) fn get_env_name_from_cli_name(name: &str) -> String {
    "SB_".to_owned() + &name.to_ascii_uppercase().replace("-", "_")
}

pub(super) fn merge_option_with_args_and_env<T: From<String>>(
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

pub(super) fn merge_flag_with_args_and_env(
    config_option: &mut Option<bool>,
    args: &clap::ArgMatches,
    name: &str,
) {
    if args.is_present(name) || std::env::var(get_env_name_from_cli_name(name)).is_ok() {
        *config_option = Some(true);
    }
}
