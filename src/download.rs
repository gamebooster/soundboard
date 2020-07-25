use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use super::config;
use super::utils;

#[cfg(feature = "text-to-speech")]
pub mod ttsclient;

fn resolve_local_sound_path(sound: &config::Sound, sound_path: PathBuf) -> Result<PathBuf> {
    if sound_path.is_absolute() {
        if !sound_path.exists() || !sound_path.is_file() {
            return Err(anyhow!(
                "expected local sound file at {}",
                sound_path.display()
            ));
        }
        return Ok(sound_path);
    }
    let parent_board_sounds_path = config::get_soundboards()
        .values()
        .find(|sb| sb.get_sounds().get(&sound.get_id()).is_some())
        .ok_or_else(|| anyhow!("unknown sound id"))?
        .get_sounds_path()?;
    let mut new_path = parent_board_sounds_path;
    new_path.push(sound_path);
    if !new_path.exists() || !new_path.is_file() {
        return Err(anyhow!(
            "expected local sound file at {}",
            new_path.to_str().unwrap()
        ));
    }
    Ok(new_path)
}

pub fn local_path_for_sound_config_exists(sound: &config::Sound) -> Result<Option<PathBuf>> {
    match sound.get_source() {
        config::Source::Http { url, headers } => {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            let string_hash = utils::calculate_hash(&(&url, &headers_tuple)).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                Ok(Some(file_path))
            } else {
                Ok(None)
            }
        }
        config::Source::Local { path } => {
            Ok(Some(resolve_local_sound_path(sound, path.to_path_buf())?))
        }
        config::Source::Youtube { id } => {
            let string_hash = utils::calculate_hash(&id).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                Ok(Some(file_path))
            } else {
                Ok(None)
            }
        }
        #[cfg(feature = "text-to-speech")]
        config::Source::TTS { ssml, lang } => {
            let string_hash = utils::calculate_hash(&(&ssml, lang)).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                Ok(Some(file_path))
            } else {
                Ok(None)
            }
        }
        #[cfg(not(feature = "text-to-speech"))]
        config::Source::TTS { ssml: _, lang: _ } => {
            Err(anyhow!("text-to-speech feature not compiled in binary"))
        }
    }
}

pub fn get_local_path_from_sound_config(sound: &config::Sound) -> Result<PathBuf> {
    use std::io::{self, Write};

    match sound.get_source() {
        config::Source::Http { url, headers } => {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            download_file_if_needed(&url, headers_tuple)
        }
        config::Source::Local { path } => resolve_local_sound_path(sound, path.to_path_buf()),
        config::Source::Youtube { id } => {
            let string_hash = utils::calculate_hash(&id).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                return Ok(file_path);
            }

            let temp_file_path = format!("{}{}", file_path.to_str().unwrap(), "_temp");
            let output = Command::new("youtube-dl")
                .args(&[
                    "-f",
                    "250/251/249",
                    &format!("https://youtube.com/watch?v={}", id),
                    "-o",
                    &temp_file_path,
                ])
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
                Ok(file_path)
            } else {
                Err(anyhow!("unknown youtube download error"))
            }
        }
        #[cfg(feature = "text-to-speech")]
        config::Source::TTS { ssml, lang } => {
            let string_hash = utils::calculate_hash(&(&ssml, &lang)).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                return Ok(file_path);
            }

            let mut client =
                ttsclient::TTSClient::connect().context("tts: failed to connect to service")?;
            let data = client
                .synthesize_speech(ssml, lang, None)
                .context("tts: failed to synthesize speech")?;
            std::fs::write(&file_path, data).context("tts: failed to write result file")?;
            if file_path.exists() {
                Ok(file_path)
            } else {
                Err(anyhow!("unknown text to speech download error"))
            }
        }

        #[cfg(not(feature = "text-to-speech"))]
        config::Source::TTS { ssml: _, lang: _ } => {
            Err(anyhow!("text-to-speech feature not compiled in binary"))
        }
    }
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
