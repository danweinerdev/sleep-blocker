---
title: "Sleep Block — Architecture and Design"
type: design
status: approved
created: 2026-08-10
updated: 2026-08-10
tags: [linux, desktop, dbus, systemd, gui, rust, architecture]
related: [Specs/SleepBlock, Plans/SleepBlock]
---

# Sleep Block — Architecture and Design

## Overview

Sleep Block is a two-crate Rust workspace producing **two binaries**:

- `sleep-blockd` — the daemon. Owns every inhibitor and the tray icon.
- `sleep-block` — the window. Owns nothing; a client of the daemon over D-Bus.

The process split is not organisational preference. On Wayland a window cannot
hide itself: `set_visible` is an empty function, un-minimising is explicitly
ignored, and `focus_window` does nothing. So a tray icon has no way to bring a
hidden window back within one process. Making the window a separate, disposable
process turns "show the window" into "start the window", which needs no
cooperation from the compositor at all.

That forces the ownership split: a logind inhibitor lives exactly as long as its
file descriptor, so a window holding one would release it the moment it closed —
precisely what hide-to-tray must not do. The daemon therefore owns the locks and
the window holds none.

`sleep-block-core` remains GUI-free, which is what keeps the mechanism testable
headlessly (**AC-01**–**AC-05**).

This design is retrospective and documents the system as built.

## Non-Goals

- **No abstraction over the two inhibitor mechanisms.** They differ in bus,
  interface, and release semantics (**FR-03** vs **FR-05**). A unifying trait
  would hide exactly the distinction that causes bugs here, so the two types stay
  separate and are composed by the state layer instead.
- **No notification/observer machinery in `SleepBlock`.** Only the tray needs to
  learn about out-of-band changes, and it can poll cheaply. Building a general
  subscription mechanism for one consumer would be unearned generality.
- **No persistence layer.** Per the spec's non-goals, state is process-lifetime
  only.
