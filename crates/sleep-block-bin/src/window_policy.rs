//! Decides what the window should do on each frame, independently of egui.
//!
//! This exists to be testable. The rules are small but easy to get subtly
//! wrong — the first implementation hid the window and then immediately showed
//! it again, because it re-used a status snapshot taken before the hide. That
//! class of bug is invisible in a type check and expensive to find by hand,
//! since reproducing it needs a running compositor.
//!
//! Keeping the decision here means the `eframe::App` impl only translates the
//! outcome into viewport commands, and the rules themselves can be exercised
//! headlessly.

use sleep_block_core::SleepBlock;

/// What the caller should ask the viewport to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowAction {
    /// Cancel the pending close so the process survives.
    pub cancel_close: bool,
    /// Hide the window without exiting.
    pub hide: bool,
    /// Show the window and raise it.
    pub show: bool,
    /// The tray needs to re-read state because this frame changed it.
    pub refresh_tray: bool,
}

/// The inputs a frame's decision depends on. Grouped into a struct so a new
/// input cannot be silently dropped at a call site.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// The window manager asked the window to close.
    pub close_requested: bool,
    /// The in-window Quit button was pressed; a deliberate quit outranks
    /// hide-on-close.
    pub quitting: bool,
}

/// Tracks what the previous frame saw, so transitions can be detected.
#[derive(Debug, Default)]
pub struct WindowPolicy {
    was_hidden: bool,
}

impl WindowPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one frame's inputs and returns what the window should do.
    ///
    /// Mutates the shared state as a side effect, because hiding is a state
    /// change that the tray must observe.
    pub fn step(&mut self, state: &SleepBlock, frame: Frame) -> WindowAction {
        let mut status = state.snapshot();
        let mut action = WindowAction::default();

        if frame.close_requested && status.keep_running_in_tray && !frame.quitting {
            action.cancel_close = true;
            action.hide = true;
            action.refresh_tray = true;
            // Re-read from the setter rather than reusing the snapshot above:
            // that one still reports the window as visible, and the show check
            // below would immediately undo the hide.
            status = state.set_window_hidden(true);
        }

        // A transition from hidden to not-hidden means something (the tray's
        // "Show window", or the setting being switched off) asked for the
        // window back.
        if !status.window_hidden && self.was_hidden {
            action.show = true;
        }
        self.was_hidden = status.window_hidden;

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Frame {
        Frame {
            close_requested: false,
            quitting: false,
        }
    }

    fn closing() -> Frame {
        Frame {
            close_requested: true,
            quitting: false,
        }
    }

    #[test]
    fn close_exits_when_keep_running_is_off() {
        let state = SleepBlock::new();
        let mut policy = WindowPolicy::new();

        let action = policy.step(&state, closing());

        // Nothing is cancelled, so eframe proceeds with the close.
        assert!(!action.cancel_close, "close must not be intercepted");
        assert!(!action.hide);
        assert!(!state.snapshot().window_hidden);
    }

    #[test]
    fn close_hides_when_keep_running_is_on() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();

        let action = policy.step(&state, closing());

        assert!(action.cancel_close, "close must be cancelled to stay alive");
        assert!(action.hide);
        assert!(
            action.refresh_tray,
            "the tray must learn the window is gone"
        );
        assert!(state.snapshot().window_hidden);
    }

    /// A closed window must stay closed. This is the property the user sees;
    /// the tests that actually pin down the stale-snapshot bug are the three
    /// covering the *show* path, since reusing the pre-hide snapshot leaves
    /// `was_hidden` a frame behind and it is reopening that breaks.
    #[test]
    fn hidden_window_stays_hidden_on_later_frames() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();

        let closing_action = policy.step(&state, closing());
        assert!(
            !closing_action.show,
            "the frame that hides the window must not also show it"
        );

        for frame in 0..5 {
            let action = policy.step(&state, idle());
            assert!(
                !action.show,
                "frame {frame} tried to show a window the user closed"
            );
            assert!(state.snapshot().window_hidden, "frame {frame} un-hid it");
        }
    }

    #[test]
    fn tray_show_request_reopens_the_window() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();
        policy.step(&state, closing());

        // What the tray's "Show window" item does.
        state.set_window_hidden(false);
        let action = policy.step(&state, idle());

        assert!(action.show, "the window should come back");
        assert!(!state.snapshot().window_hidden);
    }

    #[test]
    fn show_is_requested_once_not_every_frame() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();
        policy.step(&state, closing());
        state.set_window_hidden(false);

        assert!(policy.step(&state, idle()).show);
        // Repeatedly raising and focusing a window the user is already looking
        // at would steal focus on every frame.
        assert!(!policy.step(&state, idle()).show, "show must not repeat");
    }

    #[test]
    fn quit_button_bypasses_hide_on_close() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();

        let action = policy.step(
            &state,
            Frame {
                close_requested: true,
                quitting: true,
            },
        );

        assert!(
            !action.cancel_close,
            "a deliberate quit must not be turned into a hide"
        );
        assert!(!action.hide);
    }

    /// Turning the setting off while hidden must not strand the application
    /// with no window and no way to get one.
    #[test]
    fn disabling_keep_running_reopens_a_hidden_window() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();
        policy.step(&state, closing());
        assert!(state.snapshot().window_hidden);

        state.set_keep_running_in_tray(false);
        let action = policy.step(&state, idle());

        assert!(action.show, "the window must come back");
        assert!(!state.snapshot().window_hidden);
    }

    #[test]
    fn repeated_closes_are_idempotent() {
        let state = SleepBlock::new();
        state.set_keep_running_in_tray(true);
        let mut policy = WindowPolicy::new();

        for _ in 0..3 {
            let action = policy.step(&state, closing());
            assert!(action.cancel_close);
            assert!(state.snapshot().window_hidden);
            assert!(!action.show, "a close must never also request a show");
        }
    }
}
