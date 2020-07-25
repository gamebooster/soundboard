use super::config;
use super::sound;
use anyhow::{anyhow, Context, Result};

use super::hotkey;
use bytes::BufMut;
use futures::{Future, Stream, StreamExt, TryFutureExt, TryStreamExt};
use log::{error, info, trace, warn};
use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use ulid::Ulid;
use warp::http::StatusCode;
use warp::{reject, sse::ServerSentEvent, Filter, Rejection, Reply};

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
    name: String,
    hotkey: Option<String>,
    path: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundAddRequest {
    name: String,
    hotkey: Option<String>,
    path: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct SoundCopyRequest {
    source_soundboard_id: Ulid,
    source_sound_id: Ulid,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundboardInfo {
    name: String,
    hotkey: Option<String>,
    position: Option<usize>,
    id: Ulid,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct ExtendedSoundboardInfo {
    name: String,
    hotkey: Option<String>,
    position: Option<usize>,
    id: Ulid,
    sounds: Vec<StrippedSoundInfo>,
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct StrippedSoundInfo {
    name: String,
    hotkey: Option<String>,
    path: String,
    id: Ulid,
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
struct UnknownSoundboardError(Ulid);
impl reject::Reject for UnknownSoundboardError {}

#[derive(Debug)]
struct UnknownSoundError(Ulid);
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

fn check_soundboard_id(
) -> impl Filter<Extract = ((config::Soundboard, Ulid),), Error = Rejection> + Clone {
    warp::path("soundboards")
        .and(warp::path::param::<Ulid>())
        .and_then(move |id: ulid::Ulid| {
            let soundboard = config::get_soundboard(id);
            if let Some(soundboard) = soundboard {
                futures::future::ok((soundboard.clone(), index))
            } else {
                futures::future::err(reject::custom(UnknownSoundboardError(id)))
            }
        })
}

fn check_sound_id(
) -> impl Filter<Extract = ((config::Soundboard, Ulid, config::Sound, Ulid),), Error = Rejection> + Clone
{
    check_soundboard_id()
        .and(warp::path("sounds"))
        .and(warp::path::param::<Ulid>())
        .and_then(
            move |(soundboard, soundboard_id): (config::Soundboard, Ulid), sound_id: ulid::Ulid| {
                let sound = config::get_sound(soundboard_id, sound_id);
                if let Some(sound) = sound {
                    let sound = (*sound).clone();
                    futures::future::ok((soundboard, soundboard_id, sound, sound_id))
                } else {
                    futures::future::err(reject::custom(UnknownSoundError(sound_id)))
                }
            },
        )
}

type HotkeySenders = Arc<Mutex<Vec<mpsc::UnboundedSender<HotkeyMessage>>>>;

#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/web/"]
struct WebAsset;

#[tokio::main]
pub async fn run(
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    gui_receiver: crossbeam_channel::Receiver<sound::Message>,
) {
    let soundboards_route = warp::path!("soundboards")
        .and(
            warp::filters::query::raw()
                .or(warp::any().map(String::default))
                .unify(),
        )
        .map(move |query: String| {
            let mut soundboards = Vec::new();

            if query.contains("reload") {
                if let Err(err) = config::load_soundboards_from_disk() {
                    error!("{:#}", err);
                    return format_json_error(err);
                }
            }

            for (id, soundboard) in config::get_soundboards().iter() {
                soundboards.push(StrippedSoundboardInfo {
                    name: soundboard.get_name().to_owned(),
                    hotkey: soundboard.get_hotkey().clone(),
                    position: soundboard.get_position(),
                    id,
                });
            }
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(soundboards)),
                warp::http::StatusCode::OK,
            )
        });

    let soundboards_soundboard_route = check_soundboard_id()
        .and(warp::path::end())
        .and(warp::get())
        .map(move |(soundboard, index): (config::Soundboard, Ulid)| {
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(ExtendedSoundboardInfo {
                    name: soundboard.get_name().to_owned(),
                    hotkey: soundboard.hotkey.clone(),
                    id: index,
                    position: soundboard.position,
                    sounds: soundboard.sounds.iter().fold(Vec::new(), |mut v, a| {
                        v.push(StrippedSoundInfo {
                            name: a.get_name().clone(),
                            hotkey: a.hotkey.clone(),
                            id: a.id,
                            path: a.get_path(),
                        });
                        v
                    }),
                })),
                warp::http::StatusCode::OK,
            )
        });

    let soundboards_soundboard_change_route = check_soundboard_id()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .map(
            move |(old_soundboard, index): (config::Soundboard, Ulid),
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

    let soundboards_sounds_route = check_soundboard_id()
        .and(warp::path!("sounds"))
        .and(warp::get())
        .map(move |(soundboard, _): (config::Soundboard, Ulid)| {
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
                                path: a.path.clone(),
                                id: v.len(),
                            });
                            v
                        }),
                )),
                warp::http::StatusCode::OK,
            )
        });

    type AddSoundMultipartResult = (
        (config::Soundboard, Ulid, config::Sound, usize),
        Vec<(String, Vec<u8>)>,
    );

    let soundboards_soundboard_add_sound_upload_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::multipart::form().max_length(1024 * 1024 * 100))
        .and_then(
            move |(soundboard, soundboard_index, sound, sound_index),
                  form: warp::multipart::FormData| async move {
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
                        Ok(((soundboard, soundboard_index, sound, sound_index), part))
                    } else {
                        Err(part.unwrap_err())
                    }
                };
                final_result
            },
        )
        .map(
            move |((mut soundboard, soundboard_index, _sound, sound_index), uploads): AddSoundMultipartResult| {
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

                    if !sound_path.exists() {
                        if let Err(err) = std::fs::write(&sound_path, upload_data) {
                            return format_json_error(err);
                        }
                    }

                    let sound_config = config::Sound {
                        name: upload_name.clone(),
                        path: upload_name,
                        full_path: sound_path.to_str().unwrap().to_owned(),
                        .. config::Sound::default()
                    };

                    let new_id = sound_index + added_sounds.len();
                    soundboard.sounds.as_mut().unwrap().insert(new_id, sound_config.clone());

                    added_sounds.push(StrippedSoundInfo {
                        name: sound_config.name.clone(),
                        hotkey: sound_config.hotkey.clone(),
                        path: sound_config.path.clone(),
                        id: new_id,
                    });
                }

                if let Err(err) = config::MainConfig::change_soundboard(soundboard_index, soundboard) {
                    return format_json_error(err);
                }

                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(added_sounds)),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_soundboard_copy_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::exact("x-method", "copy"))
        .and(warp::body::json())
        .map(
            move |(mut soundboard, soundboard_index, _sound, sound_index): (
                config::Soundboard,
                usize,
                config::Sound,
                usize,
            ),
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
                        if source_sound.full_path != new_sound_path.to_str().unwrap() {
                            if let Err(err) =
                                std::fs::copy(&source_sound.full_path, &new_sound_path)
                            {
                                return format_json_error(err);
                            }
                        }
                    }
                }

                soundboard
                    .sounds
                    .as_mut()
                    .unwrap()
                    .insert(sound_index, source_sound.clone());

                if let Err(err) =
                    config::MainConfig::change_soundboard(soundboard_index, soundboard)
                {
                    return format_json_error(err);
                }
                let main_config = config::MainConfig::read();
                let sounds = main_config.soundboards[soundboard_index]
                    .sounds
                    .as_ref()
                    .unwrap();
                let sound = sounds[sound_index].clone();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        path: sound.path,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_soundboard_add_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::exact("x-method", "create"))
        .and(warp::body::json())
        .map(
            move |(mut soundboard, soundboard_index, _sound, sound_index): (
                config::Soundboard,
                usize,
                config::Sound,
                usize,
            ),
                  sound_add_request: SoundAddRequest| {
                soundboard.sounds.as_mut().unwrap().insert(
                    sound_index,
                    config::Sound {
                        name: sound_add_request.name,
                        path: sound_add_request.path.clone(),
                        hotkey: sound_add_request.hotkey,
                        ..config::Sound::default()
                    },
                );

                if let Err(err) =
                    config::MainConfig::change_soundboard(soundboard_index, soundboard)
                {
                    return format_json_error(err);
                }
                let main_config = config::MainConfig::read();
                let sounds = main_config.soundboards[soundboard_index]
                    .sounds
                    .as_ref()
                    .unwrap();
                let sound = sounds[sound_index].clone();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        path: sound.path,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::get())
        .map(
            move |(_soundboard, _soundboard_index, sound, index): (
                config::Soundboard,
                usize,
                config::Sound,
                usize,
            )| {
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name.clone(),
                        hotkey: sound.hotkey,
                        path: sound.path,
                        id: index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_change_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::put())
        .and(warp::body::json())
        .map(
            move |(mut soundboard, soundboard_index, _, sound_index): (
                config::Soundboard,
                usize,
                config::Sound,
                usize,
            ),
                  change_request: SoundChangeRequest| {
                let mut sound = &mut soundboard.sounds.as_mut().unwrap()[sound_index];

                if change_request.name.is_empty() {
                    return format_json_error("Invalid name specified");
                }
                sound.name = change_request.name;
                if let Some(hotkey) = change_request.hotkey {
                    if hotkey.is_empty() {
                        return format_json_error("Invalid hotkey specified");
                    }
                    if let Err(err) = config::parse_hotkey(&hotkey) {
                        return format_json_error(format!("Invalid hotkey specified: {}", err));
                    }
                    sound.hotkey = Some(hotkey);
                } else {
                    sound.hotkey = None;
                }

                if change_request.path.is_empty() {
                    return format_json_error("Invalid path specified");
                }
                sound.path = change_request.path.clone();
                sound.full_path = change_request.path;

                if let Err(err) =
                    config::MainConfig::change_soundboard(soundboard_index, soundboard)
                {
                    return format_json_error(err);
                }
                let main_config = config::MainConfig::read();
                let sounds = main_config.soundboards[soundboard_index]
                    .sounds
                    .as_ref()
                    .unwrap();
                let sound = sounds[sound_index].clone();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo {
                        name: sound.name,
                        hotkey: sound.hotkey,
                        path: sound.path,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_delete_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::delete())
        .map(
            move |(mut soundboard, soundboard_index, sound, sound_index): (
                config::Soundboard,
                usize,
                config::Sound,
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
                        path: sound.path,
                        id: sound_index,
                    })),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let gui_sender_clone = gui_sender.clone();
    let sounds_play_route = check_sound_id()
        .and(warp::path!("play"))
        .and(warp::post())
        .and(warp::body::json())
        .map(
            move |(_soundboard, _soundboard_index, sound, _): (
                config::Soundboard,
                usize,
                config::Sound,
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
    let sounds_stop_route = check_sound_id()
        .and(warp::path!("stop"))
        .and(warp::post())
        .map(
            move |(_soundboard, _soundboard_index, sound, _sound_index): (
                config::Soundboard,
                usize,
                config::Sound,
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

    fn sse_json(id: PlayStatusResponse) -> Result<impl ServerSentEvent, Infallible> {
        Ok(warp::sse::json(id))
    }

    let gui_sender_clone = gui_sender.clone();
    let gui_receiver_clone = gui_receiver.clone();
    let sounds_events_route = warp::path!("sounds" / "events")
        .and(warp::get())
        .map(move || {
            let gui_sender_clone = gui_sender_clone.clone();
            let gui_receiver_clone = gui_receiver_clone.clone();
            let event_stream =
                tokio::time::interval(tokio::time::Duration::from_millis(111)).map(move |_| loop {
                    gui_sender_clone
                        .send(sound::Message::PlayStatus(Vec::new(), 0.0))
                        .unwrap();
                    if let Ok(sound::Message::PlayStatus(sounds, volume)) =
                        gui_receiver_clone.recv()
                    {
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
                            });
                        }
                        let play_status_response = PlayStatusResponse {
                            sounds: sound_info,
                            volume,
                        };
                        return sse_json(play_status_response);
                    }
                });
            warp::sse::reply(warp::sse::keep_alive().stream(event_stream))
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

    let senders: HotkeySenders = HotkeySenders::default();
    let senders_filter = warp::any().map(move || senders.clone());

    fn sse_event(id: String) -> Result<impl ServerSentEvent, Infallible> {
        Ok(warp::sse::data(id))
    }

    let hotkey_events_route = warp::path!("hotkeys" / "events")
        .and(warp::get())
        .and(senders_filter.clone())
        .map(move |senders: HotkeySenders| {
            let event_stream = || {
                let (tx, rx) = mpsc::unbounded_channel::<HotkeyMessage>();
                senders.lock().push(tx);
                rx.map(|msg| match msg {
                    HotkeyMessage::Pressed(id) => sse_event(id),
                })
            };
            warp::sse::reply(warp::sse::keep_alive().stream(event_stream()))
        });

    let hotkey_manager = Arc::new(Mutex::new(hotkey::HotkeyManager::new()));

    let hotkey_manager_clone = hotkey_manager.clone();
    let hotkey_register_route = warp::path!("hotkeys")
        .and(warp::post())
        .and(warp::body::json())
        .and(senders_filter.clone())
        .map(
            move |hotkey_request: HotkeyRegisterRequest, senders: HotkeySenders| {
                let hotkey = match config::parse_hotkey(&hotkey_request.hotkey) {
                    Ok(key) => key,
                    Err(err) => return format_json_error(err),
                };

                let hotkey_request_clone = hotkey_request.clone();
                if let Err(err) = hotkey_manager_clone.lock().register(hotkey, move || {
                    let mut senders = senders.lock();
                    senders.retain(|s| {
                        if s.send(HotkeyMessage::Pressed(hotkey_request_clone.hotkey.clone()))
                            .is_err()
                        {
                            return false;
                        }
                        true
                    });
                }) {
                    if let hotkey::HotkeyManagerError::HotkeyAlreadyRegistered(_) = err {
                    } else {
                        return format_json_error(err);
                    }
                };
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(hotkey_request)),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let hotkey_manager_clone = hotkey_manager.clone();
    let hotkey_deregister_route = warp::path!("hotkeys")
        .and(warp::delete())
        .and(warp::body::json())
        .and(senders_filter)
        .map(
            move |hotkey_request: HotkeyRegisterRequest, senders: HotkeySenders| {
                let hotkey = match config::parse_hotkey(&hotkey_request.hotkey) {
                    Ok(key) => key,
                    Err(err) => return format_json_error(err),
                };

                // TODO: save hotkeys per sender
                if senders.lock().len() <= 1 {
                    if let Err(err) = hotkey_manager_clone.lock().unregister(&hotkey) {
                        return format_json_error(err);
                    };
                }

                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(hotkey_request)),
                    warp::http::StatusCode::OK,
                )
            },
        );

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
        .or(soundboards_sounds_delete_sound_route)
        .or(soundboards_sounds_change_sound_route);

    let sound_thread_routes = sounds_play_route
        .or(sounds_stop_route)
        .or(sounds_stop_all_route)
        .or(sounds_active_route)
        .or(sounds_set_volume)
        .or(sounds_events_route);

    let hotkey_routes = hotkey_events_route
        .or(hotkey_register_route)
        .or(hotkey_deregister_route);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "auth", "origin"])
        .allow_methods(vec!["GET", "POST", "PATCH", "PUT", "DELETE", "OPTIONS"]);

    async fn serve_index() -> Result<impl Reply, Rejection> {
        serve_impl("index.html")
    }

    async fn serve(path: warp::path::Tail) -> Result<impl Reply, Rejection> {
        serve_impl(path.as_str())
    }

    fn serve_impl(path: &str) -> Result<impl Reply, Rejection> {
        let asset = WebAsset::get(path).ok_or_else(warp::reject::not_found)?;
        let mime = mime_guess::from_path(path).first_or_octet_stream();

        let mut res = warp::reply::Response::new(asset.into());
        res.headers_mut().insert(
            "content-type",
            warp::http::header::HeaderValue::from_str(mime.as_ref()).unwrap(),
        );
        Ok(res)
    }

    let socket_addr: std::net::SocketAddr = {
        if let Some(socket_addr) = &config::MainConfig::read().http_socket_addr {
            socket_addr.parse().expect("Unable to parse socket address")
        } else {
            ([127, 0, 0, 1], 8080).into()
        }
    };

    let routes = warp::path("api").and(
        soundboard_routes
            .or(soundboard_sound_routes)
            .or(sound_thread_routes)
            .or(hotkey_routes)
            .or(help_api),
    );
    let browser_address = {
        if socket_addr.ip().is_unspecified() {
            ([127, 0, 0, 1], socket_addr.port()).into()
        } else {
            socket_addr
        }
    };
    if let Err(err) = webbrowser::open(&format!("http://{}", browser_address)) {
        error!("failed to open browser to display ui {}", err);
    }

    if config::MainConfig::read().no_embed_web.unwrap_or_default() {
        let routes = routes.or(warp::get().and(warp::fs::dir(web_path)));

        let routes = routes.with(cors).recover(handle_rejection);
        warp::serve(routes).run(socket_addr).await;
    } else {
        let index_html = warp::path::end().and_then(serve_index);
        let routes = routes.or(warp::get()
            .and(index_html)
            .or(warp::get().and(warp::path::tail()).and_then(serve)));

        let routes = routes.with(cors).recover(handle_rejection);
        warp::serve(routes).run(socket_addr).await;
    }

    unreachable!();
}
