//! End-to-end tests against a real daemon over D-Bus.
//!
//! These exist because the unit tests could not have caught the bug that
//! shipped: they drive `SleepBlock` directly, so they never go through a proxy
//! and never touch zbus's property cache. The GUI does both, and a daemon that
//! forgot to emit `PropertiesChanged` left it rendering stale state forever
//! while every test still passed.
//!
//! Each test starts its own daemon on a unique bus name, so a developer's real
//! daemon is untouched and tests do not interfere with one another.
//!
//! They need a session bus and a working logind. Where either is missing the
//! test reports SKIP and passes — and a skip proves nothing.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use sleep_block_core::ipc::{OBJECT_PATH, SleepBlockServiceProxyBlocking};

/// A daemon running on its own bus name, killed when the test ends.
struct TestDaemon {
    child: Child,
    name: String,
}

impl TestDaemon {
    /// Starts a daemon and waits for it to claim its name.
    ///
    /// Returns `None` when the environment cannot support one, which the
    /// caller turns into a skip rather than a failure.
    fn start(tag: &str) -> Option<Self> {
        // A unique name per test: the well-known name is a single-instance
        // lock, so sharing one would serialise or break these tests.
        let name = format!(
            "net.phantomnet.SleepBlock1.test{}{}",
            std::process::id(),
            tag
        );

        let child = Command::new(env!("CARGO_BIN_EXE_sleep-blockd"))
            .env("SLEEP_BLOCK_BUS_NAME", &name)
            .spawn()
            .ok()?;

        let daemon = Self { child, name };

        // Poll until the daemon answers rather than sleeping a fixed time,
        // which would be both slower and flakier.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if daemon.try_proxy().is_some() {
                return Some(daemon);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn try_proxy(&self) -> Option<SleepBlockServiceProxyBlocking<'static>> {
        let connection = zbus::blocking::Connection::session().ok()?;
        let proxy = SleepBlockServiceProxyBlocking::builder(&connection)
            .destination(self.name.clone())
            .ok()?
            .path(OBJECT_PATH)
            .ok()?
            .build()
            .ok()?;
        // A successful property read is the readiness check: the name can be
        // owned a moment before the object is exported.
        proxy.sleep_blocked().ok()?;
        Some(proxy)
    }

    /// Starts a GUI pointed at *this* daemon.
    ///
    /// Without the override the GUI would connect to the real bus name, spawn
    /// its own daemon, and be entirely unaffected by anything this test does —
    /// which is exactly how the first version of these tests fooled itself.
    fn spawn_gui(&self) -> std::io::Result<Child> {
        Command::new(env!("CARGO_BIN_EXE_sleep-block"))
            .env("SLEEP_BLOCK_BUS_NAME", &self.name)
            .env("SLEEP_BLOCK_GUI_BUS_NAME", format!("{}.Gui", self.name))
            .spawn()
    }

    /// Whether a GUI belonging to this daemon is running.
    fn gui_running(&self) -> bool {
        name_taken(&format!("{}.Gui", self.name))
    }

    /// A proxy with default caching — the same configuration the GUI uses, so
    /// these tests exercise the cache rather than bypassing it.
    fn proxy(&self) -> SleepBlockServiceProxyBlocking<'static> {
        self.try_proxy().expect("daemon was ready a moment ago")
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn logind_available() -> bool {
    Command::new("systemd-inhibit")
        .arg("--list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Counts locks this daemon holds, by owner, so unrelated inhibitors on a busy
/// desktop do not skew the assertion.
fn our_locks() -> usize {
    Command::new("systemd-inhibit")
        .arg("--list")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| l.starts_with("sleep-block"))
                .count()
        })
        .unwrap_or(0)
}

macro_rules! daemon_or_skip {
    ($tag:expr) => {
        match TestDaemon::start($tag) {
            Some(d) => d,
            None => {
                eprintln!("SKIP: could not start a daemon (no session bus?)");
                return;
            }
        }
    };
}

