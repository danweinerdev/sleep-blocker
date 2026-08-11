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

use std::os::fd::OwnedFd;

use zbus::blocking::Connection;

/// What we ask logind to block. `idle` covers the idle timeout that leads to
/// automatic suspend/screen blanking; `sleep` covers explicit suspend and
/// hibernate transitions. Together they are what a "keep the system awake"
/// button is expected to do.
const WHAT: &str = "idle:sleep";
const WHO: &str = "sleep-block";

/// `block` prevents the operation entirely, as opposed to `delay`, which only
/// postpones it briefly so an application can save state before sleeping.
const MODE: &str = "block";

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    /// Returns a descriptor that holds the lock until it is closed. D-Bus hands
    /// this back as a `zvariant::OwnedFd`, which we convert to the std type.
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
}

/// An active sleep inhibitor. The lock is released when this value is dropped.
#[derive(Debug)]
pub struct Inhibitor {
    // Never read: held purely so the kernel keeps the lock alive. Dropping this
    // struct closes the descriptor, which is what releases the lock in logind.
    _fd: OwnedFd,
}

impl Inhibitor {
    /// Acquires a block-mode inhibitor on the system bus.
    pub fn acquire(reason: &str) -> Result<Self, Error> {
        let connection = Connection::system().map_err(Error::Connect)?;
        let manager = ManagerProxyBlocking::new(&connection).map_err(Error::Connect)?;
        let fd = manager
            .inhibit(WHAT, WHO, reason, MODE)
            .map_err(Error::Inhibit)?;
        Ok(Self { _fd: fd.into() })
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait ScreenSaver {
    /// Returns a cookie identifying the inhibitor, released via `UnInhibit`.
    fn inhibit(&self, application_name: &str, reason: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

/// An active screen-blanking/locking inhibitor. Released on drop.
#[derive(Debug)]
pub struct ScreenInhibitor {
    /// Kept so `Drop` can call `UnInhibit`. Unlike the logind descriptor, this
    /// cookie means nothing to the desktop unless we hand it back explicitly.
    cookie: u32,
    /// The connection must outlive the cookie: some implementations drop an
    /// inhibitor when the owning bus connection disappears, and we need a live
    /// proxy at drop time regardless.
    connection: Connection,
}

impl ScreenInhibitor {
    /// Acquires a screen-lock inhibitor on the session bus.
    pub fn acquire(reason: &str) -> Result<Self, Error> {
        let connection = Connection::session().map_err(Error::Connect)?;
        let saver = ScreenSaverProxyBlocking::new(&connection).map_err(Error::Connect)?;
        let cookie = saver.inhibit(WHO, reason).map_err(Error::Inhibit)?;
        Ok(Self { cookie, connection })
    }
}

impl Drop for ScreenInhibitor {
    fn drop(&mut self) {
        // Best-effort: if the desktop already went away there is nothing left to
        // release, and failing here would be noise in an unwinding path.
        if let Ok(saver) = ScreenSaverProxyBlocking::new(&self.connection) {
            let _ = saver.un_inhibit(self.cookie);
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// Could not reach the system bus or logind at all.
    Connect(zbus::Error),
    /// logind refused or failed the Inhibit call.
    Inhibit(zbus::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "cannot reach systemd-logind: {e}"),
            Self::Inhibit(e) => write!(f, "logind refused the inhibitor: {e}"),
        }
    }
}

impl std::error::Error for Error {}
