//! Shared pieces of the sleep-block application.
//!
//! Two binaries are built from this crate: `sleep-blockd`, the daemon that owns
//! the inhibitors and the tray, and `sleep-block`, the GUI that talks to it.
//! The split exists because a Wayland window cannot be hidden or restored from
//! within its own process, so "show the window" has to mean "start the GUI".

pub mod client;
pub mod tray;

mod spawn;
pub use spawn::{GuiPresence, spawn_gui};
