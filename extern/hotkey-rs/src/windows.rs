use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use winapi::shared::windef::HWND;
use winapi::um::winuser;

use super::traits::*;

pub mod modifiers {
    use winapi::um::winuser;
    pub const ALT: u32 = winuser::MOD_ALT as u32;
    pub const CONTROL: u32 = winuser::MOD_CONTROL as u32;
    pub const SHIFT: u32 = winuser::MOD_SHIFT as u32;
    pub const SUPER: u32 = winuser::MOD_WIN as u32;
}

pub mod keys {
    use winapi::um::winuser;
    pub const BACKSPACE: u32 = winuser::VK_BACK as u32;
    pub const TAB: u32 = winuser::VK_TAB as u32;
    pub const ENTER: u32 = winuser::VK_RETURN as u32;
    pub const CAPS_LOCK: u32 = winuser::VK_CAPITAL as u32;
    pub const ESCAPE: u32 = winuser::VK_ESCAPE as u32;
    pub const SPACEBAR: u32 = winuser::VK_SPACE as u32;
    pub const PAGE_UP: u32 = winuser::VK_PRIOR as u32;
    pub const PAGE_DOWN: u32 = winuser::VK_NEXT as u32;
    pub const END: u32 = winuser::VK_END as u32;
    pub const HOME: u32 = winuser::VK_HOME as u32;
    pub const ARROW_LEFT: u32 = winuser::VK_LEFT as u32;
    pub const ARROW_RIGHT: u32 = winuser::VK_RIGHT as u32;
    pub const ARROW_UP: u32 = winuser::VK_UP as u32;
    pub const ARROW_DOWN: u32 = winuser::VK_DOWN as u32;
    pub const PRINT_SCREEN: u32 = winuser::VK_SNAPSHOT as u32;
    pub const INSERT: u32 = winuser::VK_INSERT as u32;
    pub const DELETE: u32 = winuser::VK_DELETE as u32;
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

type ListenerId = i32;
enum HotkeyMessage {
    RegisterHotkey(ListenerId, ListenerHotkey),
    RegisterHotkeyResult(Result<(), HotkeyError>),
    UnregisterHotkey(ListenerId),
    UnregisterHotkeyResult(Result<(), HotkeyError>),
    DropThread,
}
type ListenerMap = Arc<Mutex<HashMap<ListenerId, (ListenerHotkey, Box<ListenerCallback>)>>>;

pub struct Listener {
    last_id: ListenerId,
    handlers: ListenerMap,
    sender: Sender<HotkeyMessage>,
    receiver: Receiver<HotkeyMessage>,
}

impl HotkeyListener for Listener {
    fn new() -> Listener {
        let hotkeys = ListenerMap::default();

        let hotkey_map = hotkeys.clone();
        let (method_sender, thread_receiver) = mpsc::channel();
        let (thread_sender, method_receiver) = mpsc::channel();

        thread::spawn(move || unsafe {
            loop {
                let mut msg = std::mem::MaybeUninit::uninit(); // initialized from PeekMessageW
                while winuser::PeekMessageW(&mut *msg.as_mut_ptr(), 0 as HWND, 0, 0, 1) > 0 {
                    let msg = msg.assume_init();

                    if msg.wParam != 0 {
                        if let Some((_, handler)) =
                            hotkey_map.lock().unwrap().get_mut(&(msg.wParam as i32))
                        {
                            handler();
                        }
                    }
                }
                match thread_receiver.try_recv() {
                    Ok(HotkeyMessage::RegisterHotkey(id, hotkey)) => {
                        let result =
                            winuser::RegisterHotKey(0 as HWND, id, hotkey.modifiers, hotkey.key);
                        if result == 0 {
                            if let Err(err) =
                                thread_sender.send(HotkeyMessage::RegisterHotkeyResult(Err(
                                    HotkeyError::BackendApiError(
                                        winapi::um::errhandlingapi::GetLastError() as usize,
                                    ),
                                )))
                            {
                                eprintln!("hotkey: thread_sender.send error {}", err);
                            }
                        } else if let Err(err) =
                            thread_sender.send(HotkeyMessage::RegisterHotkeyResult(Ok(())))
                        {
                            eprintln!("hotkey: thread_sender.send error {}", err);
                        }
                    }
                    Ok(HotkeyMessage::UnregisterHotkey(id)) => {
                        let result = winuser::UnregisterHotKey(0 as HWND, id);
                        if result == 0 {
                            if let Err(err) =
                                thread_sender.send(HotkeyMessage::UnregisterHotkeyResult(Err(
                                    HotkeyError::BackendApiError(
                                        winapi::um::errhandlingapi::GetLastError() as usize,
                                    ),
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
                        return;
                    }
                    Err(err) => {
                        if let std::sync::mpsc::TryRecvError::Disconnected = err {
                            eprintln!("hotkey: try_recv error {}", err);
                        }
                    }
                    _ => unreachable!("other message should not arrive"),
                }

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        Listener {
            sender: method_sender,
            receiver: method_receiver,
            last_id: 0,
            handlers: hotkeys,
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
        self.last_id += 1;
        let id = self.last_id;
        self.sender
            .send(HotkeyMessage::RegisterHotkey(id, hotkey))
            .map_err(|_| HotkeyError::ChannelError())?;
        match self.receiver.recv() {
            Ok(HotkeyMessage::RegisterHotkeyResult(Ok(_))) => {
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
        let mut found_id = -1;
        for (id, (key, _)) in self.handlers.lock().unwrap().iter() {
            if *key == hotkey {
                found_id = *id;
                break;
            }
        }
        if found_id == -1 {
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
            eprintln!("hotkey: cant send close thread message {}", err);
        }
    }
}
