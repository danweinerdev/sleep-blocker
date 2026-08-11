---
title: "Sleep Block — Desktop Sleep Inhibitor"
type: spec
status: approved
created: 2026-08-10
updated: 2026-08-10
tags: [linux, desktop, dbus, systemd, gui, rust]
related: [Designs/SleepBlock, Plans/SleepBlock]
---

# Sleep Block — Desktop Sleep Inhibitor

## Overview

A small GUI utility that prevents a Linux system from going to sleep while it is
running and switched on. The user toggles the inhibitor by clicking an indicator
that is green while the system is being kept awake and red while it may suspend
normally. An optional, off-by-default setting additionally blocks screen
blanking and locking.

This specification is **retrospective**: it describes a feature that has already
been built and verified. It is written to record the requirements the
implementation actually satisfies, and — equally — to record where verification
is manual rather than automated, so later changes do not assume coverage that
does not exist.

## Goals

- Prevent system suspend and idle-triggered sleep for as long as the user asks.
- Make the current state obvious at a glance, from both a window and a tray icon.
- Release every lock promptly and reliably, including on abnormal exit.
- Offer screen-lock inhibition as a separate, explicitly opt-in concern.
- Install as a native package with a desktop entry and themed icons.

## Non-Goals

- **Blocking lid-close suspend.** The desktop environment owns the lid switch
  (on KDE, PowerDevil holds a `handle-lid-switch` inhibitor). Overriding it would
  fight the user's own configured power settings rather than serve them.
- **Faking input events.** The utility uses the documented inhibitor APIs rather
  than synthesising activity, so its locks are visible to the system and vanish
  when it exits.
- **Scheduling or timed auto-release.** The inhibitor is on until the user turns
  it off.
- **Cross-platform support.** The mechanisms are Linux-specific.
- **Persisting state across restarts.** Launching the app starts in the
  not-inhibiting state.

## Requirements

External contracts pinned below. Both are stable freedesktop interfaces; the
authoritative sources are the systemd and freedesktop specifications.

| Contract | Interface | Source | As-of |
|---|---|---|---|
| System sleep inhibition | `org.freedesktop.login1.Manager.Inhibit` | systemd `logind` D-Bus API, `org.freedesktop.login1(5)` | 2026-08-10 |
| Screen lock inhibition | `org.freedesktop.ScreenSaver.Inhibit` / `.UnInhibit` | freedesktop ScreenSaver interface, as implemented by kwin on KDE | 2026-08-10 |
| Tray icon | StatusNotifierItem | freedesktop StatusNotifierItem specification | 2026-08-10 |

The two inhibition mechanisms are **distinct and cannot be unified**: `logind`
exposes no lock-related inhibitor type, so screen blanking must be addressed
through a different interface on a different bus. Their release semantics also
differ, which is the single most error-prone aspect of this feature (see
**FR-03** and **FR-05**).

### Functional Requirements

- **FR-01**: The utility SHALL block system sleep by calling
  `org.freedesktop.login1.Manager.Inhibit` on the **system** bus with
  `What="idle:sleep"` and `Mode="block"`. `idle` covers the idle timeout that
  leads to automatic suspend; `sleep` covers explicit suspend and hibernate.
  `block` prevents the transition outright, as opposed to `delay`, which only
  postpones it.
- **FR-02**: The utility SHALL identify itself to `logind` with a `Who` of
  `sleep-block` and a human-readable `Why`, so the lock is attributable in
  `systemd-inhibit --list`.
- **FR-03**: The sleep inhibitor SHALL be released by closing the file
  descriptor returned by `Inhibit`. The lock's lifetime SHALL be exactly the
  lifetime of that descriptor, so process exit — including abnormal termination —
  releases it without requiring an explicit teardown call.
- **FR-04**: The utility SHALL optionally block screen blanking and locking by
  calling `org.freedesktop.ScreenSaver.Inhibit` on the **session** bus. This
  setting SHALL default to off, on the assumption that letting monitors sleep is
  normally desired even while the machine stays awake.
- **FR-05**: The screen-lock inhibitor SHALL be released by calling
  `UnInhibit` with the cookie returned by `Inhibit`. Because this cookie is not
  self-releasing, the utility SHALL invoke `UnInhibit` deterministically when the
  inhibitor is dropped.
- **FR-06**: The screen-lock inhibitor SHALL never be held while the sleep
  inhibitor is not held. It is an addition to sleep blocking, not an independent
  mode.
- **FR-07**: The utility SHALL present a window containing a circular indicator
  that acts as the toggle control, plus a labelled checkbox for the screen-lock
  option.
