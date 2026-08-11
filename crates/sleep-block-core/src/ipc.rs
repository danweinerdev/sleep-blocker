//! The D-Bus contract between the daemon and the GUI.
//!
//! The daemon owns every inhibitor; the GUI owns none. That split is forced by
//! the locks themselves — a logind inhibitor lives exactly as long as its file
//! descriptor, so a GUI holding it would release it the moment its window
//! closed, which is precisely what hide-to-tray must not do.
//!
//! So this is not shared state. The daemon is authoritative, and the GUI is a
//! thin client that renders what it is told and asks for changes.
//!
//! The session bus is used because it is already a dependency (the tray speaks
//! it) and because the daemon is inherently per-session: inhibitors belong to a
//! login session, not to the machine.

/// The well-known name the daemon owns. A second daemon that cannot acquire it
/// knows one is already running and exits, which is the whole
/// single-instance mechanism.
pub const BUS_NAME: &str = "net.phantomnet.SleepBlock1";

/// The name a running GUI owns.
///
/// Serves two purposes: the daemon checks it before launching a GUI, so
/// "Show window" cannot pile up duplicate processes, and a second GUI that
/// cannot take it knows one is already on screen and exits.
pub const GUI_BUS_NAME: &str = "net.phantomnet.SleepBlock1.Gui";

/// The object the interface is served at.
pub const OBJECT_PATH: &str = "/net/phantomnet/SleepBlock1";

/// The interface name, kept in one place so the proxy and the service cannot
/// disagree about it.
pub const INTERFACE: &str = "net.phantomnet.SleepBlock1";

/// Client-side proxy for the daemon's interface.
///
/// Blocking rather than async: the GUI calls this from its render loop, where
/// an executor would be an awkward fit, and the calls are local IPC that
/// complete in microseconds.
#[zbus::proxy(
    interface = "net.phantomnet.SleepBlock1",
    default_service = "net.phantomnet.SleepBlock1",
    default_path = "/net/phantomnet/SleepBlock1"
)]
pub trait SleepBlockService {
    /// Turns sleep blocking on if off, and off if on. Returns the resulting
    /// state so the caller does not have to round-trip a property read.
    fn toggle(&self) -> zbus::Result<(bool, bool)>;

    /// Sets whether the screen should also be kept awake.
    fn set_keep_screen_awake(&self, wanted: bool) -> zbus::Result<(bool, bool)>;

    /// Releases every lock and exits the daemon. Used by the GUI's Quit button.
    /// The tray's Quit item ends the daemon directly instead — it runs inside
    /// the daemon, so it signals the shutdown event rather than calling itself
    /// over the bus.
    fn quit(&self) -> zbus::Result<()>;

    /// True while system sleep is blocked.
    #[zbus(property)]
    fn sleep_blocked(&self) -> zbus::Result<bool>;

    /// True while screen blanking and locking is also blocked.
    #[zbus(property)]
    fn screen_blocked(&self) -> zbus::Result<bool>;

    /// The user's screen-lock preference, which persists across toggles.
    #[zbus(property)]
    fn keep_screen_awake(&self) -> zbus::Result<bool>;

    /// Whether the daemon actually has a tray icon. Without one there would be
    /// no way to bring the window back, so the GUI greys out the
    /// keep-running-in-tray option rather than offering a trap.
    #[zbus(property)]
    fn has_tray(&self) -> zbus::Result<bool>;

    /// Whether closing the GUI window should leave the daemon running.
    #[zbus(property)]
    fn keep_running_in_tray(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_keep_running_in_tray(&self, wanted: bool) -> zbus::Result<()>;
}
