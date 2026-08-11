//! StatusNotifierItem tray integration.
//!
//! The tray runs on its own thread inside `ksni`, so it shares state with the
//! window through a [`SleepBlock`] handle rather than owning any locks itself.
//! Both surfaces are equal peers: a toggle from either is visible in the other.

use ksni::{
    Icon, MenuItem, Tray,
    blocking::TrayMethods,
    menu::{CheckmarkItem, StandardItem},
};
use sleep_block_core::SleepBlock;

/// Icons are compiled in so the binary stays self-contained — no icon theme
/// installation step, and no dependence on the running icon theme resolving
/// our name. 22px is the usual panel size; 32 and 48 cover HiDPI scaling.
const ACTIVE_PNGS: &[&[u8]] = &[
    include_bytes!("../../../dist/icons/sleep-block-active-22.png"),
    include_bytes!("../../../dist/icons/sleep-block-active-32.png"),
    include_bytes!("../../../dist/icons/sleep-block-active-48.png"),
];
const IDLE_PNGS: &[&[u8]] = &[
    include_bytes!("../../../dist/icons/sleep-block-idle-22.png"),
    include_bytes!("../../../dist/icons/sleep-block-idle-32.png"),
    include_bytes!("../../../dist/icons/sleep-block-idle-48.png"),
];

pub struct SleepTray {
    state: SleepBlock,
    /// Whether a GUI window is open. Read from the menu callback, which ksni
    /// runs synchronously inside its D-Bus handler — so it must not do I/O.
    gui: crate::GuiPresence,
    /// Set when a toggle from the tray fails, so the tooltip can explain why
    /// nothing happened — the tray has no other way to report an error.
    error: Option<String>,
}

impl SleepTray {
    pub fn new(state: SleepBlock) -> Self {
        Self::with_presence(state, crate::GuiPresence::new())
    }

    pub fn with_presence(state: SleepBlock, gui: crate::GuiPresence) -> Self {
        Self {
            state,
            gui,
            error: None,
        }
    }

    /// Starts the tray service in the background.
    ///
    /// Returns `None` when no StatusNotifierItem host is available, which is a
    /// normal condition on desktops without a system tray rather than an error:
    /// the window remains fully usable on its own.
    pub fn start(
        state: SleepBlock,
        gui: crate::GuiPresence,
    ) -> Option<ksni::blocking::Handle<Self>> {
        let handle = match Self::with_presence(state.clone(), gui).spawn() {
            Ok(handle) => handle,
            Err(e) => {
                eprintln!("tray unavailable, continuing without it: {e}");
                return None;
            }
        };

        // ksni only re-publishes its properties when `Handle::update` is
        // called. A toggle arriving over D-Bus (a left click on the icon) runs
        // `activate` and changes the state, but nothing tells the service to
        // re-read it — leaving the icon showing the previous state until some
        // unrelated update happened to fire.
        //
        // This watcher closes that gap: it polls the shared state and pokes the
        // handle whenever it differs from what was last published. Polling
        // rather than signalling keeps `SleepBlock` free of notification
        // machinery that only the tray would use.
        let watch_handle = handle.clone();
        std::thread::spawn(move || {
            let mut published = state.snapshot();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                let current = state.snapshot();
                if current != published {
                    published = current;
                    // `None` means the tray was shut down, so stop watching.
                    if watch_handle.update(|_| {}).is_none() {
                        break;
                    }
                }
            }
        });

        Some(handle)
    }
}

impl Tray for SleepTray {
    fn id(&self) -> String {
        "sleep-block".into()
    }

    fn title(&self) -> String {
        "Sleep Block".into()
    }

