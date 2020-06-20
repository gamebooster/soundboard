#![allow(dead_code)]
use super::config;
use super::sound;
use anyhow::{anyhow, Result};
use crossbeam_channel::{Receiver, Sender};
use log::{error, info, trace, warn};
use std::env;
use std::path::PathBuf;
use tgbot::longpoll::LongPoll;
use tgbot::methods::GetFile;
use tgbot::methods::SendMessage;
use tgbot::types::{Audio, Document, MessageData, Update, UpdateKind};
use tgbot::{async_trait, Api, Config, UpdateHandler};
use tokio;
use tokio::stream::StreamExt;
use tokio::{fs::File, io::AsyncWriteExt};

struct Handler {
    api: Api,
    sender: Sender<sound::Message>,
    receiver: Receiver<sound::Message>,
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

fn send_sound_config(sender: &Sender<sound::Message>, name: String, path: String) -> Result<()> {
    Ok(sender.send(sound::Message::PlaySound(
        config::SoundConfig {
            name: name,
            headers: None,
            hotkey: None,
            full_path: path.clone(),
            path: path,
        },
        sound::SoundDevices::Both,
    ))?)
}

async fn handle_audio(api: &Api, sender: &Sender<sound::Message>, audio: Audio) -> Result<String> {
    let path = download_file(api, &audio.file_id, &audio.file_unique_id).await?;

    info!("Name: {:?}", audio.title);
    info!("Mime-Type: {:?}", audio.mime_type);
    info!("File size: {:?}", audio.file_size);

    send_sound_config(
        sender,
        audio.title.clone().unwrap_or_default(),
        path.to_str().unwrap().to_string(),
    )?;

    return Ok(audio.title.clone().unwrap_or_default());
}

async fn handle_document(
    api: &Api,
    sender: &Sender<sound::Message>,
    document: Document,
) -> Result<String> {
    let path = download_file(api, &document.file_id, &document.file_unique_id).await?;

    info!("Name: {:?}", document.file_name);
    info!("Mime-Type: {:?}", document.mime_type);
    info!("File size: {:?}", document.file_size);

    send_sound_config(
        sender,
        document.file_name.clone().unwrap_or_default(),
        path.to_str().unwrap().to_string(),
    )?;

    return Ok(document.file_name.clone().unwrap_or_default());
}

#[async_trait]
impl UpdateHandler for Handler {
    async fn handle(&mut self, update: Update) {
        info!("got an update: {:?}\n", update);

        if let UpdateKind::Message(message) = update.kind {
            let chat_id = message.get_chat_id();
            if let Some(text) = message.get_text() {
                let api = self.api.clone();
                let method = SendMessage::new(chat_id, text.data.clone());
                api.execute(method).await.unwrap();
            }
            let api = self.api.clone();
            let result;
            match message.data {
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
                    let method = SendMessage::new(chat_id, format!("PlaySound {}", name));
                    api.execute(method).await.unwrap();
                }
                Err(err) => {
                    let method = SendMessage::new(chat_id, format!("PlaySoundError {}", err));
                    api.execute(method).await.unwrap();
                }
            };
        }
    }
}

#[tokio::main]
pub async fn run(
    _config_file_name: String,
    sender: Sender<sound::Message>,
    receiver: Receiver<sound::Message>,
) {
    let token = env::var("SB_TELEGRAM_TOKEN").expect("SB_TELEGRAM_TOKEN is not set");
    let api = Api::new(Config::new(token)).expect("Failed to create API");
    LongPoll::new(
        api.clone(),
        Handler {
            api,
            sender,
            receiver,
        },
    )
    .run()
    .await;
}
