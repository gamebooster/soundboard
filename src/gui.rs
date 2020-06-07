use iced::{
  button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
  Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
  Space, Subscription, Text, VerticalAlignment,
};

use log::{error, info, trace, warn};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use super::config;
use super::sound;
use std::fmt;
mod panel_view;
mod style;

pub struct Soundboard {
  config: config::Config,
  panel_view: panel_view::PanelView,
  sound_sender: Sender<sound::Message>,
  stop_button_state: button::State,
  volume_slider_state: slider::State,
  current_volume: f32,
}

#[derive(Debug, Clone)]
pub enum SoundboardMessage {
  PlaySound(String),
  StopAllSound,
  VolumeChanged(f32),
  HandlePanelViewMessage(panel_view::PanelViewMessage),
}

impl Application for Soundboard {
  type Executor = executor::Default;
  type Message = SoundboardMessage;
  type Flags = (Sender<sound::Message>, config::Config);

  fn new(flags: Self::Flags) -> (Soundboard, Command<SoundboardMessage>) {
    let soundboard = Soundboard {
      config: flags.1.clone(),
      sound_sender: flags.0,
      stop_button_state: button::State::new(),
      volume_slider_state: slider::State::new(),
      current_volume: 1.0,
      panel_view: panel_view::PanelView::new(&flags.1.clone().sounds.unwrap()),
    };
    (soundboard, Command::none())
  }

  fn title(&self) -> String {
    String::from("soundboard")
  }

  fn update(&mut self, message: SoundboardMessage) -> Command<SoundboardMessage> {
    match message {
      SoundboardMessage::PlaySound(sound_path) => {
        if let Err(err) = self.sound_sender.send(sound::Message::PlaySound(
          sound_path.clone(),
          sound::SoundDevices::Both,
        )) {
          error!("failed to play sound {}", err);
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
      SoundboardMessage::HandlePanelViewMessage(panel_view_message) => {
        if let panel_view::PanelViewMessage::PlaySound(path) = panel_view_message {
          self.update(SoundboardMessage::PlaySound(path));
        } else {
          self.panel_view.update(panel_view_message);
        }
      }
    }

    Command::none()
  }

  fn subscription(&self) -> Subscription<SoundboardMessage> {
    Subscription::none()
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
        ProgressBar::new(0.0..=100.0, 66.0)
          .height(Length::FillPortion(2))
          .width(Length::FillPortion(6)),
      )
      .push(
        Slider::new(
          &mut self.volume_slider_state,
          0.0..=1.0,
          self.current_volume,
          SoundboardMessage::VolumeChanged,
        )
        .width(Length::FillPortion(3)),
      );

    let content = Column::new()
      .width(Length::Fill)
      .height(Length::Fill)
      .spacing(10)
      .push(
        self
          .panel_view
          .view()
          .map(move |message| SoundboardMessage::HandlePanelViewMessage(message)),
      )
      .push(Space::with_height(Length::Units(5)))
      .push(bottom_row);

    Container::new(content)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(10)
      .into()
  }
}
