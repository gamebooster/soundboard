use thiserror::Error;

#[cfg(target_os = "linux")]
pub type ListenerID = (i32, u32);

#[cfg(target_os = "windows")]
pub type ListenerID = i32;

#[cfg(target_os = "macos")]
pub type ListenerID = i32;

pub type ListenerCallback = dyn 'static + FnMut() + Send;

pub trait HotkeyListener<ListenerID> {
    fn new() -> Self;
    fn register_hotkey<CB: 'static + FnMut() + Send>(
        &mut self,
        modifiers: u32,
        key: u32,
        handler: CB,
    ) -> Result<ListenerID, HotkeyError>;
    fn unregister_hotkey(&mut self, id: ListenerID) -> Result<(), HotkeyError>;
}

#[derive(Error, Debug)]
pub enum HotkeyError {
    #[error("channel error")]
    ChannelError(),
    #[error("backend api error: `{0}`")]
    BackendApiError(usize),
    #[error("unknown error")]
    Unknown,
}
