---
title: "Code Review: SleepBlock — Phase 5 (second pass)"
type: review
status: open
created: 2026-08-11
updated: 2026-08-11
tags: [review]
related: [Plans/SleepBlock]
review_of: "Plans/SleepBlock"
rev: "a2549e91c0f971c108a026ef1f49c9a41c4a69b9..5585957ac87b29f6f92c768f66a0353fc17c341b"
# Advisory, not a phase gate: three lanes returned findings, and the gate
# requires every lane Aligned. Phase 5 cannot complete on this review.
findings:
  - id: F-09
    severity: major
    title: "F-03's timeout was reported fixed but never landed in the code"
    status: fixed
  - id: F-10
    severity: major
    title: "App::client comment asserts the GUI holds the locks, the opposite of the invariant"
    status: fixed
  - id: F-11
    severity: major
    title: "tray.rs module doc describes the deleted same-process sharing model"
    status: fixed
  - id: F-12
    severity: minor
    title: "GuiPresence::watch seeds before subscribing, so a signal in the gap is lost forever"
    status: fixed
  - id: F-13
    severity: minor
    title: "screensaver guard silently gates an unrelated keep_running_in_tray assertion"
    status: fixed
  - id: F-14
    severity: minor
    title: "Design Interfaces table omits ipc, set_keep_running_in_tray, keep_running_in_tray"
    status: fixed
  - id: F-15
    severity: minor
    title: "Duplicated doc comment on DaemonClient::connect reads as a merge artifact"
    status: fixed
  - id: F-16
    severity: minor
    title: "Service::announce swallows emission failures with no log trail"
    status: deferred
  - id: F-17
    severity: minor
    title: "AC-33 is checked off without a make package evidence row in this phase"
    status: deferred
followups:
  - id: FU-01
    finding: F-16
    summary: "Log PropertiesChanged emission failures so a stuck indicator has a trail"
    tracked_in: "6.1"
  - id: FU-02
    finding: F-17
    summary: "Add a make package evidence row to phase 5, or note AC-33's container half was verified in phase 4"
    tracked_in: "6.2"
---

# Code Review: SleepBlock — Phase 5 (second pass)

**Reviewed state:** a2549e91c0f971c108a026ef1f49c9a41c4a69b9..5585957ac87b29f6f92c768f66a0353fc17c341b
**Review mode:** independent (four fresh-context lanes)

## Overall Verdict

**Alignment:** Moderate
**Lane status:** OK
**Critical issues:** 0 (3 major)

Two lanes improved markedly against the first pass — spec-compliance moved
Partial → Strong, and hidden risk moved Elevated → Low. Both of the first
round's Majors are genuinely gone.

Against that, drift-detector caught the most serious finding of either pass:
a fix recorded as complete and verified that was never in the code.

## Diff Scope

- Range `a2549e91..5585957a` (frozen), 14 commits, 21 files
- Four lanes dispatched; no project lanes discovered

## Confirmed Findings (agreed by 2+ reviewers)

None again. As in the first pass the lanes were near-disjoint, which is the
argument for running all four rather than any one.

## The finding that matters most

### F-09 — Major: a fix was reported complete but never landed

**Caught by:** drift-detector
**Location:** `crates/sleep-block-bin/src/client.rs`

The first review's F-03 (untimed D-Bus calls freezing the render thread) was
recorded in the Resolution Log as fixed, with a specific claim: "the client
connection is built with `method_timeout(2s)`". drift-detector checked and found
`method_timeout` nowhere in the tree; commit `1e9a868` touched only five lines
of `client.rs`, all doc comment.

The cause: an earlier edit reverted a wrong proxy-level attempt, and the
follow-up connection-level edit silently matched nothing because the text it
targeted had already changed. Nothing failed loudly, and the Resolution Log was
written from intent rather than from a re-read.

The scenario F-03 described was therefore still live: a daemon alive but blocked
still owns its bus name, so `daemon_gone` never fires and the window's render
thread blocks indefinitely.

**Fixed and verified behaviourally this time.** With the daemon SIGSTOPped, the
window now gives up and exits after ~7s; before the fix it hung indefinitely.
`a_stopped_daemon_does_not_hang_the_window` pins it, and mutation-checking
confirms it fails (20s) with the timeout removed.