#[test]
fn daemon_exports_its_interface() {
    let daemon = daemon_or_skip!("export");
    let proxy = daemon.proxy();

    // Every property the GUI renders from must be readable.
    assert!(proxy.sleep_blocked().is_ok());
    assert!(proxy.screen_blocked().is_ok());
    assert!(proxy.keep_screen_awake().is_ok());
    assert!(proxy.keep_running_in_tray().is_ok());
    assert!(proxy.has_tray().is_ok());
}

#[test]
fn daemon_starts_with_nothing_blocked() {
    let daemon = daemon_or_skip!("initial");
    let proxy = daemon.proxy();

    assert!(!proxy.sleep_blocked().unwrap());
    assert!(!proxy.screen_blocked().unwrap());
    assert!(!proxy.keep_screen_awake().unwrap());
}

/// The regression test for the bug that shipped.
///
/// zbus caches properties and refreshes only on `PropertiesChanged`. A daemon
/// that never emits it leaves this proxy returning its first reading forever —
/// which is exactly what the GUI showed. Reading through *one* long-lived proxy
/// across a toggle is what makes the cache visible; a fresh proxy per read
/// would pass even with the bug present.
#[test]
fn property_reads_track_state_through_a_cached_proxy() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let daemon = daemon_or_skip!("cache");
    let proxy = daemon.proxy();

    assert!(!proxy.sleep_blocked().unwrap(), "should start unblocked");

    proxy.toggle().expect("toggle should succeed");
    // The cache is refreshed by a signal, which arrives asynchronously.
    let blocked = wait_for(|| proxy.sleep_blocked().unwrap_or(false));
    assert!(
        blocked,
        "the same proxy must observe the change; a stale cache here is the \
         exact bug that left the GUI indicator frozen"
    );

    proxy.toggle().expect("toggle back");
    let unblocked = wait_for(|| !proxy.sleep_blocked().unwrap_or(true));
    assert!(unblocked, "toggling back must also be observed");
}

/// Polls a condition briefly. Property updates arrive via a signal, so a bare
/// read immediately after a method call can legitimately race it.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

#[test]
fn toggle_acquires_a_real_logind_lock() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let daemon = daemon_or_skip!("lock");
    let proxy = daemon.proxy();
    let before = our_locks();

    proxy.toggle().expect("toggle");
    assert_eq!(
        our_locks(),
        before + 1,
        "the daemon must take an actual inhibitor, not just flip a flag"
    );

    proxy.toggle().expect("toggle back");
    assert_eq!(our_locks(), before, "and release it");
}

#[test]
fn toggle_returns_the_resulting_state() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let daemon = daemon_or_skip!("ret");
    let proxy = daemon.proxy();

    // The return value spares the GUI a round trip, so it must be correct
    // rather than merely present.
    let (sleep_blocked, _screen) = proxy.toggle().expect("toggle");
    assert!(sleep_blocked, "toggling on should report blocked");

    let (sleep_blocked, _screen) = proxy.toggle().expect("toggle");
    assert!(!sleep_blocked, "toggling off should report unblocked");
}

#[test]
fn screen_lock_preference_is_remembered_without_being_held() {
    let daemon = daemon_or_skip!("pref");
    let proxy = daemon.proxy();

    proxy
        .set_keep_screen_awake(true)
        .expect("recording a preference should not fail");

    assert!(
        wait_for(|| proxy.keep_screen_awake().unwrap_or(false)),
        "the preference must be remembered"
    );
    assert!(
        !proxy.screen_blocked().unwrap_or(true),
        "but no screen lock is held while sleep blocking is off"
    );
}

#[test]
fn keep_running_in_tray_round_trips() {
    let daemon = daemon_or_skip!("tray");
    let proxy = daemon.proxy();

    assert!(!proxy.keep_running_in_tray().unwrap_or(true));

    proxy.set_keep_running_in_tray(true).expect("set");
    assert!(
        wait_for(|| proxy.keep_running_in_tray().unwrap_or(false)),
        "a property setter must also be observable through the cache"
    );
}

