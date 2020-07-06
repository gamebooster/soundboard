
use std::{error:: Error, io::{stdout, Write}};
use tui:: {
    Terminal,
    backend::CrosstermBackend,
    layout::{Layout, Constraint, Direction},
    style::{Color, Modifier, Style},
    widgets::{Widget, Block, Borders, BorderType}
};
use crossterm::{
    event::KeyEvent, 
    execute, 
    Result, 
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, ScrollUp, ScrollDown, SetSize, size}
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
            .direction(Direction::Vertical)
            .margin(4)
            .constraints(
                [Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25)]
            .as_ref())
            .split(f.size());
        let mut i = 0;
        let current_sounds = config::MainConfig::read()
            .soundboards
            .iter()
            .find(|s| s.name == "favorites")
            .unwrap()
            .sounds
            .as_ref()
            .unwrap()
            .clone();
        for sound in &current_sounds {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50)].as_ref())
                .split(chunks[i]);
            let block = Block::default()
                .title(&sound.name)
                .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD));
            f.render_widget(block, chunks[0]);
            i = (i + 1) % 4;
        };
    })?;
    Ok(())
}

pub fn quit_terminal_ui() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen)
}