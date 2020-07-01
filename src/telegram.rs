#![allow(dead_code)]
use super::download;
use super::{config, sound};
use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{Receiver, Sender};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::{env, path::PathBuf};
use tgbot::{
    async_trait,
    longpoll::LongPoll,
    methods::{GetFile, SendMessage},
    types::{
        Audio, CallbackQuery, Command, Document, InlineKeyboardButton, Message, MessageData,
        Update, UpdateKind,
    },
    Api, Config, UpdateHandler,
};
use tokio::task;
use tokio::{self, fs::File, io::AsyncWriteExt, stream::StreamExt};

struct Handler {
    api: Api,
    sender: Sender<sound::Message>,
    receiver: Receiver<sound::Message>,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Copy, Clone)]
enum MethodType {
    #[serde(rename(serialize = "d", deserialize = "d"))]
    Download = 0,
    #[serde(rename(serialize = "p", deserialize = "p"))]
    Play = 1,
}

#[derive(Deserialize, Serialize)]
struct CallbackSoundSelectionData {
    #[serde(rename(serialize = "m", deserialize = "m"))]
    method: MethodType,
    #[serde(rename(serialize = "s", deserialize = "s"))]
    sound_name: String,
}

impl CallbackSoundSelectionData {
    fn new<S: Into<String>>(method: MethodType, value: S) -> Self {
        Self {
            method,
            sound_name: value.into(),
        }
    }
}

async fn download_file(api: &Api, file_id: &str, file_name: &str) -> Result<PathBuf> {
    let file = api.execute(GetFile::new(file_id)).await?;
    let file_path = file.file_path.unwrap();
    let mut stream = api.download_file(file_path).await?;

    let mut temp_path = std::env::temp_dir();
    temp_path.push(file_name);
    if !temp_path.is_file() {
        let mut file = File::create(&temp_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
        }
    }
    Ok(temp_path)
}

fn send_sound_config(
    sender: &Sender<sound::Message>,
    sound_config: config::SoundConfig,
) -> Result<()> {
    Ok(sender.send(sound::Message::PlaySound(
        sound_config,
        sound::SoundDevices::Both,
    ))?)
}

fn send_new_sound_config(
    sender: &Sender<sound::Message>,
    name: String,
    ext: String,
    path: String,
) -> Result<()> {
    let mut new_sound_path = config::get_soundboards_path()?;
    new_sound_path.push("telegram");
    let full_name = name.clone() + &ext;
    new_sound_path.push(&full_name);

    info!("new path for incoming sound {}", &new_sound_path.display());

    let sound_config = config::SoundConfig {
        name,
        headers: None,
        hotkey: None,
        full_path: new_sound_path.to_str().unwrap().to_owned(),
        path: full_name,
    };

    if !new_sound_path.exists() {
        let mut telegram_sounds_dir = config::get_soundboards_path()?;
        telegram_sounds_dir.push("telegram");
        if !telegram_sounds_dir.exists() {
            std::fs::create_dir(telegram_sounds_dir)?;
        }
        std::fs::copy(&path, &new_sound_path).with_context(|| {
            format!(
                "cant copy file from {} to {}",
                &path,
                &new_sound_path.display()
            )
        })?;

        let maybe_result = config::MainConfig::read()
            .soundboards
            .iter()
            .position(|s| s.name == "telegram");
        if let Some(index) = maybe_result {
            let mut soundboard = config::MainConfig::read().soundboards[index].clone();
            soundboard.sounds = Some(soundboard.sounds.unwrap_or_default());
            if soundboard
                .sounds
                .as_ref()
                .unwrap()
                .iter()
                .find(|s| **s == sound_config)
                .is_none()
            {
                soundboard
                    .sounds
                    .as_mut()
                    .unwrap()
                    .push(sound_config.clone());
                config::MainConfig::change_soundboard(index, soundboard)?;
            }
        } else {
            let mut sb = config::SoundboardConfig::default();
            sb.name = "telegram".to_string();
            let mut new_path = config::get_soundboards_path()?;
            new_path.push("telegram.toml");
            sb.path = new_path.to_str().unwrap().to_owned();
            sb.sounds = Some(Vec::new());
            sb.sounds.as_mut().unwrap().push(sound_config.clone());
            config::MainConfig::add_soundboard(sb)?;
        }
    }

    Ok(sender.send(sound::Message::PlaySound(
        sound_config,
        sound::SoundDevices::Both,
    ))?)
}

