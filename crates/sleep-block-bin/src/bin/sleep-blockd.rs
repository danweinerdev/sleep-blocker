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
use sleep_block_app::tray::SleepTray;

/// Serves the D-Bus interface over the shared state.
struct Service {
    state: SleepBlock,
    /// Whether a tray icon was obtained. The GUI needs this: without a tray,
    /// hide-on-close would leave no way to get the window back.
    has_tray: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set when the daemon should exit; the main thread waits on this rather
    /// than calling `exit` from inside a D-Bus handler, so locks unwind
    /// normally.
    done: event_listener::Event,
}

#[zbus::interface(name = "net.phantomnet.SleepBlock1")]
impl Service {
    fn toggle(&self) -> (bool, bool) {
        match self.state.toggle() {
            Ok(s) => (s.sleep_blocked, s.screen_blocked),
            Err(e) => {
                eprintln!("toggle failed: {e}");
                let s = self.state.snapshot();
                (s.sleep_blocked, s.screen_blocked)
            }
        }
    }

    fn set_keep_screen_awake(&self, wanted: bool) -> (bool, bool) {
        match self.state.set_keep_screen_awake(wanted) {
            Ok(s) => (s.sleep_blocked, s.screen_blocked),
            Err(e) => {
                eprintln!("screen lock change failed: {e}");
                let s = self.state.snapshot();
                (s.sleep_blocked, s.screen_blocked)
            }
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
        self.state.set_keep_running_in_tray(wanted);
    }

    #[zbus(signal)]
    async fn show_window_requested(
        emitter: &zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::Result<()>;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = SleepBlock::new();

    let has_tray = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let service = Service {
        state: state.clone(),
        has_tray: has_tray.clone(),
        done: event_listener::Event::new(),
    };
    let done = service.done.listen();

    // Requesting the name is also the single-instance check: if another daemon
    // holds it, this build fails here rather than racing it for the tray icon.
    let connection = match zbus::blocking::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, service)?
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sleep-blockd is already running (or the bus is unavailable): {e}");
            return Ok(());
        }
    };

    // The tray lives here rather than in the GUI so it survives the window
    // closing — the entire point of the split.
    let tray = SleepTray::start(state.clone());
    has_tray.store(tray.is_some(), std::sync::atomic::Ordering::Relaxed);
    let _tray = tray;
    // Keep the connection alive for the daemon's lifetime; dropping it would
    // release the bus name and unexport the interface.
    let _connection = connection;

    done.wait();
    Ok(())
}
