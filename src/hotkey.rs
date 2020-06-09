use super::config;
use ::hotkey as hotkeyExt;
use anyhow::{anyhow, Context, Result};
use hotkeyExt::HotkeyListener;
use hotkeyExt::ListenerID;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct HotkeyManager {
    listener: hotkeyExt::Listener,
    hashmap: HashMap<config::Hotkey, ListenerID>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        let listener = hotkeyExt::Listener::new();
        let hotkey_manager = HotkeyManager {
            listener,
            hashmap: HashMap::<config::Hotkey, ListenerID>::new(),
        };
        hotkey_manager
    }
    pub fn register<Callback: 'static + Fn() + Send + Sync>(
        &mut self,
        hotkey: config::Hotkey,
        callback: Callback,
    ) -> Result<()> {
        let result = self.listener.register_hotkey(
            hotkey.modifier_as_flag(),
            hotkey.key as u32,
            callback,
        )?;
        self.hashmap.insert(hotkey, result);
        Ok(())
    }
    #[allow(dead_code)]
    pub fn unregister(&mut self, hotkey: config::Hotkey) -> Result<()> {
        self.listener.unregister_hotkey(
            *self
                .hashmap
                .get(&hotkey)
                .ok_or(anyhow!("no hotkey registered with this id"))?,
        )?;
        Ok(())
    }
    pub fn unregister_all(&mut self) -> Result<()> {
        self.listener = hotkeyExt::Listener::new();
        Ok(())
    }
}
