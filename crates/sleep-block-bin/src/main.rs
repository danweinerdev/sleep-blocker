// Hide the console window on Windows. Harmless elsewhere; this app is
// Linux-only in practice since it talks to systemd-logind.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use eframe::egui;

use sleep_block_core::SleepBlock;

use crate::tray::SleepTray;

/// Fixed window size. Tall enough for the tallest state — the one where an
/// error line is showing under the checkbox — so no content is ever clipped.
const WINDOW_SIZE: [f32; 2] = [280.0, 300.0];

/// Colors chosen to stay legible against egui's dark background and to remain
/// distinguishable for the most common forms of color blindness — the shapes
/// and the label carry the state too, so color is never the only signal.
const GREEN: egui::Color32 = egui::Color32::from_rgb(46, 160, 67);
const GREEN_HOVER: egui::Color32 = egui::Color32::from_rgb(56, 190, 80);
const RED: egui::Color32 = egui::Color32::from_rgb(180, 48, 48);
const RED_HOVER: egui::Color32 = egui::Color32::from_rgb(210, 60, 60);

fn main() -> eframe::Result {
    // The layout is a fixed stack of controls with nothing to reflow, so the
    // window is pinned rather than resizable. Min and max are both set: some
    // compositors still allow dragging an edge when only `resizable(false)` is
    // given, and clamping both bounds removes that.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(WINDOW_SIZE)
            .with_min_inner_size(WINDOW_SIZE)
            .with_max_inner_size(WINDOW_SIZE)
            .with_resizable(false)
            .with_title("Sleep Block"),
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
}

impl App {
    fn new(state: SleepBlock, tray: Option<ksni::blocking::Handle<SleepTray>>) -> Self {
        Self {
            state,
            tray,
            error: None,
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

    /// Tells the tray to re-read the shared state. Without this the icon would
    /// keep its previous appearance until something else prompted a redraw,
    /// since ksni has no way to observe the state changing underneath it.
    fn refresh_tray(&self) {
        if let Some(tray) = &self.tray {
            tray.update(|_| {});
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The tray can toggle the state from its own thread, and egui has no
        // way to learn about that. Polling once a second keeps the window from
        // showing a stale reading without burning a core on a busy redraw loop.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));

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

                if let Some(error) = &self.error {
                    ui.add_space(8.0);
                    ui.colored_label(RED_HOVER, error);
                }
            });
        });
    }
}

impl App {
    /// Draws the circular status light and returns its click response. The
    /// circle is the button — there is no separate control to keep in sync.
    fn indicator(&self, ui: &mut egui::Ui, active: bool) -> egui::Response {
        let diameter = 88.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(diameter, diameter),
            egui::Sense::click(),
        );

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
            painter.circle_stroke(
                center,
                radius * 0.28,
                egui::Stroke::new(3.0, glyph_color),
            );
        }

        response.on_hover_text(if active {
            "Click to allow the system to sleep"
        } else {
            "Click to prevent the system from sleeping"
        })
    }
}