    /// Decoding on demand keeps the icon in step with the state without having
    /// to invalidate a cache when it changes.
    fn icon_pixmap(&self) -> Vec<Icon> {
        let source = if self.state.snapshot().sleep_blocked {
            ACTIVE_PNGS
        } else {
            IDLE_PNGS
        };
        source
            .iter()
            .filter_map(|bytes| decode_png(bytes))
            .collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = self.state.snapshot();
        let description = match (&self.error, status.sleep_blocked, status.screen_blocked) {
            (Some(e), _, _) => e.clone(),
            (None, true, true) => "Sleep and screen lock are blocked".into(),
            (None, true, false) => "Sleep is blocked".into(),
            (None, false, _) => "The system can sleep normally".into(),
        };
        ksni::ToolTip {
            title: "Sleep Block".into(),
            description,
            ..Default::default()
        }
    }

    /// Left click toggles directly, which is the whole point of the tray icon.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.error = self.state.toggle().err().map(|e| e.to_string());
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let status = self.state.snapshot();
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        // Only offered when closing the window hides it: without that setting
        // the window is always already on screen, so the item would do nothing.
        // It sits at the top because a hidden window is the state in which the
        // user most needs it.
        if status.keep_running_in_tray {
            items.push(
                StandardItem {
                    label: "Show window".into(),
                    activate: Box::new(|this: &mut Self| {
                        // Launching a fresh GUI *is* showing the window. A
                        // running GUI cannot un-hide itself on Wayland, so the
                        // daemon starts a new one instead.
                        //
                        // ksni runs this synchronously inside its D-Bus
                        // handler, so the duplicate check is an atomic load
                        // rather than a bus round trip; the process spawn
                        // itself does not wait for the child.
                        crate::spawn_gui(&this.gui);
                    }),
                    ..Default::default()
                }
                .into(),
            );
            items.push(MenuItem::Separator);
        }

