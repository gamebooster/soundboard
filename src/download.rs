extern crate reqwest;

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::prelude::*;
use tokio::{self, fs::File, io::AsyncWriteExt, stream::StreamExt};

use super::config;
use super::utils;

pub fn local_path_for_sound_config_exists(sound: &config::SoundConfig) -> Result<bool> {
    if sound.path.starts_with("http") {
        let mut headers_tuple = Vec::new();
        if let Some(headers) = &sound.headers {
            for header in headers {
                headers_tuple.push((header.name.clone(), header.value.clone()));
            }
        }
        let string_hash = utils::calculate_hash(&(&sound.path, &headers_tuple)).to_string();
        let mut file_path = std::env::temp_dir();
        file_path.push(string_hash);
        if file_path.exists() {
            return Ok(true);
        }
    } else {
        return Ok(true);
    }

    Ok(false)
}

pub fn get_local_path_from_sound_config(sound: &config::SoundConfig) -> Result<PathBuf> {
    let path = {
        if sound.path.starts_with("http") {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &sound.headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            download_file_if_needed(&sound.path, headers_tuple)?
        } else {
            PathBuf::from_str(&sound.full_path)?
        }
    };

    Ok(path)
}

pub fn download_file_if_needed(url: &str, headers: Vec<(String, String)>) -> Result<PathBuf> {
    let string_hash = utils::calculate_hash(&(&url, &headers)).to_string();
    let mut file_path = std::env::temp_dir();
    file_path.push(string_hash);

    if file_path.exists() {
        return Ok(file_path);
    }

    info!("{:?}", headers);

    let client = reqwest::blocking::Client::new();
    let mut header_map = HeaderMap::new();
    for header in headers {
        let name = HeaderName::from_bytes(header.0.as_bytes())?;
        header_map.insert(name, HeaderValue::from_str(&header.1)?);
    }
    let resp = client.get(url).headers(header_map).send()?;
    if resp.status().is_success() {
        std::fs::write(&file_path, resp.bytes().unwrap())?;
        Ok(file_path)
    } else {
        Err(anyhow!("request failed"))
    }
}

#[cfg(feature = "telegram")]
pub async fn get_local_path_from_sound_config_async(
    sound: &config::SoundConfig,
) -> Result<PathBuf> {
    let path = {
        if sound.path.starts_with("http") {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &sound.headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            download_file_if_needed_async(&sound.path, headers_tuple).await?
        } else {
            PathBuf::from_str(&sound.full_path)?
        }
    };

    Ok(path)
}

#[cfg(feature = "telegram")]
pub async fn download_file_if_needed_async(
    url: &str,
    headers: Vec<(String, String)>,
) -> Result<PathBuf> {
    let string_hash = utils::calculate_hash(&(&url, &headers)).to_string();
    let mut file_path = std::env::temp_dir();
    file_path.push(string_hash);

    if file_path.exists() {
        return Ok(file_path);
    }

    info!("{:?}", headers);

    let client = reqwest::Client::new();
    let mut header_map = HeaderMap::new();
    for header in headers {
        let name = HeaderName::from_bytes(header.0.as_bytes())?;
        header_map.insert(name, HeaderValue::from_str(&header.1)?);
    }
    let resp = client.get(url).headers(header_map).send().await?;
    if resp.status().is_success() {
        let mut file = File::create(&file_path).await?;
        file.write_all(&resp.bytes().await?).await?;
        Ok(file_path)
    } else {
        Err(anyhow!("request failed"))
    }
}
