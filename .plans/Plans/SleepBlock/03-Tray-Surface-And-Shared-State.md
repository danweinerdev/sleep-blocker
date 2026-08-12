---
title: "Tray Surface and Shared State"
type: phase
plan: "SleepBlock"
phase: 3
status: complete
created: 2026-08-10
updated: 2026-08-10
deliverable: "A StatusNotifierItem tray icon that is an equal peer of the window, sharing one state handle"
tasks:
  - id: "3.1"
    title: "Add a validated desktop entry"
    status: complete
    verification: "desktop-file-validate exits 0 with no warnings on dist/sleep-block.desktop. Serves FR-14."
    justifies: "Delivers the desktop-entry half of FR-14 and AC-10. Prevents the duplicate menu entry that a second main category would cause."
  - id: "3.2"
    title: "Create two-state tray icons"
    status: complete
    verification: "Both states render legibly at 22px against light and dark backgrounds and are visually distinct; PNGs are 8-bit RGBA. Serves FR-08, NFR-02, NFR-03."
    justifies: "Delivers the icon half of FR-08 and satisfies NFR-02, NFR-03, AC-12. Prevents an idle icon that vanishes against a dark panel."
    depends_on: ["3.1"]
  - id: "3.3"
    title: "Add tray icon sharing state with the window"
    status: complete
    verification: "cargo test --release passes 8 tests; the tray registers with StatusNotifierWatcher, and toggling it changes the published icon, updates the tooltip, and takes a real logind lock. Serves FR-06, FR-08, FR-09, FR-10, FR-13, NFR-01, NFR-06."
    justifies: "Delivers FR-06, FR-08, FR-09, FR-10, FR-13 and satisfies AC-06, AC-07, AC-08, AC-09, AC-15, AC-17, AC-18, AC-20, AC-21, AC-23, NFR-01, NFR-06. Prevents a stale tray icon that contradicts the lock actually held."
    depends_on: ["3.2"]
---

# Phase 3: Tray Surface and Shared State

## Overview

Adds the second surface and, in doing so, forces the state model to change. Once
two surfaces can each toggle, GUI-owned state is untenable, so the inhibitors
move into a shared handle in the core crate. This phase also contains the
subtlest failure in the project — a silently empty tray icon — which is why it
carries the most explicit traps.

## 3.1: Add a validated desktop entry

### Subtasks
- [x] Write the desktop entry with `Exec`, `Categories`, and `Keywords`
- [x] Validate with `desktop-file-validate` and fix all findings

### Notes
Revision boundary: the application can be launched from a desktop menu.

### Trap
`desktop-file-validate` failures are easy to dismiss as pedantry. They are not:
an unregistered `NotShowIn` value is an outright error, and listing two main
categories makes the application appear twice in the menu.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: deebef794ef7119d5d8a58ec303007a037a79042
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision deebef794ef7119d5d8a58ec303007a037a79042
- Focused review: `git show deebef794ef7119d5d8a58ec303007a037a79042`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: deebef794ef7119d5d8a58ec303007a037a79042
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `desktop-file-validate dist/sleep-block.desktop` | . | PASS (exit 0) | no output; no warnings |

## 3.2: Create two-state tray icons

### Subtasks
- [x] Draw active and idle lemniscate SVGs sharing one silhouette
- [x] Differentiate by colour and saturation rather than shape
- [x] Render at 22px and check against both light and dark backgrounds
- [x] Generate PNGs at eight sizes as 8-bit RGBA

### Notes
Revision boundary: both icon states exist, are distinguishable at panel size, and
are in the format the tray decoder accepts.

### Trap
A thin or translucent stroke reads fine on a light background and disappears on a
dark one. The panel theme is not knowable from inside the application, so both
states must be checked against both backgrounds at 22px — not at full size,
where every design looks acceptable.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: f307da466e3685ca49f9e97632175c60bf3043f3
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision f307da466e3685ca49f9e97632175c60bf3043f3
- Focused review: `git show f307da466e3685ca49f9e97632175c60bf3043f3`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: f307da466e3685ca49f9e97632175c60bf3043f3
- Review result: PASS/Aligned

| Tool / inspection | Context | Result | Observable evidence |
|---|---|---|---|
| Rendered comparison at 22px | dark and light backgrounds | PASS | both states legible and mutually distinct |
| `magick identify` | dist/icons | PASS | 8-bit sRGB TrueColorAlpha |

