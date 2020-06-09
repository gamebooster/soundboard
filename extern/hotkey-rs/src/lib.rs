#[cfg(target_os = "linux")]
mod linux;
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

#[cfg(windows)]
pub use windows::keys;
#[cfg(windows)]
pub use windows::modifiers;
