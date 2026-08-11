//! Tests for the shared state that coordinates the two inhibitors.
//!
//! These cover the rules the GUI and tray both depend on — which lock is held
//! when, and which preferences survive a toggle. They need a live logind (and,
//! for the screen-lock cases, a ScreenSaver provider) because acquiring is what
//! moves the state, but they need no compositor: nothing here draws anything.
//!
//! Where a service is missing the affected test reports SKIP and passes. A skip
//! proves nothing, so a green run on a headless machine is not evidence.

use std::process::Command;

use sleep_block_core::SleepBlock;

fn logind_available() -> bool {
    Command::new("systemd-inhibit")
        .arg("--list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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
fn starts_with_nothing_held() {
    let state = SleepBlock::new();
    let status = state.snapshot();

    assert!(!status.sleep_blocked);
    assert!(!status.screen_blocked);
    assert!(!status.keep_screen_awake);
    assert!(!status.keep_running_in_tray);
}

#[test]
fn toggle_acquires_then_releases_sleep_blocking() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let state = SleepBlock::new();

    let status = state.toggle().expect("acquire should succeed");
    assert!(status.sleep_blocked);
    assert!(state.snapshot().sleep_blocked, "snapshot must agree");

    let status = state.toggle().expect("release should succeed");
    assert!(!status.sleep_blocked);
    assert!(!state.snapshot().sleep_blocked);
}

/// **FR-06**: the screen lock is an addition to sleep blocking, never held
/// alone. Enabling the preference while sleep blocking is off must record the
/// choice without acquiring anything.
#[test]
fn screen_lock_is_not_held_while_sleep_blocking_is_off() {
    if !screensaver_available() {
        eprintln!("SKIP: no ScreenSaver provider");
        return;
    }
    let state = SleepBlock::new();

    let status = state
        .set_keep_screen_awake(true)
        .expect("recording the preference should not fail");

    assert!(status.keep_screen_awake, "the preference is remembered");
    assert!(
        !status.screen_blocked,
        "but no screen lock is held without sleep blocking"
    );
}

/// The preference must survive a full off/on cycle, so re-enabling restores
/// what the user asked for rather than silently resetting it.
#[test]
fn screen_lock_preference_survives_a_toggle_cycle() {
    if !logind_available() || !screensaver_available() {
        eprintln!("SKIP: logind or ScreenSaver unavailable");
        return;
    }
    let state = SleepBlock::new();

    state.set_keep_screen_awake(true).expect("set preference");
    let status = state.toggle().expect("acquire");
    assert!(status.sleep_blocked);
    assert!(
        status.screen_blocked,
        "screen lock follows sleep blocking on"
    );

    let status = state.toggle().expect("release");
    assert!(!status.sleep_blocked);
    assert!(!status.screen_blocked, "both released together");
    assert!(
        status.keep_screen_awake,
        "the preference must outlive the locks"
    );

    let status = state.toggle().expect("re-acquire");
    assert!(
        status.screen_blocked,
        "re-enabling must restore the screen lock"
    );
}

/// Toggling the preference while sleep blocking is already on must take effect
/// immediately rather than waiting for the next cycle.
#[test]
fn screen_lock_can_be_toggled_while_sleep_blocking_is_on() {
    if !logind_available() || !screensaver_available() {
        eprintln!("SKIP: logind or ScreenSaver unavailable");
        return;
    }
    let state = SleepBlock::new();
    state.toggle().expect("acquire sleep blocking");

    let status = state.set_keep_screen_awake(true).expect("enable");
    assert!(status.screen_blocked, "acquired without another toggle");

    let status = state.set_keep_screen_awake(false).expect("disable");
    assert!(!status.screen_blocked, "released without another toggle");
    assert!(
        status.sleep_blocked,
        "sleep blocking is untouched by the screen-lock setting"
    );
}

#[test]
fn release_all_drops_both_locks() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    let state = SleepBlock::new();
    let _ = state.set_keep_screen_awake(true);
    state.toggle().expect("acquire");

    state.release_all();

    let status = state.snapshot();
    assert!(!status.sleep_blocked);
    assert!(!status.screen_blocked);
}

#[test]
fn clones_share_one_state() {
    if !logind_available() {
        eprintln!("SKIP: systemd-logind unavailable");
        return;
    }
    // This is what makes the window and tray peers rather than two copies.
    let window = SleepBlock::new();
    let tray = window.clone();

    tray.toggle().expect("acquire via one handle");

    assert!(
        window.snapshot().sleep_blocked,
        "a change through one handle must be visible through the other"
    );
}

#[test]
fn tray_preferences_are_independent_of_the_locks() {
    let state = SleepBlock::new();

    let status = state.set_keep_running_in_tray(true);
    assert!(status.keep_running_in_tray);
    assert!(
        !status.sleep_blocked,
        "a window preference must not acquire an inhibitor"
    );
}
