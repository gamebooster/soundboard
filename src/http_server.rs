use super::app_config;
use super::hotkey;
use super::sound;
use super::soundboards;
use anyhow::{anyhow, Context, Result};
use bytes::BufMut;
use futures::{Future, Stream, StreamExt, TryFuture, TryFutureExt, TryStreamExt};
use log::{error, info, trace, warn};
use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::UnboundedReceiverStream;
use ulid::Ulid;
use warp::http::StatusCode;
use warp::{reject, sse::Event, Filter, Rejection, Reply};

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

#[derive(Debug, Deserialize, Clone, Serialize)]
struct SoundChangeRequest {
    name: String,
    hotkey: Option<String>,
    source: soundboards::Source,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct SoundAddRequest {
    name: String,
    hotkey: Option<String>,
    source: soundboards::Source,
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

impl StrippedSoundboardInfo {
    pub fn from(soundboard: &soundboards::Soundboard) -> Self {
        Self {
            name: soundboard.get_name().to_string(),
            hotkey: soundboard.get_hotkey_string_or_none(),
            position: *soundboard.get_position(),
            id: *soundboard.get_id(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
struct ExtendedSoundboardInfo {
    name: String,
    hotkey: Option<String>,
    position: Option<usize>,
    id: Ulid,
    sounds: Vec<StrippedSoundInfo>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
struct StrippedSoundInfo {
    name: String,
    hotkey: Option<String>,
    source: soundboards::Source,
    id: Ulid,
}

impl StrippedSoundInfo {
    pub fn from(sound: &soundboards::Sound) -> Self {
        Self {
            name: sound.get_name().to_string(),
            hotkey: sound.get_hotkey_string_or_none(),
            source: sound.get_source().clone(),
            id: *sound.get_id(),
        }
    }
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
    id: soundboards::SoundId,
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
) -> impl Filter<Extract = ((soundboards::Soundboard, Ulid),), Error = Rejection> + Clone {
    warp::path("soundboards")
        .and(warp::path::param::<Ulid>())
        .and_then(move |id: ulid::Ulid| {
            let soundboard = soundboards::get_soundboard(id);
            if let Some(soundboard) = soundboard {
                futures::future::ok(((*soundboard).clone(), id))
            } else {
                futures::future::err(reject::custom(UnknownSoundboardError(id)))
            }
        })
}

fn check_sound_id() -> impl Filter<
    Extract = ((soundboards::Soundboard, Ulid, soundboards::Sound, Ulid),),
    Error = Rejection,
> + Clone {
    check_soundboard_id()
        .and(warp::path("sounds"))
        .and(warp::path::param::<Ulid>())
        .and_then(
            move |(soundboard, soundboard_id): (soundboards::Soundboard, Ulid),
                  sound_id: ulid::Ulid| {
                let sound = soundboards::get_sound(soundboard_id, sound_id);
                if let Some(sound) = sound {
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
                if let Err(err) = soundboards::reload_soundboards_from_disk() {
                    error!("{:#}", err);
                    return format_json_error(err);
                }
            }

            for soundboard in soundboards::get_soundboards().values() {
                soundboards.push(StrippedSoundboardInfo::from(soundboard));
            }
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(soundboards)),
                warp::http::StatusCode::OK,
            )
        });

    let soundboards_soundboard_route = check_soundboard_id()
        .and(warp::path::end())
        .and(warp::get())
        .map(move |(soundboard, id): (soundboards::Soundboard, Ulid)| {
            warp::reply::with_status(
                warp::reply::json(&ResultData::with_data(ExtendedSoundboardInfo {
                    name: soundboard.get_name().to_owned(),
                    hotkey: soundboard.get_hotkey_string_or_none(),
                    id,
                    position: *soundboard.get_position(),
                    sounds: soundboard.iter().fold(Vec::new(), |mut v, a| {
                        v.push(StrippedSoundInfo {
                            name: a.get_name().to_string(),
                            hotkey: a.get_hotkey_string_or_none(),
                            id: *a.get_id(),
                            source: a.get_source().clone(),
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
            move |(mut soundboard, soundboard_id): (soundboards::Soundboard, Ulid),
                  soundboard_change_request: SoundboardChangeRequest| {
                if let Some(hotkey_string) = soundboard_change_request.hotkey {
                    match hotkey::parse_hotkey(&hotkey_string) {
                        Ok(hotkey) => soundboard.set_hotkey(Some(hotkey)),
                        Err(err) => return format_json_error(err),
                    }
                } else {
                    soundboard.set_hotkey(None);
                }
                soundboard.set_name(&soundboard_change_request.name);

                soundboard.set_position(soundboard_change_request.position);

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                let soundboard = soundboards::get_soundboard(soundboard_id).unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundboardInfo::from(
                        &soundboard,
                    ))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    type AddSoundMultipartResult = (
        (soundboards::Soundboard, Ulid, soundboards::Sound, Ulid),
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
            move |((mut soundboard, _soundboard_index, _sound, sound_id), uploads): AddSoundMultipartResult| {
                let mut added_sounds = Vec::new();
                let mut last_sound_id = sound_id;
                for (upload_name, upload_data) in uploads {
                    trace!("Received {} with size {}", upload_name, upload_data.len());

                    let new_sound = soundboards::Sound::new(&upload_name, soundboards::Source::Local {
                        path: upload_name.clone()
                    }).unwrap();

                    let mut cursor = std::io::Cursor::new(upload_data);
                    if let Err(err) =  soundboard.add_sound_with_reader(new_sound.clone(), &mut cursor, true){
                        return format_json_error(err);
                    }

                    match soundboard.change_sound_position(*new_sound.get_id(), last_sound_id) {
                        Ok(()) => {}
                        Err(err) => return format_json_error(err),
                    }
                    last_sound_id = *new_sound.get_id();

                    added_sounds.push(StrippedSoundInfo::from(&new_sound));
                }

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(added_sounds)),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let _soundboards_soundboard_copy_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::exact("x-method", "copy"))
        .and(warp::body::json())
        .map(
            move |(mut soundboard, soundboard_index, sound, _sound_index): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            ),
                  sound_copy_request: SoundCopyRequest| {
                let source_soundboard =
                    soundboards::get_soundboard(sound_copy_request.source_soundboard_id);

                if source_soundboard.is_none() {
                    return format_json_error("invalid source soundboard");
                }

                let source_soundboard = source_soundboard.unwrap();

                let source_sound = source_soundboard
                    .get_sounds()
                    .get(&sound_copy_request.source_sound_id);

                if source_sound.is_none() {
                    return format_json_error("invalid source sound");
                }

                let source_sound = source_sound.unwrap();

                let new_sound_id = match soundboard
                    .copy_sound_from_another_soundboard(&source_soundboard, source_sound)
                {
                    Ok(sound_id) => sound_id,
                    Err(err) => return format_json_error(err),
                };

                match soundboard.change_sound_position(new_sound_id, *sound.get_id()) {
                    Ok(()) => {}
                    Err(err) => return format_json_error(err),
                }

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(
                        &soundboards::get_sound(soundboard_index, new_sound_id).unwrap(),
                    ))),
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
            move |(mut soundboard, soundboard_index, sound, _sound_index): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            ),
                  sound_copy_request: SoundCopyRequest| {
                let source_soundboard =
                    soundboards::get_soundboard(sound_copy_request.source_soundboard_id);

                if source_soundboard.is_none() {
                    return format_json_error("invalid source soundboard");
                }

                let source_soundboard = source_soundboard.unwrap();

                let source_sound = source_soundboard
                    .get_sounds()
                    .get(&sound_copy_request.source_sound_id);

                if source_sound.is_none() {
                    return format_json_error("invalid source sound");
                }

                let source_sound = source_sound.unwrap();

                let new_sound_id = match soundboard
                    .copy_sound_from_another_soundboard(&source_soundboard, source_sound)
                {
                    Ok(sound_id) => sound_id,
                    Err(err) => return format_json_error(err),
                };

                match soundboard.change_sound_position(new_sound_id, *sound.get_id()) {
                    Ok(()) => {}
                    Err(err) => return format_json_error(err),
                }

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(
                        &soundboards::get_sound(soundboard_index, new_sound_id).unwrap(),
                    ))),
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
            move |(mut soundboard, _soundboard_id, _sound, sound_id): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            ),
                  sound_add_request: SoundAddRequest| {
                let new_sound = match soundboards::Sound::new(
                    &sound_add_request.name,
                    sound_add_request.source,
                ) {
                    Ok(sound) => sound,
                    Err(err) => return format_json_error(err),
                };
                if let Err(err) = soundboard.add_sound(new_sound.clone()) {
                    return format_json_error(err);
                }

                match soundboard.change_sound_position(*new_sound.get_id(), sound_id) {
                    Ok(()) => {}
                    Err(err) => return format_json_error(err),
                }

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(&new_sound))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::get())
        .map(
            move |(_soundboard, _soundboard_id, sound, _sound_id): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            )| {
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(&sound))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_change_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::put())
        .and(warp::body::json())
        .map(
            move |(mut soundboard, _soundboard_index, mut sound, sound_id): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            ),
                  change_request: SoundChangeRequest| {
                {
                    let changed_sound = soundboard.get_sounds_mut().get_mut(&sound_id).unwrap();
                    if let Err(err) = sound.set_name(&change_request.name) {
                        return format_json_error(err);
                    }
                    if let Some(hotkey) = change_request.hotkey {
                        if hotkey.is_empty() {
                            return format_json_error("Invalid hotkey specified");
                        }
                        if let Err(err) = hotkey::parse_hotkey(&hotkey) {
                            return format_json_error(format!("Invalid hotkey specified: {}", err));
                        }
                        changed_sound.set_hotkey(Some(hotkey::parse_hotkey(&hotkey).unwrap()));
                    } else {
                        changed_sound.set_hotkey(None);
                    }

                    if let Err(err) = changed_sound.set_source(change_request.source.clone()) {
                        return format_json_error(err);
                    }
                    sound = changed_sound.clone();
                }
                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(&sound))),
                    warp::http::StatusCode::OK,
                )
            },
        );

    let soundboards_sounds_delete_sound_route = check_sound_id()
        .and(warp::path::end())
        .and(warp::delete())
        .map(
            move |(mut soundboard, _soundboard_index, sound, _sound_index): (
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            )| {
                if let Err(err) = soundboard.remove_sound(&sound) {
                    return format_json_error(err);
                }

                if let Err(err) = soundboards::update_soundboards(soundboard) {
                    return format_json_error(err);
                }

                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(StrippedSoundInfo::from(&sound))),
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
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            ),
                  request: SoundPlayRequest| {
                gui_sender_clone
                    .send(sound::Message::PlaySound(*sound.get_id(), request.devices))
                    .unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(format!(
                        "PlaySound {:?}",
                        sound.get_source()
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
                soundboards::Soundboard,
                soundboards::SoundboardId,
                soundboards::Sound,
                soundboards::SoundId,
            )| {
                gui_sender_clone
                    .send(sound::Message::StopSound(*sound.get_id()))
                    .unwrap();
                warp::reply::with_status(
                    warp::reply::json(&ResultData::with_data(format!(
                        "StopSound {:?}",
                        sound.get_source()
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

    fn sse_json(id: PlayStatusResponse) -> Result<warp::sse::Event, Infallible> {
        Ok(warp::sse::Event::default().json_data(id).unwrap())
    }

    let gui_sender_clone = gui_sender.clone();
    let gui_receiver_clone = gui_receiver.clone();
    let sounds_events_route = warp::path!("sounds" / "events")
        .and(warp::get())
        .map(move || {
            let gui_sender_clone = gui_sender_clone.clone();
            let gui_receiver_clone = gui_receiver_clone.clone();
            let event_stream = IntervalStream::new(tokio::time::interval(
                tokio::time::Duration::from_millis(111),
            ))
            .map(move |_| loop {
                gui_sender_clone
                    .send(sound::Message::PlayStatus(Vec::new(), 0.0))
                    .unwrap();
                if let Ok(sound::Message::PlayStatus(sounds, volume)) = gui_receiver_clone.recv() {
                    let mut sound_info: Vec<StrippedSoundActiveInfo> = Vec::new();
                    for sound in sounds {
                        if let Some(full_sound) = soundboards::find_sound(sound.1) {
                            sound_info.push(StrippedSoundActiveInfo {
                                status: sound.0,
                                name: full_sound.get_name().to_string(),
                                id: sound.1,
                                play_duration: sound.2.as_secs_f32(),
                                total_duration: sound
                                    .3
                                    .unwrap_or_else(|| std::time::Duration::from_secs(0))
                                    .as_secs_f32(),
                            });
                        }
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
                        let full_sound = soundboards::find_sound(sound.1).unwrap();
                        sound_info.push(StrippedSoundActiveInfo {
                            status: sound.0,
                            name: full_sound.get_name().to_string(),
                            id: sound.1,
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

    fn sse_event(id: String) -> Result<warp::sse::Event, Infallible> {
        Ok(warp::sse::Event::default().data(id))
    }

    let hotkey_events_route = warp::path!("hotkeys" / "events")
        .and(warp::get())
        .and(senders_filter.clone())
        .map(move |senders: HotkeySenders| {
            let event_stream = || {
                let (tx, rx) = mpsc::unbounded_channel::<HotkeyMessage>();
                senders.lock().push(tx);
                UnboundedReceiverStream::new(rx).map(|msg| match msg {
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
                let hotkey = match hotkey::parse_hotkey(&hotkey_request.hotkey) {
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
                let hotkey = match hotkey::parse_hotkey(&hotkey_request.hotkey) {
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

    let mut web_path = soundboards::get_soundboards_path().expect("soundboards path");
    web_path.pop();
    web_path.push("web");

    if std::env::var("SB_WEB_DEV").is_ok() {
        web_path = std::path::PathBuf::from_str("web").unwrap();
    }

    let soundboard_routes = soundboards_route
        .or(soundboards_soundboard_change_route)
        .or(soundboards_soundboard_route);

    let soundboard_sound_routes = soundboards_sounds_sound_route
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

        let mut res = warp::reply::Response::new(asset.data.into());
        res.headers_mut().insert(
            "content-type",
            warp::http::header::HeaderValue::from_str(mime.as_ref()).unwrap(),
        );
        Ok(res)
    }

    let socket_addr: std::net::SocketAddr = {
        app_config::get_app_config()
            .http_socket_addr
            .as_ref()
            .unwrap()
            .parse()
            .expect("Unable to parse socket address")
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

    if !app_config::get_app_config().embed_web.unwrap_or_default()
        || std::env::var("SB_WEB_DEV").is_ok()
    {
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
