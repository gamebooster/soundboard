#![allow(dead_code)]
use iced::{button, container, futures, Background, Color, Subscription, Vector};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub enum Button {
    Filter { selected: bool },
    Choice { selected: bool },
    Icon,
    Destructive,
    Constructive(Color),
    Neutral,
}
impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
        match self {
            Button::Filter { selected } => {
                if *selected {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                        border_radius: 5,
                        text_color: Color::BLACK,
                        ..button::Style::default()
                    }
                } else {
                    button::Style::default()
                }
            }
            Button::Choice { selected } => {
                if *selected {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.2, 0.4, 0.7))),
                        border_color: Color::from_rgb(0.9, 0.9, 0.9),
                        border_radius: 5,
                        text_color: Color::WHITE,
                        ..button::Style::default()
                    }
                } else {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.4, 0.6, 0.9))),
                        border_color: Color::from_rgb(0.9, 0.9, 0.9),
                        border_radius: 5,
                        ..button::Style::default()
                    }
                }
            }
            Button::Icon => button::Style {
                text_color: Color::from_rgb(0.5, 0.5, 0.5),
                ..button::Style::default()
            },
            Button::Destructive => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.8, 0.2, 0.2))),
                border_radius: 5,
                text_color: Color::WHITE,
                shadow_offset: Vector::new(1.0, 1.0),
                ..button::Style::default()
            },
            Button::Constructive(color) => button::Style {
                background: Some(Background::Color(*color)),
                border_radius: 5,
                text_color: Color::WHITE,
                shadow_offset: Vector::new(1.0, 1.0),
                ..button::Style::default()
            },
            Button::Neutral => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.8, 0.8, 0.8))),
                border_radius: 5,
                text_color: Color::WHITE,
                shadow_offset: Vector::new(1.0, 1.0),
                ..button::Style::default()
            },
        }
    }

    fn hovered(&self) -> button::Style {
        let active = self.active();

        button::Style {
            text_color: match self {
                Button::Icon => Color::from_rgb(0.2, 0.2, 0.7),
                Button::Filter { selected } if !selected => Color::from_rgb(0.5, 0.5, 0.5),
                Button::Filter { selected } if !selected => Color::from_rgb(0.3, 0.5, 0.8),
                _ => active.text_color,
            },
            shadow_offset: active.shadow_offset + Vector::new(0.0, 1.0),
            ..active
        }
    }
}

pub enum Container {
    Entry,
    Background,
}
impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
        match self {
            Container::Entry => container::Style {
                text_color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
                border_radius: 5,
                border_width: 1,
                border_color: Color::from_rgb(0.9, 0.9, 0.9),
            },
            Container::Background => container::Style {
                text_color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                background: Some(Background::Color(Color::from_rgb(0.98, 0.98, 0.98))),
                border_radius: 5,
                border_width: 1,
                border_color: Color::from_rgb(0.9, 0.9, 0.9),
            },
        }
    }
}
