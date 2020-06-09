use iced::{
  button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
  Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
  Space, Subscription, Text, VerticalAlignment,
};

use log::{error, info, trace, warn};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender};

use super::config;
use super::sound;
use std::fmt;
mod list_view;
mod panel_view;
mod style;

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
  sound_sender: SyncSender<sound::Message>,
  stop_button_state: button::State,
  toggle_layout_button_state: button::State,
  volume_slider_state: slider::State,
  current_volume: f32,
  current_style: LayoutStyle,
  soundboard_button_states: Vec<SoundboardButton>,
  config: config::MainConfig,
}

#[derive(Debug, Clone)]
pub enum SoundboardMessage {
  PlaySound(String),
  StopAllSound,
  VolumeChanged(f32),
  HandlePanelViewMessage(panel_view::PanelViewMessage),
  HandleListViewMessage(list_view::ListViewMessage),
  ToggleLayout,
  ShowSoundboard(String),
}

impl Application for Soundboard {
  type Executor = executor::Default;
  type Message = SoundboardMessage;
  type Flags = (SyncSender<sound::Message>, config::MainConfig);

  fn new(flags: Self::Flags) -> (Soundboard, Command<SoundboardMessage>) {
    let start_soundboard_index = 0;

    let mut soundboard_buttons = flags.1.clone().soundboards.iter().fold(
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

    let soundboard = Soundboard {
      sound_sender: flags.0,
      config: flags.1.clone(),
      soundboard_button_states: soundboard_buttons,
      current_soundboard_index: start_soundboard_index,
      stop_button_state: button::State::new(),
      toggle_layout_button_state: button::State::new(),
      volume_slider_state: slider::State::new(),
      current_volume: 1.0,
      panel_view: panel_view::PanelView::new(
        &flags.1.clone().soundboards[start_soundboard_index]
          .sounds
          .clone()
          .unwrap(),
      ),
      list_view: list_view::ListView::new(
        &flags.1.clone().soundboards[start_soundboard_index]
          .sounds
          .clone()
          .unwrap(),
      ),
      current_style: LayoutStyle::PanelView,
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
          sound_path,
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
        let sounds = self
          .config
          .soundboards
          .clone()
          .iter()
          .find(|s| s.name.as_ref().unwrap() == &name)
          .unwrap()
          .sounds
          .clone()
          .unwrap();
        self.panel_view = panel_view::PanelView::new(&sounds.clone());
        self.list_view = list_view::ListView::new(&sounds);
      }
      SoundboardMessage::HandlePanelViewMessage(panel_view_message) => {
        if let panel_view::PanelViewMessage::PlaySound(path) = panel_view_message {
          self.update(SoundboardMessage::PlaySound(path));
        } else {
          self.panel_view.update(panel_view_message);
        }
      }
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
        row.push(
          Button::new(&mut button.state, Text::new(button.name.clone()))
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
        self
          .list_view
          .view()
          .map(SoundboardMessage::HandleListViewMessage)
      } else {
        self
          .panel_view
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
