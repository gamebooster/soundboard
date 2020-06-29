use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::mpsc;
#[cfg(not(target_os = "macos"))]
use std::sync::mpsc::Sender;
#[cfg(target_os = "macos")]
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

#[cfg(target_os = "linux")]
use x11_dl::xlib;

#[cfg(target_os = "linux")]
pub type ListenerID = (i32, u32);

#[cfg(target_os = "windows")]
pub type ListenerID = i32;

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
pub struct Listener {
    pub(crate) last_id: ListenerID,
    pub(crate) handlers: ListenerMap,
    pub(crate) sender: SyncSender<HotkeyMessage>,
}

pub type ListenerCallback = dyn FnMut() + 'static + Send;

#[cfg(not(target_os = "macos"))]
pub(crate) type ListenerMap = Arc<Mutex<HashMap<ListenerID, Box<ListenerCallback>>>>;

#[cfg(target_os = "macos")]
pub struct CarbonRef(pub *mut c_void);

#[cfg(target_os = "macos")]
impl CarbonRef {
    pub fn new(start: *mut c_void) -> Self {
        CarbonRef(start)
    }
}

#[cfg(target_os = "macos")]
unsafe impl Sync for CarbonRef {}
#[cfg(target_os = "macos")]
unsafe impl Send for CarbonRef {}

#[cfg(target_os = "macos")]
pub(crate) type ListenerMap = Arc<Mutex<HashMap<ListenerID, (Box<ListenerCallback>, CarbonRef)>>>;

#[cfg(not(target_os = "macos"))]
pub enum HotkeyMessage {
    RegisterHotkey(ListenerID, u32, u32),
    UnregisterHotkey(ListenerID),
    DropThread,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub enum HotkeyMessage {
    RegisterHotkey(ListenerID, u32, u32),
    ReceivedHotkeyMessage(ListenerID),
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
