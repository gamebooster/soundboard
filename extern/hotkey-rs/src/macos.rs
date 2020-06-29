use std::os::raw::{c_int, c_void};
use std::sync::mpsc;
use std::thread;

use super::traits::*;

pub mod modifiers {
    pub const ALT: u32 = 256;
    pub const CONTROL: u32 = 4096;
    pub const SHIFT: u32 = 512;
    pub const SUPER: u32 = 2048;
}

pub mod keys {
    pub const BACKSPACE: u32 = 0x33;
    pub const TAB: u32 = 0x30;
    pub const ENTER: u32 = 0x24;
    pub const CAPS_LOCK: u32 = 0x39;
    pub const ESCAPE: u32 = 0x35;
    pub const SPACEBAR: u32 = 0x31;
    pub const PAGE_UP: u32 = 0x74;
    pub const PAGE_DOWN: u32 = 0x79;
    pub const END: u32 = 0x77;
    pub const HOME: u32 = 0x73;
    pub const ARROW_LEFT: u32 = 0x7B;
    pub const ARROW_RIGHT: u32 = 0x7C;
    pub const ARROW_UP: u32 = 0x7E;
    pub const ARROW_DOWN: u32 = 0x7D;
    pub const PRINT_SCREEN: u32 = 0xDEAD;
    pub const INSERT: u32 = 0x72;
    pub const DELETE: u32 = 0x75;
    pub const A: u32 = 0x00;
    pub const S: u32 = 0x01;
    pub const D: u32 = 0x02;
    pub const F: u32 = 0x03;
    pub const H: u32 = 0x04;
    pub const G: u32 = 0x05;
    pub const Z: u32 = 0x06;
    pub const X: u32 = 0x07;
    pub const C: u32 = 0x08;
    pub const V: u32 = 0x09;
    pub const B: u32 = 0x0B;
    pub const Q: u32 = 0x0C;
    pub const W: u32 = 0x0D;
    pub const E: u32 = 0x0E;
    pub const R: u32 = 0x0F;
    pub const Y: u32 = 0x10;
    pub const T: u32 = 0x11;
    pub const KEY_1: u32 = 0x12;
    pub const KEY_2: u32 = 0x13;
    pub const KEY_3: u32 = 0x14;
    pub const KEY_4: u32 = 0x15;
    pub const KEY_6: u32 = 0x16;
    pub const KEY_5: u32 = 0x17;
    pub const EQUAL: u32 = 0x18;
    pub const KEY_9: u32 = 0x19;
    pub const KEY_7: u32 = 0x1A;
    pub const MINUS: u32 = 0x1B;
    pub const KEY_8: u32 = 0x1C;
    pub const KEY_0: u32 = 0x1D;
    pub const RIGHT_BRACKET: u32 = 0x1E;
    pub const O: u32 = 0x1F;
    pub const U: u32 = 0x20;
    pub const LEFT_BRACKET: u32 = 0x21;
    pub const I: u32 = 0x22;
    pub const P: u32 = 0x23;
    pub const L: u32 = 0x25;
    pub const J: u32 = 0x26;
    pub const QUOTE: u32 = 0x27;
    pub const K: u32 = 0x28;
    pub const SEMICOLON: u32 = 0x29;
    pub const BACKSLASH: u32 = 0x2A;
    pub const COMMA: u32 = 0x2B;
    pub const SLASH: u32 = 0x2C;
    pub const N: u32 = 0x2D;
    pub const M: u32 = 0x2E;
    pub const PERIOD: u32 = 0x2F;
    pub const GRAVE: u32 = 0x32;
    pub const KEYPAD_DECIMAL: u32 = 0x41;
    pub const KEYPAD_MULTIPLY: u32 = 0x43;
    pub const KEYPAD_PLUS: u32 = 0x45;
    pub const KEYPAD_CLEAR: u32 = 0x47;
    pub const KEYPAD_DIVIDE: u32 = 0x4B;
    pub const KEYPAD_ENTER: u32 = 0x4C;
    pub const KEYPAD_MINUS: u32 = 0x4E;
    pub const KEYPAD_EQUALS: u32 = 0x51;
    pub const KEYPAD_0: u32 = 0x52;
    pub const KEYPAD_1: u32 = 0x53;
    pub const KEYPAD_2: u32 = 0x54;
    pub const KEYPAD_3: u32 = 0x55;
    pub const KEYPAD_4: u32 = 0x56;
    pub const KEYPAD_5: u32 = 0x57;
    pub const KEYPAD_6: u32 = 0x58;
    pub const KEYPAD_7: u32 = 0x59;
    pub const KEYPAD_8: u32 = 0x5B;
    pub const KEYPAD_9: u32 = 0x5C;
}