- **FR-08**: The utility SHALL present a StatusNotifierItem tray icon whose
  appearance reflects the current sleep-blocking state, and which toggles that
  state when activated.
- **FR-09**: The window and the tray SHALL be equivalent peers: a change made
  through either SHALL be reflected in the other, with a single shared source of
  truth for the inhibitor state rather than per-surface copies.
- **FR-10**: The tray icon SHALL offer a menu providing the sleep toggle, the
  screen-lock option, and an explicit quit action that releases all locks before
  exiting.
- **FR-11**: When acquiring an inhibitor fails, the utility SHALL surface the
  reason to the user and SHALL leave the previous state unchanged rather than
  entering a partially-applied state.
- **FR-12**: A failure to acquire the screen-lock inhibitor SHALL NOT release or
  prevent sleep blocking, which is the primary function.
- **FR-13**: If no StatusNotifierItem host is available, the utility SHALL
  continue to run with the window alone rather than treating this as fatal.
- **FR-14**: The utility SHALL install a desktop entry and themed icons such
  that it is launchable from a desktop application menu.
- **FR-15**: The utility SHALL be installable as a binary RPM package.
- **FR-16**: The inhibitors and the tray icon SHALL be owned by a background
  process (the daemon) rather than by the window, so that closing the window
  releases nothing.
- **FR-17**: The window SHALL be a client of the daemon, holding no inhibitors
  itself, and SHALL start a daemon if none is running so it can be launched
  directly from a menu.
- **FR-18**: At most one daemon SHALL run per session, and at most one window.
  A second instance of either SHALL exit rather than compete.
- **FR-19**: Closing the window with hide-on-close enabled SHALL leave the
  daemon running; the tray SHALL offer to open a new window.
- **FR-20**: The window SHALL close when its daemon exits, whether that exit was
  requested or a crash — every control it offers is a request to that daemon.
- **FR-21**: A change made through any surface SHALL become visible in the other
  within approximately one second, without user interaction.
- **FR-22**: Each build from a distinct source revision SHALL produce a package
  that the package manager treats as an upgrade over the previous one.

### Non-Functional Requirements

- **NFR-01**: The build SHALL NOT require system development packages — in
  particular no GTK headers and no `libdbus` — so it compiles on a machine with
  only a Rust toolchain installed.
- **NFR-02**: The two tray icon states SHALL be distinguishable at 22 px, the
  common panel size.
- **NFR-03**: Both tray icon states SHALL remain legible against both light and
  dark panel backgrounds, since the panel theme is not knowable from within the
  application.
- **NFR-04**: The window and tray states SHALL be distinguishable by more than
  hue alone, so the interface remains usable without colour vision.
- **NFR-05**: The release build SHALL apply link-time optimisation and strip
  symbols to limit the size of the statically-linked GPU and font stack.
- **NFR-06**: Reading the current state for rendering SHALL NOT hold a lock
  across a repaint, so the UI cannot stall on contention.
- **NFR-07**: The packaging path SHALL gate on the test suite, lint, and format
  checks, so a regression in any of them fails the package rather than shipping.
- **NFR-08**: The daemon's D-Bus interface SHALL announce every property change,
  so a polling client observes changes made through another connection rather
  than serving a stale cache.
- **NFR-09**: The build SHALL be reproducible in a container, independent of
  what is installed on the host, and SHALL be able to produce packages for both
  x86_64 and aarch64.

## User Stories

- As someone running a long build, I want to stop my machine suspending
  mid-task, so that the work completes while I am away from the keyboard.
- As someone watching a video, I want to prevent both sleep and the screen
  locking, so that playback is not interrupted.
- As someone who normally wants the monitors to sleep, I want screen-lock
  blocking to be opt-in, so that enabling the inhibitor does not needlessly keep
  my displays lit.
- As a user with the window closed, I want to toggle the inhibitor from the tray
  icon and see its state there, so that I do not need to keep a window open.
- As a user of a desktop application menu, I want the utility to appear with a
  recognisable icon, so that I can launch it like any other application.

## Acceptance Criteria

Criteria are marked with how they are verified. This distinction is deliberate:
**AC-09** cannot be checked automatically, and recording that prevents a later
change from assuming otherwise.

### Automatically verified

- [x] **AC-01**: *(automated)* — While the sleep inhibitor is held, a lock owned
  by `sleep-block` appears in `systemd-inhibit --list`; after it is dropped, the
  lock count returns to its prior value. Satisfies **FR-01**, **FR-03**.
