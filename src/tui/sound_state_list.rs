use super::super::soundboards::Sound;
use tui::widgets::ListState;

pub struct SoundStateList {
    pub name: String,
    sounds: Vec<Sound>,
    filter: String,
    pub filtered_sounds: Vec<Sound>,
    pub state: ListState,
}

impl SoundStateList {
    pub fn new(name: &str, sounds: Vec<Sound>) -> Self {
        Self {
            name: name.to_string(),
            state: ListState::default(),
            filter: String::new(),
            filtered_sounds: sounds.clone(),
            sounds,
        }
    }
    pub fn index(&self) -> usize {
        match self.state.selected() {
            Some(i) => i,
            None => 0,
        }
    }

    pub fn update_filter(&mut self, new_filter: &str) {
        if self.filter == new_filter {
            return;
        }
        self.filter = new_filter.to_string();
        self.filtered_sounds = self
            .sounds
            .iter()
            .filter(|s| s.get_name().to_lowercase().contains(&self.filter))
            .cloned()
            .collect();
        if self.filtered_sounds.len() <= self.index() {
            if !self.filtered_sounds.is_empty() {
                self.state.select(Some(self.filtered_sounds.len() - 1))
            } else {
                self.state.select(Some(0));
            }
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.filtered_sounds.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_sounds.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}
