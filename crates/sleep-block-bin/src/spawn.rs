use std::process::Command;

/// Launches the GUI as a detached process.
///
/// This is what "show the window" means here. The GUI is disposable: it holds
/// no locks, so starting and stopping it is free, and a fresh process always
/// appears on screen — no un-minimise required.
pub fn spawn_gui() {
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
