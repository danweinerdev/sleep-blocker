use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Tracks whether a GUI window is currently open.
///
/// This exists so the tray's "Show window" callback can answer the question
/// with a single atomic load. ksni invokes menu callbacks *synchronously inside
/// its D-Bus method handler*, holding `&mut self` — so anything slow in a
/// callback stalls the tray's own dispatch, and the tray stops responding to
/// clicks. Opening a fresh bus connection there (a full auth handshake plus a
/// `NameHasOwner` round trip) is exactly that kind of work.
///
/// A background watcher keeps this current instead, driven by
/// `NameOwnerChanged` rather than polling.
#[derive(Clone, Default)]
pub struct GuiPresence {
    running: Arc<AtomicBool>,
}

impl GuiPresence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cheap enough to call from a D-Bus callback: no I/O, no allocation.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn set(&self, running: bool) {
        self.running.store(running, Ordering::Relaxed);
    }

    /// Starts watching the GUI's well-known name on a background thread.
    ///
    /// Failure to start the watch is not fatal: `is_running` then stays false,
    /// so the worst case is launching a second GUI that immediately exits when
    /// it cannot claim the name. That is the same guard the GUI already has, so
    /// the invariant holds either way — this watcher is an optimisation of
    /// *where* the check happens, not the only thing enforcing it.
    pub fn watch(&self, gui_bus_name: String) {
        let presence = self.clone();
        std::thread::spawn(move || {
            let Ok(connection) = zbus::blocking::Connection::session() else {
                eprintln!("cannot watch for the GUI: no session bus");
                return;
            };
            let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
                return;
            };

            // Seed from the current state: the GUI may already be running, and
            // signals only report changes from here on.
            if let Ok(name) = gui_bus_name.as_str().try_into() {
                presence.set(dbus.name_has_owner(name).unwrap_or(false));
            }

            let Ok(mut changes) = dbus.receive_name_owner_changed() else {
                return;
            };
            for change in &mut changes {
                let Ok(args) = change.args() else { continue };
                if args.name().as_str() != gui_bus_name {
                    continue;
                }
                // An empty new owner means the name was released — the GUI
                // exited, crashed, or was killed. All three mean the same thing
                // here.
                presence.set(args.new_owner().is_some());
            }
        });
    }
}

/// Launches the GUI as a detached process.
///
/// This is what "show the window" means here. The GUI is disposable: it holds
/// no locks, so starting and stopping it is free, and a fresh process always
/// appears on screen — no un-minimise required.
pub fn spawn_gui(presence: &GuiPresence) {
    // Without this, every "Show window" click starts another process: a
    // running GUI cannot be raised on Wayland, so there is nothing to bring
    // forward and the spawn would simply pile up duplicates.
    if presence.is_running() {
        return;
    }

    // `current_exe` resolves /proc/self/exe on Linux, so this finds the GUI
    // built alongside *this* daemon rather than whichever one happens to be on
    // PATH — which matters when a locally built daemon runs against an older
    // installed GUI.
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("sleep-block")))
        .filter(|p| p.exists());

    // The bare-name fallback only fires when the sibling is missing, which in a
    // packaged install means the package is already broken. It is kept so a
    // daemon started from an unusual location can still find a GUI on PATH.
    let exe = sibling.unwrap_or_else(|| "sleep-block".into());
    if let Err(e) = Command::new(&exe).spawn() {
        eprintln!("could not launch {}: {e}", exe.to_string_lossy());
        return;
    }

    // Assume it is coming up. The watcher corrects this either way, but setting
    // it here closes the window between spawning and the name appearing, during
    // which a second click would otherwise launch a duplicate.
    presence.set(true);
}
