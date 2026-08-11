---
title: "Packaging"
type: phase
plan: "SleepBlock"
phase: 4
status: in-progress
created: 2026-08-10
updated: 2026-08-10
deliverable: "A binary RPM built by `make package`, gated on tests, lint, and format checks"
tasks:
  - id: "4.1"
    title: "Add Makefile and binary RPM packaging"
    status: complete
    verification: "make package exits 0 and writes an installable RPM; it fails if the test suite, clippy -D warnings, cargo fmt --check, or desktop-file-validate fails; the extracted binary runs, registers its tray item, and takes a real logind lock. Serves FR-14, FR-15, NFR-05, NFR-07."
    justifies: "Delivers FR-14, FR-15, NFR-05, NFR-07 and satisfies AC-10, AC-11, AC-22. Prevents shipping a package whose lint or format regressions went unnoticed, and prevents a desktop entry pointing at an icon the package does not install."
---

# Phase 4: Packaging

## Overview

Turns the verified application into something installable. The compile and the
test run happen in the Makefile using the local toolchain; `rpmbuild` only stages
the finished artefacts. This is the endpoint of the plan.

## 4.1: Add Makefile and binary RPM packaging

### Subtasks
- [x] Write a Makefile with `build`, `test`, `check`, `install`, `uninstall`, `icons`, and `package` targets
- [x] Gate `package` on `check`, which runs tests, `clippy -D warnings`, `cargo fmt --check`, and desktop-entry validation
- [x] Write a binary RPM spec with no `%build` step and no Rust `BuildRequires`
- [x] Point the desktop entry at the application's own icon and install icons into hicolor
- [x] Stage artefacts, produce the tarball, and invoke `rpmbuild -bb`
- [x] Extract the built RPM and confirm the packaged binary functions

### Notes
Revision boundary: `make package` produces an installable RPM containing the
binary, the desktop entry, themed icons, and the licence.

### Trap
The instinctive approach is a source RPM that compiles inside `rpmbuild`. That
requires `BuildRequires: rust, cargo`, which RPM can only satisfy from
distribution packages — a rustup toolchain is invisible to it, and the build
fails on a machine where Rust is unquestionably installed. Vendoring the crates
does not help, because the failure is in dependency resolution, not the network.

Packaging the already-built binary avoids this entirely. The trade-off is that
the resulting RPM inherits the build machine's glibc requirements, which is
acceptable for local use and is the reason a clean-room build remains open as
**Q-03**.

A second trap: assuming the desktop entry's stock icon name is adequate once real
icons exist. It must name the installed icon, and the package must install it,
or launchers show a generic placeholder.

### Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 38087c19f6b02eb63a4541f03afd33821095f501
- Identity recheck: git log --format=%H, 2026-08-10 21:05, matches recorded revision 38087c19f6b02eb63a4541f03afd33821095f501
- Focused review: `git show 38087c19f6b02eb63a4541f03afd33821095f501`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary
- Reviewed candidate / final: 38087c19f6b02eb63a4541f03afd33821095f501
- Review result: PASS/Aligned

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `make package` | . | PASS (exit 0) | wrote sleep-block-0.1.0-1.fc44.x86_64.rpm, 3.9M |
| `cargo fmt --check` | . | PASS (exit 0) | passes after fixing drift the gate caught on first run |
| `rpm -qlp …x86_64.rpm` | . | PASS (exit 0) | binary, desktop entry, 8 PNG sizes, SVG, licence in expected paths |
| `desktop-file-validate usr/share/applications/sleep-block.desktop` | extracted package | PASS (exit 0) | valid; `Icon=sleep-block` resolves to an installed file |
| `./usr/bin/sleep-block` | extracted package | PASS (exit 0) | stripped; registers tray item; logind lock taken on activate |

## Acceptance Criteria
- [x] **AC-10**: The installed desktop entry validates and its `Icon` key names an installed icon.
- [x] **AC-11**: `make package` produces an installable binary RPM and fails on any check regression.
- [x] **AC-22**: The release profile applies fat LTO and strips symbols.

## Phase Completion Evidence

- Verified: 2026-08-10
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 38087c19f6b02eb63a4541f03afd33821095f501
- Identity recheck: git rev-parse 38087c1, 2026-08-10 21:05, matches recorded revision 38087c19f6b02eb63a4541f03afd33821095f501


### Completed task identities

- `4.1`: `38087c19f6b02eb63a4541f03afd33821095f501`

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `make package` | . | PASS (exit 0) | RPM written; all gates passed |
| `cargo test --release` | . | PASS (exit 0) | 8 passed; 0 failed |
