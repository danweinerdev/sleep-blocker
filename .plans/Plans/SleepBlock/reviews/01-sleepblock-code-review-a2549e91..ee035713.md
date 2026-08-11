---
title: "Code Review: SleepBlock — Phase 5: Daemon Split and State Synchronisation"
type: review
status: open
created: 2026-08-11
updated: 2026-08-11
tags: [review]
related: [Plans/SleepBlock]
review_of: "Plans/SleepBlock"
rev: "a2549e91c0f971c108a026ef1f49c9a41c4a69b9..ee0357134ba6f387d2b4e57eb4acb31d291296d4"
# Advisory, not a phase gate: the gate requires every lane Aligned, and three
# lanes returned findings. Phase 5 therefore cannot complete on this review.
findings:
  - id: F-01
    severity: major
    title: "Acquisition failures never reach the window: the daemon logs them and returns success"
    status: fixed
  - id: F-02
    severity: major
    title: "has_tray is read once during a window when the daemon has not yet started its tray"
    status: fixed
  - id: F-03
    severity: major
    title: "Untimed D-Bus calls on the render thread freeze the window against a hung daemon"
    status: fixed
  - id: F-04
    severity: major
    title: "Doc comment on logic() describes the hide-to-tray design this phase deleted"
    status: fixed
  - id: F-05
    severity: minor
    title: "Design's Error Handling table claims failures are shown in the window"
    status: fixed
  - id: F-06
    severity: minor
    title: "GIT_DESCRIBE computed but never used"
    status: fixed
  - id: F-07
    severity: minor
    title: "zbus declared directly instead of workspace = true"
    status: fixed
  - id: F-08
    severity: minor
    title: "event-listener dependency not recorded in the plan's Key Decisions"
    status: fixed
followups: []
---

# Code Review: SleepBlock — Phase 5

**Reviewed state:** a2549e91c0f971c108a026ef1f49c9a41c4a69b9..ee0357134ba6f387d2b4e57eb4acb31d291296d4
**Review mode:** independent (four fresh-context lanes)

## Overall Verdict

**Alignment:** Moderate
**Lane status:** OK
**Critical issues:** 0 (4 major)

Phase 5 delivers what it planned — drift-detector re-ran the phase's own evidence
commands and reproduced them exactly, finding no missing work. The problems are
not gaps against the plan; they are things the plan did not think to require.
Three of the four majors were invisible to the plan-aware lanes precisely because
the code does what the phase doc says.

## Diff Scope

- Range: `a2549e91..ee035713` (frozen), 9 commits, 16 files, +1267/-417
- Reviewers: `sdd-planner:drift-detector`, `sdd-planner:quality-scanner`,
  `sdd-planner:spec-compliance`, `sdd-planner:blind-spot-finder`
- No project review lanes discovered.

## Confirmed Findings (agreed by 2+ reviewers)

None. The four lanes found four largely disjoint sets of problems — itself a
result worth noting, since it means no single lane would have produced this
review.

## Disagreements

### Is the phase complete? drift says yes; spec-compliance says a requirement regressed

- **drift-detector says:** Alignment Strong. Every task and acceptance criterion
  in the phase doc maps to real code, verified by re-running the evidence.
