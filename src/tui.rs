use anyhow::{anyhow, Context, Result};
use crossterm::{
    cursor::{DisableBlinking, EnableBlinking, Hide, MoveTo, RestorePosition, SavePosition, Show},
    event::{EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{
        size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, ScrollDown, ScrollUp,
        SetSize,
    },
    ExecutableCommand,
};
use log::{error, info, trace, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{
    error::Error,
    io::{stdout, Write},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs, Widget, Wrap,
    },
    Terminal,
};

use super::app_config;
use super::hotkey;
use super::sound;
use super::soundboards;

mod sound_state_list;

fn select_soundboard(
    id: &soundboards::SoundboardId,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
) -> (sound_state_list::SoundStateList, hotkey::HotkeyManager) {
    let soundboard = soundboards::get_soundboards()
        .get(id)
        .unwrap_or_else(|| panic!("soundboard id not found {}", id))
        .clone();
    let current_sounds = soundboard.iter();
    let hotkeys = register_hotkeys(current_sounds, gui_sender);
    let mut sound_list = sound_state_list::SoundStateList::new(
        &soundboard.get_name(),
        current_sounds.cloned().collect(),
    );
    sound_list.state.select(Some(0));
    (sound_list, hotkeys)
}

fn register_hotkeys<'a>(
    sounds: impl Iterator<Item = &'a soundboards::Sound>,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
) -> hotkey::HotkeyManager {
    let mut hotkey_manager = hotkey::HotkeyManager::new();

    let stop_hotkey = {
        if let Some(stop_key) = app_config::get_app_config().stop_hotkey.as_ref() {
            hotkey::parse_hotkey(stop_key).unwrap()
        } else {
            hotkey::Hotkey {
                modifier: vec![hotkey::Modifier::CTRL, hotkey::Modifier::ALT],
                key: hotkey::Key::E,
            }
        }
    };
    let gui_sender_clone = gui_sender.clone();
    if let Err(err) = hotkey_manager.register(stop_hotkey, move || {
        gui_sender_clone.send(sound::Message::StopAll).unwrap();
    }) {
        error!("register stop hotkey failed {:#}", err);
    }

    for sound in sounds {
        if let Some(hotkey) = sound.get_hotkey() {
            let sound = sound.clone();
            let tx_clone = gui_sender.clone();
            let _result = hotkey_manager.register(hotkey.clone(), move || {
                if let Err(err) = tx_clone.send(sound::Message::PlaySound(
                    *sound.get_id(),
                    sound::SoundDevices::Both,
                )) {
                    error!("failed to play sound {}", err);
                };
            });
        }
    }
    hotkey_manager
}

struct SoundboardState {
    pub sound_state_list: sound_state_list::SoundStateList,
    pub soundboards: Vec<(String, soundboards::SoundboardId)>,
    hotkeys: hotkey::HotkeyManager,
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    index: usize,
}

impl SoundboardState {
    pub fn new(gui_sender: crossbeam_channel::Sender<sound::Message>) -> Self {
        let soundboards: Vec<(String, soundboards::SoundboardId)> = soundboards::get_soundboards()
            .values()
            .map(|s| (s.get_name().to_string(), *s.get_id()))
            .collect();

        let (sound_state_list, hotkeys) = select_soundboard(&soundboards[0].1, gui_sender.clone());

        Self {
            gui_sender,
            sound_state_list,
            hotkeys,
            index: 0,
            soundboards,
        }
    }

    pub fn get_index(&self) -> usize {
        self.index
    }

    pub fn reload_soundboards(&mut self) {
        let soundboards: Vec<(String, soundboards::SoundboardId)> = soundboards::get_soundboards()
            .values()
            .map(|s| (s.get_name().to_string(), *s.get_id()))
            .collect();

        self.soundboards = soundboards;
    }

    pub fn set_index(&mut self, mut new_index: usize) {
        if !soundboards::get_soundboards().contains_key(&self.soundboards[new_index].1) {
            self.reload_soundboards();
        }
        if !self.soundboards.is_empty() {
            if new_index >= self.soundboards.len() {
                new_index = self.soundboards.len() - 1;
            }
            let result = select_soundboard(&self.soundboards[new_index].1, self.gui_sender.clone());
            self.sound_state_list = result.0;
            self.hotkeys = result.1;
            self.index = new_index;
        } else {
            self.index = 0;
        }
    }
}

