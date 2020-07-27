use anyhow::{anyhow, Context, Result};
use log::{error, info, trace, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use super::soundboards;
use super::utils;

#[cfg(feature = "text-to-speech")]
pub mod ttsclient;

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

pub fn local_path_for_sound_config_exists(sound: &soundboards::Sound) -> Result<Option<PathBuf>> {
    match sound.get_source() {
        soundboards::Source::Http { url, headers } => {
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
        soundboards::Source::Local { path } => {
            Ok(Some(resolve_local_sound_path(sound, PathBuf::from(path))?))
        }
        soundboards::Source::Youtube { id } => {
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
        soundboards::Source::TTS { ssml, lang } => {
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
        soundboards::Source::TTS { ssml: _, lang: _ } => {
            Err(anyhow!("text-to-speech feature not compiled in binary"))
        }
        #[cfg(feature = "spotify")]
        soundboards::Source::Spotify { id } => {
            let string_hash = utils::calculate_hash(&id).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                Ok(Some(file_path))
            } else {
                Ok(None)
            }
        }
        #[cfg(not(feature = "spotify"))]
        soundboards::Source::Spotify { id: _ } => {
            Err(anyhow!("spotify feature not compiled in binary"))
        }
    }
}

pub fn get_local_path_from_sound_config(sound: &soundboards::Sound) -> Result<PathBuf> {
    use std::io::{self, Write};

    match sound.get_source() {
        soundboards::Source::Http { url, headers } => {
            let mut headers_tuple = Vec::new();
            if let Some(headers) = &headers {
                for header in headers {
                    headers_tuple.push((header.name.clone(), header.value.clone()));
                }
            }
            download_file_if_needed(&url, headers_tuple)
        }
        soundboards::Source::Local { path } => resolve_local_sound_path(sound, PathBuf::from(path)),
        soundboards::Source::Youtube { id } => {
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
        soundboards::Source::TTS { ssml, lang } => {
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
        soundboards::Source::TTS { ssml: _, lang: _ } => {
            Err(anyhow!("text-to-speech feature not compiled in binary"))
        }

        #[cfg(feature = "spotify")]
        soundboards::Source::Spotify { id } => {
            let string_hash = utils::calculate_hash(&id).to_string();
            let mut file_path = std::env::temp_dir();
            file_path.push(string_hash);
            if file_path.exists() {
                return Ok(file_path);
            }

            download_from_spotify(file_path, id)
        }
        #[cfg(not(feature = "spotify"))]
        soundboards::Source::Spotify { id: _ } => {
            Err(anyhow!("spotify feature not compiled in binary"))
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

// TODO: wait for libspotify to get tokio 0.2 support
#[cfg(feature = "spotify")]
fn download_from_spotify(file_path: PathBuf, id: &str) -> Result<PathBuf> {
    use librespot::audio::{AudioDecrypt, AudioFile, StreamLoaderController};
    use librespot::core::authentication::Credentials;
    use librespot::core::config::SessionConfig;
    use librespot::core::session::Session;
    use librespot::core::spotify_id::SpotifyId;
    use librespot::metadata::{AudioItem, FileFormat};
    use librespot::playback::config::PlayerConfig;
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio_core::reactor::Core;

    let session_config = SessionConfig::default();
    let mut core = Core::new()?;
    let handle = core.handle();

    let credentials = Credentials::with_password(
        std::env::var("SB_SPOTIFY_USER")?,
        std::env::var("SB_SPOTIFY_PASS")?,
    );

    let track = match SpotifyId::from_base62(&id) {
        Ok(track) => track,
        Err(err) => {
            return Err(anyhow!("Unable to parse spotify id {:?}", err));
        }
    };

    println!("Connecting ..");
    let session = core.run(Session::connect(session_config, credentials, None, handle))?;

    let mut audio = match core.run(AudioItem::get_audio_item(&session, track)) {
        Ok(audio) => audio,
        Err(err) => {
            return Err(anyhow!("Unable to load audio item. {:?}", err));
        }
    };

    info!("Loading <{}> with Spotify URI <{}>", audio.name, audio.uri);

    if !audio.available {
        audio = {
            if let Some(audio) = audio
                .alternatives
                .unwrap_or_default()
                .iter()
                .find_map(|alt| {
                    if let Ok(audio) = core.run(AudioItem::get_audio_item(&session, *alt)) {
                        if audio.available {
                            return Some(audio.clone());
                        }
                    }
                    return None;
                })
            {
                audio
            } else {
                Err(anyhow!("audio <{}> is not available", audio.uri))
            }
        };
    }

    let duration_ms = audio.duration as u32;

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

    let file_id = match audio.files.get(&format) {
        Some(&file_id) => file_id,
        None => {
            return Err(anyhow!(
                "<{}> in not available in format {:?}",
                audio.name,
                format
            ));
        }
    };

    const bytes_per_second: usize = 64 * 1024;
    let play_from_beginning = true;

    let key = session.audio_key().request(track, file_id);
    let encrypted_file = AudioFile::open(&session, file_id, bytes_per_second, play_from_beginning);

    let encrypted_file = match core.run(encrypted_file) {
        Ok(encrypted_file) => encrypted_file,
        Err(err) => {
            return Err(anyhow!("Unable to load encrypted file. {:?}", err));
        }
    };

    let mut stream_loader_controller = encrypted_file.get_stream_loader_controller();

    stream_loader_controller.set_stream_mode();

    let key = match core.run(key) {
        Ok(key) => key,
        Err(err) => {
            return Err(anyhow!("Unable to load decryption key. {:?}", err));
        }
    };

    let mut decrypted_file = AudioDecrypt::new(key, encrypted_file);

    println!("<{}> ({} ms) loaded", audio.name, audio.duration);

    let mut buffer = Vec::new();

    let finished = std::sync::Arc::new(AtomicBool::new(false));
    let finished_clone = finished.clone();
    let file_path_clone = file_path.clone();
    std::thread::spawn(move || {
        if let Err(err) = decrypted_file.seek(std::io::SeekFrom::Start(0xa7)) {
            finished_clone.store(true, Ordering::Relaxed);
            error!("Unable to seek file. {}", err);
            return;
        }
        if let Err(err) = decrypted_file.read_to_end(&mut buffer) {
            finished_clone.store(true, Ordering::Relaxed);
            error!("Unable to read file. {}", err);
            return;
        }
        if let Err(err) = std::fs::write(&file_path_clone, &buffer) {
            error!("Unable to write file. {}", err);
        }
        finished_clone.store(true, Ordering::Relaxed);
    });

    loop {
        core.turn(Some(std::time::Duration::from_millis(50)));
        if finished.load(Ordering::Relaxed) {
            break;
        }
    }

    if file_path.exists() {
        Ok(file_path)
    } else {
        return Err(anyhow!("Unknown spotify error"));
    }

    // use futures::{future, Future};
    // use librespot::audio::{AudioDecrypt, AudioFile, StreamLoaderController};
    // use librespot::core::authentication::Credentials;
    // use librespot::core::config::SessionConfig;
    // use librespot::core::session::Session;
    // use librespot::core::spotify_id::SpotifyId;
    // use librespot::metadata::{AudioItem, FileFormat};
    // use librespot::playback::config::PlayerConfig;
    // use std::io::{Read, Seek, SeekFrom};
    // use tokio::runtime::{Builder, Runtime};

    // let mut runtime = Builder::new()
    //     .basic_scheduler()
    //     .enable_all()
    //     .build()
    //     .unwrap();
    // let handle = runtime.handle();

    // let session_config = SessionConfig::default();

    // let credentials = Credentials::with_password(
    //     std::env::var("SB_SPOTIFY_USER").unwrap(),
    //     std::env::var("SB_SPOTIFY_PASS").unwrap(),
    // );

    // let track = SpotifyId::from_base62(&id).unwrap();

    // println!("Connecting ..");
    // let session = runtime
    //     .block_on(Session::connect(session_config, credentials, None, handle))
    //     .unwrap();

    // let audio = match runtime.block_on(AudioItem::get_audio_item(&session, track)) {
    //     Ok(audio) => audio,
    //     Err(err) => {
    //         return Err(anyhow!("Unable to load audio item. {}", err));
    //     }
    // };

    // info!("Loading <{}> with Spotify URI <{}>", audio.name, audio.uri);

    // let duration_ms = audio.duration as u32;

    // // (Most) podcasts seem to support only 96 bit Vorbis, so fall back to it
    // let formats = [
    //     FileFormat::OGG_VORBIS_96,
    //     FileFormat::OGG_VORBIS_160,
    //     FileFormat::OGG_VORBIS_320,
    // ];
    // let format = formats
    //     .iter()
    //     .find(|format| audio.files.contains_key(format))
    //     .unwrap();

    // let file_id = match audio.files.get(&format) {
    //     Some(&file_id) => file_id,
    //     None => {
    //         return Err(anyhow!(
    //             "<{}> in not available in format {:?}",
    //             audio.name,
    //             format
    //         ));
    //     }
    // };

    // const bytes_per_second: usize = 64 * 1024;
    // let play_from_beginning = true;

    // let key = session.audio_key().request(track, file_id);
    // let encrypted_file = AudioFile::open(&session, file_id, bytes_per_second, play_from_beginning);

    // let encrypted_file = match runtime.block_on(encrypted_file) {
    //     Ok(encrypted_file) => encrypted_file,
    //     Err(err) => {
    //         return Err(anyhow!("Unable to load encrypted file. {}", err));
    //     }
    // };

    // let mut stream_loader_controller = encrypted_file.get_stream_loader_controller();

    // stream_loader_controller.set_stream_mode();

    // let key = match runtime.block_on(key) {
    //     Ok(key) => key,
    //     Err(err) => {
    //         return Err(anyhow!("Unable to load decryption key. {}", err));
    //     }
    // };

    // let mut decrypted_file = AudioDecrypt::new(key, encrypted_file);

    // println!("<{}> ({} ms) loaded", audio.name, audio.duration);

    // let mut buffer = Vec::new();

    // decrypted_file.seek(std::io::SeekFrom::Start(0xa7));
    // decrypted_file.read_to_end(&mut buffer)?;

    // std::fs::write(file_path, buffer)?;

    // Ok(file_path)
}
