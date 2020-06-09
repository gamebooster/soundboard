use ::hotkey as hotkeyExt;
use anyhow::{anyhow, Context, Result};
use super::config;
use std::sync::Mutex;
use hotkeyExt::HotkeyListener;

pub struct HotkeyManager {
    listener: hotkeyExt::Listener
}

impl HotkeyManager {
    pub fn new() -> Self {
      let listener = hotkeyExt::Listener::new();
      let hotkey_manager = HotkeyManager {
        listener: listener
      };
      hotkey_manager
    }
    pub fn register<Callback : 'static + Fn() + Send + Sync>(&mut self, hotkey : config::Hotkey, callback : Callback) -> Result<()> {
        self.listener.register_hotkey(
            hotkey
                .modifier
                .iter()
                .fold(0, |acc, x| acc | (*x as u32)) as u32,
            hotkey.key as u32, callback)
        .or_else(|_s| Err(anyhow!("register key")))?;
        Ok(())
    }
}