## 3.3: Add tray icon sharing state with the window

### Subtasks
- [x] Move inhibitors into a cloneable `SleepBlock` handle in the core crate
- [x] Implement the `ksni` tray with state-dependent icon, tooltip, and menu
- [x] Add a watcher thread that republishes when shared state changes
- [x] Configure `ksni` for the `async-io` backend to match `zbus`
- [x] Add icon format tests and mutation-check them

### Notes
Revision boundary: both surfaces toggle the same state and both reflect changes
made through the other.

### Trap
Two distinct traps converge here, and both fail silently.

First: `ksni` republishes its properties only when `Handle::update` is called. A
left click arrives as a D-Bus `Activate`, mutating state without triggering a
republish — so the icon keeps showing the previous state while the lock is
genuinely held. A watcher thread is required; calling `update` from inside
`activate` is impossible because the tray does not hold its own handle.

Second: the tray decoder accepts only 8-bit RGBA. ImageMagick writes 16-bit PNGs
by default, and a rejected icon results in an *empty pixmap* rather than an
error — the application runs, the inhibitor works, and the icon is simply
missing. Diagnosing this by looking at the icon is misleading, because the
tooltip updates correctly and suggests the publish path is fine. The icon format
tests exist to convert this into a build failure.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 8a5e549a3535b654c2a6fdfbee5af6346b164b98
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 8a5e549a3535b654c2a6fdfbee5af6346b164b98
- Focused review: `git show 8a5e549a3535b654c2a6fdfbee5af6346b164b98`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 8a5e549a3535b654c2a6fdfbee5af6346b164b98
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release` | . | PASS (exit 0) | 8 passed: 5 inhibitor, 3 icon |
| `busctl --user get-property org.kde.StatusNotifierWatcher /StatusNotifierWatcher org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems` | . | PASS (exit 0) | item registered with the watcher alongside other tray items |
| `busctl --user call <item> /StatusNotifierItem org.kde.StatusNotifierItem Activate ii 0 0` | . | PASS (exit 0) | pixmap hash changes on toggle and returns on second toggle; logind lock count 0→1→0 |

## Acceptance Criteria
- [x] **AC-06**: Every embedded tray icon is 8-bit RGBA.
- [x] **AC-07**: Every embedded icon matches the dimensions in its filename.
- [x] **AC-08**: Active and idle icons are not byte-identical at any size.
- [x] **AC-10**: The desktop entry validates and its `Icon` key names an installed icon.
- [x] **AC-12**: Both icon states are distinguishable and legible on light and dark panels at 22px.
- [x] **AC-09** *(manual)*: With the screen-lock option enabled, the inhibitor appears in KDE's *Power & Battery* tray popup and disappears when disabled. Confirms **FR-04** registration, which no D-Bus probe can assert.
- [x] **AC-15**: The screen-lock inhibitor is acquired only while sleep blocking is held, satisfying **FR-06**.
- [x] **AC-21**: No dependency requires GTK headers or `libdbus`, satisfying **NFR-01**; the `async-io` backend keeps a single executor in-process.
- [x] **AC-23**: Reading state for rendering copies values out under a short-lived lock, satisfying **NFR-06**.
- [x] **AC-17**: Window and tray read and write one shared state handle.
- [x] **AC-18**: The tray menu offers toggle, screen-lock option, and quit.
- [x] **AC-20**: Absence of a StatusNotifierItem host is non-fatal.

## Phase Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 8a5e549a3535b654c2a6fdfbee5af6346b164b98
- Identity recheck: git rev-parse 8a5e549, 2026-08-10 21:05, matches recorded revision 8a5e549a3535b654c2a6fdfbee5af6346b164b98


| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --release` | . | PASS (exit 0) | 8 passed; 0 failed |
| `cargo clippy --release --all-targets -- -D warnings` | . | PASS (exit 0) | no warnings |

### Completed task identities

- `3.1`: `deebef794ef7119d5d8a58ec303007a037a79042`
- `3.2`: `f307da466e3685ca49f9e97632175c60bf3043f3`
- `3.3`: `8a5e549a3535b654c2a6fdfbee5af6346b164b98`
