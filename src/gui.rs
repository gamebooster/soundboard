use iced::{
    button, executor, futures, keyboard, pane_grid, scrollable, slider, Align, Application, Button,
    Column, Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings,
    Slider, Space, Subscription, Text, VerticalAlignment,
};

use crossbeam_channel;
use log::{error, info, trace, warn};
use std::path::{Path, PathBuf};

use super::config;
use super::download;
use super::sound;
use std::fmt;
mod list_view;
mod panel_view;
mod style;
use super::hotkey;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum LayoutStyle {
    PanelView,
    ListView,
}

#[derive(Debug, Clone, Default)]
struct SoundboardButton {
    state: button::State,
    name: String,
    selected: bool,
}

pub struct Soundboard {
    panel_view: panel_view::PanelView,
    list_view: list_view::ListView,
    sound_sender: crossbeam_channel::Sender<sound::Message>,
    sound_receiver: crossbeam_channel::Receiver<sound::Message>,
    stop_button_state: button::State,
    toggle_layout_button_state: button::State,
    volume_slider_state: slider::State,
    current_volume: f32,
    current_style: LayoutStyle,
    soundboard_button_states: Vec<SoundboardButton>,
    config: config::MainConfig,
    hotkey_manager: hotkey::HotkeyManager,
}

#[derive(Debug, Clone)]
pub enum SoundboardMessage {
    PlaySound(config::SoundConfig),
    StopSound(config::SoundConfig),
    StopAllSound,
    VolumeChanged(f32),
    HandlePanelViewMessage(panel_view::PanelViewMessage),
    HandleListViewMessage(list_view::ListViewMessage),
    ToggleLayout,
    ShowSoundboard(String),
    Tick,
}

impl Application for Soundboard {
    type Executor = executor::Default;
    type Message = SoundboardMessage;
    type Flags = (
        crossbeam_channel::Sender<sound::Message>,
        crossbeam_channel::Receiver<sound::Message>,
        config::MainConfig,
    );

    fn new(flags: Self::Flags) -> (Soundboard, Command<SoundboardMessage>) {
        let start_soundboard_index = 0;

        let mut soundboard_buttons = flags.2.clone().soundboards.unwrap().iter().fold(
            Vec::<SoundboardButton>::new(),
            |mut buttons, soundboard| {
                buttons.push(SoundboardButton {
                    state: button::State::new(),
                    name: soundboard.name.clone().unwrap(),
                    selected: false,
                });
                buttons
            },
        );

        soundboard_buttons[start_soundboard_index].selected = true;

        let mut soundboard = Soundboard {
            sound_sender: flags.0,
            sound_receiver: flags.1,
            config: flags.2.clone(),
            soundboard_button_states: soundboard_buttons,
            stop_button_state: button::State::new(),
            toggle_layout_button_state: button::State::new(),
            volume_slider_state: slider::State::new(),
            current_volume: 1.0,
            panel_view: panel_view::PanelView::new(&Vec::new()),
            list_view: list_view::ListView::new(&Vec::new()),
            current_style: LayoutStyle::PanelView,
            hotkey_manager: hotkey::HotkeyManager::new(),
        };
        soundboard.update(SoundboardMessage::ShowSoundboard(
            soundboard.config.soundboards.as_ref().unwrap()[start_soundboard_index]
                .name
                .clone()
                .unwrap(),
        ));
        (soundboard, Command::none())
    }

    fn title(&self) -> String {
        String::from("soundboard")
    }