#[test]
fn a_second_daemon_refuses_to_take_the_name() {
    let daemon = daemon_or_skip!("single");

    // Single-instance is enforced by bus-name ownership: a second daemon on the
    // same name must not end up co-owning the tray or the locks.
    let second = Command::new(env!("CARGO_BIN_EXE_sleep-blockd"))
        .env("SLEEP_BLOCK_BUS_NAME", &daemon.name)
        .output()
        .expect("second daemon should at least run");

    assert!(
        second.status.success(),
        "the loser should exit cleanly rather than crash"
    );
    let said = String::from_utf8_lossy(&second.stderr);
    assert!(
        said.contains("already running") || said.contains("unavailable"),
        "it should explain why it exited, got: {said}"
    );
}

#[test]
fn quit_releases_locks_and_stops_the_daemon() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let mut daemon = daemon_or_skip!("quit");
    let proxy = daemon.proxy();
    let before = our_locks();

    proxy.toggle().expect("acquire a lock");
    assert_eq!(our_locks(), before + 1);

    let _ = proxy.quit();

    // The lock must go promptly, not at some later teardown.
    assert!(
        wait_for(|| our_locks() == before),
        "quitting must release the inhibitor"
    );
    assert!(
        wait_for(|| daemon.child.try_wait().ok().flatten().is_some()),
        "and the process must actually exit"
    );
}

/// The GUI owns a well-known name for its lifetime, which is what lets the
/// daemon tell whether a window is already open. Without it, every "Show
/// window" click would start another process: on Wayland a running GUI cannot
/// be raised, so there is nothing to bring forward.
#[test]
fn a_running_gui_owns_its_bus_name() {
    let daemon = daemon_or_skip!("guiname");
    let _ = daemon.proxy();

    assert!(
        !daemon.gui_running(),
        "no GUI should be running before this test starts one"
    );

    let mut gui = match daemon.spawn_gui() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: could not launch the GUI");
            return;
        }
    };

    // The GUI needs a display; without one it exits and the name never
    // appears, which is a skip rather than a failure.
    if !wait_for(|| daemon.gui_running()) {
        let _ = gui.kill();
        let _ = gui.wait();
        eprintln!("SKIP: the GUI did not start (no display?)");
        return;
    }

    // Killing it must release the name, or the daemon would believe a window
    // is open forever and refuse to ever show one again.
    let _ = gui.kill();
    let _ = gui.wait();
    assert!(
        wait_for(|| !daemon.gui_running()),
        "the name must be released when the GUI exits"
    );
}

/// A second GUI must not open a second window.
#[test]
fn a_second_gui_exits_instead_of_opening_a_window() {
    let daemon = daemon_or_skip!("guidup");
    let _ = daemon.proxy();

    let mut first = match daemon.spawn_gui() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: could not launch the GUI");
            return;
        }
    };
    if !wait_for(|| daemon.gui_running()) {
        let _ = first.kill();
        let _ = first.wait();
        eprintln!("SKIP: the GUI did not start (no display?)");
        return;
    }

    // Spawned rather than run with `output()`: the *first* GUI keeps its
    // window open indefinitely, and a blocking wait on the second would hang
    // the suite if this check ever regressed.
    let mut second = daemon.spawn_gui().expect("second GUI should run");

    let exited = wait_for(|| second.try_wait().ok().flatten().is_some());
    if !exited {
        let _ = second.kill();
        let _ = second.wait();
        let _ = first.kill();
        let _ = first.wait();
        panic!("a duplicate GUI must exit rather than open a second window");
    }
    assert!(
        second.wait().map(|s| s.success()).unwrap_or(false),
        "a duplicate launch is not an error condition"
    );

    let _ = first.kill();
    let _ = first.wait();
}

/// Whether any process currently owns the given bus name.
fn name_taken(name: &str) -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return false;
    };
    let Ok(name) = name.try_into() else {
        return false;
    };
    proxy.name_has_owner(name).unwrap_or(false)
}

