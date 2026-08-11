---
title: "Daemon Split and State Synchronisation"
type: phase
plan: "SleepBlock"
phase: 5
status: complete
created: 2026-08-11
updated: 2026-08-11
deliverable: "A daemon owning the inhibitors and tray, a disposable GUI client, and reliable state propagation between them"
tasks:
  - id: "5.1"
    title: "Derive the RPM Release from git"
    status: complete
    verification: "make release-id reports 1 on a tag and 1.<commits>.git<sha> after it; rpm.vercmp confirms a snapshot outranks the release it came from, that commit counts order numerically, and that the next release outranks any snapshot. Serves FR-22."
    justifies: "Every build since the last bump produced an identical NEVRA, so dnf saw no difference and refused to upgrade -- which presented as the package never updating and cost a long debugging detour."
  - id: "5.2"
    title: "Split into a daemon and a GUI client"
    status: complete
    verification: "The GUI auto-starts a daemon; closing the GUI leaves the daemon holding a real logind lock; the tray relaunches the GUI; Quit stops everything. Serves FR-16, FR-17, FR-19."
    justifies: "Delivers FR-16, FR-17, FR-19 and satisfies AC-24, AC-25, AC-32. A Wayland window cannot hide or raise itself, so hide-to-tray is impossible in one process with eframe."
    depends_on: ["5.1"]
  - id: "5.3"
    title: "Announce property changes from the daemon"
    status: complete
    verification: "dbus-monitor shows two toggles producing eight PropertiesChanged signals; a property read tracks state across toggles. Serves NFR-08."
    justifies: "Delivers NFR-08 and satisfies AC-28. Without emission a zbus proxy serves its first reading forever, freezing the window's indicator."
    depends_on: ["5.2"]
  - id: "5.4"
    title: "Add end-to-end daemon tests and fix single-instance detection"
    status: complete
    verification: "cargo test --test daemon passes; a second daemon on a taken name exits cleanly and explains why. Serves FR-18, FR-21."
    justifies: "Delivers FR-18 and satisfies AC-26, AC-30. Writing the tests found that connection::Builder::name succeeds even when the name is taken, so two daemons ran silently, both holding locks and tray icons."
    depends_on: ["5.3"]
  - id: "5.5"
    title: "Stop Show window spawning duplicate GUIs"
    status: complete
    verification: "With a GUI running, a second launch exits 0 and the process count stays at one; two Show window clicks leave one GUI. Serves FR-18, FR-19."
    justifies: "Satisfies AC-27. A running window cannot be raised on Wayland, so the daemon had nothing to bring forward and simply spawned again."
    depends_on: ["5.4"]
  - id: "5.6"
    title: "Make state propagate reliably to the window"
    status: complete
    verification: "Three consecutive tray clicks produce three corresponding GUI frames, in order; setting the preference true/false/true leaves daemon and tray agreeing at every step. Serves FR-21, NFR-08."
    justifies: "Satisfies AC-28, AC-31. Two distinct staleness faults -- a within-frame snapshot and a zbus property cache -- each left the window contradicting the tray."
    depends_on: ["5.5"]
  - id: "5.7"
    title: "Close the window when its daemon exits"
    status: complete
    verification: "Tray Quit and the GUI's Quit button each stop both processes and release all locks; measured GUI exit latency 104ms. Serves FR-20."
    justifies: "Delivers FR-20 and satisfies AC-29. Quit from the tray left an orphaned window whose every control failed silently."
    depends_on: ["5.6"]
---

# Phase 5: Daemon Split and State Synchronisation

## Overview

Turns the single-process application into a daemon plus a disposable window, and
then makes the two agree. The split was forced by a platform limit rather than
chosen: on Wayland a window cannot hide or raise itself, so a tray icon cannot
bring one back within a single process.

Most of this phase's cost was not the split itself but the synchronisation that
followed. Four separate staleness faults had to be found and fixed before the
two surfaces reliably agreed, and each was invisible to the test suite that
existed at the time.

## 5.1: Derive the RPM Release from git

### Subtasks
- [x] Compute `Release` from `git describe`: `1` on a tag, `1.<commits>.git<sha>` after
- [x] Append `.dirty` for an uncommitted tree
- [x] Pass it into the container, which has no reliable git identity
- [x] Add `make release-id` to print what the tree would build

### Notes
Revision boundary: two builds from different sources can no longer share a
NEVRA.

### Trap
Auto-incrementing `Release` on every build is the obvious fix and the wrong one:
`Release` distinguishes packaging revisions of the *same* source. Incrementing
it per build would claim fourteen packagings of one version when the source
changed every time.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: c19526f38fdb943619d49eab9d5e106af405f91c
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision c19526f38fdb943619d49eab9d5e106af405f91c
- Focused review: `git show c19526f38fdb943619d49eab9d5e106af405f91c`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: c19526f38fdb943619d49eab9d5e106af405f91c
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `make release-id` | . | PASS (exit 0) | sleep-block-0.1.1-1.10.gitc19526f |
| `rpm --eval "%{lua:print(rpm.vercmp(...))}"` | . | PASS (exit 0) | snapshot > release, 1.9 < 1.10, snapshot < next release |

