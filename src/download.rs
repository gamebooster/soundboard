extern crate reqwest;

use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::PathBuf;

use super::config;
use super::utils;

pub fn get_local_path_from_sound_config(sound: &config::SoundConfig) -> Result<String> {
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
            let mut path = std::env::current_exe()?;
            path.pop();
            path.push("sounds");
            path.push(&sound.path);
            path.to_str().unwrap().into()
        }
    };

    Ok(path)
}

pub fn download_file_if_needed(url: &str, headers: Vec<(String, String)>) -> Result<String> {
    let string_hash = utils::calculate_hash(&(&url, &headers)).to_string();
    let mut file_path = std::env::temp_dir();
    file_path.push(string_hash);

    if file_path.exists() {
        return Ok(file_path.to_str().unwrap().into());
    }

    info!("{:?}", headers);

    let client = Client::new();
    let mut header_map = HeaderMap::new();
    for header in headers {
        let name = HeaderName::from_bytes(header.0.as_bytes())?;
        header_map.insert(name, HeaderValue::from_str(&header.1)?);
    }
    let resp = client.get(url).headers(header_map).send()?;
    if resp.status().is_success() {
        std::fs::write(&file_path, resp.bytes().unwrap())?;
        Ok(file_path.to_str().unwrap().into())
    } else {
        Err(anyhow!("request failed"))
    }
}
