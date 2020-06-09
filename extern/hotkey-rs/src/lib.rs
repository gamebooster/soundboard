#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

mod traits;

pub use traits::Listener;
pub use traits::HotkeyListener;

#[cfg(target_os = "linux")]
pub use linux::modifiers;
#[cfg(target_os = "linux")]
pub use linux::keys;

#[cfg(windows)]
pub use windows::modifiers;
#[cfg(windows)]
pub use windows::keys;