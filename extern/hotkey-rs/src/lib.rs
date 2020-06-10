#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

mod traits;

pub use traits::HotkeyListener;
pub use traits::Listener;
pub use traits::ListenerID;

#[cfg(target_os = "linux")]
pub use linux::keys;
#[cfg(target_os = "linux")]
pub use linux::modifiers;

#[cfg(target_os = "macos")]
pub use macos::keys;
#[cfg(target_os = "macos")]
pub use macos::modifiers;

#[cfg(windows)]
pub use windows::keys;
#[cfg(windows)]
pub use windows::modifiers;
