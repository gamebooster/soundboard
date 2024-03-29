use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use super::app_config;
use super::soundboards;
use super::utils;

#[cfg(feature = "text-to-speech")]
pub mod ttsclient;

pub fn get_local_path_from_sound_config(
    sound: &soundboards::Sound,
    download: bool,
) -> Result<Option<PathBuf>> {
    use std::io::{self, Write};

    match sound.get_source() {
        soundboards::Source::Http { url, headers } => {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            let file_path = get_file_path_from_hash(&(&url, &headers_tuple));
            if file_path.is_file() {
                Ok(Some(file_path))
            } else if !download {
                Ok(None)
            } else {
                Ok(Some(download_from_http(file_path, url, headers_tuple)?))
            }
        }
        soundboards::Source::Local { path } => {
            Ok(Some(resolve_local_sound_path(sound, PathBuf::from(path))?))
        }
        soundboards::Source::Youtube { id } => {
            let file_path = get_file_path_from_hash(&id);
            if file_path.is_file() {
                return Ok(Some(file_path));
            } else if !download {
                return Ok(None);
            }

            let temp_file_path = format!("{}{}", file_path.to_str().unwrap(), "_temp");

            let output = Command::new(get_command_path("youtube-dl")?)
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

            let output = Command::new(get_command_path("mkvextract")?)
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

            if file_path.is_file() {
                Ok(Some(file_path))
            } else {
                Err(anyhow!("unknown youtube download error"))
            }
        }
        #[cfg(feature = "text-to-speech")]
        soundboards::Source::Tts { ssml, lang } => {
            let file_path = get_file_path_from_hash(&(&ssml, &lang));
            if file_path.is_file() {
                return Ok(Some(file_path));
            } else if !download {
                return Ok(None);
            }

            let tts_key = app_config::get_app_config()
                .tts_key
                .clone()
                .unwrap_or_default();

            if tts_key.is_empty() {
                return Err(anyhow!("tts: no key specified"));
            }

            let mut client = ttsclient::TTSClient::connect(&tts_key)
                .context("tts: failed to connect to service")?;
            let data = client
                .synthesize_speech(ssml, lang, None)
                .context("tts: failed to synthesize speech")?;
            std::fs::write(&file_path, data).context("tts: failed to write result file")?;

            if file_path.is_file() {
                Ok(Some(file_path))
            } else {
                Err(anyhow!("unknown text to speech download error"))
            }
        }

        #[cfg(not(feature = "text-to-speech"))]
        soundboards::Source::Tts { ssml: _, lang: _ } => {
            Err(anyhow!("text-to-speech feature not compiled in binary"))
        }

        #[cfg(feature = "spotify")]
        soundboards::Source::Spotify { id } => {
            let file_path = get_file_path_from_hash(&id);
            if file_path.is_file() {
                Ok(Some(file_path))
            } else if !download {
                Ok(None)
            } else {
                Ok(Some(download_from_spotify(file_path, id)?))
            }
        }
        #[cfg(not(feature = "spotify"))]
        soundboards::Source::Spotify { id: _ } => {
            Err(anyhow!("spotify feature not compiled in binary"))
        }
    }
}

fn get_command_path(command_name: &str) -> Result<PathBuf> {
    let mut local_command_path = std::env::current_exe()?;
    local_command_path.pop();
    local_command_path.push(command_name);
    let command_path = {
        if local_command_path.is_file() {
            local_command_path
        } else if local_command_path.with_extension("exe").is_file() {
            local_command_path.with_extension("exe")
        } else {
            PathBuf::from(command_name)
        }
    };
    Ok(command_path)
}

fn resolve_local_sound_path(sound: &soundboards::Sound, sound_path: PathBuf) -> Result<PathBuf> {
    if sound_path.is_absolute() {
        if !sound_path.exists() || !sound_path.is_file() {
            return Err(anyhow!(
                "expected local sound file at {}",
                sound_path.display()
            ));
        }
        return Ok(sound_path);
    }
    let parent_board_sounds_path = soundboards::get_soundboards()
        .values()
        .find(|sb| sb.get_sounds().get(sound.get_id()).is_some())
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

fn get_file_path_from_hash<T: std::hash::Hash>(t: &T) -> PathBuf {
    let string_hash = utils::calculate_hash(t).to_string();
    let mut file_path = std::env::temp_dir();
    file_path.push(string_hash);
    file_path
}

fn download_from_http(
    file_path: PathBuf,
    url: &str,
    headers: Vec<(String, String)>,
) -> Result<PathBuf> {
    // info!("{:?}", headers);

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
        Err(anyhow!("http request failed {}", resp.status()))
    }
}

