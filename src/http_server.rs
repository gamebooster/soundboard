use super::config;
use super::sound;
use anyhow::{anyhow, Context, Result};

use log::{error, info, trace, warn};
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::{reject, Filter, Rejection, Reply};

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundRequest {
    name: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct VolumeRequest {
    volume: f32,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundboardInfo {
    name: String,
    hotkey: String,
    id: usize,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundInfo {
    name: String,
    hotkey: String,
    id: usize,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundActiveInfo {
    name: String,
    hotkey: String,
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

#[derive(Debug)]
struct UnknownSoundboardError(usize);
impl reject::Reject for UnknownSoundboardError {}

#[derive(Debug)]
struct UnknownSoundError(usize);
impl reject::Reject for UnknownSoundError {}

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
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        code = StatusCode::METHOD_NOT_ALLOWED;
        title = "MethodNotAllowed";
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        title = "InternalServerError";
    }

    let json = warp::reply::json(&ResultErrors::with_error(
        &code.as_u16().to_string(),
        title,
        &detail,
    ));

    Ok(warp::reply::with_status(json, code))
}

fn check_soundboard_index(
    config_file: config::MainConfig,
) -> impl Filter<Extract = ((config::SoundboardConfig, usize),), Error = Rejection> + Clone {
    warp::path("soundboards")
        .and(warp::path::param::<usize>())
        .and_then(move |index: usize| {
            let maybe_soundboard = config_file.soundboards.as_ref().unwrap().iter().nth(index);
            if let Some(soundboard) = maybe_soundboard {
                futures::future::ok((soundboard.clone(), index))
            } else {
                futures::future::err(reject::custom(UnknownSoundboardError(index)))
            }
        })
}

fn check_sound_index(
    config_file: config::MainConfig,
) -> impl Filter<Extract = ((config::SoundConfig, usize),), Error = Rejection> + Clone {
    check_soundboard_index(config_file)
        .and(warp::path("sounds"))
        .and(warp::path::param::<usize>())
        .and_then(
            move |soundboard: (config::SoundboardConfig, _), index: usize| {
                let maybe_sound = soundboard.0.sounds.as_ref().unwrap().iter().nth(index);
                if let Some(sound) = maybe_sound {
                    futures::future::ok((sound.clone(), index))
                } else {
                    futures::future::err(reject::custom(UnknownSoundError(index)))
                }
            },
        )
}

#[tokio::main]
pub async fn run(
    config_file: config::MainConfig,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    gui_receiver: crossbeam_channel::Receiver<sound::Message>,
) {
    let config_file_clone = config_file.clone();
    let soundboards_route = warp::path!("soundboards").map(move || {
        let mut soundboards = Vec::new();
        for (id, soundboard) in config_file_clone
            .soundboards
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
        {
            soundboards.push(StrippedSoundboardInfo {
                name: soundboard.name.clone().unwrap_or_default(),
                hotkey: soundboard.hotkey.clone().unwrap_or_default(),
                id,
            });
        }
        warp::reply::json(&ResultData::with_data(soundboards))
    });

    let config_file_clone = config_file.clone();
    let soundboards_soundboard_route = check_soundboard_index(config_file_clone)
        .and(warp::path::end())
        .map(
            move |(soundboard, index): (config::SoundboardConfig, usize)| {
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundboardInfo {
                        name: soundboard.name.unwrap_or_default(),
                        hotkey: soundboard.hotkey.unwrap_or_default(),
                        id: index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let config_file_clone = config_file.clone();
    let soundboards_sounds_route = check_soundboard_index(config_file_clone)
        .and(warp::path!("sounds"))
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
                                hotkey: a.hotkey.clone().unwrap_or_default(),
                                id: v.len(),
                            });
                            v
                        }),
                )),
                warp::http::StatusCode::OK,
            )
        });

    let config_file_clone = config_file.clone();
    let soundboards_sounds_sound_route = check_sound_index(config_file_clone)
        .and(warp::path::end())
        .map(move |(sound, index): (config::SoundConfig, usize)| {
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                    name: sound.name.clone(),
                    hotkey: sound.hotkey.unwrap_or_default(),
                    id: index,
                })),
                warp::http::StatusCode::OK,
            )
        });

    let gui_sender_clone = gui_sender.clone();
    let config_file_clone = config_file.clone();
    let sounds_play_route = check_sound_index(config_file_clone)
        .and(warp::path!("play"))
        .and(warp::post())
        .map(move |(sound, _): (config::SoundConfig, usize)| {
            gui_sender_clone
                .send(sound::Message::PlaySound(
                    sound.clone(),
                    sound::SoundDevices::Both,
                ))
                .unwrap();
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(format!(
                    "PlaySound {:?}",
                    &sound.path
                ))),
                warp::http::StatusCode::OK,
            )
        });

    let gui_sender_clone = gui_sender.clone();
    let config_file_clone = config_file.clone();
    let sounds_stop_route = check_sound_index(config_file_clone)
        .and(warp::path!("stop"))
        .and(warp::post())
        .map(move |(sound, _): (config::SoundConfig, usize)| {
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
        });

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
    let sounds_active_route = warp::path!("sounds" / "active").map(move || {
        gui_sender_clone
            .send(sound::Message::PlayStatus(Vec::new()))
            .unwrap();
        match gui_receiver.recv() {
            Ok(sound::Message::PlayStatus(sounds)) => {
                let mut sound_info: Vec<StrippedSoundActiveInfo> = Vec::new();
                for sound in sounds {
                    sound_info.push(StrippedSoundActiveInfo {
                        name: sound.0.name,
                        hotkey: sound.0.hotkey.unwrap_or_default(),
                        play_duration: sound.1.as_secs_f32(),
                        total_duration: sound
                            .2
                            .unwrap_or_else(|| std::time::Duration::from_secs(0))
                            .as_secs_f32(),
                    })
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(sound_info)),
                    warp::http::StatusCode::OK,
                )
            }
            Err(err) => warp::reply::with_status(
                warp::reply::json(&ResultErrors::with_error(
                    "500",
                    &"Internal Server Error",
                    &format!("{}", err),
                )),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
            _ => warp::reply::with_status(
                warp::reply::json(&ResultErrors::with_error(
                    "500",
                    &"Internal Server Error",
                    &"",
                )),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    });

    let help_api = warp::path::end()
        .map(|| "This is the Soundboard API. Try calling /api/soundboards or /api/sounds/active");

    let routes = (warp::path("api").and(
        soundboards_route
            .or(soundboards_soundboard_route)
            .or(soundboards_sounds_route)
            .or(soundboards_sounds_sound_route)
            .or(sounds_play_route)
            .or(sounds_stop_route)
            .or(sounds_stop_all_route)
            .or(sounds_active_route)
            .or(sounds_set_volume)
            .or(help_api),
    ))
    .or(warp::get().and(warp::fs::dir("web")))
    .recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
    unreachable!();
}