enum TUIEvent<I> {
    Input(I),
    Tick,
}

const TICK_RATE_MS: u64 = 100;

pub fn draw_terminal(
    gui_sender: crossbeam_channel::Sender<sound::Message>,
    gui_receiver: crossbeam_channel::Receiver<sound::Message>,
) -> Result<()> {
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), Hide)?;
    crossterm::terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut soundboard_state = SoundboardState::new(gui_sender.clone());
    let mut active_sounds: sound::PlayStatusVecType = sound::PlayStatusVecType::new();
    let mut current_volume = 1.0;

    // Setup input handling
    let (tui_sender, tui_receiver) = crossbeam_channel::unbounded();

    let tick_rate = std::time::Duration::from_millis(TICK_RATE_MS);
    let tui_sender_clone = tui_sender.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    std::thread::spawn(move || {
        let mut last_tick = std::time::Instant::now();
        loop {
            // poll for tick rate duration, if no events, sent tick event.
            if crossterm::event::poll(tick_rate - last_tick.elapsed()).unwrap() {
                if let Event::Key(key) = crossterm::event::read().unwrap() {
                    tui_sender_clone.send(TUIEvent::Input(key)).unwrap();
                }
            }
            if last_tick.elapsed() >= tick_rate {
                if let Err(err) = tui_sender_clone.send(TUIEvent::Tick) {
                    if shutdown_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    error!("send channel error {}", err);
                }
                last_tick = std::time::Instant::now();
            }
        }
    });

    let mut filter_input_mode = false;
    let mut current_filter = String::new();

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let main_chunks = Layout::default()
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .direction(Direction::Vertical)
                .margin(2)
                .split(size);

            let tabs = Tabs::new(
                soundboard_state
                    .soundboards
                    .iter()
                    .cloned()
                    .map(|x| Spans::from(x.0))
                    .collect(),
            )
            .block(Block::default().borders(Borders::ALL).title("soundboards"))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(soundboard_state.get_index())
            .divider(tui::symbols::DOT);

            let horizontal_chunks = Layout::default()
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .direction(Direction::Horizontal)
                .margin(0)
                .split(main_chunks[1]);

            soundboard_state
                .sound_state_list
                .update_filter(&current_filter);

            let items = soundboard_state
                .sound_state_list
                .filtered_sounds
                .iter()
                .map(|sound: &soundboards::Sound| -> ListItem {
                    let active_sound = {
                        active_sounds
                            .iter()
                            .any(|active_sound| active_sound.1 == *sound.get_id())
                    };
                    let style = {
                        if active_sound {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::White)
                        }
                    };
                    let sound_spans = {
                        if let Some(ref hotkey) = sound.get_hotkey() {
                            Spans::from(vec![
                                Span::styled(sound.get_name(), style),
                                Span::raw(" ("),
                                Span::raw(hotkey.to_string()),
                                Span::raw(")"),
                            ])
                        } else {
                            Spans::from(vec![Span::styled(sound.get_name(), style)])
                        }
                    };

                    ListItem::new(sound_spans)
                })
                .collect::<Vec<ListItem>>();

            let sound_list = List::new(items)
                .block(Block::default().title("sounds").borders(Borders::ALL))
                .start_corner(Corner::TopLeft)
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().bg(Color::LightGreen));

            let sidebar_chunks = Layout::default()
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
                .direction(Direction::Vertical)
                .margin(0)
                .split(horizontal_chunks[1]);

            let active_sounds_names: Vec<String> = active_sounds
                .iter()
                .map(|s| {
                    let play_seconds = s.2.as_secs() % 60;
                    let play_minutes = (s.2.as_secs() / 60) % 60;
                    if let Some(sound) = soundboards::find_sound(s.1) {
                        if s.0 == sound::SoundStatus::Downloading {
                            return format!("{}\n  downloading", sound.get_name());
                        }
                        if let Some(dur) = s.3 {
                            let total_seconds = dur.as_secs() % 60;
                            let total_minutes = (dur.as_secs() / 60) % 60;
                            format!(
                                "{}\n  {}:{}/{}:{}",
                                sound.get_name(),
                                play_minutes,
                                play_seconds,
                                total_minutes,
                                total_seconds
                            )
                        } else {
                            format!("{}\n  {}:{}", sound.get_name(), play_minutes, play_seconds)
                        }
                    } else if s.0 == sound::SoundStatus::Downloading {
                        "<error: unknown id>\n  downloading".to_string()
                    } else {
                        format!("<error: unknown id>\n  {}:{}", play_minutes, play_seconds)
                    }
                })
                .collect();

            let active_sounds_list = List::new(
                active_sounds_names
                    .iter()
                    .map(|s| ListItem::new(Text::from(s.as_str())))
                    .collect::<Vec<ListItem>>(),
            )
            .block(
                Block::default()
                    .title("active sounds")
                    .borders(Borders::ALL),
            )
            .start_corner(Corner::TopLeft)
            .style(Style::default().fg(Color::White));

            let volume_string = format!("volume: {}", current_volume);
            let filter_mode_string = format!("filter_mode: {}", filter_input_mode);
            let filter_string = format!("filter: {}", current_filter);
            let settings_list = List::new(vec![
                ListItem::new(volume_string.as_str()),
                ListItem::new(filter_mode_string.as_str()),
                ListItem::new(filter_string.as_str()),
            ])
            .block(Block::default().title("settings").borders(Borders::ALL))
            .start_corner(Corner::TopLeft)
            .style(Style::default().fg(Color::White));

            f.render_widget(tabs, main_chunks[0]);
            f.render_stateful_widget(
                sound_list,
                horizontal_chunks[0],
                &mut soundboard_state.sound_state_list.state,
            );
            f.render_widget(active_sounds_list, sidebar_chunks[0]);
            f.render_widget(settings_list, sidebar_chunks[1]);
        })?;

        match tui_receiver.recv()? {
            TUIEvent::Tick => {
                gui_sender
                    .send(sound::Message::PlayStatus(Vec::new(), 0.0))
                    .unwrap();
                if let Ok(sound::Message::PlayStatus(sounds, volume)) = gui_receiver.recv() {
                    active_sounds = sounds;
                    current_volume = volume;
                } else {
                    panic!("could not get active play status");
                }
            }
            TUIEvent::Input(event) => {
                if filter_input_mode {
                    match event.code {
                        KeyCode::Char(c) => {
                            current_filter.push(c);
                        }
                        KeyCode::Backspace => {
                            if current_filter.is_empty() {
                                filter_input_mode = false;
                            }
                            current_filter.pop();
                        }
                        _ => {
                            filter_input_mode = false;
                            tui_sender
                                .send(TUIEvent::Input(event))
                                .expect("failed to send event");
                        }
                    }
                    current_filter = current_filter.to_lowercase();
                } else {
                    match event.code {
                        KeyCode::Char('q') => {
                            break;
                        }
                        KeyCode::Char('f') => {
                            current_filter.clear();
                            filter_input_mode = true
                        }
                        KeyCode::Char('e') => {
                            if let Err(err) = gui_sender.send(sound::Message::StopAll) {
                                error!("failed to send stop message {}", err);
                            };
                        }
                        KeyCode::Right | KeyCode::Char('d') => {
                            let sb_count = soundboards::get_soundboards().len();
                            if soundboard_state.get_index() + 1 == sb_count {
                                soundboard_state.set_index(0);
                            } else {
                                soundboard_state.set_index(soundboard_state.get_index() + 1);
                            }
                        }
                        KeyCode::Left | KeyCode::Char('a') => {
                            let sb_count = soundboards::get_soundboards().len();
                            if soundboard_state.get_index() == 0 {
                                soundboard_state.set_index(sb_count - 1);
                            } else {
                                soundboard_state.set_index(soundboard_state.get_index() - 1);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('s') => {
                            soundboard_state.sound_state_list.next();
                        }
                        KeyCode::Up | KeyCode::Char('w') => {
                            soundboard_state.sound_state_list.previous();
                        }
                        KeyCode::Enter | KeyCode::Char('r') => {
                            let selected_index =
                                soundboard_state.sound_state_list.state.selected().unwrap();
                            let sound_id = soundboard_state.sound_state_list.filtered_sounds
                                [selected_index]
                                .get_id();
                            if let Err(err) = gui_sender.send(sound::Message::PlaySound(
                                *sound_id,
                                sound::SoundDevices::Both,
                            )) {
                                error!("failed to send play message {}", err);
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    crossterm::terminal::disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
