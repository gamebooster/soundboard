#![allow(dead_code)]
use super::{config, sound};
use anyhow::{anyhow, Result};
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
use tokio::{self, fs::File, io::AsyncWriteExt, stream::StreamExt};

type ConfigLockType = std::sync::Arc<std::sync::RwLock<config::MainConfig>>;

struct Handler {
    api: Api,
    sender: Sender<sound::Message>,
    receiver: Receiver<sound::Message>,
    config_file_name: String,
    config: ConfigLockType,
}

#[derive(Deserialize, Serialize)]
struct CallbackSoundSelectionData {
    sound_name: String,
}

impl CallbackSoundSelectionData {
    fn new<S: Into<String>>(value: S) -> Self {
        Self {
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
    path: String,
) -> Result<()> {
    Ok(sender.send(sound::Message::PlaySound(
        config::SoundConfig {
            name,
            headers: None,
            hotkey: None,
            full_path: path.clone(),
            path,
        },
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
        path.to_str().unwrap().to_string(),
    )?;

    Ok(document.file_name.clone().unwrap_or_default())
}

async fn handle_stopall_command(
    _api: &Api,
    sender: &Sender<sound::Message>,
    _config: ConfigLockType,
    _message: &Message,
    _raw_args: String,
) {
    sender
        .send(sound::Message::StopAll)
        .expect("sound channel error");
}

async fn handle_play_command(
    api: &Api,
    sender: &Sender<sound::Message>,
    config: ConfigLockType,
    message: &Message,
    raw_args: String,
) {
    info!("handle_play_command arg: {}", raw_args);
    if raw_args.is_empty() {
        let method = SendMessage::new(
            message.get_chat_id(),
            "You need to specify search string after /play!".to_string(),
        );
        api.execute(method).await.unwrap();
        return;
    }

    let mut possible_matches: Vec<(i64, config::SoundConfig)> = Vec::new();
    let matcher = SkimMatcherV2::default();
    let max_matches = 8;

    for soundboard in &config.read().unwrap().soundboards {
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
        api.execute(method).await.unwrap();
    } else if possible_matches.len() == 1 {
        send_sound_config(sender, possible_matches[0].1.clone()).expect("sound channel error");
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("Playing sound: {}", possible_matches[0].1.name),
        );
        api.execute(method).await.unwrap();
    } else if possible_matches.len() <= max_matches {
        let all_matches = possible_matches.iter().fold(Vec::new(), |mut acc, elem| {
            acc.push(
                InlineKeyboardButton::with_callback_data_struct(
                    acc.len().to_string(),
                    &CallbackSoundSelectionData::new(elem.1.name.clone()),
                )
                .unwrap(),
            );
            acc
        });
        let mut index = 0;
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
        api.execute(method).await.unwrap();
    } else {
        let method = SendMessage::new(
            message.get_chat_id(),
            format!("Over {} matches!", possible_matches.len()),
        );
        api.execute(method).await.unwrap();
    }
}

fn play_sound_with_name(sender: &Sender<sound::Message>, config: ConfigLockType, name: &str) {
    for soundboard in &config.read().unwrap().soundboards {
        for sound in soundboard.sounds.as_ref().unwrap() {
            if sound.name == name {
                send_sound_config(sender, sound.clone()).expect("sound channel error");
            }
        }
    }
}

#[async_trait]
impl UpdateHandler for Handler {
    async fn handle(&mut self, update: Update) {
        // info!("got an update: {:?}\n", update);

        match update.kind {
            UpdateKind::CallbackQuery(query) => {
                if query.message.is_some() {
                    let data = query
                        .parse_data::<CallbackSoundSelectionData>()
                        .unwrap()
                        .unwrap();
                    play_sound_with_name(
                        &self.sender,
                        self.config.clone(),
                        &data.sound_name.clone(),
                    );
                    let method = tgbot::methods::AnswerCallbackQuery::new(query.id)
                        .text(format!("Playing sound: {}", &data.sound_name));
                    if let Err(err) = self.api.execute(method).await {
                        error!("telegram api error: {}", err);
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
                            handle_play_command(
                                &self.api,
                                &self.sender,
                                self.config.clone(),
                                command.get_message(),
                                raw_args,
                            )
                            .await;
                        }
                        "/stopall" => {
                            handle_stopall_command(
                                &self.api,
                                &self.sender,
                                self.config.clone(),
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
                                format!("PlaySoundError {}", err),
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
pub async fn run(
    config_file_name: String,
    sender: Sender<sound::Message>,
    receiver: Receiver<sound::Message>,
) {
    let token = env::var("SB_TELEGRAM_TOKEN").expect("SB_TELEGRAM_TOKEN is not set");
    let api = Api::new(Config::new(token)).expect("Failed to create API");
    api.execute(tgbot::methods::SetMyCommands::new(vec![
        tgbot::types::BotCommand::new("play", "play the sound with the provided name (fuzzy)")
            .unwrap(),
        tgbot::types::BotCommand::new("stopall", "stop all sounds playing").unwrap(),
    ]))
    .await
    .expect("SetMyCommands failed");

    let config_file = config::load_and_parse_config(&config_file_name).unwrap();
    let config = std::sync::Arc::new(std::sync::RwLock::new(config_file));

    LongPoll::new(
        api.clone(),
        Handler {
            api,
            sender,
            receiver,
            config_file_name,
            config,
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
