use iced::{
    button, executor, Align, Application, Button, Column, Command, Container, Element, Length, Row,
    Settings, Subscription, Text,
};

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

mod style;
use super::config;
use super::sound;

#[derive(Debug, Clone)]
struct SoundButton {
    state: button::State,
    name: String,
    path: String,
    hotkey: String,
}

#[derive(Debug)]
pub struct Soundboard {
    buttons: Vec<SoundButton>,
    status_text: String,
    config: config::Config,
    sender: Sender<PathBuf>,
    increment_button: button::State,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlaySound(String),
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = (Sender<PathBuf>, config::Config);

    fn new(flags: Self::Flags) -> (Soundboard, Command<Message>) {
        let mut soundboard = Soundboard {
            buttons: Vec::<SoundButton>::new(),
            status_text: String::new(),
            config: flags.1,
            sender: flags.0,
            increment_button: button::State::new(),
        };
        soundboard.buttons = soundboard.config.sounds.as_ref().unwrap().into_iter().fold(
            Vec::<SoundButton>::new(),
            |mut buttons, sound| {
                buttons.push(SoundButton {
                    state: button::State::new(),
                    path: sound.path.clone(),
                    name: sound.name.clone(),
                    hotkey: format!(
                        "{}-{}",
                        sound.hotkey_modifier.clone().into_iter().fold(
                            String::new(),
                            |all, one| {
                                if all.len() > 0 {
                                    format!("{}-{}", all, one)
                                } else {
                                    one.to_string()
                                }
                            }
                        ),
                        sound.hotkey_key.to_string()
                    ),
                });
                buttons
            },
        );
        (soundboard, Command::none())
    }

    fn title(&self) -> String {
        String::from("soundboard")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PlaySound(sound_path) => {
                let _result = sound::send_playsound(self.sender.clone(), Path::new(&sound_path));
                self.status_text = "Start playing sound...".to_string();
            }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        let column = self.buttons.iter_mut().fold(
            Column::new()
                .padding(10)
                .spacing(5)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_items(Align::Center),
            |column, button| {
                let row_contents = Row::new()
                    .padding(10)
                    .spacing(20)
                    .align_items(Align::Center)
                    .push(Text::new(button.name.clone()))
                    .push(Text::new(button.hotkey.clone()))
                    .push(
                        Button::new(&mut button.state, Text::new("Play"))
                            .on_press(Message::PlaySound(button.path.clone()))
                            .style(style::Button::Constructive),
                    );
                column.push(Container::new(row_contents).style(style::Container::Entry))
            },
        );
        let container = Container::new(column)
            .padding(10)
            .style(style::Container::Background)
            .width(Length::Fill)
            .height(Length::Fill);
        container.into()
    }
}
