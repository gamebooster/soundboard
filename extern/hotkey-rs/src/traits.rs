use thiserror::Error;

pub type ListenerCallback = dyn 'static + FnMut() + Send;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ListenerHotkey {
    pub modifiers: u32,
    pub key: u32,
}

impl ListenerHotkey {
    pub fn new(modifiers: u32, key: u32) -> Self {
        Self { modifiers, key }
    }
}

pub trait HotkeyListener {
    fn new() -> Self;
    fn register_hotkey<F>(
        &mut self,
        hotkey: ListenerHotkey,
        callback: F,
    ) -> Result<(), HotkeyError>
    where
        F: 'static + FnMut() + Send;
    fn unregister_hotkey(&mut self, hotkey: ListenerHotkey) -> Result<(), HotkeyError>;
    fn registered_hotkeys(&self) -> Vec<ListenerHotkey>;
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum HotkeyError {
    #[error("channel error")]
    ChannelError(),
    #[error("hotkey already registered: `{0:?}`")]
    HotkeyAlreadyRegistered(ListenerHotkey),
    #[error("hotkey not registered: `{0:?}`")]
    HotkeyNotRegistered(ListenerHotkey),
    #[error("backend api error: `{0}`")]
    BackendApiError(usize),
    #[error("unknown error")]
    Unknown,
}
