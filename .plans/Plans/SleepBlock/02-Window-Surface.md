---
title: "Window Surface"
type: phase
plan: "SleepBlock"
phase: 2
status: in-progress
created: 2026-08-10
updated: 2026-08-10
deliverable: "An egui window exposing the sleep toggle and the optional screen-lock setting"
tasks:
  - id: "2.1"
    title: "Add egui window with sleep toggle and screen-lock checkbox"
    status: complete
    verification: "cargo build --release succeeds; the window presents a circular toggle and an off-by-default checkbox; a screen-lock failure leaves sleep blocking intact. Serves FR-07, FR-11, FR-12, NFR-04."
    justifies: "Delivers FR-07, FR-11, FR-12 and the opt-in default in FR-04, and satisfies AC-13, AC-16, AC-19. Prevents the failure where a secondary screen-lock error silently drops the primary sleep lock."
  - id: "2.2"
    title: "Document the sleep versus screen-lock distinction"
    status: complete
    verification: "README states that logind exposes no lock inhibitor type, that lid-close is not covered, and which behaviours the app does and does not block. Serves FR-05, FR-06."
    justifies: "Prevents the recurring misunderstanding that one mechanism covers both, which would lead a maintainer to fold ScreenInhibitor into Inhibitor and break FR-05's explicit release."
    depends_on: ["2.1"]
  - id: "2.3"
    title: "Fill the window frame and pin its size"
    status: complete
    verification: "cargo build --release succeeds; the panel frame fills the viewport leaving no unpainted region, and min and max inner size are both pinned. Serves FR-07."
    justifies: "Fixes an unpainted strip below the last control, and prevents a resize from exposing it again. Serves FR-07 by making the window's presentation correct rather than merely present."
    depends_on: ["2.2"]
---

# Phase 2: Window Surface

## Overview

Puts a usable interface on the mechanism from Phase 1. The window owns the
inhibitors at this stage; that ownership is deliberately replaced in Phase 3 once
a second surface exists. Building it this way first kept the state model as
simple as the requirements then demanded.

## 2.1: Add egui window with sleep toggle and screen-lock checkbox

### Subtasks
- [x] Add `eframe` with default features disabled, selecting `glow`, `wayland`, `x11`
- [x] Render a circular indicator that acts as the toggle control
- [x] Add an "Also keep screen on" checkbox, defaulting to off
- [x] Surface acquisition errors inline
- [x] Keep sleep blocking held when the screen-lock acquisition fails

### Notes
Revision boundary: the application is usable — sleep can be toggled and the
screen-lock option enabled, with failures reported rather than swallowed.

### Trap
The obvious modelling error is a separate `bool` for "is inhibiting" alongside
the inhibitor itself. The two can then disagree, showing green while holding no
lock. `Option<Inhibitor>` is the single source of truth precisely so that state
cannot drift.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 70acc5209d1107b73d135dc2c9d3701618663a34
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 70acc5209d1107b73d135dc2c9d3701618663a34
- Focused review: `git show 70acc5209d1107b73d135dc2c9d3701618663a34`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 70acc5209d1107b73d135dc2c9d3701618663a34
- Review result: PASS

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo build --release` | . | PASS (exit 0) | binary builds |

## 2.2: Document the sleep versus screen-lock distinction

### Subtasks
- [x] Explain that sleep and screen lock are separate mechanisms on separate buses
- [x] Record that lid-close suspend is deliberately not blocked, and why
- [x] Tabulate which behaviours are and are not blocked

### Notes
Revision boundary: the distinction that governs the whole design is written down
where a maintainer will find it.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 18801ae1d8880344e3856d907e2391a8d0c59397
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 18801ae1d8880344e3856d907e2391a8d0c59397
- Focused review: `git show 18801ae1d8880344e3856d907e2391a8d0c59397`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 18801ae1d8880344e3856d907e2391a8d0c59397
- Review result: PASS

| Tool / inspection | Context | Result | Observable evidence |
|---|---|---|---|
| Document inspection | README.md | PASS | Sleep-vs-screen-lock section and blocked-behaviour table present |

## 2.3: Fill the window frame and pin its size

### Subtasks
- [x] Expand the central panel frame to the available size
- [x] Pin min and max inner size and disable resizing
- [x] Size the window for its tallest state, including a visible error line

### Notes
Revision boundary: the window renders correctly with no unpainted region and
cannot be resized into one.

### Trap
Setting `resizable(false)` alone is insufficient — some compositors still permit
an edge drag. Both `min_inner_size` and `max_inner_size` must be pinned.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 025c10c6e3c57569a531baa69509576c167b05ca
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 025c10c6e3c57569a531baa69509576c167b05ca
- Focused review: `git show 025c10c6e3c57569a531baa69509576c167b05ca`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 025c10c6e3c57569a531baa69509576c167b05ca
- Review result: PASS

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo build --release` | . | PASS (exit 0) | binary builds |
| `./target/release/sleep-block` | . | PASS (exit 0) | user confirmed the black bar is gone and the window is fixed-size |

## Acceptance Criteria
- [x] **AC-13**: The indicator conveys state through shape and text as well as colour, satisfying **NFR-04**.
- [x] **AC-16**: The window presents a circular indicator and a labelled screen-lock checkbox.
- [x] **AC-19**: A failed acquisition leaves prior state unchanged and records the reason.

## Phase Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 025c10c6e3c57569a531baa69509576c167b05ca
- Identity recheck: git rev-parse 025c10c, 2026-08-10 21:05, matches recorded revision 025c10c6e3c57569a531baa69509576c167b05ca


### Completed task identities

- `2.1`: `70acc5209d1107b73d135dc2c9d3701618663a34`
- `2.2`: `18801ae1d8880344e3856d907e2391a8d0c59397`
- `2.3`: `025c10c6e3c57569a531baa69509576c167b05ca`

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo build --release` | . | PASS (exit 0) | binary builds |
| `cargo test --release` | . | PASS (exit 0) | 5 passed; 0 failed |
