use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

#[cfg(target_os = "linux")]
pub type ListenerID = (i32, u32);

#[cfg(target_os = "windows")]
pub type ListenerID = i32;

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

#[cfg(target_os = "linux")]
pub struct Listener {
    pub(crate) display: *mut xlib::Display,
    pub(crate) xlib: xlib::Xlib,
    pub(crate) sender: Sender<HotkeyMessage>,
    pub(crate) handlers: ListenerMap,
}

#[cfg(target_os = "windows")]
pub struct Listener {
    pub(crate) last_id: ListenerID,
    pub(crate) handlers: ListenerMap,
    pub(crate) sender: Sender<HotkeyMessage>,
}

pub type ListenerCallback = dyn FnMut() + 'static + Send;
pub(crate) type ListenerMap = Arc<Mutex<HashMap<ListenerID, Box<ListenerCallback>>>>;

pub enum HotkeyMessage {
    RegisterHotkey(ListenerID, u32, u32),
    UnregisterHotkey(ListenerID),
    DropThread,
}

#[derive(Error, Debug)]
pub enum HotkeyError {
    #[error("channel error")]
    ChannelError(#[from] mpsc::SendError<HotkeyMessage>),
    // #[error("lock error")]
    // LockError(#[from] mpsc::SendError<HotkeyMessage>),
    #[error("unknown error")]
    Unknown,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.sender.send(HotkeyMessage::DropThread);
    }
}
