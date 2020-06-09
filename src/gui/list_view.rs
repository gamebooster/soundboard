use iced::{
    button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
    Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
    Space, Subscription, Text, VerticalAlignment,
};

use super::config;
use super::style;
use log::{error, info, trace, warn};

#[derive(Debug, Clone, Default)]
struct SoundButton {
    state: button::State,
    name: String,
    path: String,
    hotkey: String,
}

pub struct ListView {
    scroll_state: scrollable::State,
    buttons: Vec<SoundButton>,
}

#[derive(Debug, Clone)]
pub enum ListViewMessage {
    PlaySound(String),
}

impl ListView {
    pub fn new(sounds: &Vec<config::SoundConfig>) -> Self {
        let buttons = sounds
            .iter()
            .fold(Vec::<SoundButton>::new(), |mut buttons, sound| {
                let hotkey_string = {
                    if sound.hotkey.is_some() {
                        format!(
                            "{}",
                            config::parse_hotkey(&sound.hotkey.as_ref().unwrap()).unwrap()
                        )
                    } else {
                        String::new()
                    }
                };
                buttons.push(SoundButton {
                    state: button::State::new(),
                    path: sound.path.clone(),
                    name: sound.name.clone(),
                    hotkey: hotkey_string,
                });
                buttons
            });
        ListView {
            scroll_state: scrollable::State::new(),
            buttons,
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
                    .push(Text::new(button.name.clone()))
                    .push(Text::new(button.hotkey.clone()))
                    .push(
                        Button::new(&mut button.state, Text::new("Play"))
                            .on_press(ListViewMessage::PlaySound(button.path.clone()))
                            .style(style::Button::Constructive(iced::Color::from_rgb(
                                0.2, 0.8, 0.2,
                            ))),
                    );
                column.push(Container::new(row_contents).style(style::Container::Entry))
            },
        );

        column.into()
    }
}
