use iced::{
    button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
    Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
    Space, Subscription, Text, VerticalAlignment,
};

use super::config;
use super::style;
use log::{error, info, trace, warn};

pub struct PanelView {
    panes: pane_grid::State<PanelButtonView>,
    pub active_sounds: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PanelViewMessage {
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    PlaySound(String),
    StopSound(String),
}

impl PanelView {
    pub fn new(sounds: &Vec<config::SoundConfig>) -> Self {
        let (mut pane_state, first) =
            pane_grid::State::<PanelButtonView>::new(PanelButtonView::new(SoundButton::default()));
        let mut panels: Vec<pane_grid::Pane> = Vec::new();
        panels.push(first);
        sounds.iter().for_each(|sound| {
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
            let new_panel = pane_state
                .split(
                    best_axis,
                    &panels[index],
                    PanelButtonView::new(SoundButton {
                        state: button::State::new(),
                        path: sound.path.clone(),
                        name: sound.name.clone(),
                        hotkey: hotkey_string,
                    }),
                )
                .unwrap();
            panels.push(new_panel);
        });
        pane_state.close(&first);
        PanelView {
            panes: pane_state,
            active_sounds: Vec::new(),
        }
    }

    pub fn update(&mut self, message: PanelViewMessage) -> Command<PanelViewMessage> {
        match message {
            PanelViewMessage::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(&split, ratio);
            }
            PanelViewMessage::Dragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.swap(&pane, &target);
            }
            PanelViewMessage::Dragged(_) => {}
            _ => {
                unimplemented!();
            }
        }
        Command::none()
    }

    pub fn view(&mut self) -> Element<PanelViewMessage> {
        let sounds = self.active_sounds.clone();
        self.panes.iter_mut().for_each(|(_, state)| {
            state.playing = {
                if sounds.contains(&state.sound_button.path) {
                    true
                } else {
                    false
                }
            };
        });

        PaneGrid::new(&mut self.panes, |pane, content, focus| {
            content.view(pane, focus)
        })
        .width(Length::Fill)
        .height(Length::FillPortion(18))
        .on_drag(PanelViewMessage::Dragged)
        .on_resize(PanelViewMessage::Resized)
        .spacing(10)
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

struct PanelButtonView {
    sound_button: SoundButton,
    stop_button_state: button::State,
    background_color: iced::Color,
    pub playing: bool,
}

impl PanelButtonView {
    fn new(sound_button: SoundButton) -> Self {
        //let random_color= RandomColor::new()
        //.luminosity(Luminosity::Light).to_rgb_array();
        PanelButtonView {
            sound_button,
            stop_button_state: button::State::new(),
            background_color: iced::Color::from_rgb(0.2, 0.8, 0.2),
            playing: false, //iced::Color::from_rgb((random_color[0] as f32) / 255.0, (random_color[1] as f32) / 255.0, (random_color[2] as f32) / 255.0)
        }
    }
    fn view(
        &mut self,
        _pane: pane_grid::Pane,
        _focus: Option<pane_grid::Focus>,
    ) -> Element<PanelViewMessage> {
        let PanelButtonView {
            sound_button,
            stop_button_state,
            background_color,
            playing,
        } = self;

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

        if *playing == false {
            Button::new(&mut sound_button.state, cont)
                .on_press(PanelViewMessage::PlaySound(sound_button.path.clone()))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(style::Button::Constructive(*background_color))
                .into()
        } else {
            let button_play = Button::new(&mut sound_button.state, cont)
                .on_press(PanelViewMessage::PlaySound(sound_button.path.clone()))
                .width(Length::Fill)
                .height(Length::FillPortion(10))
                .style(style::Button::Constructive(*background_color));

            Column::new()
                .spacing(0)
                .align_items(Align::Center)
                .width(Length::Fill)
                .height(Length::Fill)
                .push(button_play)
                .push(
                    Button::new(
                        stop_button_state,
                        Text::new("Stop").horizontal_alignment(iced::HorizontalAlignment::Center),
                    )
                    .on_press(PanelViewMessage::StopSound(sound_button.path.clone()))
                    .width(Length::Fill)
                    .height(Length::FillPortion(3))
                    .style(style::Button::Destructive),
                )
                .into()
        }
    }
}
