# sleep-block

A small GUI utility that stops Linux from going to sleep. Click the circle to
toggle: green means the system is being kept awake, red means it can suspend
normally.

## Why

`caffeine`-style tools often work by faking input events or by shelling out to
`systemd-inhibit` and holding the child process open. This one calls the same
D-Bus API `systemd-inhibit(1)` uses directly, so the lock shows up in
`systemd-inhibit --list` like any other and disappears the moment the app exits
— including if it is killed.

## Sleep and screen lock are separate

These are two unrelated mechanisms, which is worth knowing before deciding what
to enable:

- **Sleep** is a kernel power state arbitrated by systemd-logind. This is what
  the main toggle blocks, via `org.freedesktop.login1.Manager.Inhibit` on the
  system bus.
- **Screen blanking and locking** is a desktop concern handled by your
  compositor or screen locker. logind has no lock-related inhibitor type at
  all, so it cannot be folded into the above. The optional checkbox blocks it
  separately via `org.freedesktop.ScreenSaver` on the session bus — the same
  call browsers and video players make.

The checkbox is off by default, on the assumption that letting the monitors
sleep is usually what you want even while the machine stays up.

With sleep blocking on and the checkbox off:

| Behaviour | Blocked? |
| --- | --- |
| Idle suspend | yes |
| Explicit suspend (`systemctl suspend`) | yes |
| Screen blank / lock after idle | no — tick the checkbox |
| Lid-close suspend | no — see below |

**Lid close is not covered.** On a desktop that handles the lid switch itself
(KDE's PowerDevil, for instance, takes a `handle-lid-switch` lock) closing the
lid will still suspend. Blocking that means taking a `handle-lid-switch`
inhibitor, which would override your desktop's own power settings, so it is
deliberately left out.

## Build

Requires a Rust toolchain. No system development packages are needed — both
`egui` and `zbus` are pure Rust, so there is no dependency on GTK headers or
`libdbus`.

```sh
cargo build --release
./target/release/sleep-block
```

The release profile enables fat LTO and strips symbols, which takes the binary
from roughly 15 MB to 9.4 MB. Most of what remains is the statically linked GPU
and font stack.

## Layout

- `crates/sleep-block-core` — the inhibitor logic, with no GUI dependency. Can
  be used as a library.
- `crates/sleep-block-bin` — the egui front end.

## Tests

```sh
cargo test
```

The integration tests run against the real logind and ScreenSaver services
rather than mocks, because the interaction with those services is the behaviour
worth testing: a test asserts that a lock actually appears in
`systemd-inhibit --list` while held and is gone after the value is dropped.

They are therefore environment-dependent. Where a service is missing — a CI
container with no session bus, a headless builder with no screen locker — the
affected tests print `SKIP` and pass. **A skip proves nothing**, so a green run
in such an environment is not evidence that the inhibitors work; run the suite
in a real desktop session to get that.

One limitation worth naming: `org.freedesktop.ScreenSaver` exposes only
`Inhibit` and `UnInhibit`, with no way to read inhibition state back. Neither
PowerDevil's `HasInhibition` nor its deprecated `ListInhibitions` reports a
kwin-held screen inhibitor, so there is no D-Bus probe to assert against. The
screen lock tests can therefore confirm that acquire and release succeed, but
cannot automatically confirm the desktop registered them.

That last step has been verified by hand instead: with the checkbox ticked,
`sleep-block` appears in KDE's *Power & Battery* tray popup as a blocking
application, and disappears when it is unticked. Re-check that way after
touching `ScreenInhibitor`.

## Licence

MIT — see [LICENSE](LICENSE).