async fn handle_audio(api: &Api, sender: &Sender<sound::Message>, audio: &Audio) -> Result<String> {
    let path = download_file(api, &audio.file_id, &audio.file_unique_id).await?;

    info!("Name: {:?}", audio.title);
    info!("Mime-Type: {:?}", audio.mime_type);
    info!("File size: {:?}", audio.file_size);

    send_new_sound_config(
        sender,
        audio.title.clone().unwrap_or_default(),
        ".".to_owned()
            + audio
                .mime_type
                .clone()
                .unwrap_or_default()
                .split('/')
                .last()
                .unwrap_or_default(),
        path.to_str().unwrap().to_string(),
    )?;

    Ok(audio.title.clone().unwrap_or_default())
}

async fn handle_document(
    api: &Api,
    sender: &Sender<sound::Message>,
    document: &Document,
) -> Result<String> {
    let path = download_file(api, &document.file_id, &document.file_unique_id).await?;

    info!("Name: {:?}", document.file_name);
    info!("Mime-Type: {:?}", document.mime_type);
    info!("File size: {:?}", document.file_size);

    send_new_sound_config(
        sender,
        document.file_name.clone().unwrap_or_default(),
        ".".to_owned()
            + document
                .mime_type
                .clone()
                .unwrap_or_default()
                .split('/')
                .last()
                .unwrap_or_default(),
        path.to_str().unwrap().to_string(),
    )?;

    Ok(document.file_name.clone().unwrap_or_default())
}

async fn handle_stopall_command(
    _api: &Api,
    sender: &Sender<sound::Message>,
    _message: &Message,
    _raw_args: String,
) {
    sender
        .send(sound::Message::StopAll)
        .expect("sound channel error");
}

async fn handle_sound_command(
    api: &Api,
    _sender: &Sender<sound::Message>,
    message: &Message,
    raw_args: String,
    method: MethodType,
) {
    info!("handle_sound_command arg: {}", raw_args);

    let method_name = {
        if method == MethodType::Play {
            "/play"
        } else {
            "/download"
        }
    };

    if raw_args.is_empty() {
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("You need to specify search string after {} !", method_name),
        );
        if let Err(err) = api.execute(method).await {
            error!("telegram api error: {}", err);
        }
        return;
    }

    let mut possible_matches: Vec<(i64, config::SoundConfig)> = Vec::new();
    let matcher = SkimMatcherV2::default();
    let max_matches = 8;

    for soundboard in &config::MainConfig::read().soundboards {
        for sound in soundboard.sounds.as_ref().unwrap() {
            if let Some(score) = matcher.fuzzy_match(&sound.name, &raw_args) {
                if possible_matches.len() < max_matches {
                    possible_matches.push((score, sound.clone()));
                    possible_matches.sort_unstable_by_key(|e| std::cmp::Reverse(e.0));
                } else if possible_matches.last().unwrap().0 < score {
                    possible_matches.push((score, sound.clone()));
                    possible_matches.sort_unstable_by_key(|e| std::cmp::Reverse(e.0));
                    possible_matches.pop();
                }
            }
        }
    }

    if possible_matches.is_empty() {
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("No matches for sound name: {}", raw_args),
        );
        if let Err(err) = api.execute(method).await {
            error!("telegram api error: {}", err);
        }
    // } else if possible_matches.len() == 1 {
    //     send_sound_config(sender, possible_matches[0].1.clone()).expect("sound channel error");
    //     let method = SendMessage::new(
    //         message.get_chat_id(),
    //         format!("Playing sound: {}", possible_matches[0].1.name),
    //     );
    //     api.execute(method).await.unwrap();
    } else if possible_matches.len() <= max_matches {
        let all_matches = possible_matches.iter().fold(Vec::new(), |mut acc, elem| {
            acc.push(
                InlineKeyboardButton::with_callback_data_struct(
                    acc.len().to_string(),
                    &CallbackSoundSelectionData::new(method, elem.1.name.clone()),
                )
                .unwrap(),
            );
            acc
        });

        let mut index: usize = 0;
        let all_matches_string = possible_matches.iter().fold(String::new(), |acc, elem| {
            let res = format!("{} \n {}. {} ({})", acc, index, elem.1.name.clone(), elem.0);
            index += 1;
            res
        });
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("Matches: \n {}", all_matches_string),
        )
        .reply_markup(vec![all_matches]);
        if let Err(err) = api.execute(method).await {
            error!("telegram api error: {}", err);
        }
    } else {
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("Over {} matches!", possible_matches.len()),
        );
        if let Err(err) = api.execute(method).await {
            error!("telegram api error: {}", err);
        }
    }
}

