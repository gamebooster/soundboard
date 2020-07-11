
use std::{error:: Error, io::{stdout, Write}};
use tui:: {
    Terminal,
    backend::CrosstermBackend,
    layout::{Layout, Constraint, Direction},
    style::{Color, Modifier, Style},
    widgets::{Widget, Block, Borders, BorderType, List, Text, ListState}
};
use crossterm::{
    event::KeyEvent, 
    ExecutableCommand,
    execute, 
    Result, 
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, ScrollUp, ScrollDown, SetSize, size},
    cursor::{DisableBlinking, EnableBlinking, MoveTo, RestorePosition, SavePosition}
};

use super::config;

pub fn draw_terminal() -> Result<()> {
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), ScrollDown(20))?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|mut f| {
        let size = f.size();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        f.render_widget(block, size); 
        let chunks = Layout::default()
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20), 
            Constraint::Percentage(20),
            Constraint::Percentage(20), 
            Constraint::Percentage(20)
            ].as_ref())
        .direction(Direction::Horizontal)
        .margin(2)
        .split(size);
        let current_sounds = config::MainConfig::read()
            .soundboards
            .iter()
            .find(|s| s.name == "deutsche memes")
            .unwrap()
            .sounds
            .as_ref()
            .unwrap()
            .clone();
        for i in 0..=3 {
            let mut state = ListState::default();
            if i == 0 {
                state.select(Some(0));
            }
            let snipped_length = current_sounds.len()/4;
            let sound_names = current_sounds[i*snipped_length..(i+1)*snipped_length].iter().map(|sound| Text::raw(&sound.name));
            let list = List::new(sound_names)
                .block(Block::default().borders(Borders::NONE))
                .style(Style::default().fg(Color::Blue))
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            f.render_stateful_widget(list, chunks[i], &mut state);
        }
    })?;
    Ok(())
}

pub fn quit_terminal_ui() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen)
}