## 5.2: Split into a daemon and a GUI client

### Subtasks
- [x] Define the D-Bus contract in `sleep-block-core::ipc`
- [x] Add `sleep-blockd` owning `SleepBlock`, the tray, and both inhibitors
- [x] Reduce the GUI to a client holding no locks
- [x] Start a daemon from the GUI when none is running
- [x] Package both binaries

### Notes
Revision boundary: the application survives its window closing, and the tray can
bring a window back.

### Trap
Three single-process approaches fail before this one. `set_visible` is an empty
function on Wayland; un-minimising is explicitly ignored; and destroying a child
viewport does hide *that* window but leaves eframe's mandatory root window on
screen as an empty grey panel. Only the process split avoids needing the
compositor's cooperation.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 83e2fb257caca24183d8d43aea84af0ca0b8f35f
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision 83e2fb257caca24183d8d43aea84af0ca0b8f35f
- Focused review: `git show 83e2fb257caca24183d8d43aea84af0ca0b8f35f`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 83e2fb257caca24183d8d43aea84af0ca0b8f35f
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `./target/release/sleep-block` then kill it | . | PASS (exit 0) | daemon auto-started, survived the GUI, still held a logind lock |
| `busctl --user call … Toggle` | . | PASS (exit 0) | lock count 0 → 1 → 0 with no GUI running |

## 5.3: Announce property changes from the daemon

### Subtasks
- [x] Emit `PropertiesChanged` for every property a mutation can move
- [x] Keep the emission out of the `#[interface]` block so it is not exported

### Notes
Revision boundary: a client observing the daemon sees state change.

### Trap
zbus caches properties lazily and refreshes only on `PropertiesChanged`. A
server that never emits it leaves every client serving the value it read first —
which looks exactly like a frozen UI while the daemon itself is correct.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 4886fa6af79dc15336a9095172462c17d0a4a64c
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision 4886fa6af79dc15336a9095172462c17d0a4a64c
- Focused review: `git show 4886fa6af79dc15336a9095172462c17d0a4a64c`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 4886fa6af79dc15336a9095172462c17d0a4a64c
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `dbus-monitor` during two toggles | . | PASS (exit 0) | 8 PropertiesChanged signals, 4 properties each |

## 5.4: Add end-to-end daemon tests and fix single-instance detection

### Subtasks
- [x] Make the bus name overridable so tests do not fight the real daemon
- [x] Add tests driving a real daemon over D-Bus
- [x] Request the bus name explicitly and inspect the reply

### Notes
Revision boundary: the daemon's behaviour is covered by tests that exercise the
same path the window uses.

