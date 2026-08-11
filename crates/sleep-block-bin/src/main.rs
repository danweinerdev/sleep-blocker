// Hide the console window on Windows. Harmless elsewhere; this app is
// Linux-only in practice since it talks to systemd-logind.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod window_policy;

use eframe::egui;

use sleep_block_core::SleepBlock;

use crate::tray::SleepTray;
use crate::window_policy::{Frame as PolicyFrame, WindowPolicy};

/// Fixed window size. Tall enough for the tallest state — the one where an
/// error line is showing under the checkbox — so no content is ever clipped.
const WINDOW_SIZE: [f32; 2] = [300.0, 330.0];

/// Colors chosen to stay legible against egui's dark background and to remain
/// distinguishable for the most common forms of color blindness — the shapes
/// and the label carry the state too, so color is never the only signal.
const GREEN: egui::Color32 = egui::Color32::from_rgb(46, 160, 67);
const GREEN_HOVER: egui::Color32 = egui::Color32::from_rgb(56, 190, 80);
const RED: egui::Color32 = egui::Color32::from_rgb(180, 48, 48);
const RED_HOVER: egui::Color32 = egui::Color32::from_rgb(210, 60, 60);

/// Window icon, shown by the taskbar and window switcher. 256px so it stays
/// sharp wherever the desktop scales it down.
const WINDOW_ICON: &[u8] = include_bytes!("../../../dist/icons/sleep-block-active-256.png");

/// Matches the desktop entry's basename. On Wayland this is how the compositor
/// associates the window with its .desktop file, which is where the taskbar
/// gets the icon from — without it the window shows a blank placeholder even
/// when an icon is set directly.
const APP_ID: &str = "sleep-block";

/// Decodes the embedded window icon into eframe's straight-RGBA form.
///
/// Separate from the tray's decoder, which produces premultiplied ARGB for the
/// StatusNotifierItem pixmap format. Returns `None` rather than panicking: a
/// missing window icon is a cosmetic loss, not a reason to fail startup.
fn window_icon() -> Option<egui::IconData> {
    let decoder = png::Decoder::new(std::io::Cursor::new(WINDOW_ICON));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(egui::IconData {
        rgba: buf,
        width: info.width,
        height: info.height,
    })
}

fn main() -> eframe::Result {
    // The layout is a fixed stack of controls with nothing to reflow, so the
    // window is pinned rather than resizable. Min and max are both set: some
    // compositors still allow dragging an edge when only `resizable(false)` is
    // given, and clamping both bounds removes that.
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_SIZE)
        .with_max_inner_size(WINDOW_SIZE)
        .with_resizable(false)
        .with_app_id(APP_ID)
        .with_title("Sleep Block");

    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // One shared state, two surfaces. The tray owns a clone of the handle, not
    // a copy of the locks, so a toggle from either side is seen by both.
    let state = SleepBlock::new();
    let tray = SleepTray::start(state.clone());

    eframe::run_native(
        "sleep-block",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(state, tray)))),
    )
}

struct App {
    /// Shared with the tray. The locks live here, not in the GUI, so both
    /// surfaces read and write the same state.
    state: SleepBlock,
    /// Kept alive for as long as the app runs: dropping the handle would stop
    /// the tray service. `None` when no StatusNotifierItem host was found.
    tray: Option<ksni::blocking::Handle<SleepTray>>,
    /// Set when an action fails, so the user sees why nothing happened.
    error: Option<String>,
    /// Owns the hide/show decision, kept separate so it can be unit tested.
    policy: WindowPolicy,
    /// Set by the in-window Quit button. Without it the close command that
    /// button sends would be caught by the hide-on-close handler and turned
    /// into another hide, making the button do nothing.
    quitting: bool,
}

impl App {
    fn new(state: SleepBlock, tray: Option<ksni::blocking::Handle<SleepTray>>) -> Self {
        Self {
            state,
            tray,
            error: None,
            policy: WindowPolicy::new(),
            quitting: false,
        }
    }

    fn toggle(&mut self) {
        self.error = self.state.toggle().err().map(|e| e.to_string());
        self.refresh_tray();
    }

