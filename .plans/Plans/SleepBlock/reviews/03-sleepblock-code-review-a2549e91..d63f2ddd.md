---
title: "Code Review: SleepBlock — Phase 5 (third pass)"
type: review
status: open
created: 2026-08-11
updated: 2026-08-11
tags: [review]
related: [Plans/SleepBlock]
review_of: "Plans/SleepBlock"
rev: "a2549e91c0f971c108a026ef1f49c9a41c4a69b9..d63f2ddd0c061af0a8f4d27e0c3758a8a7a6d7de"
findings:
  - id: F-18
    severity: major
    title: "Tray Quit bypassed the graceful shutdown the daemon was built around"
    status: fixed
  - id: F-19
    severity: minor
    title: "ipc.rs claimed the tray's Quit item calls the quit() method; it does not"
    status: fixed
  - id: F-20
    severity: minor
    title: "Design NFR-08 row claims all three mutators announce four properties"
    status: fixed
  - id: F-21
    severity: minor
    title: "libc dev-dependency undocumented, against the precedent F-08 set"
    status: fixed
  - id: F-22
    severity: minor
    title: "hide-on-close terminology survives where nothing is hidden"
    status: fixed
  - id: F-23
    severity: question
    title: "Blocking logind acquire runs on the daemon's async dispatch thread"
    status: answered
  - id: F-24
    severity: question
    title: "Reported Critical: hide-to-tray never cancels the window close"
    status: rejected
followups: []
---

# Code Review: SleepBlock — Phase 5 (third pass)

**Reviewed state:** a2549e91c0f971c108a026ef1f49c9a41c4a69b9..d63f2ddd0c061af0a8f4d27e0c3758a8a7a6d7de
**Review mode:** independent (four fresh-context lanes)

## Overall Verdict

**Alignment:** Moderate
**Lane status:** OK
**Critical issues:** 0 — one was reported and rejected on evidence (F-24)

| Lane | Pass 1 | Pass 2 | Pass 3 |
|---|---|---|---|
| drift-detector | Strong | Moderate | **Strong** |
| spec-compliance | Partial | Strong | **Strong** |
| blind-spot-finder | Elevated | Low | Elevated |
| quality-scanner | Strong | Acceptable | Concerning* |

\* driven by F-24, which does not survive verification.

One genuine code finding (F-18), four documentation findings, one question
answered by measurement, and one reported Critical rejected.

## Findings

### F-18 — Major: tray Quit bypassed the graceful shutdown
**Impugns:** FR-16, `crates/sleep-block-bin/src/tray.rs`
**Scenario:** The daemon has a `done` event whose stated purpose is that the main
thread unwinds normally "rather than calling `exit` from inside a D-Bus
handler". The tray's Quit item called `process::exit(0)` directly from inside
ksni's synchronous callback, bypassing it entirely.
**Why it matters:** A window with an in-flight `toggle()` sees a dropped
connection instead of the error reply the daemon deliberately propagates — the
same "did it fail or did it vanish?" ambiguity the graceful path exists to
prevent. Untestable through the menu, so both prior passes missed it.
**Recommendation:** Signal `done` instead. Fixed.

### F-19 — Minor: `ipc.rs` misdescribed who calls `quit()`
**Impugns:** `crates/sleep-block-core/src/ipc.rs`
**Scenario:** The doc claimed `quit()` is "Used by the tray's Quit item and by
the GUI's Quit button". The tray never called it.
**Why it matters:** It described the architecture the code was supposed to have,
which is how F-18 stayed invisible.
**Recommendation:** State that the tray signals shutdown directly. Fixed.

### F-20 — Minor: design overstates the announce contract
**Impugns:** NFR-08, `.plans/Designs/SleepBlock/README.md`
**Scenario:** The Requirement Realisation row says "every mutating method
announces all four properties". `set_keep_running_in_tray` relies on zbus's own
setter signal instead.
**Why it matters:** NFR-08 still holds — that call changes one property — but
this is the third retrospective-edit staleness in the same document, after the
Error Handling table (F-05) and the Interfaces table (F-14).
**Recommendation:** Describe the two cases separately. Fixed.