        items.extend([
            StandardItem {
                label: if status.sleep_blocked {
                    "Allow sleeping".into()
                } else {
                    "Keep system awake".into()
                },
                activate: Box::new(|this: &mut Self| {
                    this.error = this.state.toggle().err().map(|e| e.to_string());
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Also keep screen on".into(),
                checked: status.keep_screen_awake,
                activate: Box::new(|this: &mut Self| {
                    let wanted = !this.state.snapshot().keep_screen_awake;
                    this.error = this
                        .state
                        .set_keep_screen_awake(wanted)
                        .err()
                        .map(|e| e.to_string());
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Keep running in tray when closed".into(),
                checked: status.keep_running_in_tray,
                activate: Box::new(|this: &mut Self| {
                    let wanted = !this.state.snapshot().keep_running_in_tray;
                    this.state.set_keep_running_in_tray(wanted);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    // Release before exiting so the locks disappear immediately
                    // rather than waiting on process teardown.
                    this.state.release_all();
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]);

        items
    }
}

/// Decodes a PNG into the ARGB32 layout the StatusNotifierItem spec requires.
///
/// Returns `None` on malformed input rather than panicking: a broken icon
/// should cost us the icon, not the process.
fn decode_png(bytes: &[u8]) -> Option<Icon> {
    // png 0.18 requires a seekable reader, hence the cursor over the slice.
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;

    // The source PNGs are written as 8-bit RGBA; anything else means the icon
    // generation step changed and the conversion below would misread it.
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }

    // RGBA -> ARGB, and straight alpha -> premultiplied, which is what the
    // spec's pixmap format expects.
    let mut argb = Vec::with_capacity(buf.len());
    for px in buf.chunks_exact(4) {
        let (r, g, b, a) = (px[0] as u32, px[1] as u32, px[2] as u32, px[3] as u32);
        let pm = |c: u32| ((c * a + 127) / 255) as u8;
        argb.extend_from_slice(&[a as u8, pm(r), pm(g), pm(b)]);
    }

    Some(Icon {
        width: info.width as i32,
        height: info.height as i32,
        data: argb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the menu's labels in order. Separators become `None` so their
    /// placement can be asserted too.
    fn labels(tray: &SleepTray) -> Vec<Option<String>> {
        tray.menu()
            .into_iter()
            .map(|item| match item {
                MenuItem::Standard(i) => Some(i.label),
                MenuItem::Checkmark(i) => Some(i.label),
                MenuItem::Separator => None,
                _ => Some(String::from("<other>")),
            })
            .collect()
    }

    fn text(tray: &SleepTray) -> Vec<String> {
        labels(tray).into_iter().flatten().collect()
    }

    #[test]
    fn show_window_is_absent_until_hide_on_close_is_enabled() {
        let tray = SleepTray::new(SleepBlock::new());

        assert!(
            !text(&tray).iter().any(|l| l.contains("Show window")),
            "offering to show an already-visible window would be noise"
        );
    }

    #[test]
    fn show_window_is_the_first_item_once_enabled() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let tray = SleepTray::new(state);

        let items = text(&tray);
        assert_eq!(
            items.first().map(String::as_str),
            Some("Show window"),
            "a hidden window is what the user most needs back, so it goes first"
        );
    }

    #[test]
    fn show_window_is_followed_by_a_separator() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let tray = SleepTray::new(state);

        let items = labels(&tray);
        assert_eq!(items.first(), Some(&Some("Show window".into())));
        assert_eq!(
            items.get(1),
            Some(&None),
            "the show item is separated from the toggles"
        );
    }

    #[test]
    fn menu_always_offers_the_toggle_and_quit() {
        for keep_running in [false, true] {
            let state = SleepBlock::new();
            state.set_keep_running_in_tray(keep_running);
            let tray = SleepTray::new(state);
            let items = text(&tray);

            assert!(
                items.iter().any(|l| l.contains("keep screen on")),
                "screen-lock option missing (keep_running={keep_running})"
            );
            assert!(
                items.iter().any(|l| l == "Quit"),
                "quit missing (keep_running={keep_running}) — the app would be \
                 unquittable from the tray"
            );
        }
    }

    #[test]
    fn toggle_label_reflects_the_current_state() {
        let state = SleepBlock::new();
        let tray = SleepTray::new(state.clone());
        assert!(
            text(&tray).iter().any(|l| l == "Keep system awake"),
            "idle state should offer to start blocking"
        );

        // Simulate the lock being held without needing logind: the label is
        // driven by the snapshot, so this covers the rendering rule itself.
        if state.toggle().is_ok() && state.snapshot().sleep_blocked {
            assert!(
                text(&tray).iter().any(|l| l == "Allow sleeping"),
                "blocked state should offer to stop blocking"
            );
        } else {
            eprintln!("SKIP: could not acquire a lock to check the active label");
        }
    }

    #[test]
    fn icon_changes_with_the_sleep_state() {
        let state = SleepBlock::new();
        let tray = SleepTray::new(state.clone());

        let idle = tray.icon_pixmap();
        assert!(!idle.is_empty(), "an icon must always be published");

        if state.toggle().is_ok() && state.snapshot().sleep_blocked {
            let active = tray.icon_pixmap();
            assert_ne!(
                idle[0].data, active[0].data,
                "the icon must differ between states or it conveys nothing"
            );
        } else {
            eprintln!("SKIP: could not acquire a lock to compare icons");
        }
    }

    /// Every embedded icon must decode. A rejected icon is published as an
    /// empty pixmap, which is invisible rather than an error — the exact
    /// failure that shipped once already.
    #[test]
    fn every_embedded_icon_decodes() {
        for (name, set) in [("active", ACTIVE_PNGS), ("idle", IDLE_PNGS)] {
            for (i, bytes) in set.iter().enumerate() {
                assert!(
                    decode_png(bytes).is_some(),
                    "{name} icon {i} failed to decode; the tray would show nothing"
                );
            }
        }
    }

    #[test]
    fn decoded_icons_are_square_and_sized() {
        for icon in SleepTray::new(SleepBlock::new()).icon_pixmap() {
            assert_eq!(icon.width, icon.height, "tray icons should be square");
            assert_eq!(
                icon.data.len() as i32,
                icon.width * icon.height * 4,
                "ARGB32 payload must match the declared dimensions"
            );
        }
    }
}