/// A client must see changes made by *someone else*.
///
/// This is the shape that broke twice. The GUI polls its proxy every frame, but
/// zbus caches properties and refreshes them from `PropertiesChanged`; when
/// that refresh does not land, the window shows state that changed elsewhere
/// long ago while every other surface is correct.
///
/// The change here is made through a *separate* connection, exactly as a tray
/// click does. Driving it through the same proxy would refresh the cache as a
/// side effect and hide the bug.
#[test]
fn a_polling_client_observes_changes_made_elsewhere() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let daemon = daemon_or_skip!("observe");

    // The "GUI": a long-lived proxy that only ever reads.
    let watcher = daemon.proxy();
    assert!(!watcher.sleep_blocked().unwrap(), "should start unblocked");

    // The "tray": a different connection making the change.
    let actor = daemon.proxy();
    actor.toggle().expect("toggle from another connection");

    assert!(
        wait_for(|| watcher.sleep_blocked().unwrap_or(false)),
        "a reader must observe a change made through another connection; \
         a stale cache here is what left the window disagreeing with the tray"
    );

    actor.toggle().expect("toggle back");
    assert!(
        wait_for(|| !watcher.sleep_blocked().unwrap_or(true)),
        "and must observe it being undone"
    );
}

/// The same property, via the preference rather than the lock — this is the one
/// the user reported as the tray and window disagreeing about a checkbox.
#[test]
fn a_polling_client_observes_preference_changes_made_elsewhere() {
    let daemon = daemon_or_skip!("observepref");
    let watcher = daemon.proxy();
    let actor = daemon.proxy();

    assert!(!watcher.keep_screen_awake().unwrap_or(true));

    actor
        .set_keep_screen_awake(true)
        .expect("set from another connection");
    assert!(
        wait_for(|| watcher.keep_screen_awake().unwrap_or(false)),
        "the checkbox state must propagate to a reader on another connection"
    );

    actor.set_keep_screen_awake(false).expect("unset");
    assert!(
        wait_for(|| !watcher.keep_screen_awake().unwrap_or(true)),
        "and must propagate when turned back off"
    );
}

/// Quitting the daemon must take any open GUI with it.
///
/// The GUI owns nothing: every control is a request to the daemon. Left
/// running without one it is a window whose buttons all fail silently, so it
/// follows the daemon out — whether the daemon left via the tray's Quit or
/// crashed.
#[test]
fn quitting_the_daemon_closes_an_open_gui() {
    let daemon = daemon_or_skip!("guiquit");
    let proxy = daemon.proxy();

    let mut gui = match daemon.spawn_gui() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: could not launch the GUI");
            return;
        }
    };
    if !wait_for(|| daemon.gui_running()) {
        let _ = gui.kill();
        let _ = gui.wait();
        eprintln!("SKIP: the GUI did not start (no display?)");
        return;
    }

    let _ = proxy.quit();

    let closed = wait_for(|| gui.try_wait().ok().flatten().is_some());
    if !closed {
        let _ = gui.kill();
        let _ = gui.wait();
        panic!("the GUI must not outlive the daemon it depends on");
    }
}

/// A failed acquisition must reach the caller, not just the daemon's log.
///
/// The window is a separate process now, so a daemon that prints an error to
/// its own stderr has told nobody. This asserts the *shape* — that the D-Bus
/// method can return an error at all — because provoking a genuine logind
/// refusal from a test is not reliably possible on a working desktop.
#[test]
fn acquisition_failures_can_reach_the_caller() {
    let daemon = daemon_or_skip!("errshape");
    let proxy = daemon.proxy();

    // Point a proxy at a path the daemon does not serve. The transport-level
    // failure this produces travels the same return path a domain failure now
    // does, so a method typed to swallow errors could not surface it.
    let connection = zbus::blocking::Connection::session().expect("session bus");
    let broken = SleepBlockServiceProxyBlocking::builder(&connection)
        .destination(daemon.name.clone())
        .expect("destination")
        .path("/net/phantomnet/NoSuchObject")
        .expect("path")
        .build()
        .expect("proxy");

    assert!(
        broken.toggle().is_err(),
        "a call that cannot succeed must return Err rather than a fabricated \
         success -- the window has no other way to learn it failed"
    );

    // And the real proxy still succeeds, so the error path has not broken the
    // happy path.
    assert!(proxy.sleep_blocked().is_ok());
}
