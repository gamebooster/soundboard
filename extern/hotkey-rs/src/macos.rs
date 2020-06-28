use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use super::traits::*;

pub mod modifiers {
    pub const ALT: u32 = 0;
    pub const CONTROL: u32 = 1;
    pub const SHIFT: u32 = 2;
    pub const SUPER: u32 = 3;
}

pub mod keys {
    pub const BACKSPACE: u32 = 0;
    pub const TAB: u32 = 1;
    pub const ENTER: u32 = 2;
    pub const CAPS_LOCK: u32 = 3;
    pub const ESCAPE: u32 = 4;
    pub const SPACEBAR: u32 = 5;
    pub const PAGE_UP: u32 = 6;
    pub const PAGE_DOWN: u32 = 7;
    pub const END: u32 = 8;
    pub const HOME: u32 = 9;
    pub const ARROW_LEFT: u32 = 10;
    pub const ARROW_RIGHT: u32 = 11;
    pub const ARROW_UP: u32 = 12;
    pub const ARROW_DOWN: u32 = 13;
    pub const PRINT_SCREEN: u32 = 14;
    pub const INSERT: u32 = 15;
    pub const DELETE: u32 = 16;
}

impl HotkeyListener<ListenerID> for Listener {
    fn new() -> Listener {
        let hotkeys = Arc::new(Mutex::new(
            HashMap::<ListenerID, Box<ListenerCallback>>::new(),
        ));

        // let hotkey_map = hotkeys.clone();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || loop {
            match rx.try_recv() {
                Ok(HotkeyMessage::RegisterHotkey(_id, _modifiers, _key)) => {
                    unimplemented!();
                }
                Ok(HotkeyMessage::UnregisterHotkey(_id)) => {
                    unimplemented!();
                }
                Ok(HotkeyMessage::DropThread) => {
                    break;
                }
                Err(_) => {}
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        });

        Listener {
            sender: tx,
            handlers: hotkeys,
        }
    }

    fn register_hotkey<CB: 'static + FnMut() + Send>(
        &mut self,
        _modifiers: u32,
        _key: u32,
        _handler: CB,
    ) -> Result<ListenerID, HotkeyError> {
        Ok(0)
    }

    fn unregister_hotkey(&mut self, _id: i32) -> Result<(), HotkeyError> {
        Ok(())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.sender
            .send(HotkeyMessage::DropThread)
            .expect("cant close thread");
    }
}