### Trap
`connection::Builder::name()` succeeds even when another process owns the name,
so it cannot serve as a single-instance check. A taken name surfaces as either a
non-primary reply or `Err(NameTaken)`; both must be handled.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 62b114bc77066a256d4d0953fec80647987e526a
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision 62b114bc77066a256d4d0953fec80647987e526a
- Focused review: `git show 62b114bc77066a256d4d0953fec80647987e526a`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 62b114bc77066a256d4d0953fec80647987e526a
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release --test daemon` | . | PASS (exit 0) | 9 passed at this revision |
| second daemon on a taken name | . | PASS (exit 0) | printed "already running", exited 0, one process left |

## 5.5: Stop Show window spawning duplicate GUIs

### Subtasks
- [x] Give the window its own bus name, released when it exits
- [x] Check that name before the daemon launches a window
- [x] Exit with `process::exit` rather than returning from `main`
- [x] Remove the unused `ShowWindowRequested` signal

### Notes
Revision boundary: "Show window" is idempotent.

### Trap
zbus keeps executor threads alive, so returning from `main` leaves the losing
process sleeping in `ep_poll` instead of exiting. Any early-return path in these
binaries needs an explicit exit.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 3d2c7ec839aa84296f51d27d644153e06ed9d6d5
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision 3d2c7ec839aa84296f51d27d644153e06ed9d6d5
- Focused review: `git show 3d2c7ec839aa84296f51d27d644153e06ed9d6d5`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 3d2c7ec839aa84296f51d27d644153e06ed9d6d5
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| second GUI launch with one running | . | PASS (exit 0) | exited 0; process count stayed at 1 |
| two "Show window" clicks | . | PASS (exit 0) | one GUI process |

## 5.6: Make state propagate reliably to the window

### Subtasks
- [x] Re-read state after a checkbox click rather than reusing the frame's snapshot
- [x] Build the client's proxy with `CacheProperties::No`
- [x] Add a waker thread calling `request_repaint`
- [x] Add tests for a reader observing changes made on another connection

### Notes
Revision boundary: a change made on either surface appears on the other without
user interaction.

### Trap
Two independent staleness faults present identically, and fixing one leaves the
other. A within-frame snapshot goes stale between reading and rendering; a zbus
property cache goes stale for the life of the connection. Both look like "the
window disagrees with the tray".

Testing this requires the change to be made through a *different* connection
than the reader. Driving both through one proxy refreshes the cache as a side
effect and hides the bug entirely.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 34418f1101978216f81388439c9a6d2aaa2f5585
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision 34418f1101978216f81388439c9a6d2aaa2f5585
- Focused review: `git show 34418f1101978216f81388439c9a6d2aaa2f5585`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 34418f1101978216f81388439c9a6d2aaa2f5585
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| instrumented GUI, three tray clicks | . | PASS (exit 0) | three corresponding GUI frames, in order |
| `cargo test --test daemon observe` | . | PASS (exit 0) | 2 passed; both fail when announcements are removed |

## 5.7: Close the window when its daemon exits

### Subtasks
- [x] Propagate read failures instead of `unwrap_or(false)`
- [x] Close the window once the daemon stops answering
- [x] Point test GUIs at their own daemon via the bus-name override

### Notes
Revision boundary: no orphaned window survives its daemon.

### Trap
`unwrap_or(false)` renders a dead daemon as "nothing is blocked", which is
indistinguishable from a normal idle state — the failure has to be allowed to
surface before it can be handled.

A subtler one: the existing GUI tests started a daemon on a private bus name but
the GUI connected to the compiled-in default, so it spawned its own daemon and
was unaffected by anything the test did. Those tests passed while testing
nothing.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: ee0357134ba6f387d2b4e57eb4acb31d291296d4
- Identity recheck: git log --format=%H, 2026-08-11 09:40, matches recorded revision ee0357134ba6f387d2b4e57eb4acb31d291296d4
- Focused review: `git show ee0357134ba6f387d2b4e57eb4acb31d291296d4`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: ee0357134ba6f387d2b4e57eb4acb31d291296d4
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| tray Quit with a GUI open | . | PASS (exit 0) | both processes gone; 0 locks held; GUI exited in 104ms |
| `cargo test --release` | . | PASS (exit 0) | 39 passed |

## Acceptance Criteria
- [x] **AC-24**: The daemon exports its interface and every rendered property is readable.
- [x] **AC-25**: A toggle through the daemon acquires and releases a real logind lock.
- [x] **AC-26**: A second daemon exits cleanly rather than co-owning the locks (**FR-18**).
- [x] **AC-27**: A second window exits rather than opening a duplicate (**FR-19**).
- [x] **AC-28**: A reader observes changes made through another connection (**FR-21**, **NFR-08**).
- [x] **AC-29**: Quitting the daemon closes an open window (**FR-20**).
- [x] **AC-30**: Quitting releases every lock and the daemon exits.
- [x] **AC-31**: A setter's return value agrees with the next reader (**FR-21**).
- [x] **AC-32**: The window starts a daemon when none is running (**FR-17**).
- [x] **AC-33**: `make package` builds both architectures in a container, with a release that supersedes the previous build (**FR-22**, **NFR-09**).

## Phase Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 14b3c77aff8f002d8b386bedbcf566a36605ea9f
- Identity recheck: git rev-parse HEAD, 2026-08-11 11:20, matches recorded revision 14b3c77aff8f002d8b386bedbcf566a36605ea9f
- Final aligned review: Plans/SleepBlock/reviews/03-sleepblock-code-review-a2549e91..d63f2ddd.md; frozen: a2549e91c0f971c108a026ef1f49c9a41c4a69b9..d63f2ddd0c061af0a8f4d27e0c3758a8a7a6d7de

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release -- --test-threads=1` | . | PASS (exit 0) | 43 passed across five suites; 0 failed |
| `cargo clippy --release --all-targets -- -D warnings` | . | PASS (exit 0) | no warnings emitted |
| `cargo fmt --check` | . | PASS (exit 0) | no formatting diff |

### Completed task identities

- `5.1`: `c19526f38fdb943619d49eab9d5e106af405f91c`
- `5.2`: `83e2fb257caca24183d8d43aea84af0ca0b8f35f`
- `5.3`: `4886fa6af79dc15336a9095172462c17d0a4a64c`
- `5.4`: `62b114bc77066a256d4d0953fec80647987e526a`
- `5.5`: `3d2c7ec839aa84296f51d27d644153e06ed9d6d5`
- `5.6`: `34418f1101978216f81388439c9a6d2aaa2f5585`
- `5.7`: `ee0357134ba6f387d2b4e57eb4acb31d291296d4`

The two follow-ups this phase's reviews raised (F-16, F-17) moved to phase 6
rather than being deferred inside a completed phase.

### Completion caveat

This phase was closed on an explicit user decision, not by satisfying the
completion gate. Three four-lane reviews were run; none returned Aligned across
all four lanes. The third pass left `quality-scanner` at Concerning — resting
entirely on a finding this review rejected on evidence (F-24) — and
`blind-spot-finder` at Elevated, on an architectural note answered by
measurement rather than fixed (F-23).

Recorded here rather than papered over: a reader should not infer from
`status: complete` that the gate was met.