    fn set_keep_screen_awake(&mut self, wanted: bool) {
        self.error = self
            .state
            .set_keep_screen_awake(wanted)
            .err()
            .map(|e| format!("Screen lock not blocked: {e}"));
        self.refresh_tray();
    }

    fn set_keep_running_in_tray(&mut self, wanted: bool) {
        self.state.set_keep_running_in_tray(wanted);
        self.refresh_tray();
    }

    /// Tells the tray to re-read the shared state. Without this the icon would
    /// keep its previous appearance until something else prompted a redraw,
    /// since ksni has no way to observe the state changing underneath it.
    fn refresh_tray(&self) {
        if let Some(tray) = &self.tray {
            tray.update(|_| {});
        }
    }
}

/// Builds the visible window's viewport description.
///
/// Used for the deferred child viewport that carries the actual UI. Hiding is
/// implemented by *not* showing this viewport on a frame, which destroys the
/// window — the only way to hide on Wayland, where `set_visible` is a no-op.
fn window_viewport() -> egui::ViewportBuilder {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_SIZE)
        .with_max_inner_size(WINDOW_SIZE)
        .with_resizable(false)
        .with_app_id(APP_ID)
        .with_title("Sleep Block");
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    viewport
}

impl eframe::App for App {
    /// Runs every frame, including while the window is destroyed — which is
    /// what keeps the process (and therefore the tray) alive with no window.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // A close arriving on the ROOT viewport means the whole app should go:
        // the root has no window of its own that the user could have closed,
        // so this is a genuine shutdown request.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.state.release_all();
        }

        // While the window is destroyed nothing else drives repaints, so poll:
        // this is what notices a tray "Show window" click.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    /// The root viewport draws nothing: the real window is a child viewport so
    /// that it can be destroyed and recreated, which is the only way to hide a
    /// window on Wayland (`set_visible` is a documented no-op there).
    ///
    /// Not showing the child on a frame destroys its window; showing it again
    /// recreates one. That is what GTK's and Qt's `hide()` do underneath, so
    /// this is the conventional mechanism rather than a workaround.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The tray can toggle the state from its own thread, and egui has no
        // way to learn about that. Polling once a second keeps the window from
        // showing a stale reading without burning a core on a busy redraw loop.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));

        if self.state.snapshot().window_hidden {
            return;
        }

        // `immediate` rather than `deferred`: the callback may borrow `self`,
        // which a deferred viewport's `Send + Sync + 'static` bound forbids.
        // The cost is that the root repaints with the child, which is nothing
        // for a window that redraws about once a second.
        let mut close_requested = false;
        egui::Context::show_viewport_immediate(
            ui.ctx(),
            egui::ViewportId::from_hash_of("sleep-block-window"),
            window_viewport(),
            |ui, _class| {
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    close_requested = true;
                }
                self.window_contents(ui);
            },
        );

        // Handled out here so the borrow of `self` inside the closure ends
        // first. A close with hide-on-close off falls through to the root
        // viewport's own close handling, which exits.
        if close_requested {
            self.on_window_close_requested(ui.ctx());
        }
    }
}

