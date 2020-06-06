use iced::{
    button, executor, keyboard, pane_grid, scrollable, Align, Application, Button, Column, Command,
    Container, Element, Length, PaneGrid, Row, Scrollable, Settings, Subscription, Text,
    VerticalAlignment,
};

use random_color::{Color, Luminosity, RandomColor};

use log::{error, info, trace, warn};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

mod style;
use super::config;
use super::sound;
use std::fmt;

pub struct Soundboard {
    config: config::Config,
    sender: Sender<sound::Message>,
    panes: pane_grid::State<Content>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PlaySound(String),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = (Sender<sound::Message>, config::Config);

    fn new(flags: Self::Flags) -> (Soundboard, Command<Message>) {
        let (panes, first) = pane_grid::State::<Content>::new(Content::new(SoundButton::default()));
        let mut soundboard = Soundboard {
            config: flags.1,
            sender: flags.0,
            panes: panes,
        };
        let sounds = &soundboard.config.clone().sounds.unwrap();
        let mut panels: Vec<pane_grid::Pane> = Vec::new();
        panels.push(first);
        sounds.iter().for_each(|sound| {
            let modifier_string = sound
                .hotkey_modifier
                .clone()
                .unwrap_or_default()
                .into_iter()
                .fold(String::new(), |all, one| {
                    if all.len() > 0 {
                        format!("{}-{}", all, one)
                    } else {
                        one.to_string()
                    }
                });
            let hotkey_string = {
                if sound.hotkey_key.is_some() {
                    if modifier_string.len() > 0 {
                        format!(
                            "{}-{}",
                            modifier_string,
                            sound.hotkey_key.unwrap().to_string()
                        )
                    } else {
                        sound.hotkey_key.unwrap().to_string()
                    }
                } else {
                    String::new()
                }
            };

            let power = 2;
            let power2_less = (panels.len() as f64).log(power as f64) as usize;
            let index = { panels.len() - (power as usize).pow(power2_less as u32) };
            let best_axis = {
                if power2_less % power == 0 {
                    pane_grid::Axis::Horizontal
                } else {
                    pane_grid::Axis::Vertical
                }
            };
            let new_panel = soundboard
                .panes
                .split(
                    best_axis,
                    &panels[index],
                    Content::new(SoundButton {
                        state: button::State::new(),
                        path: sound.path.clone(),
                        name: sound.name.clone(),
                        hotkey: hotkey_string,
                    }),
                )
                .unwrap();
            panels.push(new_panel);
        });
        soundboard.panes.close(&first);
        (soundboard, Command::none())
    }

    fn title(&self) -> String {
        String::from("soundboard")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PlaySound(sound_path) => {
              self.sender.send(sound::Message::PlaySound(sound_path.clone(), sound::SoundDevices::Both));
            }
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(&split, ratio);
            }
            Message::Dragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.swap(&pane, &target);
            }
            Message::Dragged(_) => {}
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&mut self.panes, |pane, content, focus| {
            content.view(pane, focus, total_panes)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .on_drag(Message::Dragged)
        .on_resize(Message::Resized)
        .spacing(10);

        Container::new(pane_grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }
}

#[derive(Debug, Clone, Default)]
struct SoundButton {
    state: button::State,
    name: String,
    path: String,
    hotkey: String,
}

struct Content {
    scroll: scrollable::State,
    sound_button: SoundButton,
    background_color: iced::Color,
}

impl Content {
    fn new(sound_button: SoundButton) -> Self {
        //let random_color= RandomColor::new()
        //.luminosity(Luminosity::Light).to_rgb_array();
        Content {
            scroll: scrollable::State::new(),
            sound_button: sound_button,
            background_color: iced::Color::from_rgb(0.2, 0.8, 0.2),
            //iced::Color::from_rgb((random_color[0] as f32) / 255.0, (random_color[1] as f32) / 255.0, (random_color[2] as f32) / 255.0)
        }
    }
    fn view(
        &mut self,
        _pane: pane_grid::Pane,
        _focus: Option<pane_grid::Focus>,
        _total_panes: usize,
    ) -> Element<Message> {
        let Content {
            scroll: _,
            sound_button,
            background_color,
        } = self;

        // let content = Scrollable::new(scroll)
        //     .width(Length::Fill)
        //     .spacing(10)
        //     .align_items(Align::Center)
        //     .push(Text::new("Pane").size(30));

        let left_column = Column::new()
            .spacing(5)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(
                Text::new(sound_button.name.clone())
                    .size(18)
                    .vertical_alignment(VerticalAlignment::Center),
            )
            .push(
                Text::new(sound_button.hotkey.clone())
                    .size(14)
                    .vertical_alignment(VerticalAlignment::Center),
            );

        let cont = Container::new(left_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .center_y();

        let button = Button::new(&mut sound_button.state, cont)
            .on_press(Message::PlaySound(sound_button.path.clone()))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::Button::Constructive(*background_color));

        Container::new(button)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .center_y()
            .into()
    }
}
