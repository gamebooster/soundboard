use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use x11_dl::xlib;

use super::traits::*;

pub mod modifiers {
    use x11_dl::xlib;
    pub const ALT: u32 = xlib::Mod1Mask;
    pub const CONTROL: u32 = xlib::ControlMask;
    pub const SHIFT: u32 = xlib::ShiftMask;
    pub const SUPER: u32 = xlib::Mod4Mask;
}

pub mod keys {
    use x11_dl::keysym;
    pub const BACKSPACE: u32 = keysym::XK_BackSpace;
    pub const TAB: u32 = keysym::XK_Tab;
    pub const ENTER: u32 = keysym::XK_Return;
    pub const CAPS_LOCK: u32 = keysym::XK_Caps_Lock;
    pub const ESCAPE: u32 = keysym::XK_Escape;
    pub const SPACEBAR: u32 = keysym::XK_space;
    pub const PAGE_UP: u32 = keysym::XK_Page_Up;
    pub const PAGE_DOWN: u32 = keysym::XK_Page_Down;
    pub const END: u32 = keysym::XK_End;
    pub const HOME: u32 = keysym::XK_Home;
    pub const ARROW_LEFT: u32 = keysym::XK_Left;
    pub const ARROW_RIGHT: u32 = keysym::XK_Right;
    pub const ARROW_UP: u32 = keysym::XK_Up;
    pub const ARROW_DOWN: u32 = keysym::XK_Down;
    pub const PRINT_SCREEN: u32 = keysym::XK_Print;
    pub const INSERT: u32 = keysym::XK_Insert;
    pub const DELETE: u32 = keysym::XK_Delete;
    pub const KEY_0: u32 = '0' as u32;
    pub const KEY_1: u32 = '1' as u32;
    pub const KEY_2: u32 = '2' as u32;
    pub const KEY_3: u32 = '3' as u32;
    pub const KEY_4: u32 = '4' as u32;
    pub const KEY_5: u32 = '5' as u32;
    pub const KEY_6: u32 = '6' as u32;
    pub const KEY_7: u32 = '7' as u32;
    pub const KEY_8: u32 = '8' as u32;
    pub const KEY_9: u32 = '9' as u32;
    pub const A: u32 = 'A' as u32;
    pub const B: u32 = 'B' as u32;
    pub const C: u32 = 'C' as u32;
    pub const D: u32 = 'D' as u32;
    pub const E: u32 = 'E' as u32;
    pub const F: u32 = 'F' as u32;
    pub const G: u32 = 'G' as u32;
    pub const H: u32 = 'H' as u32;
    pub const I: u32 = 'I' as u32;
    pub const J: u32 = 'J' as u32;
    pub const K: u32 = 'K' as u32;
    pub const L: u32 = 'L' as u32;
    pub const M: u32 = 'M' as u32;
    pub const N: u32 = 'N' as u32;
    pub const O: u32 = 'O' as u32;
    pub const P: u32 = 'P' as u32;
    pub const Q: u32 = 'Q' as u32;
    pub const R: u32 = 'R' as u32;
    pub const S: u32 = 'S' as u32;
    pub const T: u32 = 'T' as u32;
    pub const U: u32 = 'U' as u32;
    pub const V: u32 = 'V' as u32;
    pub const W: u32 = 'W' as u32;
    pub const X: u32 = 'X' as u32;
    pub const Y: u32 = 'Y' as u32;
    pub const Z: u32 = 'Z' as u32;
}

enum HotkeyMessage {
    RegisterHotkey(ListenerId, u32, u32),
    RegisterHotkeyResult(Result<ListenerId, HotkeyError>),
    UnregisterHotkey(ListenerId),
    UnregisterHotkeyResult(Result<(), HotkeyError>),
    DropThread,
}

type ListenerId = (i32, u32);

pub struct Listener {
    handlers: ListenerMap,
    sender: Sender<HotkeyMessage>,
    receiver: Receiver<HotkeyMessage>,
}

type ListenerMap = Arc<Mutex<HashMap<ListenerId, (ListenerHotkey, Box<ListenerCallback>)>>>;

