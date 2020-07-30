use iced::{
    button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
    Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
    Space, Subscription, Text, VerticalAlignment,
};

use super::sound;
use super::soundboards;
use super::style;
use log::{error, info, trace, warn};

#[derive(Debug, Clone)]
struct SoundButton {
    state: button::State,
    sound: soundboards::Sound,
}

pub struct ListView {
    scroll_state: scrollable::State,
    buttons: Vec<SoundButton>,
    pub active_sounds: sound::PlayStatusVecType,
}

#[derive(Debug, Clone)]
pub enum ListViewMessage {
    PlaySound(soundboards::SoundId),
}

impl ListView {
    pub fn new(sounds: &[soundboards::Sound]) -> Self {
        let buttons = sounds
            .iter()
            .fold(Vec::<SoundButton>::new(), |mut buttons, sound| {
                buttons.push(SoundButton {
                    state: button::State::new(),
                    sound: sound.clone(),
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
                let hotkey_text = {
                    if let Some(hotkey) = button.sound.get_hotkey() {
                        hotkey.to_string()
                    } else {
                        String::new()
                    }
                };
                let row_contents = Row::new()
                    .padding(10)
                    .spacing(20)
                    .align_items(Align::Center)
                    .push(Text::new(button.sound.get_name()))
                    .push(Text::new(&hotkey_text))
                    .push(
                        Button::new(&mut button.state, Text::new("Play"))
                            .on_press(ListViewMessage::PlaySound(*button.sound.get_id()))
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