- [x] **AC-02**: *(automated)* — The registered lock reports mode `block` and
  covers both `sleep` and `idle`. A `delay`-mode lock would not prevent suspend,
  so this is asserted explicitly. Satisfies **FR-01**.
- [x] **AC-03**: *(automated)* — Two concurrently-held sleep inhibitors are
  independent — releasing one does not release the other. Satisfies **FR-03**,
  and underpins **FR-12**.
- [x] **AC-04**: *(automated)* — Acquiring and releasing the screen-lock inhibitor
  completes without error against a live `org.freedesktop.ScreenSaver` provider.
  Satisfies **FR-04**, **FR-05**.
- [x] **AC-05**: *(automated)* — Two concurrent screen-lock inhibitors can be held
  and released in any order. Satisfies **FR-05**.
- [x] **AC-06**: *(automated)* — Every embedded tray icon is 8-bit RGBA. Any other
  format is rejected by the decoder and results in no icon being published —
  a silent failure, hence an explicit guard. Satisfies **FR-08**.
- [x] **AC-07**: *(automated)* — Every embedded tray icon has the pixel dimensions
  its filename declares. Satisfies **FR-08**.
- [x] **AC-08**: *(automated)* — The active and idle tray icons are not byte-identical
  at any embedded size. Satisfies **FR-08**, **NFR-02**.
- [x] **AC-10**: *(automated)* — The installed desktop entry passes
  `desktop-file-validate`, and its `Icon` key names an icon the package installs
  into the hicolor theme. Satisfies **FR-14**.
- [x] **AC-11**: *(automated)* — `make package` produces an installable binary RPM,
  and fails if the test suite, `clippy -D warnings`, `cargo fmt --check`, or
  desktop-entry validation fails. Satisfies **FR-15**, **NFR-07**.
- [x] **AC-24**: *(automated)* — The daemon exports its interface and every
  property the window renders from is readable. Satisfies **FR-16**.
- [x] **AC-25**: *(automated)* — A toggle through the daemon acquires a real
  logind lock and releases it, with no window running. Satisfies **FR-16**.
- [x] **AC-26**: *(automated)* — A second daemon on the same bus name exits
  cleanly and explains why, rather than co-owning the locks and the tray.
  Satisfies **FR-18**.
- [x] **AC-27**: *(automated)* — A second window exits rather than opening a
  duplicate, and a running window owns a bus name that is released when it
  exits. Satisfies **FR-18**, and is what stops "Show window" stacking up
  processes (**FR-19**).
- [x] **AC-28**: *(automated)* — A long-lived reader observes a change made
  through a *different* connection, for both the sleep lock and the screen-lock
  preference. Driving both through one connection would refresh the cache as a
  side effect and hide the failure. Satisfies **FR-21**, **NFR-08**.
- [x] **AC-29**: *(automated)* — Quitting the daemon closes an open window.
  Satisfies **FR-20**.
- [x] **AC-30**: *(automated)* — Quitting releases every lock promptly and the
  daemon process actually exits. Satisfies **FR-16**.
- [x] **AC-31**: *(automated)* — A setter's return value agrees with what the
  next reader sees, so the two surfaces cannot disagree about one setting.
  Satisfies **FR-21**.
- [x] **AC-14**: *(automated)* — The lock registered with `logind` is attributable
  to this application: the entry in `systemd-inhibit --list` carries a `WHO` of
  `sleep-block`. Asserted by the same test that filters the listing by owner.
  Satisfies **FR-02**.

### Manually verified

- [x] **AC-09**: *(manual — no automated check possible)* — With the screen-lock
  option enabled, the inhibitor is registered by the desktop and appears in
  KDE's *Power & Battery* tray popup as a blocking application; it disappears
  when the option is disabled. Satisfies **FR-04**.

  This cannot be automated: `org.freedesktop.ScreenSaver` exposes only `Inhibit`
  and `UnInhibit` with no readback, and PowerDevil's `HasInhibition` and
  (deprecated) `ListInhibitions` do not report kwin-held inhibitors. **AC-04**
  confirms only that the calls succeed, not that the desktop honours them.
  Re-check this by hand after any change to the screen-lock path.

### Verified by inspection

Inspection means a human confirmed it by reading the code or looking at the
running application. These are real checks, but nothing re-runs them: a
refactor can break any of them silently. Where that risk is material it is
noted against the criterion.

- [x] **AC-12**: *(inspection)* — The two tray icon states remain distinguishable
  from one another and legible against both light and dark backgrounds when
  rendered at 22 px. Satisfies **NFR-02**, **NFR-03**.