// TODO: wait for libspotify to get tokio 0.2 support
#[cfg(feature = "spotify")]
#[tokio::main(flavor = "current_thread")]
async fn download_from_spotify(file_path: PathBuf, id: &str) -> Result<PathBuf> {
    use futures::stream::FuturesUnordered;
    use librespot::audio::{AudioDecrypt, AudioFile, StreamLoaderController};
    use librespot::core::authentication::Credentials;
    use librespot::core::config::SessionConfig;
    use librespot::core::session::Session;
    use librespot::core::spotify_id::SpotifyId;
    use librespot::metadata::{AudioItem, FileFormat};
    use librespot::playback::config::PlayerConfig;
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::Sleep;

    if app_config::get_app_config()
        .spotify_user
        .clone()
        .unwrap_or_default()
        .is_empty()
    {
        return Err(anyhow!("spotify: no spotify_user specified"));
    }

    if app_config::get_app_config()
        .spotify_pass
        .clone()
        .unwrap_or_default()
        .is_empty()
    {
        return Err(anyhow!("spotify: no spotify_pass specified"));
    }

    let session_config = SessionConfig::default();

    let credentials = Credentials::with_password(
        app_config::get_app_config()
            .spotify_user
            .clone()
            .unwrap_or_default(),
        app_config::get_app_config()
            .spotify_pass
            .clone()
            .unwrap_or_default(),
    );

    let track = match SpotifyId::from_base62(id) {
        Ok(track) => track,
        Err(err) => {
            return Err(anyhow!("Unable to parse spotify id {:?}", err));
        }
    };

    let session = Session::connect(session_config, credentials, None).await?;

    let mut audio = match AudioItem::get_audio_item(&session, track).await {
        Ok(audio) => audio,
        Err(err) => {
            return Err(anyhow!("spotify: Unable to load audio item. {:?}", err));
        }
    };

    info!(
        "spotify: Loading <{}> with Spotify URI <{}>",
        audio.name, audio.uri
    );

    if !audio.available {
        audio = {
            let ids = audio.alternatives.unwrap_or_default();
            let mut found_audio = None;
            for id in ids {
                if let Ok(audio) = AudioItem::get_audio_item(&session, id).await {
                    if audio.available {
                        found_audio = Some(audio);
                        break;
                    }
                }
            }
            if let Some(audio) = found_audio {
                audio
            } else {
                return Err(anyhow!("spotify: audio <{}> is not available", audio.uri));
            }
        };
    }

    // (Most) podcasts seem to support only 96 bit Vorbis, so fall back to it
    let formats = [
        FileFormat::OGG_VORBIS_96,
        FileFormat::OGG_VORBIS_160,
        FileFormat::OGG_VORBIS_320,
    ];
    let format = formats
        .iter()
        .find(|format| audio.files.contains_key(format))
        .unwrap();

    let file_id = match audio.files.get(format) {
        Some(&file_id) => file_id,
        None => {
            return Err(anyhow!(
                "spotify: <{}> in not available in format {:?}",
                audio.name,
                format
            ));
        }
    };

    const BYTES_PER_SECOND: usize = 64 * 1024;
    let play_from_beginning = true;

    let key = session.audio_key().request(track, file_id);
    let encrypted_file = AudioFile::open(&session, file_id, BYTES_PER_SECOND, play_from_beginning);

    let encrypted_file = match encrypted_file.await {
        Ok(encrypted_file) => encrypted_file,
        Err(err) => {
            return Err(anyhow!("spotify: Unable to load encrypted file. {:?}", err));
        }
    };

    let stream_loader_controller = encrypted_file.get_stream_loader_controller();

    stream_loader_controller.set_stream_mode();

    let key = match key.await {
        Ok(key) => key,
        Err(err) => {
            return Err(anyhow!("spotify: Unable to load decryption key. {:?}", err));
        }
    };

    let mut decrypted_file = AudioDecrypt::new(key, encrypted_file);

    info!("spotify: <{}> ({} ms) loaded", audio.name, audio.duration);

    let mut buffer = Vec::new();

    let finished = std::sync::Arc::new(AtomicBool::new(false));
    let finished_clone = finished.clone();
    let file_path_clone = file_path.clone();
    std::thread::spawn(move || {
        if let Err(err) = decrypted_file.seek(std::io::SeekFrom::Start(0xa7)) {
            finished_clone.store(true, Ordering::Relaxed);
            error!("spotify: Unable to seek file. {}", err);
            return;
        }
        if let Err(err) = decrypted_file.read_to_end(&mut buffer) {
            finished_clone.store(true, Ordering::Relaxed);
            error!("spotify: Unable to read file. {}", err);
            return;
        }
        if let Err(err) = std::fs::write(&file_path_clone, &buffer) {
            error!("spotify: Unable to write file. {}", err);
        }
        finished_clone.store(true, Ordering::Relaxed);
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if finished.load(Ordering::Relaxed) {
            break;
        }
    }

    if file_path.exists() {
        Ok(file_path)
    } else {
        Err(anyhow!("Unknown spotify error"))
    }
}
