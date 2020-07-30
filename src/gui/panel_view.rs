use iced::{
    button, executor, keyboard, pane_grid, scrollable, slider, Align, Application, Button, Column,
    Command, Container, Element, Length, PaneGrid, ProgressBar, Row, Scrollable, Settings, Slider,
    Space, Subscription, Text, VerticalAlignment,
};

use super::sound;
use super::soundboards;
use super::style;
use log::{error, info, trace, warn};

pub struct PanelView {
    panes: pane_grid::State<PanelButtonView>,
    pub active_sounds: sound::PlayStatusVecType,
}

#[derive(Debug, Clone)]
pub enum PanelViewMessage {
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    PlaySound(soundboards::SoundId),
    StopSound(soundboards::SoundId),
}

impl PanelView {
    pub fn new(sounds: &[soundboards::Sound]) -> Self {
        if sounds.len() > 0 {
            let (mut pane_state, first) =
                pane_grid::State::<PanelButtonView>::new(PanelButtonView::new(SoundButton {
                    state: button::State::new(),
                    sound: sounds[0].clone(),
                }));
            let mut panels: Vec<pane_grid::Pane> = Vec::new();
            panels.push(first);
            sounds.iter().skip(1).for_each(|sound| {
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
                let new_panel = pane_state
                    .split(
                        best_axis,
                        &panels[index],
                        PanelButtonView::new(SoundButton {
                            state: button::State::new(),
                            sound: sound.clone(),
                        }),
                    )
                    .unwrap();
                panels.push(new_panel);
            });
            PanelView {
                panes: pane_state,
                active_sounds: Vec::new(),
            }
        } else {
            let (mut pane_state, first) =
                pane_grid::State::<PanelButtonView>::new(PanelButtonView::new(SoundButton {
                    state: button::State::new(),
                    sound: soundboards::Sound::new(
                        "invalid",
                        soundboards::Source::Youtube {
                            id: "invalid".to_string(),
                        },
                    )
                    .unwrap(),
                }));
            PanelView {
                panes: pane_state,
                active_sounds: Vec::new(),
            }
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
            if let Some(sound) = sounds
                .iter()
                .find(|(_, s, _, _)| s == state.sound_button.sound.get_id())
            {
                state.playing = true;
                state.status = sound.0;
                state.play_duration = sound.2;
                state.total_duration = sound.3.unwrap_or_else(|| std::time::Duration::from_secs(0));
            } else {
                state.playing = false;
            }
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

#[derive(Debug, Clone)]
struct SoundButton {
    state: button::State,
    sound: soundboards::Sound,
}

struct PanelButtonView {
    sound_button: SoundButton,
    stop_button_state: button::State,
    background_color: iced::Color,
    pub playing: bool,
    pub status: sound::SoundStatus,
    pub play_duration: std::time::Duration,
    pub total_duration: std::time::Duration,
}

impl PanelButtonView {
    fn new(sound_button: SoundButton) -> Self {
        //let random_color= RandomColor::new()
        //.luminosity(Luminosity::Light).to_rgb_array();
        PanelButtonView {
            sound_button,
            play_duration: std::time::Duration::new(0, 0),
            total_duration: std::time::Duration::new(0, 0),
            stop_button_state: button::State::new(),
            background_color: iced::Color::from_rgb(0.2, 0.8, 0.2),
            playing: false, //iced::Color::from_rgb((random_color[0] as f32) / 255.0, (random_color[1] as f32) / 255.0, (random_color[2] as f32) / 255.0)
            status: sound::SoundStatus::Downloading,
        }
    }
    fn view(
        &mut self,
        _pane: pane_grid::Pane,
        _focus: Option<pane_grid::Focus>,
    ) -> Element<PanelViewMessage> {
        let hotkey_text = {
            if let Some(hotkey) = self.sound_button.sound.get_hotkey() {
                hotkey.to_string()
            } else {
                String::new()
            }
        };
        let column = Column::new()
            .spacing(5)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(
                Text::new(self.sound_button.sound.get_name())
                    .size(18)
                    .vertical_alignment(VerticalAlignment::Center),
            )
            .push(
                Text::new(&hotkey_text)
                    .size(14)
                    .vertical_alignment(VerticalAlignment::Center),
            );

        let cont = Container::new(column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .center_y();

        if !self.playing {
            Button::new(&mut self.sound_button.state, cont)
                .on_press(PanelViewMessage::PlaySound(
                    *self.sound_button.sound.get_id(),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(style::Button::Constructive(self.background_color))
                .into()
        } else {
            let button_play = Button::new(&mut self.sound_button.state, cont)
                .on_press(PanelViewMessage::PlaySound(
                    *self.sound_button.sound.get_id(),
                ))
                .width(Length::Fill)
                .height(Length::FillPortion(10))
                .style(style::Button::Constructive(self.background_color));

            let progress_bar = ProgressBar::new(
                0.0..=self.total_duration.as_secs_f32(),
                self.play_duration.as_secs_f32(),
            )
            .width(Length::Fill)
            .height(Length::FillPortion(3));

            let mut column = Column::new()
                .spacing(0)
                .align_items(Align::Center)
                .width(Length::Fill)
                .height(Length::Fill)
                .push(button_play);
            if self.status == sound::SoundStatus::Playing {
                column = column.push(progress_bar);
            } else {
                column = column.push(
                    Text::new("Downloading")
                        .horizontal_alignment(iced::HorizontalAlignment::Center),
                );
            }
            column
                .push(
                    Button::new(
                        &mut self.stop_button_state,
                        Text::new("Stop").horizontal_alignment(iced::HorizontalAlignment::Center),
                    )
                    .on_press(PanelViewMessage::StopSound(
                        *self.sound_button.sound.get_id(),
                    ))
                    .width(Length::Fill)
                    .height(Length::FillPortion(3))
                    .style(style::Button::Destructive),
                )
                .into()
        }
    }
}
