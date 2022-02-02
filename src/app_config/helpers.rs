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

pub(super) fn get_env_name_from_cli_name(name: &str) -> String {
    "SB_".to_owned() + &name.to_ascii_uppercase().replace('-', "_")
}

pub(super) fn merge_option_with_args_and_env<T: From<String>>(
    config_option: &mut Option<T>,
    args: &clap::ArgMatches,
    name: &str,
) {
    if args.occurrences_of(name) > 0 {
        *config_option = Some(args.value_of(name).unwrap().to_owned().into())
    } else if let Ok(value) = std::env::var(get_env_name_from_cli_name(name)) {
        *config_option = Some(value.into());
    }
}

pub(super) fn merge_bool_option_with_args_and_env(
    config_option: &mut Option<bool>,
    args: &clap::ArgMatches,
    name: &str,
) -> Result<()> {
    let mut value = None;
    if args.occurrences_of(name) > 0 {
        value = Some(args.value_of(name).unwrap().to_owned());
    } else if let Ok(new_value) = std::env::var(get_env_name_from_cli_name(name)) {
        value = Some(new_value);
    }

    if value.is_none() {
        return Ok(());
    }

    match value.unwrap_or_default().as_str() {
        "true" => *config_option = Some(true),
        "false" => *config_option = Some(false),
        value => {
            return Err(anyhow!(
                "Unsupported value for boolean option {} = {}",
                name,
                value
            ))
        }
    }

    Ok(())
}

pub(super) fn merge_flag_with_args_and_env(
    config_option: &mut Option<bool>,
    args: &clap::ArgMatches,
    name: &str,
) {
    if args.occurrences_of(name) > 0 || std::env::var(get_env_name_from_cli_name(name)).is_ok() {
        *config_option = Some(true);
    }
}