fn play_sound_with_name(sender: &Sender<sound::Message>, name: &str) {
    for soundboard in &config::MainConfig::read().soundboards {
        for sound in soundboard.sounds.as_ref().unwrap() {
            if sound.name == name {
                send_sound_config(sender, sound.clone()).expect("sound channel error");
            }
        }
    }
}

async fn send_sound_with_name(api: &Api, message: Message, name: &str) -> Result<()> {
    let mut maybe_sound = None;
    for soundboard in &config::MainConfig::read().soundboards {
        for sound in soundboard.sounds.as_ref().unwrap() {
            if sound.name == name {
                maybe_sound = Some(sound.clone());
            }
        }
    }

    if let Some(sound) = maybe_sound {
        let sound_clone = sound.clone();
        let local_path = download::get_local_path_from_sound_config_async(&sound_clone).await?;
        let file = tgbot::types::InputFile::path(local_path.as_path())
            .await
            .unwrap();
        let method = tgbot::methods::SendAudio::new(message.get_chat_id(), file).title(&sound.name);
        if let Err(err) = api.execute(method).await {
            error!("telegram api error: {}", err);
            let file = tgbot::types::InputFile::path(local_path.as_path())
                .await
                .unwrap();
            let method =
                tgbot::methods::SendDocument::new(message.get_chat_id(), file).caption(&sound.name);
            if let Err(err) = api.execute(method).await {
                error!("telegram api error: {}", err);
            }
        }
        return Ok(());
    }

    Err(anyhow!("could not find sound {}", name))
}

