
use std::{error:: Error, io::{stdout, Write}};
use tui:: {
    Terminal,
    backend::CrosstermBackend,
    layout::{Layout, Constraint, Direction, Corner},
    style::{Color, Modifier, Style},
    widgets::{Widget, Block, Borders, BorderType, List, Text, ListState}
};
use crossterm::{
    event::{KeyEvent, EnableMouseCapture, KeyCode, read, Event}, 
    ExecutableCommand,
    execute, 
    Result, 
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, ScrollUp, ScrollDown, SetSize, size, Clear, ClearType},
    cursor::{DisableBlinking, EnableBlinking, MoveTo, RestorePosition, SavePosition, Show, Hide}
};
use log::{error, info, trace, warn};

use super::config;
use super::sound;
use super::hotkey;

use crossbeam_channel::unbounded;

mod sound_state_list;

pub fn draw_terminal() -> Result<()> {
    let mut hotkey_manager = hotkey::HotkeyManager::new();
    let current_sounds = config::MainConfig::read()
        .soundboards
        .iter()
        .find(|s| s.name == "deutsche memes")
        .unwrap()
        .sounds
        .as_ref()
        .unwrap()
        .clone();
    let (sound_sender, sound_receiver ) = unbounded();
    for sound in &current_sounds {
        if sound.hotkey.is_none() {
            continue;
        }
        let hotkey = config::parse_hotkey(&sound.hotkey.as_ref().unwrap()).unwrap();
        let sound = sound.clone();
        let tx_clone = sound_sender.clone();
        let _result = hotkey_manager.register(hotkey, move || {
            if let Err(err) = tx_clone.send(sound::Message::PlaySound(
                sound.clone(),
                sound::SoundDevices::Both,
            )) {
                error!("failed to play sound {}", err);
            };
        });
    }
    if let Err(err) = sound_sender.send(sound::Message::SetVolume(1.0)){
        error!("failed to set volume {}", err);
    };
    execute!(stdout(), EnableMouseCapture)?;
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), Hide)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut sound_list = sound_state_list::SoundStateList::new(current_sounds);
    sound_list.state.select(Some(0));
    loop {
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50)
                    ].as_ref())
                .direction(Direction::Horizontal)
                .margin(2)
                .split(size);
            let items = sound_list.sounds.iter().map(|i| Text::raw(&i.name));
            let list = List::new(items)
                .block(Block::default().borders(Borders::NONE))
                .start_corner(Corner::TopLeft)
                .style(Style::default().fg(Color::Green))
                .highlight_style(Style::default().fg(Color::Red).modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            f.render_stateful_widget(list, chunks[0], &mut sound_list.state);
        })?;
        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Char('q') => {
                    break;
                }
                KeyCode::Left => {
                    sound_list.unselect();
                }
                KeyCode::Down => {
                    sound_list.next();
                }
                KeyCode::Up => {
                    sound_list.previous();
                }
                KeyCode::Enter => {
                    let selected_index = sound_list.state.selected().unwrap();
                    let sound_config = sound_list.sounds[selected_index].clone();
                    if let Err(err) = sound_sender.send(sound::Message::PlaySound(
                        sound_config,
                        sound::SoundDevices::Both,
                    )) {
                        error!("failed to play sound {}", err);
                    };

                }
                _ => {}
            }
            _ => {}
        }
    }
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
