//! Integration tests against the real D-Bus services.
//!
//! These exercise live systemd-logind and ScreenSaver implementations rather
//! than mocks, because the behaviour worth testing *is* the interaction with
//! them: that a lock actually appears while held and is gone once dropped.
//!
//! They are therefore environment-dependent. On a machine with no session bus
//! or no screen locker (CI containers, headless builders) the relevant service
//! is missing, and these tests report as skipped rather than failing — but note
//! a skip proves nothing, so a green run on such a machine is not evidence the
//! inhibitors work. Run them on a real desktop session to get that evidence.

use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use sleep_block_core::{Inhibitor, ScreenInhibitor};

/// Serialises the tests that count locks.
///
/// The counts are global to the session: a test that asserts "one more lock
/// than before" sees locks taken by any test running concurrently, so the
/// default multi-threaded harness makes them fail intermittently. Holding this
/// for the duration of each counting test removes the interference without
/// requiring callers to remember `--test-threads=1`.
fn lock_counting() -> MutexGuard<'static, ()> {
    static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
    SERIAL
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Reads the `WHO` column of `systemd-inhibit --list`, which is the same view
/// an administrator gets. Returns `None` when logind is unreachable.
fn logind_locks() -> Option<String> {
    let output = Command::new("systemd-inhibit")
        .arg("--list")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Counts locks belonging to this application specifically, so a busy desktop
/// with unrelated inhibitors (NetworkManager, the compositor) does not skew the
/// assertions.
fn our_lock_count() -> Option<usize> {
    Some(
        logind_locks()?
            .lines()
            .filter(|line| line.starts_with("sleep-block"))
            .count(),
    )
}

#[test]
fn sleep_inhibitor_appears_in_logind_and_is_released_on_drop() {
    // Held for the whole test: the assertions below compare global lock counts.
    let _serial = lock_counting();
    let Some(before) = our_lock_count() else {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    };

    let lock = match Inhibitor::acquire("integration test") {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("SKIP: could not acquire sleep inhibitor: {e}");
            return;
        }
    };

    let during = our_lock_count().expect("logind was reachable a moment ago");
    assert_eq!(
        during,
        before + 1,
        "acquiring an inhibitor should add exactly one lock owned by sleep-block"
    );

    // The lock is released by closing the descriptor, so this drop is the
    // behaviour under test, not mere cleanup.
    drop(lock);

    let after = our_lock_count().expect("logind was reachable a moment ago");
    assert_eq!(
        after, before,
        "dropping the inhibitor should release the lock it took"
    );
}

#[test]
fn sleep_inhibitor_registers_the_expected_lock_types() {
    // Held for the whole test: the assertions below compare global lock counts.
    let _serial = lock_counting();
    let Some(_) = logind_locks() else {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    };

    let Ok(_lock) = Inhibitor::acquire("integration test") else {
        eprintln!("SKIP: could not acquire sleep inhibitor");
        return;
    };

    let listing = logind_locks().expect("logind was reachable a moment ago");
    let ours = listing
        .lines()
        .find(|line| line.starts_with("sleep-block"))
        .expect("our lock should be listed while held");

    // `block` rather than `delay` is what actually prevents suspend; a `delay`
    // lock would only postpone it briefly, silently failing at the app's job.
    assert!(
        ours.contains("block"),
        "lock must be block mode, got: {ours}"
    );
    assert!(ours.contains("sleep"), "lock must cover sleep, got: {ours}");
    assert!(ours.contains("idle"), "lock must cover idle, got: {ours}");
}

#[test]
fn sleep_inhibitors_are_independent() {
    // Held for the whole test: the assertions below compare global lock counts.
    let _serial = lock_counting();
    let Some(before) = our_lock_count() else {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    };

    let Ok(first) = Inhibitor::acquire("integration test 1") else {
        eprintln!("SKIP: could not acquire sleep inhibitor");
        return;
    };
    let Ok(second) = Inhibitor::acquire("integration test 2") else {
        eprintln!("SKIP: could not acquire second sleep inhibitor");
        return;
    };

    assert_eq!(our_lock_count().unwrap(), before + 2);

    // Releasing one must not disturb the other: the GUI relies on this when the
    // screen lock is toggled while sleep blocking stays on.
    drop(first);
    assert_eq!(
        our_lock_count().unwrap(),
        before + 1,
        "dropping one inhibitor must not release the other"
    );

    drop(second);
    assert_eq!(our_lock_count().unwrap(), before);
}

/// Whether a screen-locker implementing `org.freedesktop.ScreenSaver` is present.
/// On this KDE session the name is owned by kwin; GNOME and others provide it too.
fn screensaver_available() -> bool {
    Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            "s",
            "org.freedesktop.ScreenSaver",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn screen_inhibitor_acquires_and_releases() {
    if !screensaver_available() {
        eprintln!("SKIP: no org.freedesktop.ScreenSaver provider on the session bus");
        return;
    }

    let screen = match ScreenInhibitor::acquire("integration test") {
        Ok(screen) => screen,
        Err(e) => panic!("ScreenSaver is present but Inhibit failed: {e}"),
    };

    // The ScreenSaver interface exposes no way to read inhibition state back —
    // kwin implements only Inhibit/UnInhibit, and PowerDevil's HasInhibition
    // and ListInhibitions do not report kwin-held inhibitors — so there is no
    // assertion to make about the desktop's view here. What this test does pin
    // down is that the call succeeds and that release runs without error or
    // panic, which is where the cookie-based API is easy to get wrong.
    //
    // That the desktop actually registers the inhibitor has been confirmed
    // manually: it appears in KDE's Power & Battery tray popup while held.
    drop(screen);
}

#[test]
fn screen_inhibitors_can_overlap() {
    if !screensaver_available() {
        eprintln!("SKIP: no org.freedesktop.ScreenSaver provider on the session bus");
        return;
    }

    let Ok(first) = ScreenInhibitor::acquire("integration test 1") else {
        eprintln!("SKIP: could not acquire screen inhibitor");
        return;
    };
    let Ok(second) = ScreenInhibitor::acquire("integration test 2") else {
        panic!("second concurrent screen inhibitor should be grantable");
    };

    // Each acquire must yield a distinct cookie; releasing one must not
    // invalidate the other. Dropping in reverse order exercises that.
    drop(second);
    drop(first);
}