impl App {
    /// Decides what a close of the *window* means. Hide-on-close applies here,
    /// not to the root viewport: this is the window the user actually clicked
    /// the X on.
    fn on_window_close_requested(&mut self, ctx: &egui::Context) {
        let action = self.policy.step(
            &self.state,
            PolicyFrame {
                close_requested: true,
                quitting: self.quitting,
            },
        );

        if action.refresh_tray {
            self.refresh_tray();
        }

        // Nothing intercepted it, so the user wants out. Releasing here rather
        // than relying on process teardown makes the locks disappear promptly.
        if !action.cancel_close {
            self.state.release_all();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Draws the window's actual contents. Called from inside the child
    /// viewport, so `ui` here is the child's, not the root's.
    fn window_contents(&mut self, ui: &mut egui::Ui) {
        // The frame must be told to fill the window. Left to itself it wraps
        // only its contents, leaving the remainder of the viewport unpainted —
        // which shows up as a black bar below the last control.
        ui.set_min_size(ui.available_size());
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);

                // Read once per frame. The tray may have changed this state on
                // another thread, so the window renders from the shared value
                // rather than any local copy.
                let status = self.state.snapshot();

                ui.heading(if status.sleep_blocked {
                    "Awake"
                } else {
                    "Sleep allowed"
                });
                ui.add_space(12.0);

                if self.indicator(ui, status.sleep_blocked).clicked() {
                    self.toggle();
                }

                ui.add_space(12.0);
                ui.label(if status.sleep_blocked {
                    "Sleep is blocked.\nClick to allow sleeping again."
                } else {
                    "The system can suspend normally.\nClick to keep it awake."
                });

                ui.add_space(14.0);
                // `checkbox` needs somewhere to write, but the shared state is
                // authoritative: seed the local from it each frame and push any
                // change straight back, so the tray's view cannot be clobbered.
                let mut keep_screen_awake = status.keep_screen_awake;
                let toggled = ui
                    .checkbox(&mut keep_screen_awake, "Also keep screen on")
                    .on_hover_text(
                        "Additionally block screen blanking and locking.\n\
                         Leave off to let the monitors sleep as usual.",
                    )
                    .changed();
                if toggled {
                    // Clear a stale failure so the retry isn't shown alongside
                    // the previous attempt's error.
                    self.error = None;
                    self.set_keep_screen_awake(keep_screen_awake);
                }

                ui.add_space(4.0);
                // Without a tray there would be no way to bring the window
                // back, so the option is unavailable rather than a trap.
                let has_tray = self.tray.is_some();
                let mut keep_running = status.keep_running_in_tray;
                let tray_toggled = ui
                    .add_enabled(
                        has_tray,
                        egui::Checkbox::new(&mut keep_running, "Keep running in tray when closed"),
                    )
                    .on_hover_text(if has_tray {
                        "Closing the window hides it instead of quitting.\n\
                         Reopen it from the tray icon's menu."
                    } else {
                        "Unavailable: no system tray was found, so a hidden\n\
                         window could not be reopened."
                    })
                    .changed();
                if tray_toggled {
                    self.set_keep_running_in_tray(keep_running);
                }

                // With hide-on-close enabled, the window's close button no
                // longer quits, so the window needs its own way out. Relying on
                // the tray menu alone strands the user if the tray icon is not
                // visible for any reason.
                if status.keep_running_in_tray {
                    ui.add_space(10.0);
                    if ui
                        .button("Quit")
                        .on_hover_text("Release all locks and exit.")
                        .clicked()
                    {
                        self.quitting = true;
                        self.state.release_all();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                if let Some(error) = &self.error {
                    ui.add_space(8.0);
                    ui.colored_label(RED_HOVER, error);
                }
            });
        });
    }
    /// Draws the circular status light and returns its click response. The
    /// circle is the button — there is no separate control to keep in sync.
    fn indicator(&self, ui: &mut egui::Ui, active: bool) -> egui::Response {
        let diameter = 88.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(diameter, diameter), egui::Sense::click());

        let hovered = response.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let fill = match (active, hovered) {
            (true, false) => GREEN,
            (true, true) => GREEN_HOVER,
            (false, false) => RED,
            (false, true) => RED_HOVER,
        };

        let painter = ui.painter();
        let center = rect.center();
        let radius = diameter / 2.0;

        // A soft halo while active, so the "on" state reads at a glance even
        // from across the room.
        if active {
            painter.circle_filled(
                center,
                radius + 6.0,
                egui::Color32::from_rgba_unmultiplied(46, 160, 67, 40),
            );
        }

        painter.circle_filled(center, radius, fill);
        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(2.0, egui::Color32::from_black_alpha(70)),
        );

        // A filled circle for "awake", a hollow one for "sleep allowed" — the
        // state survives a screenshot in grayscale.
        let glyph_color = egui::Color32::WHITE;
        if active {
            painter.circle_filled(center, radius * 0.28, glyph_color);
        } else {
            painter.circle_stroke(center, radius * 0.28, egui::Stroke::new(3.0, glyph_color));
        }

        response.on_hover_text(if active {
            "Click to allow the system to sleep"
        } else {
            "Click to prevent the system from sleeping"
        })
    }
}
