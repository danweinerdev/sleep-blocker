//! The sleep-block daemon: owns the inhibitors and the tray icon.
//!
//! Splitting the daemon from the GUI exists for one reason: on Wayland a
//! window cannot be hidden. `set_visible` is an empty function there,
//! un-minimising is explicitly ignored, and `focus_window` does nothing — so
//! there is no way for a tray icon to bring a hidden window back within a
//! single process. Making the GUI a separate, disposable process turns
//! "show the window" into "launch the GUI", which needs no compositor
//! cooperation at all.
//!
//! The daemon therefore owns every lock. The GUI holds none, so closing its
//! window cannot release anything — which is exactly the property hide-to-tray
//! requires.

use sleep_block_core::{
    SleepBlock,
    ipc::{BUS_NAME, OBJECT_PATH},
};

use event_listener::Listener as _;
use sleep_block_app::{GuiPresence, tray::SleepTray};

/// Serves the D-Bus interface over the shared state.
struct Service {
    state: SleepBlock,
    /// Whether a tray icon was obtained. The GUI needs this: without a tray,
    /// keeping the daemon alive on close would leave no way to get the window back.
    has_tray: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set when the daemon should exit; the main thread waits on this rather
    /// than calling `exit` from inside a D-Bus handler, so locks unwind
    /// normally.
    done: std::sync::Arc<event_listener::Event>,
}

#[zbus::interface(name = "net.phantomnet.SleepBlock1")]
impl Service {
    /// Toggling changes several properties at once, so each is announced.
    ///
    /// zbus proxies cache properties and refresh only on PropertiesChanged; a
    /// server that never emits it leaves every client stuck on the value it
    /// read first. That is not a nicety — without these emissions the GUI's
    /// indicator never changes.
    async fn toggle(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<(bool, bool)> {
        let result = self.state.toggle();
        // Announce before returning either way: a failure can still have moved
        // state (a screen-lock failure leaves sleep blocking held), and the
        // window must see that even though the call reports an error.
        self.announce(&emitter).await;
        match result {
            Ok(s) => Ok((s.sleep_blocked, s.screen_blocked)),
            // Propagated rather than logged: the window is a separate process
            // now, so the daemon's stderr is not somewhere the user will look.
            // Swallowing it here is what left the toggle silently doing nothing.
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    async fn set_keep_screen_awake(
        &self,
        wanted: bool,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<(bool, bool)> {
        let result = self.state.set_keep_screen_awake(wanted);
        self.announce(&emitter).await;
        match result {
            Ok(s) => Ok((s.sleep_blocked, s.screen_blocked)),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    fn quit(&self) {
        // Release before signalling, so the locks are gone by the time anything
        // observes the daemon exiting.
        self.state.release_all();
        self.done.notify(usize::MAX);
    }

    #[zbus(property)]
    fn sleep_blocked(&self) -> bool {
        self.state.snapshot().sleep_blocked
    }

    #[zbus(property)]
    fn screen_blocked(&self) -> bool {
        self.state.snapshot().screen_blocked
    }

    #[zbus(property)]
    fn keep_screen_awake(&self) -> bool {
        self.state.snapshot().keep_screen_awake
    }

    #[zbus(property)]
    fn has_tray(&self) -> bool {
        self.has_tray.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[zbus(property)]
    fn keep_running_in_tray(&self) -> bool {
        self.state.snapshot().keep_running_in_tray
    }

    #[zbus(property)]
    fn set_keep_running_in_tray(&self, wanted: bool) {
        // A property setter's own change signal is emitted by zbus.
        self.state.set_keep_running_in_tray(wanted);
    }
}

impl Service {
    /// Emits PropertiesChanged for everything a mutation can move. Emitting a
    /// few unchanged values is cheaper than reasoning about which ones moved.
    ///
    /// A failed emission is logged but never propagated: the state change it
    /// announces has already happened, and clients converge through their poll
    /// regardless. Without the log line, that same poll would hide the failure
    /// completely — a stuck-looking indicator with nothing to grep for.
    async fn announce(&self, emitter: &zbus::object_server::SignalEmitter<'_>) {
        for (name, result) in [
            ("SleepBlocked", self.sleep_blocked_changed(emitter).await),
            ("ScreenBlocked", self.screen_blocked_changed(emitter).await),
            (
                "KeepScreenAwake",
                self.keep_screen_awake_changed(emitter).await,
            ),
            (
                "KeepRunningInTray",
                self.keep_running_in_tray_changed(emitter).await,
            ),
        ] {
            if let Err(e) = result {
                eprintln!("failed to announce {name} changed: {e}");
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The bus name is overridable so a test can run its own daemon without
    // fighting the user's real one for the well-known name. Unset in normal
    // use, which is the only case that matters for correctness.
    let bus_name = std::env::var("SLEEP_BLOCK_BUS_NAME").unwrap_or_else(|_| BUS_NAME.to_string());

    let state = SleepBlock::new();

    // One event, shared by the D-Bus quit method and the tray's Quit item, so
    // both end the daemon the same way.
    let done_event = std::sync::Arc::new(event_listener::Event::new());
    let has_tray = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let service = Service {
        state: state.clone(),
        has_tray: has_tray.clone(),
        done: done_event.clone(),
    };
    let done = service.done.listen();

    // The tray starts *before* the bus name is claimed, so `has_tray` is
    // settled before any client can observe it. The other order looks natural
    // but races: ksni's spawn() is a multi-round-trip registration with the
    // StatusNotifierWatcher, and a client polling every 50ms can see the
    // interface exported and read `has_tray` as false while that is still in
    // flight — permanently greying out the window's keep-running-in-tray option.
    //
    // The cost is that a losing daemon briefly registers a tray icon. That is
    // the better trade: it disappears when this process exits moments later,
    // whereas the race leaves a window wrong for its entire lifetime.
    // Watching the GUI's name here keeps the tray callback free of I/O.
    let gui = GuiPresence::new();
    gui.watch(
        std::env::var("SLEEP_BLOCK_GUI_BUS_NAME")
            .unwrap_or_else(|_| sleep_block_core::ipc::GUI_BUS_NAME.to_string()),
    );
    let tray = SleepTray::start(state.clone(), gui, done_event.clone());
    has_tray.store(tray.is_some(), std::sync::atomic::Ordering::Relaxed);
    let _tray = tray;

    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at(OBJECT_PATH, service)?
        .build()?;

    // Requesting the name explicitly rather than via the builder: the builder
    // reports success even when another process already owns it, so it cannot
    // serve as the single-instance check. `PrimaryOwner` is the only reply that
    // means this process actually got it.
    use zbus::fdo::{RequestNameFlags, RequestNameReply};
    // A taken name comes back as an Err(NameTaken) rather than a non-primary
    // reply, so both shapes are treated as "someone else is running".
    match connection.request_name_with_flags(bus_name.as_str(), RequestNameFlags::DoNotQueue.into())
    {
        Ok(RequestNameReply::PrimaryOwner) => {}
        Ok(_) | Err(zbus::Error::NameTaken) => {
            eprintln!("sleep-blockd is already running ({bus_name} is taken); exiting");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }

    // Keep the connection alive for the daemon's lifetime; dropping it would
    // release the bus name and unexport the interface.
    let _connection = connection;

    done.wait();
    Ok(())
}
