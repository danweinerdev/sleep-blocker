//! The GUI's view of the daemon.
//!
//! Deliberately shaped like [`sleep_block_core::SleepBlock`] so the rendering
//! code did not have to change: same `snapshot`/`toggle`/`set_*` surface,
//! backed by D-Bus calls instead of local locks.
//!
//! The GUI holds no inhibitors. That is the whole reason the daemon exists —
//! a lock owned here would be released the instant this window closed.

use sleep_block_core::Status;
use sleep_block_core::ipc::{GUI_BUS_NAME, SleepBlockServiceProxyBlocking};

/// Why the GUI could not start.
#[derive(Debug)]
pub enum ConnectError {
    /// Another GUI already has a window open.
    AlreadyRunning,
    /// No daemon could be reached or started.
    DaemonUnavailable,
    /// The session bus itself is unusable.
    Bus(zbus::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "a sleep-block window is already open"),
            Self::DaemonUnavailable => write!(f, "the sleep-block daemon did not start"),
            Self::Bus(e) => write!(f, "session bus unavailable: {e}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Connection to the daemon, plus the last state it reported.
pub struct DaemonClient {
    proxy: SleepBlockServiceProxyBlocking<'static>,
    /// Held for the GUI's lifetime. Dropping it releases the GUI's bus name,
    /// which is how the daemon learns the window is gone.
    _connection: zbus::blocking::Connection,
    /// Whether the daemon has a tray, read once at connect.
    ///
    /// Safe to read once only because the daemon starts its tray *before*
    /// claiming the bus name, so this is already settled by the time this
    /// client can talk to it at all.
    has_tray: bool,
    /// Cached so a failed call can keep rendering the last known-good values
    /// rather than flickering to defaults.
    last: Status,
    /// Set once the daemon stops answering.
    daemon_gone: bool,
}

impl DaemonClient {
    /// Connects to a running daemon, starting one if necessary, and claims the
    /// GUI's well-known name so only one window runs.
    ///
    /// Launching the daemon here means the window can be started directly, from
    /// a launcher or the command line, without the user knowing a daemon
    /// exists.
    ///
    /// Returns `Err(AlreadyRunning)` when another window holds the name — the
    /// caller should exit quietly rather than opening a second one.
    pub fn connect() -> Result<Self, ConnectError> {
        // Bounded so a hung daemon cannot freeze the window. These calls run on
        // the render thread once per frame; without a limit a daemon that is
        // alive but blocked — stuck in its own logind round trip while holding
        // the state mutex, say — would hang the UI indefinitely, close button
        // included, and never reach the daemon-gone handling. Two seconds is far
        // longer than local IPC needs and short enough to stay a blip.
        let connection = zbus::blocking::connection::Builder::session()
            .map_err(ConnectError::Bus)?
            .method_timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(ConnectError::Bus)?;

        // Claiming this is what stops "Show window" stacking up windows: the
        // daemon checks the name first, and a GUI that loses the race exits.
        use zbus::fdo::{RequestNameFlags, RequestNameReply};
        let gui_name =
            std::env::var("SLEEP_BLOCK_GUI_BUS_NAME").unwrap_or_else(|_| GUI_BUS_NAME.to_string());
        match connection
            .request_name_with_flags(gui_name.as_str(), RequestNameFlags::DoNotQueue.into())
        {
            Ok(RequestNameReply::PrimaryOwner) => {}
            Ok(_) | Err(zbus::Error::NameTaken) => return Err(ConnectError::AlreadyRunning),
            Err(e) => return Err(ConnectError::Bus(e)),
        }

        let proxy = match uncached_proxy(&connection) {
            Ok(p) if p.sleep_blocked().is_ok() => p,
            _ => {
                // No daemon yet. Start one and wait briefly for it to claim the
                // bus name; the retry loop is short because this is a local
                // process start, not a network round trip.
                Self::spawn_daemon();
                let mut found = None;
                for _ in 0..40 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if let Ok(p) = uncached_proxy(&connection)
                        && p.sleep_blocked().is_ok()
                    {
                        found = Some(p);
                        break;
                    }
                }
                found.ok_or(ConnectError::DaemonUnavailable)?
            }
        };

        let last = read_status(&proxy).ok_or(ConnectError::DaemonUnavailable)?;
        let has_tray = proxy.has_tray().unwrap_or(false);
        Ok(Self {
            proxy,
            _connection: connection,
            has_tray,
            last,
            daemon_gone: false,
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
    ///
    /// Keeps the last known values if the daemon has gone, so the window does
    /// not flash to a wrong state in the moment before it closes.
    pub fn snapshot(&mut self) -> Status {
        match read_status(&self.proxy) {
            Some(status) => {
                self.last = status;
                self.daemon_gone = false;
            }
            None => self.daemon_gone = true,
        }
        self.last
    }

    /// Whether the daemon has stopped answering.
    ///
    /// The GUI is useless without it — every control is a request to a process
    /// that is no longer there — so the window closes rather than sitting
    /// inert. This also covers a daemon crash, not just an orderly Quit.
    pub fn daemon_gone(&self) -> bool {
        self.daemon_gone
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

/// A proxy that always asks the daemon rather than serving cached values.
///
/// zbus caches properties and refreshes them from `PropertiesChanged`. The GUI
/// re-reads every frame regardless, so the cache saves nothing — and when a
/// refresh is missed it leaves the window showing state that changed elsewhere
/// minutes ago, which is worse than the round trip it avoids.
fn uncached_proxy(
    connection: &zbus::blocking::Connection,
) -> zbus::Result<SleepBlockServiceProxyBlocking<'static>> {
    // Honours the same override the daemon uses, so a test can point both
    // halves at a private bus name instead of the user's real daemon.
    let name = std::env::var("SLEEP_BLOCK_BUS_NAME")
        .unwrap_or_else(|_| sleep_block_core::ipc::BUS_NAME.to_string());
    SleepBlockServiceProxyBlocking::builder(connection)
        .destination(name)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
}

/// Reads every property, or `None` if the daemon has gone.
///
/// The distinction matters: `unwrap_or(false)` would render a dead daemon as
/// "nothing is blocked", which looks like a normal idle state rather than the
/// application having lost its other half.
fn read_status(proxy: &SleepBlockServiceProxyBlocking<'static>) -> Option<Status> {
    Some(Status {
        sleep_blocked: proxy.sleep_blocked().ok()?,
        screen_blocked: proxy.screen_blocked().ok()?,
        keep_screen_awake: proxy.keep_screen_awake().ok()?,
        keep_running_in_tray: proxy.keep_running_in_tray().ok()?,
    })
}