- **spec-compliance says:** Compliance Partial. FR-11 ("the utility SHALL surface
  the reason to the user") is broken for the window.
- **What this means:** Both are correct, and the gap between them is the finding.
  Phase 5 never listed FR-11 as one of its requirements, so a plan-aware reviewer
  has nothing to catch. The regression is real but *out of the phase's declared
  scope* — which is exactly how requirements get silently dropped during a
  refactor. The plan-vs-spec split found it; neither lane alone would have.
- **Recommendation:** Treat F-01 as in scope for this phase regardless of the
  phase doc, since this phase caused it.

## Blind Spots Only `blind-spot-finder` Caught

Both are startup- and load-dependent, which is why manual testing and the D-Bus
test suite missed them.

### Major — `has_tray` can be permanently wrong for a window's whole lifetime

**Location:** `sleep-blockd.rs:149-162`, `client.rs:77-99`
**Scenario:** The daemon claims its bus name and exports the interface *before*
starting the tray. `SleepTray::start` calls ksni's blocking `spawn()`, a
multi-round-trip D-Bus registration. The client's readiness gate is only
`sleep_blocked().is_ok()`, so on a loaded session bus at login the first 50ms
poll can succeed while the tray is still registering. The client then reads
`has_tray` once — under a comment asserting it "cannot change while the daemon
runs", which is false during exactly this window — and greys out the
"Keep running in tray when closed" checkbox for that window's entire life.
**Recommendation:** Start the tray before requesting the bus name, so `has_tray`
is settled before any client can observe it.

### Major — a hung daemon freezes the window with no path to the error handling

**Location:** `main.rs:200-267`, `client.rs:128-178`
**Scenario:** Every property read is a synchronous zbus call issued from `ui()`
on the render thread, once per frame, with no timeout configured anywhere. The
`daemon_gone` detection only catches a daemon that is *dead* (the call returns
`Err`). A daemon that is alive and still owns its name but blocked — e.g. inside
`Inhibitor::acquire`'s own logind round trip, while holding the state mutex —
makes every frame block indefinitely. The window, including its close button,
freezes.
**Recommendation:** Give the client's proxy a bounded timeout so a hung daemon
degrades into the existing `daemon_gone` path rather than hanging the UI.

## Quality (from quality-scanner)

### Major — stale doc comment describes the deleted design

`main.rs:168-170` still says `logic()` "runs every frame *and* while the window
is hidden, which is what makes hide-to-tray work". This phase deleted that
design: `window_policy.rs` is gone, `Status::window_hidden` is gone, and the
window is never hidden — closing exits the process. The comment will mislead the
next person to touch close behaviour.

### Minor — F-06, F-07

`GIT_DESCRIBE` (Makefile:50) is computed and never used. `zbus` is pinned
directly in `sleep-block-bin/Cargo.toml` while `sleep-block-core` correctly uses
`zbus.workspace = true`; they agree today and nothing keeps them agreeing.

## Spec Compliance (from spec-compliance)

### Major — F-01: FR-11 error surfacing regressed for the window

`Service::toggle` and `Service::set_keep_screen_awake` receive an
`Err(Error::Connect)` / `Err(Error::Inhibit)`, print it to the *daemon's* stderr,
and return `Ok` with a fallback snapshot. `DaemonClient` can therefore only ever
surface a zbus transport error, never the domain error. Before this phase,
`App::toggle` read `state.toggle().err()` directly and displayed it.

The user-visible consequence: if logind refuses the inhibitor, the window's
toggle silently does nothing with no explanation. The tray still explains it,
because it calls `state.toggle()` in-process.

**Recommendation:** Return `fdo::Error` from the D-Bus methods, or expose a
`last_error` property the client reads.

### Minor — F-05

The design's Error Handling table still claims failures are "shown in the
window". Update it when F-01 is fixed, or now to record the gap.

## Drift (from drift-detector)

No missing work and no approach drift. One minor: `event-listener` was added as
a dependency without a note in the plan's Key Decisions, which does document
other crate choices at that granularity (`zbus` reuse, the `async-io` backend).

## Open Questions

- **Crash path for FR-20 is untested.** `daemon_gone` should fire for a crashed
  daemon exactly as for a quit one, but only the orderly `quit()` path has a
  test. AC-29's wording covers only the orderly case, so no automated claim is
  overstated — but "or a crash" is inspection-only. *(spec-compliance)*
- **Does `spawn_gui()` block the tray's dispatch thread?** It is called
  synchronously from a ksni `activate` callback and opens a new session-bus
  connection. If that shares the tray's D-Bus servicing thread, a saturated bus
  would freeze the tray icon, not just delay the window. Not confirmed against
  ksni's dispatcher internals. *(blind-spot-finder)*
- **PATH fallback in `spawn_gui`/`spawn_daemon`.** Both fall back to a bare
  binary name when the sibling-of-`current_exe()` check fails. No concrete
  exploit was constructible from the diff alone. *(blind-spot-finder)*

## Findings

### F-01 — Major: acquisition failures never reach the window
**Impugns:** FR-11, `crates/sleep-block-bin/src/bin/sleep-blockd.rs:44-64`
**Scenario:** logind refuses the inhibitor. `Service::toggle` receives `Err(Error::Inhibit)`, prints it to the *daemon's* stderr, and returns `Ok` with a fallback snapshot. `DaemonClient` sees success.
**Why it matters:** The window's toggle silently does nothing with no explanation. Before this phase, `App::toggle` read the error directly and displayed it. The tray is unaffected — it calls `state.toggle()` in-process.
**Recommendation:** Return `fdo::Error` from the D-Bus methods, or expose a `last_error` property.

### F-02 — Major: `has_tray` can be permanently wrong
**Impugns:** FR-19, `sleep-blockd.rs:149-162`, `client.rs:77-99`
**Scenario:** The daemon claims its bus name and exports the interface before starting the tray, whose `spawn()` is a multi-round-trip D-Bus registration. The client's readiness gate checks only `sleep_blocked()`, so a 50ms poll can succeed mid-registration. `has_tray` is then read once and cached for the window's life.
**Why it matters:** At login on a loaded bus, "Keep running in tray when closed" is greyed out for that whole window, recoverable only by relaunching.
**Recommendation:** Start the tray before requesting the bus name.

### F-03 — Major: untimed D-Bus calls on the render thread
**Impugns:** FR-20, `main.rs:200-267`, `client.rs:128-178`
**Scenario:** Every property read is a synchronous zbus call from `ui()`, once per frame, with no timeout. `daemon_gone` only detects a daemon that returns `Err`. A daemon blocked inside `Inhibitor::acquire` while holding the state mutex still owns its name and never errors.
**Why it matters:** The window freezes indefinitely, including its close button, with no path to the error handling that exists.
**Recommendation:** Give the client's proxy a bounded timeout so a hung daemon degrades into `daemon_gone`.

### F-04 — Major: stale doc comment describes the deleted design
**Impugns:** `crates/sleep-block-bin/src/main.rs:168-170`
**Scenario:** The comment claims `logic()` "runs every frame *and* while the window is hidden, which is what makes hide-to-tray work". This phase deleted that design.
**Why it matters:** It will mislead the next person to touch close behaviour toward a mechanism that no longer exists.
**Recommendation:** Describe what happens now — closing exits the process, optionally leaving the daemon running.

### F-05 — Minor: design's Error Handling table is now wrong
**Impugns:** `.plans/Designs/SleepBlock/README.md:369-377`
**Scenario:** The table states failures are "shown in the window", which F-01 makes false for the GUI half.
**Why it matters:** A retrospective design that describes behaviour the code does not have is worse than no design.
**Recommendation:** Update when F-01 lands, or now to record the gap.

### F-06 — Minor: `GIT_DESCRIBE` computed but never used
**Impugns:** `Makefile:50`
**Scenario:** Assigned; `RPM_RELEASE` is built from `COMMITS_SINCE_TAG`/`GIT_SHA`/`GIT_DIRTY` instead.
**Why it matters:** Dead code invites the belief it is load-bearing.
**Recommendation:** Remove it, or use it and drop the separate shell-outs.

### F-07 — Minor: `zbus` bypasses the workspace dependency
**Impugns:** `crates/sleep-block-bin/Cargo.toml:29`
**Scenario:** Pinned as `zbus = "5.19.0"` while `sleep-block-core` uses `zbus.workspace = true`.
**Why it matters:** They agree today; a future bump to the workspace entry silently stops applying here.
**Recommendation:** Use `zbus.workspace = true`.

### F-08 — Minor: `event-listener` undocumented
**Impugns:** `crates/sleep-block-bin/Cargo.toml`, Plan Key Decisions
**Scenario:** Added for the daemon's shutdown signal; the plan documents other crate choices at this granularity but not this one.
**Why it matters:** The Key Decisions section claims to enumerate this class of choice.
**Recommendation:** Add a line when the plan is next touched.

## Resolution Log

<!-- Append-only; one entry per disposition. -->

### F-01 — fixed (2026-08-11)
`Service::toggle` and `Service::set_keep_screen_awake` now return
`zbus::fdo::Result` and propagate the domain error as `fdo::Error::Failed`
instead of printing it to the daemon's stderr. `announce()` runs before the
return either way, because a failed call can still have moved state (a
screen-lock failure leaves sleep blocking held) and the window must see that.
The wire signature is unchanged (`bb`), so this is additive. Added
`acquisition_failures_can_reach_the_caller` to `tests/daemon.rs`, which asserts
a call that cannot succeed returns `Err` rather than a fabricated success.

### F-02 — fixed (2026-08-11)
The daemon now starts its tray *before* claiming the bus name, so `has_tray` is
settled before any client can reach it. Verified by polling `HasTray` from the
first reachable read: `true` on attempt 1. The trade is that a losing daemon
briefly registers a tray icon before exiting, which is better than a window
being wrong for its entire lifetime. The client comment claiming `has_tray`
"cannot change while the daemon runs" was corrected — it is safe to read once
*because of* the new ordering, not inherently.

### F-03 — fixed (2026-08-11)
The client's connection is built with `method_timeout(2s)`. `Builder::method_timeout`
is on the connection, not the proxy — a first attempt at a proxy-level
`with_timeout` did not compile. A hung daemon now fails the read and falls into
the existing `daemon_gone` path rather than blocking the render thread.

### F-04 — fixed (2026-08-11)
The `logic()` doc comment described hide-to-tray, which this phase deleted. It
now states that nothing is ever hidden, closing exits the process, and
`keep_running_in_tray` decides only whether the daemon survives.

### F-05 — fixed (2026-08-11)
The design's Error Handling table is accurate again once F-01 landed; extended
to record *how* the error crosses the process boundary.

### F-06 — fixed (2026-08-11)
Removed the unused `GIT_DESCRIBE` assignment from the Makefile. `RPM_RELEASE`
was already derived from `COMMITS_SINCE_TAG`/`GIT_SHA`/`GIT_DIRTY`; verified
`make release-id` still produces the same shape afterwards.

### F-07 — fixed (2026-08-11)
`sleep-block-bin` now uses `zbus.workspace = true`, matching `sleep-block-core`.
Verified with `cargo tree` that it still resolves to zbus v5.19.0, so the two
declarations that happened to agree are now structurally unable to diverge.

### F-08 — fixed (2026-08-11)
Recorded `zbus` and `event-listener` in the plan's Dependencies section rather
than Key Decisions: neither was a weighed alternative the way the
async-io-vs-tokio choice was, so listing them as decisions would overstate the
deliberation.

### Q1 — resolved (2026-08-11)
Added `a_crashed_daemon_also_closes_an_open_gui`, which SIGKILLs the daemon
rather than calling `quit()`. Mutation-checked: disabling the `daemon_gone`
branch fails it. FR-20's "or a crash" clause now has an automated check rather
than resting on inspection.

### Q2 — resolved (2026-08-11)
Confirmed the suspicion by reading ksni's `Service::event`: menu callbacks are
invoked synchronously inside the D-Bus method handler, holding `&mut self`. The
old callback opened a fresh session-bus connection and made a `NameHasOwner`
round trip there, so a slow bus would have stalled the tray's own dispatch.

Fixed properly rather than by detaching a thread: the daemon now watches the
GUI's bus name via `NameOwnerChanged` on a background thread and keeps the
answer in an `AtomicBool`, so the callback is a single atomic load. Verified
five rapid "Show window" clicks produce exactly one GUI and the tray stays
responsive throughout.

### Q3 — closed, no change beyond a comment (2026-08-11)
Not a privilege-escalation vector: both processes run as the invoking user, so
anyone able to poison PATH can already execute code as them. The sibling lookup
already resolves `/proc/self/exe` via `current_exe()`, which is the mechanism
suggested; the bare-name fallback only fires when that sibling is missing, which
in a packaged install means the package is broken. Documented as such rather
than removed, since it is what lets a daemon started from an unusual location
still find its GUI.

## Orchestrator Observations

The four lanes produced almost entirely disjoint findings — no finding was
confirmed by two reviewers. For a review whose premise is triangulation, that is
worth stating plainly: the value here came from the lanes *not* overlapping, and
a single-pass review would have returned whichever subset matched the reviewer's
framing that day.

The drift/spec disagreement over FR-11 is the sharpest illustration. The phase
did what it said; what it said just didn't include "keep the error path working".
