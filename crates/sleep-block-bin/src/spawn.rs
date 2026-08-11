use std::process::Command;

use sleep_block_core::ipc::GUI_BUS_NAME;

/// Whether a GUI window is already open.
///
/// The GUI owns a well-known name for its lifetime, so asking the bus who owns
/// it is a reliable check that needs no cooperation from the window itself —
/// which matters on Wayland, where a running GUI cannot be raised or unhidden
/// anyway.
fn gui_already_running() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return false;
    };
    proxy
        .name_has_owner(GUI_BUS_NAME.try_into().expect("valid bus name"))
        .unwrap_or(false)
}

/// Launches the GUI as a detached process.
///
/// This is what "show the window" means here. The GUI is disposable: it holds
/// no locks, so starting and stopping it is free, and a fresh process always
/// appears on screen — no un-minimise required.
pub fn spawn_gui() {
    // Without this, every "Show window" click starts another process: a
    // running GUI cannot be raised on Wayland, so there is nothing to bring
    // forward and the spawn would simply pile up duplicates.
    if gui_already_running() {
        return;
    }

    // argv[0]'s directory is checked first so a locally built daemon launches
    // its matching GUI rather than an older installed one.
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("sleep-block")));

    let candidates: Vec<std::ffi::OsString> = sibling
        .filter(|p| p.exists())
        .map(Into::into)
        .into_iter()
        .chain(std::iter::once("sleep-block".into()))
        .collect();

    for exe in candidates {
        match Command::new(&exe).spawn() {
            Ok(_) => return,
            Err(e) => eprintln!("could not launch {}: {e}", exe.to_string_lossy()),
        }
    }
}
