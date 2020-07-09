use super::config;
use super::sound;
use anyhow::{anyhow, Context, Result};

use log::{error, info, trace, warn};
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::{reject, sse::ServerSentEvent, Filter, Rejection, Reply};
extern crate futures;
use super::hotkey;
use bytes::BufMut;
use futures::{Future, Stream, StreamExt, TryFutureExt, TryStreamExt};
use tokio::sync::mpsc;

#[derive(Debug, Deserialize, Clone, Serialize)]
struct HotkeyRegisterRequest {
    hotkey: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct HotkeyFireEvent {
    hotkey: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct SoundPlayRequest {
    devices: sound::SoundDevices,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct VolumeRequest {
    volume: f32,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundboardChangeRequest {
    name: String,
    hotkey: Option<String>,
    position: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundChangeRequest {
    name: Option<String>,
    hotkey: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundAddRequest {
    name: String,
    hotkey: Option<String>,
    path: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundCopyRequest {
    source_soundboard_id: usize,
    source_sound_id: usize,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundboardInfo {
    name: String,
    hotkey: Option<String>,
    position: Option<usize>,
    id: usize,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundInfo {
    name: String,
    hotkey: Option<String>,
    id: usize,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct PlayStatusResponse {
    volume: f32,
    sounds: Vec<StrippedSoundActiveInfo>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct StrippedSoundActiveInfo {
    status: sound::SoundStatus,
    name: String,
    hotkey: Option<String>,
    total_duration: f32,
    play_duration: f32,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct ResultData<T> {
    data: T,
}

impl<T> ResultData<T> {
    pub fn with_data(data: T) -> ResultData<T> {
        ResultData { data }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct ErrorObject {
    code: String,
    title: String,
    detail: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct ResultErrors {
    errors: Vec<ErrorObject>,
}

impl ResultErrors {
    pub fn with_error(code: &str, title: &str, detail: &str) -> ResultErrors {
        ResultErrors {
            errors: vec![ErrorObject {
                code: code.to_string(),
                title: title.to_string(),
                detail: detail.to_string(),
            }],
        }
    }
}

fn format_json_error<T: std::fmt::Display>(err: T) -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(
        warp::reply::json(&ResultErrors::with_error(
            "500",
            &"Internal Server Error",
            &format!("{:#}", err),
        )),
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    )
}

#[derive(Debug)]
enum HotkeyMessage {
    Pressed(String),
}

#[derive(Debug)]
struct UnknownSoundboardError(usize);
impl reject::Reject for UnknownSoundboardError {}

#[derive(Debug)]
struct UnknownSoundError(usize);
impl reject::Reject for UnknownSoundError {}

#[derive(Debug)]
struct UnknownServerError(String);
impl reject::Reject for UnknownServerError {}

// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let title;
    let mut detail = String::new();

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        title = "MethodNotFound";
    } else if let Some(unknown_soundboard_error) = err.find::<UnknownSoundboardError>() {
        code = StatusCode::NOT_FOUND;
        title = "UnknownSoundboardError";
        detail = format!("no soundboard at index {}", unknown_soundboard_error.0);
    } else if let Some(unknown_sound_error) = err.find::<UnknownSoundError>() {
        code = StatusCode::NOT_FOUND;
        title = "UnknownSoundError";
        detail = format!("no sound at index {}", unknown_sound_error.0);
    } else if let Some(unknown_sound_error) = err.find::<UnknownServerError>() {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        title = "UnknownServerError";
        detail = unknown_sound_error.0.clone();
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        code = StatusCode::METHOD_NOT_ALLOWED;
        title = "MethodNotAllowed";
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        title = "InternalServerError";
        detail = format!("{:#?}", err);
    }

    let json = warp::reply::json(&ResultErrors::with_error(
        &code.as_u16().to_string(),
        title,
        &detail,
    ));

    Ok(warp::reply::with_status(json, code))
}

fn check_soundboard_index(
) -> impl Filter<Extract = ((config::SoundboardConfig, usize),), Error = Rejection> + Clone {
    warp::path("soundboards")
        .and(warp::path::param::<usize>())
        .and_then(move |index: usize| {
            let config = config::MainConfig::read();
            let maybe_soundboard = config.soundboards.get(index);
            if let Some(soundboard) = maybe_soundboard {
                futures::future::ok((soundboard.clone(), index))
            } else {
                futures::future::err(reject::custom(UnknownSoundboardError(index)))
            }
        })
}

fn check_sound_index() -> impl Filter<
    Extract = ((config::SoundboardConfig, usize, config::SoundConfig, usize),),
    Error = Rejection,
> + Clone {
    check_soundboard_index()
        .and(warp::path("sounds"))
        .and(warp::path::param::<usize>())
        .and_then(
            move |(soundboard, soundboard_index): (config::SoundboardConfig, usize),
                  sound_index: usize| {
                let maybe_sound = &soundboard.sounds.as_ref().unwrap().get(sound_index);
                if let Some(sound) = maybe_sound {
                    let sound = (*sound).clone();
                    futures::future::ok((soundboard, soundboard_index, sound, sound_index))
                } else {
                    futures::future::err(reject::custom(UnknownSoundError(sound_index)))
                }
            },
        )
}

#[tokio::main]
pub async fn run(
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    gui_receiver: crossbeam_channel::Receiver<sound::Message>,
) {
    let soundboards_route = warp::path!("soundboards").map(move || {
        let mut soundboards = Vec::new();

        // if let Err(err) = config::MainConfig::reload_from_disk() {
        //     error!("{:#}", err);
        //     return format_json_error(err);
        // }

        for (id, soundboard) in config::MainConfig::read().soundboards.iter().enumerate() {
            soundboards.push(StrippedSoundboardInfo {
                name: soundboard.name.clone(),
                hotkey: soundboard.hotkey.clone(),
                position: soundboard.position,
                id,
            });
        }
        warp::reply::with_status(
            warp::reply::json(&ResultData::with_data(soundboards)),
            warp::http::StatusCode::OK,
        )
    });

    let soundboards_soundboard_route = check_soundboard_index()
        .and(warp::path::end())
        .and(warp::get())
        .map(
            move |(soundboard, index): (config::SoundboardConfig, usize)| {
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundboardInfo {
                        name: soundboard.name,
                        hotkey: soundboard.hotkey,
                        id: index,
                        position: soundboard.position,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_soundboard_change_route = check_soundboard_index()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .map(
            move |(old_soundboard, index): (config::SoundboardConfig, usize),
                  soundboard_change_request: SoundboardChangeRequest| {
                let mut new_soundboard = old_soundboard;
                if soundboard_change_request.hotkey.is_some()
                    && !soundboard_change_request
                        .hotkey
                        .as_ref()
                        .unwrap()
                        .is_empty()
                {
                    new_soundboard.hotkey = soundboard_change_request.hotkey;
                }
                new_soundboard.name = soundboard_change_request.name;
                if soundboard_change_request.position.is_some() {
                    new_soundboard.position = soundboard_change_request.position;
                }

                if let Err(err) = config::MainConfig::change_soundboard(index, new_soundboard) {
                    return format_json_error(err);
                }
                let soundboard = &config::MainConfig::read().soundboards[index];
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundboardInfo {
                        name: soundboard.name.clone(),
                        hotkey: soundboard.hotkey.clone(),
                        id: index,
                        position: soundboard.position,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_route = check_soundboard_index()
        .and(warp::path!("sounds"))
        .and(warp::get())
        .map(move |(soundboard, _): (config::SoundboardConfig, usize)| {
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(
                    &soundboard
                        .sounds
                        .as_ref()
                        .unwrap()
                        .iter()
                        .fold(Vec::new(), |mut v, a| {
                            v.push(StrippedSoundInfo {
                                name: a.name.clone(),
                                hotkey: a.hotkey.clone(),
                                id: v.len(),
                            });
                            v
                        }),
                )),
                warp::http::StatusCode::OK,
            )
        });

    type AddSoundMultipartResult = ((config::SoundboardConfig, usize), Vec<(String, Vec<u8>)>);

    let soundboards_soundboard_add_sound_upload_route = check_soundboard_index()
        .and(warp::path!("sounds"))
        .and(warp::post())
        .and(warp::multipart::form().max_length(1024 * 1024 * 100))
        .and_then(
            move |(soundboard, index), form: warp::multipart::FormData| async move {
                // Collect the fields into (name, value): (String, Vec<u8>)
                let part: Result<Vec<(String, Vec<u8>)>, warp::Rejection> = form
                    .and_then(|part| {
                        let name = part.name().to_string();
                        let value = part.stream().try_fold(Vec::new(), |mut vec, data| {
                            vec.put(data);
                            async move { Ok(vec) }
                        });
                        value.map_ok(move |vec| (name, vec))
                    })
                    .try_collect()
                    .await
                    .map_err(|e| {
                        reject::custom(UnknownServerError(format!("unknown multipart err {}", e)))
                    });
                let final_result: Result<AddSoundMultipartResult, warp::Rejection> = {
                    if let Ok(part) = part {
                        Ok(((soundboard, index), part))
                    } else {
                        Err(part.unwrap_err())
                    }
                };
                final_result
            },
        )
        .map(
            move |((mut soundboard, index), uploads): AddSoundMultipartResult| {
                let mut added_sounds = Vec::new();
                for (upload_name, upload_data) in uploads {
                    trace!("Received {} with size {}", upload_name, upload_data.len());

                    let mut sound_path = config::get_soundboard_sound_directory(
                        std::path::PathBuf::from_str(&soundboard.path)
                            .unwrap()
                            .as_path(),
                    )
                    .unwrap();
                    if !&sound_path.exists() {
                        if let Err(err) = std::fs::create_dir(&sound_path) {
                            return format_json_error(err);
                        }
                    }
                    sound_path.push(upload_name.clone());

                    trace!("file_path {}", sound_path.display());

                    if sound_path.exists() {
                        continue;
                    }

                    if let Err(err) = std::fs::write(&sound_path, upload_data) {
                        return format_json_error(err);
                    }

                    let sound_config = config::SoundConfig {
                        name: upload_name.clone(),
                        path: upload_name,
                        hotkey: None,
                        headers: None,
                        full_path: sound_path.to_str().unwrap().to_owned(),
                    };

                    added_sounds.push(StrippedSoundInfo {
                        name: sound_config.name.clone(),
                        hotkey: sound_config.hotkey.clone(),
                        id: soundboard.sounds.as_ref().unwrap().len(),
                    });

                    soundboard.sounds.as_mut().unwrap().push(sound_config);
                }

                if let Err(err) = config::MainConfig::change_soundboard(index, soundboard) {
                    return format_json_error(err);
                }

                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(added_sounds)),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_soundboard_copy_sound_route = check_soundboard_index()
        .and(warp::path!("sounds"))
        .and(warp::post())
        .and(warp::header::exact("x-method", "copy"))
        .and(warp::body::json())
        .map(
            move |(mut soundboard, index): (config::SoundboardConfig, usize),
                  sound_copy_request: SoundCopyRequest| {
                let main_config = config::MainConfig::read();
                let source_soundboard = main_config
                    .soundboards
                    .get(sound_copy_request.source_soundboard_id);

                if source_soundboard.is_none() {
                    return format_json_error("invalid source soundboard");
                }

                let source_sound = source_soundboard
                    .as_ref()
                    .unwrap()
                    .sounds
                    .as_ref()
                    .unwrap()
                    .get(sound_copy_request.source_sound_id);

                if source_sound.is_none() {
                    return format_json_error("invalid source sound");
                }

                let source_sound = source_sound.unwrap();

                if !source_sound.path.starts_with("http") {
                    let source_sound_path =
                        std::path::PathBuf::from_str(&source_sound.path).unwrap();
                    if source_sound_path.is_relative() {
                        let mut new_sound_path = config::get_soundboard_sound_directory(
                            std::path::PathBuf::from_str(&soundboard.path)
                                .unwrap()
                                .as_path(),
                        )
                        .unwrap();
                        new_sound_path.push(&source_sound.path);
                        if let Err(err) = std::fs::copy(&source_sound.full_path, &new_sound_path) {
                            return format_json_error(err);
                        }
                    }
                }

                soundboard
                    .sounds
                    .as_mut()
                    .unwrap()
                    .push(source_sound.clone());

                if let Err(err) = config::MainConfig::change_soundboard(index, soundboard) {
                    return format_json_error(err);
                }
                let main_config = config::MainConfig::read();
                let sounds = main_config.soundboards[index].sounds.as_ref().unwrap();
                let sound_index = sounds.len() - 1;
                let sound = sounds[sound_index].clone();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_soundboard_add_sound_route = check_soundboard_index()
        .and(warp::path!("sounds"))
        .and(warp::post())
        .and(warp::header::exact("x-method", "create"))
        .and(warp::body::json())
        .map(
            move |(old_soundboard, index): (config::SoundboardConfig, usize),
                  sound_add_request: SoundAddRequest| {
                let mut new_soundboard = old_soundboard;

                new_soundboard
                    .sounds
                    .as_mut()
                    .unwrap()
                    .push(config::SoundConfig {
                        name: sound_add_request.name,
                        path: sound_add_request.path.clone(),
                        hotkey: sound_add_request.hotkey,
                        headers: None,
                        full_path: sound_add_request.path,
                    });

                if let Err(err) = config::MainConfig::change_soundboard(index, new_soundboard) {
                    return format_json_error(err);
                }
                let main_config = config::MainConfig::read();
                let sounds = main_config.soundboards[index].sounds.as_ref().unwrap();
                let sound_index = sounds.len() - 1;
                let sound = sounds[sound_index].clone();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_sound_route = check_sound_index()
        .and(warp::path::end())
        .and(warp::get())
        .map(
            move |(_soundboard, _soundboard_index, sound, index): (
                config::SoundboardConfig,
                usize,
                config::SoundConfig,
                usize,
            )| {
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name.clone(),
                        hotkey: sound.hotkey,
                        id: index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_delete_sound_route = check_sound_index()
        .and(warp::path::end())
        .and(warp::delete())
        .map(
            move |(mut soundboard, soundboard_index, sound, sound_index): (
                config::SoundboardConfig,
                usize,
                config::SoundConfig,
                usize,
            )| {
                soundboard.sounds.as_mut().unwrap().remove(sound_index);

                if let Err(err) =
                    config::MainConfig::change_soundboard(soundboard_index, soundboard)
                {
                    return format_json_error(err);
                }

                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let gui_sender_clone = gui_sender.clone();
    let sounds_play_route = check_sound_index()
        .and(warp::path!("play"))
        .and(warp::post())
        .and(warp::body::json())
        .map(
            move |(_soundboard, _soundboard_index, sound, _): (
                config::SoundboardConfig,
                usize,
                config::SoundConfig,
                usize,
            ),
                  request: SoundPlayRequest| {
                gui_sender_clone
                    .send(sound::Message::PlaySound(sound.clone(), request.devices))
                    .unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(format!(
                        "PlaySound {:?}",
                        &sound.path
                    ))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let gui_sender_clone = gui_sender.clone();
    let sounds_stop_route = check_sound_index()
        .and(warp::path!("stop"))
        .and(warp::post())
        .map(
            move |(_soundboard, _soundboard_index, sound, _sound_index): (
                config::SoundboardConfig,
                usize,
                config::SoundConfig,
                usize,
            )| {
                gui_sender_clone
                    .send(sound::Message::StopSound(sound.clone()))
                    .unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(format!(
                        "StopSound {:?}",
                        &sound.path
                    ))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let gui_sender_clone = gui_sender.clone();
    let sounds_set_volume = warp::path!("sounds" / "volume")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |volume: VolumeRequest| {
            gui_sender_clone
                .send(sound::Message::SetVolume(volume.volume))
                .unwrap();
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data("SetVolume".to_string())),
                warp::http::StatusCode::OK,
            )
        });

    let gui_sender_clone = gui_sender.clone();
    let sounds_stop_all_route =
        warp::path!("sounds" / "stopall")
            .and(warp::post())
            .map(move || {
                gui_sender_clone.send(sound::Message::StopAll).unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data("StopAllSound".to_string())),
                    warp::http::StatusCode::OK,
                )
            });

    let gui_sender_clone = gui_sender.clone();
    let sounds_active_route = warp::path!("sounds" / "active")
        .and(warp::get())
        .map(move || {
            gui_sender_clone
                .send(sound::Message::PlayStatus(Vec::new(), 0.0))
                .unwrap();
            match gui_receiver.recv() {
                Ok(sound::Message::PlayStatus(sounds, volume)) => {
                    let mut sound_info: Vec<StrippedSoundActiveInfo> = Vec::new();
                    for sound in sounds {
                        sound_info.push(StrippedSoundActiveInfo {
                            status: sound.0,
                            name: sound.1.name,
                            hotkey: sound.1.hotkey,
                            play_duration: sound.2.as_secs_f32(),
                            total_duration: sound
                                .3
                                .unwrap_or_else(|| std::time::Duration::from_secs(0))
                                .as_secs_f32(),
                        })
                    }
                    let play_status_response = PlayStatusResponse {
                        sounds: sound_info,
                        volume,
                    };
                    warp::reply::with_status(
                        warp::reply::json(&ResultData::with_data(play_status_response)),
                        warp::http::StatusCode::OK,
                    )
                }
                Err(err) => format_json_error(err),
                _ => format_json_error("unknown error"),
            }
        });

    fn sse_event(id: String) -> Result<impl ServerSentEvent, Infallible> {
        Ok(warp::sse::data(id))
    }

    let senders: Arc<std::sync::Mutex<Vec<mpsc::UnboundedSender<HotkeyMessage>>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let senders_filter = warp::any().map(move || senders.clone());

    let hotkey_events_route = warp::path!("hotkeys" / "events")
        .and(warp::get())
        .and(senders_filter.clone())
        .map(
            move |senders: Arc<std::sync::Mutex<Vec<mpsc::UnboundedSender<HotkeyMessage>>>>| {
                let event_stream = || {
                    let (tx, rx) = mpsc::unbounded_channel::<HotkeyMessage>();
                    senders.lock().unwrap().push(tx);
                    rx.map(|msg| match msg {
                        HotkeyMessage::Pressed(id) => sse_event(id),
                    })
                };
                warp::sse::reply(warp::sse::keep_alive().stream(event_stream()))
            },
        );

    let hotkey_manager = std::sync::Arc::new(std::sync::Mutex::new(hotkey::HotkeyManager::new()));
    let hotkey_manager_clone = hotkey_manager.clone();

    let hotkey_register_route = warp::path!("hotkeys")
        .and(warp::post())
        .and(warp::body::json())
        .and(senders_filter)
        .map(move |hotkey_request: HotkeyRegisterRequest, senders: Arc<std::sync::Mutex<Vec<mpsc::UnboundedSender<HotkeyMessage>>>>| {
            let hotkey = match config::parse_hotkey(&hotkey_request.hotkey) {
                Ok(key) => key,
                Err(err) => return format_json_error(err),
            };

            let hotkey_request_clone = hotkey_request.clone();
            if let Err(err) = hotkey_manager_clone.lock().unwrap().register(hotkey, move || {
                for sender in senders.lock().unwrap().iter() {
                    if let Err(err) = sender.send(HotkeyMessage::Pressed(hotkey_request_clone.hotkey.clone())) {
                        warn!("failed to send hotkey server message {}", err);
                    }
                }
            }) {
                return format_json_error(err);
            };
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(hotkey_request)),
                warp::http::StatusCode::OK,
            )
        });

    let help_api = warp::path::end()
        .and(warp::get())
        .map(|| "This is the Soundboard API. Try calling /api/soundboards or /api/sounds/active");

    let mut web_path = config::get_soundboards_path().unwrap();
    web_path.pop();
    web_path.push("web");

    if std::env::var("SB_WEB_DEV").is_ok() {
        web_path = std::path::PathBuf::from_str("web").unwrap();
    }

    let soundboard_routes = soundboards_route
        .or(soundboards_soundboard_change_route)
        .or(soundboards_soundboard_route);

    let soundboard_sound_routes = soundboards_sounds_route
        .or(soundboards_sounds_sound_route)
        .or(soundboards_soundboard_add_sound_upload_route)
        .or(soundboards_soundboard_add_sound_route)
        .or(soundboards_soundboard_copy_sound_route)
        .or(soundboards_sounds_delete_sound_route);

    let sound_thread_routes = sounds_play_route
        .or(sounds_stop_route)
        .or(sounds_stop_all_route)
        .or(sounds_active_route)
        .or(sounds_set_volume);

    let hotkey_routes = hotkey_events_route.or(hotkey_register_route);

    let routes = (warp::path("api").and(
        soundboard_routes
            .or(soundboard_sound_routes)
            .or(sound_thread_routes)
            .or(hotkey_routes)
            .or(help_api),
    ))
    .or(warp::get().and(warp::fs::dir(web_path)))
    .recover(handle_rejection);

    let socket_addr: std::net::SocketAddr = {
        if let Some(socket_addr) = &config::MainConfig::read().http_socket_addr {
            socket_addr.parse().expect("Unable to parse socket address")
        } else {
            ([0, 0, 0, 0], 3030).into()
        }
    };

    warp::serve(routes).run(socket_addr).await;
    unreachable!();
}