pub type KeyCallback = unsafe extern "C" fn(c_int, *mut c_void);

#[link(name = "carbon_hotkey_binding.a", kind = "static")]
extern "C" {
    fn install_event_handler(cb: KeyCallback, data: *mut c_void) -> *mut c_void;
    fn uninstall_event_handler(handler_ref: *mut c_void) -> c_int;
    fn register_hotkey(id: i32, modifier: i32, key: i32) -> *mut c_void;
    fn unregister_hotkey(hotkey_ref: *mut c_void) -> c_int;
}

unsafe extern "C" fn trampoline<F>(result: c_int, user_data: *mut c_void)
where
    F: FnMut(c_int) + 'static,
{
    let user_data = &mut *(user_data as *mut F);
    user_data(result);
}

pub fn get_trampoline<F>() -> KeyCallback
where
    F: FnMut(c_int) + 'static,
{
    trampoline::<F>
}

pub fn register_event_handler_callback<F>(handler: *mut F) -> *mut c_void
where
    F: FnMut(i32) + 'static + Sync + Send,
{
    unsafe {
        let cb = get_trampoline::<F>();

        install_event_handler(cb, handler as *mut c_void)
    }
}

impl HotkeyListener<ListenerID> for Listener {
    fn new() -> Listener {
        let hotkeys = ListenerMap::default();

        let hotkey_map = hotkeys.clone();
        let (tx, rx) = mpsc::sync_channel(10);
        let tx_clone = tx.clone();

        thread::spawn(move || {
            let callback = Box::new(move |id| {
                eprintln!("{}", id);
                if let Err(err) = tx_clone.send(HotkeyMessage::ReceivedHotkeyMessage(id)) {
                    eprintln!("send hotkey failed {}", err);
                }
            });

            let saved_callback = Box::into_raw(callback);
            let event_handler_ref = register_event_handler_callback(saved_callback);

            if event_handler_ref.is_null() {
                eprintln!("register_event_handler_callback failed!");
                return;
            }

            loop {
                match rx.recv() {
                    Ok(HotkeyMessage::RegisterHotkey(id, modifiers, key)) => unsafe {
                        let handler_ref = register_hotkey(id, modifiers as i32, key as i32);
                        if handler_ref.is_null() {
                            eprintln!("register_hotkey failed!");
                            return;
                        }
                        if let Some((_, handler)) = hotkey_map.lock().unwrap().get_mut(&id) {
                            *handler = CarbonRef::new(handler_ref);
                        }
                    },
                    Ok(HotkeyMessage::ReceivedHotkeyMessage(id)) => {
                        if let Some((handler, _)) = hotkey_map.lock().unwrap().get_mut(&id) {
                            handler();
                        }
                    }
                    Ok(HotkeyMessage::UnregisterHotkey(id)) => unsafe {
                        if let Some((_, handler_ref)) = hotkey_map.lock().unwrap().get_mut(&id) {
                            let _result = unregister_hotkey(handler_ref.0);
                            // eprintln!("unregister_hotkey: {}", result);
                        }
                    },
                    Ok(HotkeyMessage::DropThread) => unsafe {
                        for (_, handler_ref) in hotkey_map.lock().unwrap().values() {
                            let _result = unregister_hotkey(handler_ref.0);
                            // eprintln!("unregister_hotkey: {}", result);
                        }
                        let _result = uninstall_event_handler(event_handler_ref);
                        // eprintln!("uninstall_event_handler: {}", result);
                        Box::from_raw(saved_callback);
                        break;
                    },
                    Err(_) => {}
                }
            }
        });

        Listener {
            sender: tx,
            handlers: hotkeys,
            last_id: 0,
        }
    }

    fn register_hotkey<CB: 'static + FnMut() + Send>(
        &mut self,
        modifiers: u32,
        key: u32,
        handler: CB,
    ) -> Result<ListenerID, HotkeyError> {
        self.last_id += 1;
        let id = self.last_id;
        self.sender
            .send(HotkeyMessage::RegisterHotkey(id, modifiers, key))
            .unwrap();
        self.handlers.lock().unwrap().insert(
            id,
            (Box::new(handler), CarbonRef::new(std::ptr::null_mut())),
        );
        Ok(id)
    }

    fn unregister_hotkey(&mut self, id: i32) -> Result<(), HotkeyError> {
        self.sender
            .send(HotkeyMessage::UnregisterHotkey(id))
            .unwrap();
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
