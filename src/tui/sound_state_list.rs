use tui::widgets::ListState;
use super::super::config::SoundConfig;

pub struct SoundStateList<SoundConfig> {
    pub sounds: Vec<SoundConfig>,
    pub state: ListState,
}

impl<SoundConfig> SoundStateList <SoundConfig> {

    pub fn new(sounds: Vec<SoundConfig>) -> SoundStateList<SoundConfig> {
        SoundStateList {
            state: ListState::default(),
            sounds,
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.sounds.len() - 1 {
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
                    self.sounds.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }
}
