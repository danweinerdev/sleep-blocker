//! The GUI's view of the daemon.
//!
//! Deliberately shaped like [`sleep_block_core::SleepBlock`] so the rendering
//! code did not have to change: same `snapshot`/`toggle`/`set_*` surface,
//! backed by D-Bus calls instead of local locks.
//!
//! The GUI holds no inhibitors. That is the whole reason the daemon exists —
//! a lock owned here would be released the instant this window closed.

use sleep_block_core::Status;
use sleep_block_core::ipc::SleepBlockServiceProxyBlocking;

/// Connection to the daemon, plus the last state it reported.
pub struct DaemonClient {
    proxy: SleepBlockServiceProxyBlocking<'static>,
    /// Whether the daemon has a tray, read once at connect: it cannot change
    /// while the daemon runs.
    has_tray: bool,
    /// Cached so a failed call can keep rendering the last known-good values
    /// rather than flickering to defaults.
    last: Status,
}

impl DaemonClient {
    /// Connects to a running daemon, starting one if necessary.
    ///
    /// Launching the daemon here means the GUI can be started directly (from a
    /// launcher or the command line) without the user knowing a daemon exists.
    pub fn connect() -> Result<Self, zbus::Error> {
        let connection = zbus::blocking::Connection::session()?;

        let proxy = match SleepBlockServiceProxyBlocking::new(&connection) {
            Ok(p) if p.sleep_blocked().is_ok() => p,
            _ => {
                // No daemon yet. Start one and wait briefly for it to claim the
                // bus name; the retry loop is short because this is a local
                // process start, not a network round trip.
                Self::spawn_daemon();
                let mut found = None;
                for _ in 0..40 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if let Ok(p) = SleepBlockServiceProxyBlocking::new(&connection)
                        && p.sleep_blocked().is_ok()
                    {
                        found = Some(p);
                        break;
                    }
                }
                found.ok_or_else(|| zbus::Error::Failure("daemon did not start".into()))?
            }
        };

        let last = read_status(&proxy);
        let has_tray = proxy.has_tray().unwrap_or(false);
        Ok(Self {
            proxy,
            has_tray,
            last,
        })
    }

    fn spawn_daemon() {
        // Prefer a daemon next to this binary so a locally built GUI does not
        // silently drive an older installed daemon.
        let sibling = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("sleep-blockd")))
            .filter(|p| p.exists());

        let exe = sibling.unwrap_or_else(|| "sleep-blockd".into());
        if let Err(e) = std::process::Command::new(&exe).spawn() {
            eprintln!("could not start the daemon: {e}");
        }
    }

    /// Re-reads the daemon's state. Called once per frame; the calls are local
    /// IPC and complete in microseconds.
    pub fn snapshot(&mut self) -> Status {
        self.last = read_status(&self.proxy);
        self.last
    }

    /// Whether the daemon has a tray icon.
    pub fn has_tray(&self) -> bool {
        self.has_tray
    }

    /// The last state read, without a bus round trip.
    pub fn cached(&self) -> Status {
        self.last
    }

    pub fn toggle(&mut self) -> Result<Status, zbus::Error> {
        self.proxy.toggle()?;
        Ok(self.snapshot())
    }

    pub fn set_keep_screen_awake(&mut self, wanted: bool) -> Result<Status, zbus::Error> {
        self.proxy.set_keep_screen_awake(wanted)?;
        Ok(self.snapshot())
    }

    pub fn set_keep_running_in_tray(&mut self, wanted: bool) -> Result<Status, zbus::Error> {
        self.proxy.set_keep_running_in_tray(wanted)?;
        Ok(self.snapshot())
    }

    /// Tells the daemon to release everything and exit. Used by the GUI's Quit
    /// button, which must stop the whole application rather than just close a
    /// window.
    pub fn quit_daemon(&self) {
        let _ = self.proxy.quit();
    }
}

fn read_status(proxy: &SleepBlockServiceProxyBlocking<'static>) -> Status {
    Status {
        sleep_blocked: proxy.sleep_blocked().unwrap_or(false),
        screen_blocked: proxy.screen_blocked().unwrap_or(false),
        keep_screen_awake: proxy.keep_screen_awake().unwrap_or(false),
        keep_running_in_tray: proxy.keep_running_in_tray().unwrap_or(false),
    }
}
