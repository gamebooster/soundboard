use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use x11_dl::xlib;
use traits;

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
}

impl HotkeyListener<ListenerID> for Listener {
  pub fn new() -> Listener {
    let hotkeys = Arc::new(Mutex::new(
      HashMap::<ListenerID, Box<ListenerCallback>>::new(),
    ));

    let hotkey_map = hotkeys.clone();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
      let xlib = xlib::Xlib::open().unwrap();
      unsafe {
        let display = (xlib.XOpenDisplay)(ptr::null());
        let root = (xlib.XDefaultRootWindow)(display);

        // Only trigger key release at end of repeated keys
        let mut supported_rtrn: i32 = mem::MaybeUninit::uninit().assume_init();
        (xlib.XkbSetDetectableAutoRepeat)(display, 1, &mut supported_rtrn);

        (xlib.XSelectInput)(display, root, xlib::KeyReleaseMask);
        let mut event: xlib::XEvent = mem::MaybeUninit::uninit().assume_init();
        loop {
          if (xlib.XPending)(display) > 0 {
            (xlib.XNextEvent)(display, &mut event);
            match event.get_type() {
              xlib::KeyRelease => {
                if let Some(handler) = hotkey_map
                  .lock()
                  .unwrap()
                  .get_mut(&(event.key.keycode as i32, event.key.state))
                {
                  handler();
                }
              }
              _ => (),
            }
          }
          match rx.try_recv() {
            Ok(HotkeyMessage::RegisterHotkey((keycode, modifiers), modifier, key)) => {
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
                println!("{}", "Failed to register hotkey".to_string());
              }
            }
            Ok(HotkeyMessage::UnregisterHotkey(id)) => {
              let result = (xlib.XUngrabKey)(
                display,
                id.0,
                id.1,
                root
              );
              if result == 0 {
                println!("{}", "Failed to unregister hotkey".to_string());
              }
            }
            Ok(HotkeyMessage::DropThread) => {
              break;
            }
            Err(_) => {}
          };

          std::thread::sleep(std::time::Duration::from_millis(50));
        }
      }
    });

    unsafe {
      let xlib = xlib::Xlib::open().unwrap();
      let display = (xlib.XOpenDisplay)(ptr::null());

      Listener {
        display: display,
        xlib,
        handlers: hotkeys,
        sender: tx,
      }
    }
  }

  pub fn register_hotkey<CB: 'static + FnMut() + Send>(
    &mut self,
    modifiers: u32,
    key: u32,
    handler: CB,
  ) -> Result<ListenerID, String> {
    let keycode: i32;
    unsafe {
      keycode = (self.xlib.XKeysymToKeycode)(self.display, key as u64) as i32;
    }
    let id = (keycode, modifiers);
    self.sender.send(HotkeyMessage::RegisterHotkey(id, modifiers, key)).unwrap();
    self.handlers.lock().unwrap().insert(id, Box::new(handler));
    Ok(id)
  }

  pub fn unregister_hotkey(&mut self, id: ListenerID) -> Result<(), HotkeyError> {
    self.sender.send(HotkeyMessage::UnregisterHotkey(id))?;
    self.handlers.lock().unwrap().remove(&id);
    Ok(())
  }
}
