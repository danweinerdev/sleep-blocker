// Hide the console window on Windows. Harmless elsewhere; this app is
// Linux-only in practice since it talks to systemd-logind.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

use sleep_block_core::{Inhibitor, ScreenInhibitor};

const REASON: &str = "User requested the system stay awake";

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

    eframe::run_native(
        "sleep-block",
        options,
        Box::new(|_cc| Ok(Box::<App>::default())),
    )
}

#[derive(Default)]
struct App {
    /// `Some` exactly when sleep is currently inhibited. This single field is
    /// the source of truth for the UI state — there is no separate bool that
    /// could drift out of sync with the lock we actually hold.
    inhibitor: Option<Inhibitor>,
    /// `Some` only when sleep is blocked *and* the user opted into keeping the
    /// screen awake too. Screen blanking and locking are a separate mechanism
    /// from sleep, so this is a second lock rather than a flag on the first.
    screen_inhibitor: Option<ScreenInhibitor>,
    /// The checkbox state. Deliberately independent of `screen_inhibitor`: the
    /// preference persists while sleep blocking is off, so re-enabling restores
    /// the user's choice instead of silently resetting it.
    keep_screen_awake: bool,
    /// Set when acquiring fails, so the user sees why nothing happened.
    error: Option<String>,
}

impl App {
    fn is_active(&self) -> bool {
        self.inhibitor.is_some()
    }

    fn toggle(&mut self) {
        self.error = None;

        if self.inhibitor.is_some() {
            // Dropping the inhibitors releases both locks: the logind descriptor
            // closes, and `ScreenInhibitor`'s `Drop` calls `UnInhibit`.
            self.inhibitor = None;
            self.screen_inhibitor = None;
            return;
        }

        match Inhibitor::acquire(REASON) {
            Ok(inhibitor) => self.inhibitor = Some(inhibitor),
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        }

        self.sync_screen_inhibitor();
    }

    /// Brings the screen lock in line with the checkbox. Called both when the
    /// main toggle turns on and when the checkbox itself changes, so ticking the
    /// box mid-session takes effect immediately rather than on the next toggle.
    ///
    /// A failure here leaves sleep blocking intact: the screen lock is the
    /// secondary concern, and losing it is not a reason to let the machine
    /// suspend. The error text explains what happened.
    fn sync_screen_inhibitor(&mut self) {
        let wanted = self.keep_screen_awake && self.inhibitor.is_some();

        match (wanted, self.screen_inhibitor.is_some()) {
            (true, false) => match ScreenInhibitor::acquire(REASON) {
                Ok(screen) => self.screen_inhibitor = Some(screen),
                Err(e) => {
                    self.keep_screen_awake = false;
                    self.error = Some(format!("Screen lock not blocked: {e}"));
                }
            },
            (false, true) => self.screen_inhibitor = None,
            _ => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The frame must be told to fill the window. Left to itself it wraps
        // only its contents, leaving the remainder of the viewport unpainted —
        // which shows up as a black bar below the last control.
        ui.set_min_size(ui.available_size());
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);

                let active = self.is_active();
                ui.heading(if active { "Awake" } else { "Sleep allowed" });
                ui.add_space(12.0);

                if self.indicator(ui, active).clicked() {
                    self.toggle();
                }

                ui.add_space(12.0);
                ui.label(if self.is_active() {
                    "Sleep is blocked.\nClick to allow sleeping again."
                } else {
                    "The system can suspend normally.\nClick to keep it awake."
                });

                ui.add_space(14.0);
                let toggled = ui
                    .checkbox(&mut self.keep_screen_awake, "Also keep screen on")
                    .on_hover_text(
                        "Additionally block screen blanking and locking.\n\
                         Leave off to let the monitors sleep as usual.",
                    )
                    .changed();
                if toggled {
                    // Clear a stale failure so the retry isn't shown alongside
                    // the previous attempt's error.
                    self.error = None;
                    self.sync_screen_inhibitor();
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
