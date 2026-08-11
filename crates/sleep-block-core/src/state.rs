//! Shared inhibitor state.
//!
//! The tray and the window are equal peers: either can toggle, and both must
//! reflect the result. That rules out keeping the locks inside the GUI struct,
//! since the tray runs on its own thread. [`SleepBlock`] owns them instead and
//! is cheap to clone, so each surface holds a handle to the same state.

use std::sync::{Arc, Mutex};

use crate::{Error, Inhibitor, ScreenInhibitor};

/// A snapshot of what is currently inhibited, for rendering.
///
/// Returned by [`SleepBlock::snapshot`] rather than exposing the lock, so
/// callers cannot hold the mutex across a repaint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Whether system sleep is currently blocked.
    pub sleep_blocked: bool,
    /// Whether screen blanking and locking is also blocked. Always false when
    /// `sleep_blocked` is false — the screen lock is an add-on to sleep
    /// blocking, never held alone.
    pub screen_blocked: bool,
    /// Whether the user wants the screen kept awake. Retained while sleep
    /// blocking is off so the preference survives a toggle cycle.
    pub keep_screen_awake: bool,
}

#[derive(Debug, Default)]
struct Inner {
    inhibitor: Option<Inhibitor>,
    screen: Option<ScreenInhibitor>,
    keep_screen_awake: bool,
}

/// Handle to the inhibitor state. Cloning yields another handle to the same
/// underlying locks, not a copy of them.
#[derive(Debug, Clone, Default)]
pub struct SleepBlock {
    inner: Arc<Mutex<Inner>>,
}

impl SleepBlock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the current state. Takes the lock only for the duration of the
    /// copy, so it is safe to call from a render loop.
    pub fn snapshot(&self) -> Status {
        let inner = self.lock();
        Status {
            sleep_blocked: inner.inhibitor.is_some(),
            screen_blocked: inner.screen.is_some(),
            keep_screen_awake: inner.keep_screen_awake,
        }
    }

    /// Turns sleep blocking on if off, and off if on.
    ///
    /// Returns the resulting state, or the error that prevented the change. On
    /// failure nothing is altered, so the caller can surface the message
    /// without having to undo a partial transition.
    pub fn toggle(&self) -> Result<Status, Error> {
        let mut inner = self.lock();

        if inner.inhibitor.is_some() {
            // Dropping releases both: the logind descriptor closes and
            // ScreenInhibitor's Drop calls UnInhibit.
            inner.screen = None;
            inner.inhibitor = None;
            return Ok(Self::status_of(&inner));
        }

        inner.inhibitor = Some(Inhibitor::acquire(REASON)?);

        // Restore the screen lock if the user had asked for it. A failure here
        // is reported but leaves sleep blocking active: the screen lock is the
        // secondary concern and losing it is no reason to allow suspend.
        if inner.keep_screen_awake {
            match ScreenInhibitor::acquire(REASON) {
                Ok(screen) => inner.screen = Some(screen),
                Err(e) => {
                    inner.keep_screen_awake = false;
                    return Err(e);
                }
            }
        }

        Ok(Self::status_of(&inner))
    }

    /// Sets whether the screen should also be kept awake, acquiring or
    /// releasing that lock immediately if sleep blocking is already on.
    ///
    /// The preference is recorded even when sleep blocking is off, so enabling
    /// it later restores the user's choice.
    pub fn set_keep_screen_awake(&self, wanted: bool) -> Result<Status, Error> {
        let mut inner = self.lock();
        inner.keep_screen_awake = wanted;

        let should_hold = wanted && inner.inhibitor.is_some();
        match (should_hold, inner.screen.is_some()) {
            (true, false) => match ScreenInhibitor::acquire(REASON) {
                Ok(screen) => inner.screen = Some(screen),
                Err(e) => {
                    inner.keep_screen_awake = false;
                    return Err(e);
                }
            },
            (false, true) => inner.screen = None,
            _ => {}
        }

        Ok(Self::status_of(&inner))
    }

    /// Releases everything. Used on shutdown so the locks go away promptly
    /// rather than at process teardown.
    pub fn release_all(&self) {
        let mut inner = self.lock();
        inner.screen = None;
        inner.inhibitor = None;
    }

    fn status_of(inner: &Inner) -> Status {
        Status {
            sleep_blocked: inner.inhibitor.is_some(),
            screen_blocked: inner.screen.is_some(),
            keep_screen_awake: inner.keep_screen_awake,
        }
    }

    /// Recovers from a poisoned mutex rather than propagating the panic. The
    /// guarded data is a pair of Options; a panic mid-toggle can leave a lock
    /// held or dropped, but never in a state that is unsafe to inspect.
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

const REASON: &str = "User requested the system stay awake";
