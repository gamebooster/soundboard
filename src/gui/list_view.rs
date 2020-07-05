use iced::{
    button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
    Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
    Space, Subscription, Text, VerticalAlignment,
};

use super::config;
use super::sound;
use super::style;
use log::{error, info, trace, warn};

#[derive(Debug, Clone, Default)]
struct SoundButton {
    state: button::State,
    config: config::SoundConfig,
}

pub struct ListView {
    scroll_state: scrollable::State,
    buttons: Vec<SoundButton>,
    pub active_sounds: Vec<(
        sound::SoundStatus,
        config::SoundConfig,
        std::time::Duration,
        Option<std::time::Duration>,
    )>,
}

#[derive(Debug, Clone)]
pub enum ListViewMessage {
    PlaySound(config::SoundConfig),
}

impl ListView {
    pub fn new(sounds: &[config::SoundConfig]) -> Self {
        let buttons = sounds
            .iter()
            .fold(Vec::<SoundButton>::new(), |mut buttons, sound| {
                buttons.push(SoundButton {
                    state: button::State::new(),
                    config: sound.clone(),
                });
                buttons
            });
        ListView {
            scroll_state: scrollable::State::new(),
            buttons,
            active_sounds: Vec::new(),
        }
    }

    pub fn update(&mut self, message: ListViewMessage) -> Command<ListViewMessage> {
        match message {
            ListViewMessage::PlaySound(_) => {
                unimplemented!();
            }
        }
        // Command::none()
    }

    pub fn view(&mut self) -> Element<ListViewMessage> {
        let column = self.buttons.iter_mut().fold(
            Scrollable::new(&mut self.scroll_state)
                .spacing(5)
                .width(Length::Fill)
                .height(Length::FillPortion(18))
                .align_items(Align::Start),
            |column, button| {
                let row_contents = Row::new()
                    .padding(10)
                    .spacing(20)
                    .align_items(Align::Center)
                    .push(Text::new(&button.config.name))
                    .push(Text::new(
                        button.config.hotkey.as_ref().unwrap_or(&String::new()),
                    ))
                    .push(
                        Button::new(&mut button.state, Text::new("Play"))
                            .on_press(ListViewMessage::PlaySound(button.config.clone()))
                            .style(style::Button::Constructive(iced::Color::from_rgb(
                                0.2, 0.8, 0.2,
                            ))),
                    );
                column.push(
                    Container::new(row_contents)
                        .width(Length::Fill)
                        .style(style::Container::Entry),
                )
            },
        );

        column.into()
    }
}