## Blind Spots Only `blind-spot-finder` Caught

### F-12 — Minor: seed-before-subscribe loses signals permanently

`GuiPresence::watch` read `name_has_owner` to seed its flag, then called
`receive_name_owner_changed()` — which is what actually registers the match
rule. Any acquisition or release in that gap is dropped by the bus with no
queue to recover it, leaving the flag stuck for the daemon's lifetime and
"Show window" a permanent no-op.

Narrow window, unbounded consequence, invisible to the existing tests, which
poll `name_has_owner` fresh rather than exercising this path. Fixed by
subscribing first and seeding afterwards, so anything in the gap is still queued
on the stream.

### F-16 — Minor: silent emission failures *(deferred)*

`Service::announce` uses `let _ = ...await` on all four emissions. Self-healing
within ~0.5s via polling, so low impact — but a maintainer debugging a stuck
indicator has nothing to grep for. Tracked as FU-01, plan task 6.1.

## Quality (from quality-scanner)

### F-10, F-11 — Major: two comments assert the opposite of the invariant

`App::client` claimed "the locks live here, not in the GUI"; the daemon split
exists precisely so the GUI holds none. `tray.rs`'s module doc claimed the tray
"shares state ... rather than owning any locks itself"; the tray is now the one
component that does own them.

Same class as the first pass's F-04. That one was fixed where it was pointed at
without sweeping for others of its kind, which is why this recurred.

### F-13, F-15 — Minor

A `screensaver_available()` guard wrapped a `keep_running_in_tray` assertion
that needs no screensaver, silently dropping that coverage on machines without
one — split into its own ungated test. And `DaemonClient::connect` carried two
stacked summary sentences from a merge.

## Spec Compliance (from spec-compliance)

**Strong.** FR-16 through FR-22 all covered, and FR-11/FR-20 were verified end to
end by building and running the suite rather than by commit presence. The
`(inspection)` marker on AC-19 was checked and found accurate rather than
overstated.

### F-14 — Minor

The design's Interfaces table fell out of sync during the retrospective edit:
still "four public items", omitting `ipc`, `set_keep_running_in_tray`, and
`Status.keep_running_in_tray`. Fixed.

## Drift (from drift-detector)

Beyond F-09: no missing work and no scope creep. One minor deferred — AC-33 is
checked off in the phase doc without a `make package` evidence row in this
phase; its container/multi-arch half was inherited from phase 4 and only the
release-supersession half is new here (F-17, FU-02).

## Open Questions

- **PATH fallback in `spawn_gui`/`spawn_daemon`.** Raised again, and again not
  escalated: no session-bus privilege boundary makes it more than "runs code as
  the user who could already do anything". Same disposition as Q3 last pass.

## Findings

### F-09 — Major: a fix was reported complete but never landed
**Impugns:** FR-20, `crates/sleep-block-bin/src/client.rs`
**Scenario:** A daemon alive but blocked still owns its bus name, so `daemon_gone` never fires and the window's render thread blocks indefinitely.
**Why it matters:** The first review recorded this fixed with a specific claim (`method_timeout(2s)`); `method_timeout` was nowhere in the tree. Commit `1e9a868` touched five lines of `client.rs`, all doc comment.
**Recommendation:** Add the timeout and verify it behaviourally, not by intent.

### F-10 — Major: `App::client` comment asserts the opposite of the invariant
**Impugns:** `crates/sleep-block-bin/src/main.rs`
**Scenario:** The comment claimed "the locks live here, not in the GUI".
**Why it matters:** The daemon split exists precisely so the window holds no locks; a reader would conclude the opposite of the design's central guarantee.
**Recommendation:** State that the client holds no inhibitors.

### F-11 — Major: `tray.rs` module doc describes the deleted sharing model
**Impugns:** `crates/sleep-block-bin/src/tray.rs`
**Scenario:** The doc claimed the tray "shares state ... rather than owning any locks itself".
**Why it matters:** Backwards — the tray runs in the daemon and holds `SleepBlock` directly, while the window is the one owning nothing.
**Recommendation:** Describe the daemon-resident tray and the D-Bus-reached window.