#[async_trait]
impl UpdateHandler for Handler {
    async fn handle(&mut self, update: Update) {
        // info!("got an update: {:?}\n", update);

        match update.kind {
            UpdateKind::CallbackQuery(query) => {
                if query.message.is_some() {
                    let parsed = query.parse_data::<CallbackSoundSelectionData>();
                    if parsed.is_err() || parsed.unwrap().is_none() {
                        error!("callback query parse error");
                        return;
                    }
                    let data = query
                        .parse_data::<CallbackSoundSelectionData>()
                        .unwrap()
                        .unwrap();
                    match data.method {
                        MethodType::Download => {
                            let method = tgbot::methods::SendChatAction::new(
                                query.message.as_ref().unwrap().get_chat_id(),
                                tgbot::types::ChatAction::UploadAudio,
                            );
                            if let Err(err) = self.api.execute(method).await {
                                error!("telegram api error: {}", err);
                            }
                            if let Err(err) = send_sound_with_name(
                                &self.api,
                                query.message.unwrap(),
                                &data.sound_name,
                            )
                            .await
                            {
                                error!("send sound error: {}", err);
                            } else {
                                let method = tgbot::methods::AnswerCallbackQuery::new(query.id)
                                    .text(format!("Send sound: {}", &data.sound_name));
                                if let Err(err) = self.api.execute(method).await {
                                    error!("telegram api error: {}", err);
                                }
                            }
                        }
                        MethodType::Play => {
                            play_sound_with_name(&self.sender, &data.sound_name.clone());
                            let method = tgbot::methods::AnswerCallbackQuery::new(query.id)
                                .text(format!("Playing sound: {}", &data.sound_name));
                            if let Err(err) = self.api.execute(method).await {
                                error!("telegram api error: {}", err);
                            }
                        }
                    }
                }
            }
            UpdateKind::Message(message) => {
                if let Ok(command) = Command::try_from(message.clone()) {
                    let pos = command.get_message().commands.as_ref().unwrap()[0]
                        .data
                        .offset
                        + command.get_message().commands.as_ref().unwrap()[0]
                            .data
                            .length;
                    let raw_args: Vec<u16> = message
                        .get_text()
                        .unwrap()
                        .data
                        .encode_utf16()
                        .skip(pos)
                        .collect();
                    let raw_args = String::from_utf16(&raw_args).unwrap().trim().to_owned();

                    match command.get_name() {
                        "/play" => {
                            handle_sound_command(
                                &self.api,
                                &self.sender,
                                command.get_message(),
                                raw_args,
                                MethodType::Play,
                            )
                            .await;
                        }
                        "/download" => {
                            handle_sound_command(
                                &self.api,
                                &self.sender,
                                command.get_message(),
                                raw_args,
                                MethodType::Download,
                            )
                            .await;
                        }
                        "/stopall" => {
                            handle_stopall_command(
                                &self.api,
                                &self.sender,
                                command.get_message(),
                                raw_args,
                            )
                            .await;
                        }
                        _ => {
                            let method = SendMessage::new(
                                command.get_message().get_chat_id(),
                                format!("Unsupported command received: {}", command.get_name()),
                            );
                            if let Err(err) = self.api.execute(method).await {
                                error!("telegram api error: {}", err);
                            }
                        }
                    }
                } else {
                    let result;
                    match &message.data {
                        MessageData::Audio { data, .. } => {
                            result = handle_audio(&self.api, &self.sender, data).await;
                        }
                        MessageData::Document { data, .. } => {
                            result = handle_document(&self.api, &self.sender, data).await;
                        }
                        _ => {
                            return;
                        }
                    }

                    match result {
                        Ok(name) => {
                            let method = SendMessage::new(
                                message.get_chat_id(),
                                format!("PlaySound {}", name),
                            );
                            if let Err(err) = self.api.execute(method).await {
                                error!("telegram api error: {}", err);
                            }
                        }
                        Err(err) => {
                            let method = SendMessage::new(
                                message.get_chat_id(),
                                format!("PlaySoundError {:#}", err),
                            );
                            if let Err(err) = self.api.execute(method).await {
                                error!("telegram api error: {}", err);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
pub async fn run(sender: Sender<sound::Message>, receiver: Receiver<sound::Message>) {
    let token = env::var("SB_TELEGRAM_TOKEN").expect("SB_TELEGRAM_TOKEN is not set");
    let api = Api::new(Config::new(token)).expect("Failed to create API");
    api.execute(tgbot::methods::SetMyCommands::new(vec![
        tgbot::types::BotCommand::new("play", "play the sound with the provided name (fuzzy)")
            .unwrap(),
        tgbot::types::BotCommand::new(
            "download",
            "Download the sound with the provided name (fuzzy)",
        )
        .unwrap(),
        tgbot::types::BotCommand::new("stopall", "stop all sounds playing").unwrap(),
    ]))
    .await
    .expect("SetMyCommands failed");

    LongPoll::new(
        api.clone(),
        Handler {
            api,
            sender,
            receiver,
        },
    )
    .options(
        tgbot::longpoll::LongPollOptions::default()
            .allowed_update(tgbot::types::AllowedUpdate::Message)
            .allowed_update(tgbot::types::AllowedUpdate::CallbackQuery),
    )
    .run()
    .await;
}