- [x] **AC-13**: *(inspection)* — The window's indicator conveys its state through
  shape and accompanying text as well as colour. Satisfies **NFR-04**.
- [x] **AC-15**: *(automated)* — The screen-lock inhibitor is acquired only when
  sleep blocking is already held; enabling the preference alone records it
  without taking a lock, and the preference survives a full toggle cycle.
  Satisfies **FR-06**.
- [x] **AC-16**: *(inspection)* — The window presents a circular indicator acting
  as the toggle control and a labelled checkbox for the screen-lock option.
  Satisfies **FR-07**.
- [x] **AC-17**: *(inspection)* — The window and tray read from and write to one
  shared state handle rather than per-surface copies, and the tray republishes
  when that state changes. Satisfies **FR-09**. *Refactor risk: no test exercises
  a round trip between the two surfaces; the synchronisation was confirmed by
  driving the tray over D-Bus and observing the published state change.*
- [x] **AC-18**: *(automated)* — The tray menu always offers the sleep toggle,
  the screen-lock option, and quit; the toggle's label tracks the current state;
  and "Show window" appears first only when hide-on-close is enabled. Satisfies
  **FR-10**.
- [x] **AC-19**: *(inspection)* — A failed acquisition leaves the prior state
  unchanged and records the reason for display, rather than applying a partial
  transition. Satisfies **FR-11**, **FR-12**.
- [x] **AC-20**: *(inspection)* — Failure to reach a StatusNotifierItem host is
  reported and the application continues with the window alone. Satisfies
  **FR-13**.
- [x] **AC-21**: *(inspection)* — No dependency requires GTK headers or `libdbus`;
  the GUI, D-Bus, and tray libraries are all pure Rust. Satisfies **NFR-01**.
  *Refactor risk: adding a C-backed dependency would violate this silently.*
- [x] **AC-22**: *(inspection)* — The release profile applies fat link-time
  optimisation and strips symbols. Satisfies **NFR-05**.
- [x] **AC-32**: *(inspection)* — The window starts a daemon when none is
  running, so it can be launched directly. Satisfies **FR-17**.
- [x] **AC-33**: *(inspection)* — `make package` builds in a container and
  produces packages for both architectures; the release field carries the commit
  distance and hash so each build supersedes the last. Satisfies **FR-22**,
  **NFR-09**. *Verified by running it and comparing with `rpm.vercmp`, but
  nothing re-checks it.*
- [x] **AC-23**: *(inspection)* — Reading state for rendering copies the values out
  under a short-lived lock and returns them by value, so no lock is held across
  a repaint. Satisfies **NFR-06**.

## Constraints

- **C-01**: Pure-Rust dependencies only, per **NFR-01**: `eframe`/`egui` for the
  GUI, `zbus` for D-Bus, and `ksni` for the tray. GTK-based tray crates and the
  `libdbus`-backed `dbus` crate are excluded by this constraint.
- **C-02**: A single async runtime. `zbus` uses the `async-io` backend by
  default, so the tray library is configured to match; selecting a different
  backend would run a second executor in the same process for no benefit.
- **C-03**: The utility depends on `systemd-logind` at runtime. Without it the
  application starts but can never acquire a sleep lock.
- **C-04**: Screen-lock inhibition depends on a running provider of
  `org.freedesktop.ScreenSaver`. Its absence is a normal condition on some
  desktops and must degrade gracefully.
- **C-05**: The tray requires a StatusNotifierItem host. Its absence must not
  prevent the application from running (**FR-13**).

## Dependencies

- `systemd-logind` — system sleep inhibition (**FR-01**).
- A `org.freedesktop.ScreenSaver` provider, e.g. kwin — screen lock inhibition
  (**FR-04**).
- A StatusNotifierItem host, e.g. the Plasma system tray — tray icon (**FR-08**).
- `hicolor-icon-theme` — icon installation target (**FR-14**).
- ImageMagick — regenerating PNG icons from SVG sources; build-time only, and not
  required to compile, since the generated PNGs are committed.

## Open Questions

- Should lid-close suspend be blockable behind an additional opt-in? — **non-blocking** — the stated requirements hold regardless; adding it would be a new, separately-specified opt-in rather than a change to any requirement above.
- Should the inhibitor state persist across restarts? — **non-blocking** — no stated requirement depends on the answer; persistence would be additive.
- Should packaging target distributions beyond a local binary RPM, such as a source RPM built in mock or COPR? — **non-blocking** — FR-15 and AC-11 are satisfied by the binary RPM; broader targets would be an addition rather than a revision.