### F-12 — Minor: seed-before-subscribe loses signals permanently
**Impugns:** `crates/sleep-block-bin/src/spawn.rs`
**Scenario:** `receive_name_owner_changed()` registers the match rule; a seed read before it leaves a gap in which acquisitions or releases are dropped by the bus.
**Why it matters:** Narrow window, but the flag then sticks for the daemon's lifetime and "Show window" becomes a permanent no-op.
**Recommendation:** Subscribe first, then seed.

### F-13 — Minor: screensaver guard gates an unrelated assertion
**Impugns:** `crates/sleep-block-core/tests/state.rs`
**Scenario:** A `screensaver_available()` guard wrapped the whole test, including a `keep_running_in_tray` check needing no screensaver.
**Why it matters:** Silently drops that coverage wherever no screen locker exists.
**Recommendation:** Split it into its own ungated test.

### F-14 — Minor: design Interfaces table out of sync
**Impugns:** `.plans/Designs/SleepBlock/README.md`
**Scenario:** Still says "four public items", omitting `ipc`, `set_keep_running_in_tray`, `Status.keep_running_in_tray`.
**Why it matters:** The surrounding sections were rewritten in the same edit, so this reads as current rather than stale.
**Recommendation:** Update the table.

### F-15 — Minor: duplicated doc comment
**Impugns:** `crates/sleep-block-bin/src/client.rs`
**Scenario:** Two stacked summary sentences on `connect()` from a merge.
**Why it matters:** Reads as an edit artifact.
**Recommendation:** Merge into one block.

### F-16 — Minor: silent emission failures
**Impugns:** `crates/sleep-block-bin/src/bin/sleep-blockd.rs`
**Scenario:** `Service::announce` uses `let _ = ...await` on all four emissions.
**Why it matters:** Self-healing via polling within ~0.5s, but a stuck-looking indicator leaves no trail to grep.
**Recommendation:** Log failures at warn level.

### F-17 — Minor: AC-33 checked off without evidence in this phase
**Impugns:** `.plans/Plans/SleepBlock/05-Daemon-Split-And-Sync.md`
**Scenario:** No task's evidence table runs `make package`; the container/multi-arch half came from phase 4.
**Why it matters:** A checked criterion without an evidence row in its own phase.
**Recommendation:** Add the row, or note the split with phase 4.

## Resolution Log

### F-09 — fixed (2026-08-11)
Added `method_timeout(2s)` to the client's connection builder — this time
verified present in the tree, not just written. Behavioural check: with the
daemon SIGSTOPped the window exits after ~7s instead of hanging.
`a_stopped_daemon_does_not_hang_the_window` added and mutation-checked (fails at
20s with the timeout removed).

### F-10 — fixed (2026-08-11)
`App::client`'s comment now states that the client holds no inhibitors and that
every control is a request to the daemon.

### F-11 — fixed (2026-08-11)
`tray.rs`'s module doc now states that the tray lives inside the daemon and
holds its `SleepBlock` directly, while the window is a separate process reached
only over D-Bus.

### F-12 — fixed (2026-08-11)
`GuiPresence::watch` now calls `receive_name_owner_changed()` before seeding, so
a signal arriving in the former gap is queued on the stream rather than lost.

### F-13 — fixed (2026-08-11)
Split the tray-preference assertions into
`tray_preference_setter_agrees_with_a_later_snapshot`, which needs no
ScreenSaver provider and therefore runs everywhere.

### F-14 — fixed (2026-08-11)
Interfaces table updated: `ipc` listed, `set_keep_running_in_tray` and
`Status.keep_running_in_tray` added.

### F-15 — fixed (2026-08-11)
Merged the duplicated summary sentences on `DaemonClient::connect`.

### F-16 — deferred (2026-08-11)
Tracked as FU-01, plan task 6.1. Self-healing via polling; worth a log line, not worth
blocking on.

### F-17 — deferred (2026-08-11)
Tracked as FU-02, plan task 6.2. A documentation-evidence gap, not a functional one.

## Orchestrator Observations

F-09 is the finding I would least have caught alone, and it is a failure of my
own process rather than of the code: I recorded a fix as verified without
re-reading the file. The Resolution Log said "verified" because I intended the
edit, not because I confirmed it. Every fix in this round was checked against
the tree afterwards.
