---
title: "Core Inhibitor Mechanism"
type: phase
plan: "SleepBlock"
phase: 1
status: complete
created: 2026-08-10
updated: 2026-08-10
deliverable: "A GUI-free crate that acquires and releases both inhibitors, with integration tests against live D-Bus services"
tasks:
  - id: "1.1"
    title: "Scaffold workspace with licence and editor config"
    status: complete
    verification: "Repository contains LICENSE, .editorconfig and .gitignore; no build target exists yet. Serves FR-15, NFR-07."
    justifies: "Establishes the MIT licensing the package metadata later declares (FR-15) and fixes formatting conventions before any source lands, so NFR-07's fmt gate has a defined target."
  - id: "1.2"
    title: "Implement Inhibitor and ScreenInhibitor with integration tests"
    status: complete
    verification: "cargo test --release passes 5 integration tests: lock appears in systemd-inhibit --list while held and is gone after drop; mode is block covering sleep and idle; multiple inhibitors independent; screen inhibitor acquires/releases; screen inhibitors overlap. Serves FR-01, FR-02, FR-03, FR-04, FR-05."
    justifies: "Delivers FR-01, FR-02, FR-03, FR-04, FR-05 and proves AC-01, AC-02, AC-03, AC-04, AC-05, AC-14. Prevents the specific failure of a delay-mode lock that would silently fail to block suspend."
    depends_on: ["1.1"]
---

# Phase 1: Core Inhibitor Mechanism

## Overview

Builds the mechanism and proves it works before any interface exists to display
it. This ordering is forced by observability: the core is testable headlessly
against live logind, whereas neither surface is testable in this environment.
Everything downstream depends on this being correct.

## 1.1: Scaffold workspace with licence and editor config

### Subtasks
- [x] Add MIT `LICENSE`
- [x] Add `.editorconfig` with UTF-8, LF, and per-filetype indentation
- [x] Add `.gitignore` excluding build output

### Notes
Revision boundary: repository carries its licence and formatting conventions.
No Cargo project exists at this revision, so `cargo build` has nothing to build —
this is expected and not a broken tree.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: bb60d3438f5958d583633491a9b7436e617b8c99
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision bb60d3438f5958d583633491a9b7436e617b8c99
- Focused review: `git show bb60d3438f5958d583633491a9b7436e617b8c99`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: bb60d3438f5958d583633491a9b7436e617b8c99
- Review result: PASS/Aligned

| Tool / inspection | Context | Result | Observable evidence |
|---|---|---|---|
| File inspection | . | PASS | LICENSE, .editorconfig, .gitignore present at this revision |

## 1.2: Implement Inhibitor and ScreenInhibitor with integration tests

### Subtasks
- [x] Add `zbus` dependency with default features
- [x] Implement `Inhibitor` calling `login1.Manager.Inhibit` with `idle:sleep` / `block`, holding the returned descriptor
- [x] Implement `ScreenInhibitor` calling `ScreenSaver.Inhibit`, releasing its cookie via `Drop`
- [x] Write 5 integration tests asserting observable effects against live services
- [x] Mutation-check the assertions by flipping `block` to `delay` and confirming failure

### Notes
Revision boundary: both inhibitor mechanisms work and are proven against real
services. The crate has no GUI dependency, which is what makes these tests
runnable at all.

### Trap
The tempting shortcut is to assert only that the D-Bus calls return `Ok`. That
passes even when the lock has no effect — a `delay`-mode lock returns success
and still permits suspend. The tests must assert the *observable system state*
(`systemd-inhibit --list`), not the call's return value.

A second trap: writing these tests against mocks. The behaviour worth testing is
the interaction with logind itself, so a mock would only confirm the code calls
what it was written to call.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 553f02db97710586570fcd1e1bab5b773eac7cb9
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 553f02db97710586570fcd1e1bab5b773eac7cb9
- Focused review: `git show 553f02db97710586570fcd1e1bab5b773eac7cb9`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 553f02db97710586570fcd1e1bab5b773eac7cb9
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release -- --test-threads=1` | . | PASS (exit 0) | 5 passed; 0 failed |
| `cargo build -p sleep-block-core --release` | . | PASS (exit 0) | core crate builds standalone |

## Acceptance Criteria
- [x] **AC-01**: Lock appears in `systemd-inhibit --list` while held and is released on drop.
- [x] **AC-02**: Registered lock reports mode `block` covering `sleep` and `idle`.
- [x] **AC-03**: Concurrent sleep inhibitors are independent.
- [x] **AC-04**: Screen-lock inhibitor acquires and releases without error.
- [x] **AC-05**: Concurrent screen-lock inhibitors release in any order.
- [x] **AC-14**: The registered lock carries `WHO` of `sleep-block`.

## Phase Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 553f02db97710586570fcd1e1bab5b773eac7cb9
- Identity recheck: git rev-parse 553f02d, 2026-08-10 21:05, matches recorded revision 553f02db97710586570fcd1e1bab5b773eac7cb9


| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release -- --test-threads=1` | . | PASS (exit 0) | 5 passed; 0 failed |
| `cargo clippy --release --all-targets` | . | PASS (exit 0) | no warnings |

### Completed task identities

- `1.1`: `bb60d3438f5958d583633491a9b7436e617b8c99`
- `1.2`: `553f02db97710586570fcd1e1bab5b773eac7cb9`