impl HotkeyListener for Listener {
    fn new() -> Listener {
        let hotkeys = ListenerMap::default();

        let hotkey_map = hotkeys.clone();
        let (method_sender, thread_receiver) = mpsc::channel();
        let (thread_sender, method_receiver) = mpsc::channel();

        std::thread::spawn(move || {
            let xlib = xlib::Xlib::open().unwrap();
            unsafe {
                let display = (xlib.XOpenDisplay)(ptr::null());
                let root = (xlib.XDefaultRootWindow)(display);

                // Only trigger key release at end of repeated keys
                (xlib.XkbSetDetectableAutoRepeat)(display, 1, ptr::null_mut());

                (xlib.XSelectInput)(display, root, xlib::KeyReleaseMask);
                let mut event = mem::MaybeUninit::uninit(); // initialized from XNextEvent
                loop {
                    if (xlib.XPending)(display) > 0
                        && (xlib.XNextEvent)(display, event.as_mut_ptr()) == 0
                    {
                        let event = event.assume_init();

                        if let xlib::KeyRelease = event.get_type() {
                            if let Some((_, handler)) = hotkey_map
                                .lock()
                                .unwrap()
                                .get_mut(&(event.key.keycode as i32, event.key.state))
                            {
                                handler();
                            }
                        }
                    }
                    match thread_receiver.try_recv() {
                        Ok(HotkeyMessage::RegisterHotkey(_, modifiers, key)) => {
                            let keycode = (xlib.XKeysymToKeycode)(display, key as u64) as i32;

                            let result = (xlib.XGrabKey)(
                                display,
                                keycode,
                                modifiers,
                                root,
                                0,
                                xlib::GrabModeAsync,
                                xlib::GrabModeAsync,
                            );
                            if result == 0 {
                                if let Err(err) =
                                    thread_sender.send(HotkeyMessage::RegisterHotkeyResult(Err(
                                        HotkeyError::BackendApiError(0),
                                    )))
                                {
                                    eprintln!("hotkey: thread_sender.send error {}", err);
                                }
                            } else if let Err(err) = thread_sender.send(
                                HotkeyMessage::RegisterHotkeyResult(Ok((keycode, modifiers))),
                            ) {
                                eprintln!("hotkey: thread_sender.send error {}", err);
                            }
                        }
                        Ok(HotkeyMessage::UnregisterHotkey(id)) => {
                            let result = (xlib.XUngrabKey)(display, id.0, id.1, root);
                            if result == 0 {
                                if let Err(err) =
                                    thread_sender.send(HotkeyMessage::UnregisterHotkeyResult(Err(
                                        HotkeyError::BackendApiError(0),
                                    )))
                                {
                                    eprintln!("hotkey: thread_sender.send error {}", err);
                                }
                            } else if let Err(err) =
                                thread_sender.send(HotkeyMessage::UnregisterHotkeyResult(Ok(())))
                            {
                                eprintln!("hotkey: thread_sender.send error {}", err);
                            }
                        }
                        Ok(HotkeyMessage::DropThread) => {
                            (xlib.XCloseDisplay)(display);
                            return;
                        }
                        Err(err) => {
                            if let std::sync::mpsc::TryRecvError::Disconnected = err {
                                eprintln!("hotkey: try_recv error {}", err);
                            }
                        }
                        _ => unreachable!("other message should not arrive"),
                    };

                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        });

        Listener {
            handlers: hotkeys,
            sender: method_sender,
            receiver: method_receiver,
        }
    }

    fn register_hotkey<F>(&mut self, hotkey: ListenerHotkey, handler: F) -> Result<(), HotkeyError>
    where
        F: 'static + FnMut() + Send,
    {
        for (key, _) in self.handlers.lock().unwrap().values() {
            if *key == hotkey {
                return Err(HotkeyError::HotkeyAlreadyRegistered(hotkey));
            }
        }
        self.sender
            .send(HotkeyMessage::RegisterHotkey(
                (0, 0),
                hotkey.modifiers,
                hotkey.key,
            ))
            .map_err(|_| HotkeyError::ChannelError())?;
        match self.receiver.recv() {
            Ok(HotkeyMessage::RegisterHotkeyResult(Ok(id))) => {
                self.handlers
                    .lock()
                    .unwrap()
                    .insert(id, (hotkey, Box::new(handler)));
                Ok(())
            }
            Ok(HotkeyMessage::RegisterHotkeyResult(Err(err))) => Err(err),
            Err(_) => Err(HotkeyError::ChannelError()),
            _ => Err(HotkeyError::Unknown),
        }
    }

    fn unregister_hotkey(&mut self, hotkey: ListenerHotkey) -> Result<(), HotkeyError> {
        let mut found_id = (-1, 0);
        for (id, (key, _)) in self.handlers.lock().unwrap().iter() {
            if *key == hotkey {
                found_id = *id;
                break;
            }
        }
        if found_id == (-1, 0) {
            return Err(HotkeyError::HotkeyNotRegistered(hotkey));
        }
        self.sender
            .send(HotkeyMessage::UnregisterHotkey(found_id))
            .map_err(|_| HotkeyError::ChannelError())?;
        if self.handlers.lock().unwrap().remove(&found_id).is_none() {
            panic!("hotkey should never be none")
        };
        match self.receiver.recv() {
            Ok(HotkeyMessage::UnregisterHotkeyResult(Ok(_))) => Ok(()),
            Ok(HotkeyMessage::UnregisterHotkeyResult(Err(err))) => Err(err),
            Err(_) => Err(HotkeyError::ChannelError()),
            _ => Err(HotkeyError::Unknown),
        }
    }
    fn registered_hotkeys(&self) -> Vec<ListenerHotkey> {
        let mut result = Vec::new();
        for v in self.handlers.lock().unwrap().values() {
            result.push(v.0);
        }
        result
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(err) = self.sender.send(HotkeyMessage::DropThread) {
            eprintln!("cant send close thread message {}", err);
        }
    }
}