- **No plugin or backend abstraction for desktops.** The ScreenSaver interface is
  a freedesktop standard; desktops that implement it work, and those that do not
  degrade per **FR-13**/**C-04**.

## Architecture

### Components

```mermaid
graph TD
    subgraph daemon["sleep-blockd (owns everything)"]
        Service[Service<br/>D-Bus interface]
        Tray[SleepTray<br/>StatusNotifierItem]
        Watcher[watcher thread<br/>republishes the tray]
        State[SleepBlock<br/>Arc-Mutex state]
        Inhibitor[Inhibitor<br/>logind, system bus]
        Screen[ScreenInhibitor<br/>ScreenSaver, session bus]
    end

    subgraph gui["sleep-block (owns nothing)"]
        App[App<br/>egui window]
        Client[DaemonClient<br/>uncached proxy]
        Waker[waker thread<br/>forces repaints]
    end

    Logind[["systemd-logind"]]
    Saver[["kwin / screen locker"]]
    Host[["StatusNotifierItem host"]]

    App --> Client
    Waker -.->|request_repaint| App
    Client -->|"D-Bus: net.phantomnet.SleepBlock1"| Service
    Service --> State
    Tray --> State
    Watcher --> State
    Watcher -.->|Handle::update| Tray
    State --> Inhibitor
    State --> Screen
    Inhibitor -->|Inhibit → fd| Logind
    Screen -->|Inhibit → cookie| Saver
    Tray -.->|publishes icon| Host
    Service -.->|spawns| App

```

`SleepBlock` remains the single source of truth (**FR-09**), but it now lives
only in the daemon. The window reaches it over D-Bus and caches nothing
authoritative — the daemon is always asked.

### Data Flow

A toggle originating from either surface follows the same path, which is what
makes the two peers rather than a primary and a mirror:

```mermaid
sequenceDiagram
    participant U as User
    participant S as Surface (window or tray)
    participant B as SleepBlock
    participant L as logind
    participant SS as ScreenSaver

    U->>S: click / activate
    S->>B: toggle()
    activate B
    alt currently inhibiting
        B->>B: drop Inhibitor (fd closes)
        B->>SS: UnInhibit(cookie) via Drop
    else not inhibiting
        B->>L: Inhibit("idle:sleep", "block")
        L-->>B: file descriptor
        opt keep_screen_awake
            B->>SS: Inhibit(app, reason)
            SS-->>B: cookie
        end
    end
    B-->>S: Status
    deactivate B
    S->>S: render new state
```

Propagation between the two surfaces is where this design has failed most
often, so it is worth stating precisely. Both directions are **polling**, not
push:

```mermaid
flowchart LR
    subgraph d["Daemon → tray"]
        DS[state changes] --> DW["watcher thread<br/>polls every 250ms"] --> DU["Handle::update"] --> DP[tray republishes]
    end
    subgraph g["Daemon → window"]
        GS[state changes] --> GW["window reads properties<br/>every frame, uncached"] --> GR[window repaints]
    end
```

Two mechanisms make the window half work, and both were bugs before they were
features:

- **Uncached reads.** zbus caches properties and refreshes them from
  `PropertiesChanged`. When that refresh does not land, the window serves its
  first reading forever while every other surface is correct. The window
  re-reads every frame anyway, so the cache saved nothing and cost correctness
  (**NFR-08**).
- **A waker thread.** `request_repaint_after` only schedules a timeout, which an
  unfocused window may not service promptly — and the window is unfocused by
  definition while the user is clicking the tray. A thread calling
  `request_repaint` unconditionally cannot be starved that way.

### Interfaces

`sleep-block-core` exposes four types plus the IPC contract:

| Item | Role |
|---|---|
| `Inhibitor` | Holds a sleep lock. Releases on drop by closing its descriptor. |
| `ScreenInhibitor` | Holds a screen-lock cookie. Releases on drop via `UnInhibit`. |
| `SleepBlock` | Cloneable handle to the state. `toggle`, `set_keep_screen_awake`, `set_keep_running_in_tray`, `snapshot`, `release_all`. |
| `Status` | Plain copyable snapshot: `sleep_blocked`, `screen_blocked`, `keep_screen_awake`, `keep_running_in_tray`. |
| `ipc` | The D-Bus contract both binaries share: the bus and object names, and the client proxy. Lives here so neither binary can drift from the other's idea of the interface. |

`snapshot` returns `Status` **by value** so no lock is held across a repaint
(**NFR-06**). Exposing the guard would have made that mistake easy to make.

### Requirement Realisation

Where each requirement is realised in this design:

| Requirement | Realised by |
|---|---|
| **FR-01**, **FR-02** | `Inhibitor` — `login1.Manager.Inhibit` with `idle:sleep` / `block`, identifying as `sleep-block`. |
| **FR-03** | `Inhibitor` holds the returned descriptor; no explicit release path exists, so exit always releases. |
| **FR-04**, **FR-05** | `ScreenInhibitor` — `ScreenSaver.Inhibit`, with `Drop` calling `UnInhibit`. |
| **FR-06** | `SleepBlock` guards screen acquisition on the sleep inhibitor being held. |
| **FR-07** | `App` — circular indicator plus screen-lock checkbox. |
| **FR-08** | `SleepTray` — state-dependent `icon_pixmap`, toggling on `activate`. |
| **FR-09** | `SleepBlock` as the single shared handle; see Decision 3. |
| **FR-10** | `SleepTray::menu` — toggle, screen-lock checkmark, and quit that calls `release_all`. |
| **FR-11**, **FR-12** | Error table below: failures leave prior state intact; screen-lock failure retains sleep blocking. |
| **FR-13** | `SleepTray::start` returns `None` and the window continues alone. |
| **FR-14**, **FR-15** | Packaging: desktop entry plus hicolor icons, staged into a binary RPM. |
| **FR-16** | `sleep-blockd` owns `SleepBlock`, the tray, and both inhibitors. |
| **FR-17** | `DaemonClient::connect` starts a daemon when none answers. |
| **FR-18** | Bus-name ownership: `net.phantomnet.SleepBlock1` for the daemon, `…​.Gui` for the window; the loser exits. |
| **FR-19** | Closing the window exits that process only; the tray's "Show window" starts a new one. |
| **FR-20** | A failed property read marks the daemon gone and the window closes. |
| **FR-21** | Uncached property reads plus the waker thread; see the propagation diagram. |
| **FR-22** | `Release` derived from `git describe`: `1` on a tag, `1.<commits>.git<sha>` after it. |
| **NFR-01** | Pure-Rust dependency set; see Decision 5 and the Constraints in the spec. |
| **NFR-02**, **NFR-03** | Two icon states differing in colour and saturation, verified at 22px on both backgrounds. |
| **NFR-04** | Indicator uses fill/outline and text alongside colour. |
| **NFR-05** | Release profile: fat LTO, `codegen-units = 1`, symbols stripped. |
| **NFR-06** | `snapshot` returns `Status` by value; no guard escapes. |
| **NFR-07** | `make check` gates `make package` on tests, clippy, fmt, and desktop validation. |
| **NFR-08** | `toggle` and `set_keep_screen_awake` announce all four properties, since either can move several at once; `set_keep_running_in_tray` relies on zbus's own setter signal, which suffices because it changes only itself. The window reads uncached besides. |
| **NFR-09** | `Containerfile` carries both toolchains; `make package` builds native and cross in one run. |

## Design Decisions

### Decision 1: Call the D-Bus API directly rather than spawning `systemd-inhibit`

**Context:** The lock could be obtained by running `systemd-inhibit` as a child
process and keeping it alive.

**Options Considered:**
1. Spawn `systemd-inhibit --what=idle:sleep` and hold the child.
2. Call `org.freedesktop.login1.Manager.Inhibit` over D-Bus directly.

**Decision:** Option 2.

**Rationale:** The subprocess approach makes lock lifetime depend on child
process management, adds a runtime dependency on the binary being installed, and
leaves an orphan risk if the parent dies abnormally. Calling the API directly
ties the lock to a file descriptor the kernel closes on process exit — the
release path is then impossible to get wrong (**FR-03**). It is also the same
call `systemd-inhibit` itself makes, so nothing is lost.

### Decision 2: Two separate inhibitor types, not one abstraction

**Context:** Sleep and screen-lock inhibition are superficially similar —
acquire a lock, release it later.

**Options Considered:**
1. One `Inhibitor` trait with two implementations.
2. Two concrete types composed by the state layer.

**Decision:** Option 2.

**Rationale:** The similarity is superficial and the differences are precisely
where bugs live. The logind lock is self-releasing via its descriptor; the
ScreenSaver cookie leaks unless `UnInhibit` is called explicitly, which is why
only `ScreenInhibitor` carries a `Drop` impl. A shared trait would have obscured
that asymmetry behind a uniform interface and invited treating them
interchangeably. They also live on different buses and are governed by different
requirements (**FR-01** vs **FR-04**).

### Decision 3: `SleepBlock` as a shared handle rather than GUI-owned state

**Context:** Initially the window owned the inhibitors directly. Adding a tray
that can toggle independently (**FR-09**) broke that model, since `ksni` runs the
tray on its own thread.

**Options Considered:**
1. Tray sends messages to the GUI thread, which remains the owner.
2. Both surfaces share an `Arc<Mutex<…>>` handle.

**Decision:** Option 2.

**Rationale:** Option 1 makes the tray depend on the window being alive and
responsive, which contradicts the peer model — a tray toggle would stall if the
window were busy. Sharing the state makes neither surface privileged. The mutex
is uncontended in practice: it is held only for the duration of a state
transition or a field copy, never across I/O or a repaint.

### Decision 4: A watcher thread to republish tray state

**Context:** A left click on the tray icon arrives as a D-Bus `Activate`, which
mutates state through `SleepTray::activate`. The icon did not update.

**Options Considered:**
1. Call `Handle::update` from inside `activate`.
2. A background thread that polls the shared state and calls `Handle::update` on
   change.
3. Add change notification to `SleepBlock`.

**Decision:** Option 2.

**Rationale:** Option 1 is impossible — the tray does not hold its own handle,
which only exists after `spawn` returns. `ksni` republishes properties only in
response to an explicit `Handle::update`, and a D-Bus method call does not
trigger one, so without this the icon silently shows the previous state. Option 3
would add subscription machinery to the core crate for a single consumer. The
poll is 250 ms and compares a three-field `Copy` struct, which is far cheaper
than the notification infrastructure it replaces.

### Decision 5: `async-io` backend rather than `tokio`

**Context:** `ksni` defaults to a tokio runtime.

**Options Considered:**
1. Accept the tokio default.
2. Configure `ksni` for `async-io` to match `zbus`.

**Decision:** Option 2.

**Rationale:** `zbus` — already a dependency of the core crate — defaults to the
`async-io` backend. Measured, the tokio default puts *both* runtimes in the tree
(496 dependency lines, tokio and async-io present) against 490 lines with
async-io alone. Tokio would therefore not replace async-io but run a second
executor in the same process. The dependency-count difference is negligible; the
second-executor argument is the deciding one. Nothing in this application is
async at the application level, so the runtime is pure substrate either way.

### Decision 6: Embed tray icons in the binary and also install them to the theme

**Context:** The tray needs pixmaps; the desktop entry needs a themed icon name.

**Options Considered:**
1. Reference a theme icon name from both.
2. Embed the PNGs and rely on the theme only for the desktop entry.

**Decision:** Option 2.

**Rationale:** The tray must render correctly regardless of whether the package's
icons were installed or the running icon theme resolves the name, so embedding
removes a runtime failure mode. The desktop entry cannot embed anything, so it
resolves `Icon=sleep-block` from hicolor, which the package populates
(**FR-14**). The two mechanisms serve different consumers.

### Decision 7: Split the daemon from the window

**Context:** Hide-to-tray requires the application to survive its window
closing. Three single-process approaches were tried and all failed.

**Options Considered:**
1. `ViewportCommand::Visible(false)` — winit's `set_visible` is an empty
   function on Wayland (`// Not possible on Wayland.`). The close was cancelled,
   so the process survived, but the window stayed on screen.
2. Minimise instead of hide — `set_minimized` works, but un-minimising is
   explicitly ignored on Wayland and `focus_window` is empty, so the tray could
   never restore the window.
3. Move the UI into a child viewport and destroy it — this *does* hide the
   window, confirmed with a standalone probe. But eframe always creates a root
   window and it cannot be hidden either, leaving an empty grey window on screen.
4. Two processes: a daemon owning the locks, and a disposable window.

**Decision:** Option 4.

**Rationale:** Options 1–3 all founder on the same fact: a Wayland window cannot
be hidden or raised by its own process. Option 4 sidesteps the problem instead
of fighting it — "show the window" becomes "start the window", which needs no
compositor cooperation. The cost is real (two binaries, an IPC contract, the
locks migrating to the daemon), and it would be the wrong trade for an
application that did not need to outlive its window. Here that is the entire
feature.

Worth recording: this is *not* the conventional answer. GTK and Qt applications
do hide-to-tray in one process, because their `hide()` destroys and recreates
the surface — which is what option 3 imitates. The blocker is specifically
eframe's mandatory root window, not Wayland alone.

### Decision 8: The daemon is authoritative; the window caches nothing

**Context:** With two processes, the window could hold a mirror of the state or
ask the daemon each time.

**Options Considered:**
1. Mirror the state in the window, synchronised by signals.
2. Read from the daemon on every frame, uncached.

**Decision:** Option 2.

**Rationale:** A mirror has to be invalidated correctly, and every failure in
this feature has been an invalidation failure — a stale zbus property cache
twice, and a stale within-frame snapshot twice more. The window redraws about
once a second, so a property read per frame is free by comparison. Removing the
cache removed the whole class of bug rather than fixing instances of it.

## Error Handling

Failures are surfaced, never swallowed, and never leave a partial transition
(**FR-11**):

| Condition | Handling |
|---|---|
| System bus or logind unreachable | `Error::Connect`; state unchanged; message shown in the window and the tray tooltip. The daemon returns it as an `fdo::Error` rather than logging it — with the window in another process, the daemon's stderr is not somewhere the user looks. |
| logind refuses the inhibit | `Error::Inhibit`; state unchanged. |
| ScreenSaver acquire fails | Sleep blocking is **retained**; the preference is reset and the error reported (**FR-12**). Losing the secondary lock is not a reason to allow suspend. |
| No StatusNotifierItem host | Logged; the app runs with the window alone (**FR-13**). |
| Icon fails to decode | That icon is skipped rather than panicking — a broken icon costs the icon, not the process. |
| Mutex poisoned | Recovered via `into_inner`. The guarded data is two `Option`s; a panic mid-transition may leave a lock held or dropped, but never in a state unsafe to read. |
| `UnInhibit` fails during drop | Ignored deliberately. If the desktop has gone away there is nothing to release, and failing in a drop path would be noise. |

## Testing Strategy

The strategy follows from what is observable, which varies sharply between the
two mechanisms.

**Integration over live services, not mocks.** The behaviour worth testing *is*
the interaction with logind: that a lock appears while held and is gone once
dropped. Mocks would assert only that the code calls what it was written to call.
Tests skip when a service is unavailable, and the spec records that a skip proves
nothing.

**Asymmetry of observability.** logind is fully observable through
`systemd-inhibit --list`, so **AC-01**–**AC-03** assert real effects. The
ScreenSaver interface exposes no readback at all — `Inhibit`/`UnInhibit` only —
and PowerDevil's `HasInhibition`/`ListInhibitions` do not report kwin-held
inhibitors. **AC-04** can therefore confirm only that calls succeed; registration
is confirmed manually (**AC-09**).

**Guarding silent failures.** The icon tests exist because a malformed icon fails
invisibly: the decoder rejects it, no pixmap is published, and everything else
keeps working. Format and dimension assertions convert that into a build failure.

**Mutation-checking the assertions.** Both suites were verified to fail when the
expectation is deliberately broken — flipping `block` to `delay`, and
regenerating an icon at 16-bit — so a passing run means something.

### Structural Verification

- `cargo clippy --release --all-targets -- -D warnings`
- `cargo fmt --check`
- `desktop-file-validate` on the desktop entry
- All three gate `make package` (**NFR-07**), so a lint or format regression
  fails packaging rather than shipping.

## Migration / Rollout

Not applicable — this is a new standalone utility with no predecessor and no
persisted state to migrate. Rollout is installation of the RPM, which places the
binary, desktop entry, and hicolor icons; `make uninstall` reverses a
non-packaged install.
