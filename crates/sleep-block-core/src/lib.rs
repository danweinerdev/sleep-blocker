//! Sleep and screen-lock inhibition via D-Bus.
//!
//! These are two unrelated mechanisms on two different buses:
//!
//! * [`Inhibitor`] blocks *system sleep* through systemd-logind on the system
//!   bus — the same mechanism `systemd-inhibit(1)` uses. `Inhibit` returns a
//!   file descriptor and the lock holds for exactly as long as it stays open,
//!   so the lifetime of the value *is* the lifetime of the lock.
//!
//! * [`ScreenInhibitor`] blocks *screen blanking and locking* through the
//!   `org.freedesktop.ScreenSaver` interface on the session bus — the call
//!   browsers and video players use. logind has no lock-related inhibitor type,
//!   so this cannot be folded into the one above.
//!
//! The two differ in how they are released, which is easy to get wrong: the
//! logind lock frees itself when its descriptor closes, whereas ScreenSaver
//! returns an opaque cookie that leaks until `UnInhibit` is called explicitly.
//! [`ScreenInhibitor`] therefore carries a `Drop` impl; [`Inhibitor`] needs none.

mod inhibit;
pub mod ipc;
mod state;

pub use inhibit::{Error, Inhibitor, ScreenInhibitor};
pub use state::{SleepBlock, Status};
