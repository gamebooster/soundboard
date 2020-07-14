use super::config;
use anyhow::{anyhow, Context, Result};
use hotkey_soundboard::HotkeyListener;
use hotkey_soundboard::Listener;
use hotkey_soundboard::ListenerHotkey;
use log::{error, info, trace, warn};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

type GlobalListener = Lazy<Arc<Mutex<Listener>>>;
type GlobalHotkeyMap =
    Arc<Mutex<HashMap<config::Hotkey, HashMap<usize, Box<dyn 'static + FnMut() + Send>>>>>;

static GLOBAL_LISTENER: GlobalListener = Lazy::new(|| Arc::new(Mutex::new(Listener::new())));
static GLOBAL_HOTKEY_MAP: Lazy<GlobalHotkeyMap> = Lazy::new(GlobalHotkeyMap::default);
static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct HotkeyManager {
    registered_hotkeys: Vec<config::Hotkey>,
    id: usize,
}

impl HotkeyManager {
    pub fn new() -> Self {
        HotkeyManager {
            registered_hotkeys: Vec::new(),
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
    pub fn register<F>(&mut self, hotkey: config::Hotkey, callback: F) -> Result<()>
    where
        F: 'static + FnMut() + Send,
    {
        let position = self.registered_hotkeys.iter().position(|h| h == &hotkey);
        if position.is_some() {
            return Err(anyhow!("hotkey already registered {}", hotkey));
        }

        let hotkey_clone = hotkey.clone();
        match GLOBAL_HOTKEY_MAP.lock().entry(hotkey.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.insert(self.id, Box::new(callback));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let hotkey_clone = hotkey.clone();
                GLOBAL_LISTENER
                    .lock()
                    .register_hotkey(
                        ListenerHotkey::new(hotkey.modifier_as_flag(), hotkey.key as u32),
                        move || {
                            if let Some(entry) = GLOBAL_HOTKEY_MAP.lock().get_mut(&hotkey) {
                                for (_, cb) in entry.iter_mut() {
                                    cb();
                                }
                            }
                        },
                    )
                    .with_context(|| format!("Failed to register hotkey {}", hotkey_clone))?;
                let mut new_map: HashMap<usize, Box<dyn 'static + FnMut() + Send>> = HashMap::new();
                new_map.insert(self.id, Box::new(callback));
                entry.insert(new_map);
            }
        }
        info!("register hotkey {}", &hotkey_clone);
        self.registered_hotkeys.push(hotkey_clone);
        Ok(())
    }

    pub fn unregister(&mut self, hotkey: &config::Hotkey) -> Result<()> {
        let position = self.registered_hotkeys.iter().position(|h| h == hotkey);
        if position.is_none() {
            return Err(anyhow!("hotkey not registered {}", hotkey));
        }
        self.registered_hotkeys.remove(position.unwrap());

        match GLOBAL_HOTKEY_MAP.lock().entry(hotkey.clone()) {
            std::collections::hash_map::Entry::Occupied(mut occ_entry) => {
                let entry = occ_entry.get_mut();
                if entry.remove(&self.id).is_none() {
                    panic!("should never be vacant");
                }
                if entry.is_empty() {
                    occ_entry.remove_entry();
                    GLOBAL_LISTENER
                        .lock()
                        .unregister_hotkey(ListenerHotkey::new(
                            hotkey.modifier_as_flag(),
                            hotkey.key as u32,
                        ))
                        .with_context(|| format!("Failed to unregister hotkey {}", hotkey))?;
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                panic!("should never be vacant");
            }
        }
        info!("unregister hotkey {}", hotkey);
        Ok(())
    }
    pub fn unregister_all(&mut self) -> Result<()> {
        let mut result = Ok(());
        for hotkey in self.registered_hotkeys.clone().iter() {
            result = self.unregister(hotkey);
        }
        result
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        if let Err(err) = self.unregister_all() {
            error!("drop: failed to unregister all hotkeys {:?}", err);
        }
    }
}
