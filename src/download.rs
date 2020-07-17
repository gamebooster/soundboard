use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use super::config;
use super::utils;

#[cfg(feature = "text-to-speech")]
pub mod ttsclient;

pub fn local_path_for_sound_config_exists(sound: &config::SoundConfig) -> Result<Option<PathBuf>> {
    #[cfg(feature = "text-to-speech")]
    if sound.path.contains("<speak>") {
        let string_hash =
            utils::calculate_hash(&(&sound.path, &sound.tts_language, &sound.tts_options))
                .to_string();
        let mut file_path = std::env::temp_dir();
        file_path.push(string_hash);
        if file_path.exists() {
            return Ok(Some(file_path));
        } else {
            return Ok(None);
        }
    }

    #[cfg(not(feature = "text-to-speech"))]
    if sound.path.contains("<speak>") {
        return Err(anyhow!("text-to-speech feature not compiled in binary"));
    }

    if sound.path.contains("youtube.com") || sound.path.contains("youtu.be") {
        let string_hash = utils::calculate_hash(&sound.path).to_string();
        let mut file_path = std::env::temp_dir();
        file_path.push(string_hash);
        if file_path.exists() {
            return Ok(Some(file_path));
        }
    } else if sound.path.starts_with("http") {
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
            return Ok(Some(file_path));
        }
    } else {
        return Ok(Some(PathBuf::from_str(&sound.full_path)?));
    }

    Ok(None)
}

pub fn get_local_path_from_sound_config(sound: &config::SoundConfig) -> Result<PathBuf> {
    use std::io::{self, Write};

    let path = {
        #[cfg(feature = "text-to-speech")]
        if sound.path.contains("<speak>") {
            let string_hash =
                utils::calculate_hash(&(&sound.path, &sound.tts_language, &sound.tts_options))
                    .to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                return Ok(file_path);
            }

            let mut client =
                ttsclient::TTSClient::connect().context("tts: failed to connect to service")?;
            let default_language = "en-GB".to_owned();
            let data = client
                .synthesize_speech(
                    sound.path.clone(),
                    sound
                        .tts_language
                        .as_ref()
                        .unwrap_or_else(|| &default_language)
                        .clone(),
                    sound.tts_options.clone(),
                )
                .context("tts: failed to synthesize speech")?;
            std::fs::write(&file_path, data).context("tts: failed to write result file")?;
            if file_path.exists() {
                return Ok(file_path);
            } else {
                return Err(anyhow!("unknown text to speech download error"));
            }
        }

        #[cfg(not(feature = "text-to-speech"))]
        if sound.path.contains("<speak>") {
            return Err(anyhow!("text-to-speech feature not compiled in binary"));
        }

        if sound.path.contains("youtube.com") || sound.path.contains("youtu.be") {
            let string_hash = utils::calculate_hash(&sound.path).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                return Ok(file_path);
            }

            let temp_file_path = format!("{}{}", file_path.to_str().unwrap(), "_temp");
            let output = Command::new("youtube-dl")
                .args(&["-f", "250/251/249", &sound.path, "-o", &temp_file_path])
                .output()
                .context("executing youtube-dl failed")?;
            info!("youtube-dl status: {}", output.status);
            if !output.status.success() {
                io::stdout().write_all(&output.stdout).unwrap();
                io::stderr().write_all(&output.stderr).unwrap();
                return Err(anyhow!("youtube-dl error"));
            }
            let output = Command::new("mkvextract")
                .args(&[
                    &temp_file_path,
                    "tracks",
                    &format!("0:{}", file_path.to_str().unwrap()),
                ])
                .output()
                .context("executing mkvextract failed")?;
            std::fs::remove_file(temp_file_path).context("could not delete youtube temp file")?;
            info!("mkvextract status: {}", output.status);
            if !output.status.success() {
                io::stdout().write_all(&output.stdout).unwrap();
                io::stderr().write_all(&output.stderr).unwrap();
                return Err(anyhow!("mkvextract error"));
            }
            if file_path.exists() {
                return Ok(file_path);
            } else {
                return Err(anyhow!("unknown youtube download error"));
            }
        } else if sound.path.starts_with("http") {
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
    use tokio::prelude::*;
    use tokio::{self, fs::File, io::AsyncWriteExt, stream::StreamExt};

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
