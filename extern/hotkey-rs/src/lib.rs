#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

mod traits;
pub use traits::HotkeyError;
pub use traits::HotkeyListener;
pub use traits::ListenerHotkey;

#[cfg(target_os = "linux")]
pub use linux::keys;
#[cfg(target_os = "linux")]
pub use linux::modifiers;
#[cfg(target_os = "linux")]
pub use linux::Listener;

#[cfg(target_os = "macos")]
pub use macos::keys;
#[cfg(target_os = "macos")]
pub use macos::modifiers;
#[cfg(target_os = "macos")]
pub use macos::Listener;

#[cfg(target_os = "windows")]
pub use windows::keys;
#[cfg(target_os = "windows")]
pub use windows::modifiers;
#[cfg(target_os = "windows")]
pub use windows::Listener;