    fn update(&mut self, message: SoundboardMessage) -> Command<SoundboardMessage> {
        match message {
            SoundboardMessage::Tick => {
                self.sound_sender
                    .send(sound::Message::PlayStatus(Vec::new()))
                    .expect("sound channel error");
                match self.sound_receiver.try_recv() {
                    Ok(sound::Message::PlayStatus(sounds)) => {
                        self.list_view.active_sounds = sounds.clone();
                        self.panel_view.active_sounds = sounds;
                    }
                    _ => {}
                }
            }
            SoundboardMessage::PlaySound(sound_config) => {
                if let Err(err) = self.sound_sender.send(sound::Message::PlaySound(
                    sound_config,
                    sound::SoundDevices::Both,
                )) {
                    error!("failed to play sound {}", err);
                };
            }
            SoundboardMessage::StopSound(sound_config) => {
                if let Err(err) = self
                    .sound_sender
                    .send(sound::Message::StopSound(sound_config))
                {
                    error!("failed to stop sound {}", err);
                };
            }
            SoundboardMessage::StopAllSound => {
                if let Err(err) = self.sound_sender.send(sound::Message::StopAll) {
                    error!("failed to stop all sound {}", err);
                };
            }
            SoundboardMessage::VolumeChanged(new_volume) => {
                self.current_volume = new_volume;
                if let Err(err) = self
                    .sound_sender
                    .send(sound::Message::SetVolume(self.current_volume))
                {
                    error!("failed to set volume {}", err);
                };
            }
            SoundboardMessage::ToggleLayout => {
                self.current_style = {
                    if self.current_style == LayoutStyle::ListView {
                        LayoutStyle::PanelView
                    } else {
                        LayoutStyle::ListView
                    }
                };
            }
            SoundboardMessage::ShowSoundboard(name) => {
                for button in &mut self.soundboard_button_states {
                    button.selected = false;
                    if button.name == name {
                        button.selected = true;
                    }
                }

                if let Err(err) = self.hotkey_manager.unregister_all() {
                    error!("Unregister all hotkeys failed {}", err);
                }

                let stop_hotkey = {
                    if self.config.stop_hotkey.is_some() {
                        config::parse_hotkey(&self.config.stop_hotkey.as_ref().unwrap()).unwrap()
                    } else {
                        config::Hotkey {
                            modifier: vec![config::Modifier::ALT],
                            key: config::Key::S,
                        }
                    }
                };
                let tx_clone = self.sound_sender.clone();
                if let Err(err) = self.hotkey_manager.register(stop_hotkey, move || {
                    let _result = tx_clone.send(sound::Message::StopAll);
                }) {
                    error!("register hotkey failed {}", err);
                }
                let tx_clone = self.sound_sender.clone();
                let sounds = self
                    .config
                    .soundboards
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|s| s.name.as_ref().unwrap() == &name)
                    .unwrap()
                    .sounds
                    .clone()
                    .unwrap();

                for sound in sounds.clone() {
                    if sound.hotkey.is_none() {
                        continue;
                    }
                    let hotkey = config::parse_hotkey(&sound.hotkey.as_ref().unwrap()).unwrap();
                    let tx_clone = tx_clone.clone();
                    let _result = self.hotkey_manager.register(hotkey, move || {
                        if let Err(err) = tx_clone.send(sound::Message::PlaySound(
                            sound.clone(),
                            sound::SoundDevices::Both,
                        )) {
                            error!("failed to play sound {}", err);
                        };
                    });
                }

                self.panel_view = panel_view::PanelView::new(&sounds);
                self.list_view = list_view::ListView::new(&sounds);
            }
            SoundboardMessage::HandlePanelViewMessage(panel_view_message) => {
                if let panel_view::PanelViewMessage::PlaySound(path) = panel_view_message {
                    self.update(SoundboardMessage::PlaySound(path));
                } else if let panel_view::PanelViewMessage::StopSound(path) = panel_view_message {
                    self.update(SoundboardMessage::StopSound(path));
                } else {
                    self.panel_view.update(panel_view_message);
                }
            }
            #[allow(irrefutable_let_patterns)]
            SoundboardMessage::HandleListViewMessage(list_view_message) => {
                if let list_view::ListViewMessage::PlaySound(path) = list_view_message {
                    self.update(SoundboardMessage::PlaySound(path));
                } else {
                    self.list_view.update(list_view_message);
                }
            }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<SoundboardMessage> {
        every(Duration::from_millis(10)).map(|_| SoundboardMessage::Tick)
    }

    fn view(&mut self) -> Element<SoundboardMessage> {
        let stop_button_column = Column::new()
            .spacing(5)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(
                Text::new("Stop")
                    .size(18)
                    .vertical_alignment(VerticalAlignment::Center),
            );

        let stop_button_container = Container::new(stop_button_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .center_y();

        let toggle_layout_button_column = Column::new()
            .spacing(5)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(
                Text::new("Toggle Layout")
                    .size(18)
                    .vertical_alignment(VerticalAlignment::Center),
            );

        let toggle_layout_button_container = Container::new(toggle_layout_button_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .center_y();

        let soundboard_row = self.soundboard_button_states.iter_mut().fold(
            Row::new()
                .spacing(5)
                .width(Length::FillPortion(6))
                .height(Length::Fill)
                .align_items(Align::Start),
            |row, button| {
                let soundboard_button_column = Column::new()
                    .spacing(5)
                    .align_items(Align::Center)
                    .width(Length::Fill)
                    .push(
                        Text::new(button.name.clone())
                            .size(18)
                            .vertical_alignment(VerticalAlignment::Center),
                    );

                let soundboard_button_container = Container::new(soundboard_button_column)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(3)
                    .center_y();
                row.push(
                    Button::new(&mut button.state, soundboard_button_container)
                        .on_press(SoundboardMessage::ShowSoundboard(button.name.clone()))
                        .style(style::Button::Choice {
                            selected: button.selected,
                        })
                        .height(Length::Fill)
                        .width(Length::Fill),
                )
            },
        );

        let bottom_row = Row::new()
            .spacing(5)
            .width(Length::Fill)
            .height(Length::FillPortion(2))
            .push(
                Button::new(&mut self.stop_button_state, stop_button_container)
                    .on_press(SoundboardMessage::StopAllSound)
                    .height(Length::Fill)
                    .width(Length::FillPortion(2))
                    .style(style::Button::Destructive),
            )
            .push(
                Button::new(
                    &mut self.toggle_layout_button_state,
                    toggle_layout_button_container,
                )
                .on_press(SoundboardMessage::ToggleLayout)
                .height(Length::Fill)
                .width(Length::FillPortion(2))
                .style(style::Button::Neutral),
            )
            .push(soundboard_row)
            // .push(
            //   ProgressBar::new(0.0..=100.0, 66.0)
            //     .height(Length::FillPortion(2))
            //     .width(Length::FillPortion(6)),
            // )
            .push(
                Slider::new(
                    &mut self.volume_slider_state,
                    0.0..=1.0,
                    self.current_volume,
                    SoundboardMessage::VolumeChanged,
                )
                .width(Length::FillPortion(2)),
            );

        let sound_view = {
            if self.current_style == LayoutStyle::ListView {
                self.list_view
                    .view()
                    .map(SoundboardMessage::HandleListViewMessage)
            } else {
                self.panel_view
                    .view()
                    .map(SoundboardMessage::HandlePanelViewMessage)
            }
        };

        let content = Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(10)
            .push(sound_view)
            .push(Space::with_height(Length::Units(5)))
            .push(bottom_row);
        //.push(soundboard_row);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }
}

pub fn every(duration: std::time::Duration) -> iced::Subscription<std::time::Instant> {
    iced::Subscription::from_recipe(Every(duration))
}

struct Every(std::time::Duration);

impl<H, E> iced_native::subscription::Recipe<H, E> for Every
where
    H: std::hash::Hasher,
{
    type Output = std::time::Instant;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
        self.0.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, E>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        use futures::stream::StreamExt;

        let start = tokio::time::Instant::now() + self.0;

        tokio::time::interval_at(start, self.0)
            .map(|_| std::time::Instant::now())
            .boxed()
    }
}
