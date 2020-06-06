use iced::{
  button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
  Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
  Space, Subscription, Text, VerticalAlignment,
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
  stop_button_state: button::State,
  volume_slider_state: slider::State,
  current_volume: f32,
}

#[derive(Debug, Clone)]
pub enum Message {
  PlaySound(String),
  StopAllSound,
  Dragged(pane_grid::DragEvent),
  Resized(pane_grid::ResizeEvent),
  VolumeChanged(f32),
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
      stop_button_state: button::State::new(),
      volume_slider_state: slider::State::new(),
      current_volume: 1.0,
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
        if let Err(err) = self.sender.send(sound::Message::PlaySound(
          sound_path.clone(),
          sound::SoundDevices::Both,
        )) {
          error!("failed to play sound {}", err);
        };
      }
      Message::StopAllSound => {
        if let Err(err) = self.sender.send(sound::Message::StopAll) {
          error!("failed to stop all sound {}", err);
        };
      }
      Message::VolumeChanged(new_volume) => {
        self.current_volume = new_volume;
        if let Err(err) = self
          .sender
          .send(sound::Message::SetVolume(self.current_volume))
        {
          error!("failed to set volume {}", err);
        };
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
    .height(Length::FillPortion(18))
    .on_drag(Message::Dragged)
    .on_resize(Message::Resized)
    .spacing(10);

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
          .on_press(Message::StopAllSound)
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
          Message::VolumeChanged,
        )
        .width(Length::FillPortion(3)),
      );

    let content = Column::new()
      .width(Length::Fill)
      .height(Length::Fill)
      .spacing(10)
      .push(pane_grid)
      .push(Space::with_height(Length::Units(5)))
      .push(bottom_row);

    Container::new(content)
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
  //scroll: scrollable::State,
  sound_button: SoundButton,
  background_color: iced::Color,
}

impl Content {
  fn new(sound_button: SoundButton) -> Self {
    //let random_color= RandomColor::new()
    //.luminosity(Luminosity::Light).to_rgb_array();
    Content {
      //scroll: scrollable::State::new(),
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
      //scroll: _,
      sound_button,
      background_color,
    } = self;

    // let content = Scrollable::new(scroll)
    //     .width(Length::Fill)
    //     .spacing(10)
    //     .align_items(Align::Center)
    //     .push(Text::new("Pane").size(30));

    let column = Column::new()
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

    let cont = Container::new(column)
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