### F-21 — Minor: `libc` dev-dependency undocumented
**Impugns:** Plan §Dependencies
**Scenario:** Added for `SIGSTOP`/`SIGCONT` in the hung-daemon test, after F-08
had already established that dependencies get recorded there.
**Recommendation:** Record it. Fixed.

### F-22 — Minor: "hide-on-close" survives where nothing is hidden
**Impugns:** `main.rs`, `sleep-blockd.rs`, `ipc.rs`, `tray.rs`
**Scenario:** Six sites, plus a test name, still labelled the behaviour
"hide-on-close" after the design moved to a disposable window.
**Why it matters:** Same class as F-04 and F-10/F-11 — fixed where pointed at,
never swept. This pass swept all of them, including the test name.
**Recommendation:** Use `keep-running-in-tray`, matching the property. Fixed.

### F-23 — Question: blocking acquire on the async dispatch thread
**Impugns:** `sleep-blockd.rs`, `sleep-block-core/src/inhibit.rs`
**Scenario:** `toggle` is an `async fn` D-Bus handler that calls
`Inhibitor::acquire`, which opens a blocking connection and makes a synchronous
logind round trip — stalling the daemon's dispatch thread for its duration. If
that exceeded the client's 2s timeout, the window would conclude the daemon was
dead.
**Answer:** Structurally accurate, not currently reachable. Measured toggle round
trips of 3ms, 23ms and 19ms against a 2000ms timeout — roughly a 100× margin.
Recorded rather than fixed: the fix means moving acquisition off the dispatch
thread and reworking the mutex discipline around it, which is a larger change
than the evidence justifies. Worth revisiting if logind ever gets slow.

### F-24 — Rejected: "hide-to-tray never cancels the window close"
**Impugns:** Reported against `main.rs` as Critical
**Scenario as reported:** `ViewportCommand::CancelClose` is never sent, so
closing the window always exits the process and hide-to-tray is
"silently non-functional".
**Why rejected:** The premise is correct — `CancelClose` is absent — but the
conclusion belongs to the single-process design. In the two-process
architecture the window is *meant* to exit; the daemon survives and the tray's
"Show window" launches a fresh one. Verified: with `keep_running_in_tray` set,
closing the GUI leaves the daemon running and holding its locks.
`CancelClose` would in fact break the design, since a Wayland window cannot be
hidden — which is the whole reason for the split.

## Resolution Log

### F-18 — fixed (2026-08-11)
`SleepTray` now holds an `Arc<Event>` shared with the daemon's `done`, and the
Quit item releases the locks then signals it. Notifying is a local wakeup, so it
is still safe from a ksni callback, which must not do I/O. Verified: tray Quit
exits the daemon and releases the lock (1 → 0).

### F-19 — fixed (2026-08-11)
`ipc.rs` now records that the tray signals shutdown directly, because it runs
inside the daemon, rather than calling `quit()` over the bus.

### F-20 — fixed (2026-08-11)
The NFR-08 row now distinguishes the two cases: `toggle` and
`set_keep_screen_awake` announce all four properties; `set_keep_running_in_tray`
relies on its own setter signal.

### F-21 — fixed (2026-08-11)
`libc` recorded in the plan's Dependencies section, noting it is a
dev-dependency used for the SIGSTOP/SIGCONT hung-daemon test.

### F-22 — fixed (2026-08-11)
Swept every occurrence of "hide-on-close" rather than only the cited ones —
four comments, one doc comment, and a test name now say
`keep-running-in-tray`, matching the property. Verified none remain.

### F-23 — answered (2026-08-11)
Measured rather than argued: 3–23ms against a 2s timeout. Recorded as a known
architectural characteristic, not a defect.

### F-24 — rejected (2026-08-11)
Verified behaviourally. The reported Critical assumes a design this phase
deliberately replaced.

## Orchestrator Observations

Three passes, and the lane that most consistently earned its place was
whichever one held a perspective the others structurally could not — the
diff-only lane in passes 1 and 3, the plan-aware lane in pass 2.

This pass also produced the first false positive worth naming. quality-scanner
reasoned correctly from the wrong architecture, and its verdict of Concerning
rests entirely on it. That is not a reason to distrust the lane — an
intent-blind reviewer *should* report what the code alone suggests — but it is a
reason the synthesis step exists, and a reason to verify a Critical before
acting on it rather than after.
