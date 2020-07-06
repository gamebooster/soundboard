use super::config;
use anyhow::{anyhow, Context, Result};
use hotkey_soundboard::HotkeyListener;
use hotkey_soundboard::Listener;
use hotkey_soundboard::ListenerID;
use log::{error, info, trace, warn};
use std::collections::HashMap;

pub struct HotkeyManager {
    listener: Listener,
    hashmap: HashMap<config::Hotkey, ListenerID>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        let listener = Listener::new();
        HotkeyManager {
            listener,
            hashmap: HashMap::<config::Hotkey, ListenerID>::new(),
        }
    }
    pub fn register<Callback: 'static + Fn() + Send>(
        &mut self,
        hotkey: config::Hotkey,
        callback: Callback,
    ) -> Result<()> {
        let result = self.listener.register_hotkey(
            hotkey.modifier_as_flag(),
            hotkey.key as u32,
            callback,
        )?;
        info!("register hotkey {}", &hotkey);
        self.hashmap.insert(hotkey, result);
        Ok(())
    }
    #[allow(dead_code)]
    pub fn unregister(&mut self, hotkey: config::Hotkey) -> Result<()> {
        self.listener.unregister_hotkey(
            *self
                .hashmap
                .get(&hotkey)
                .ok_or_else(|| anyhow!("no hotkey registered with this id"))?,
        )?;
        info!("unregister hotkey {}", &hotkey);
        Ok(())
    }
    pub fn unregister_all(&mut self) -> Result<()> {
        self.listener = Listener::new();
        Ok(())
    }
}
