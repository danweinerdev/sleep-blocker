---
title: "Review Follow-ups"
type: phase
plan: "SleepBlock"
phase: 6
status: complete
created: 2026-08-11
updated: 2026-08-11
deliverable: "The two follow-ups phase 5's reviews raised but did not block on"
tasks:
  - id: "6.1"
    title: "Log PropertiesChanged emission failures"
    status: complete
    verification: "A failed property emission leaves a line in the daemon's log rather than only a slightly late indicator. Serves NFR-08."
    justifies: "Carries review 02 finding F-16. `Service::announce` discards all four emission results, so a stuck-looking indicator has no trail to grep — the polling fallback hides the failure rather than reporting it."
  - id: "6.2"
    title: "Give AC-33 an evidence row"
    status: complete
    verification: "An evidence row runs `make package`, or the plan records that AC-33's container half was verified in phase 4 and only release supersession is new."
    justifies: "Carries review 02 finding F-17. AC-33 is checked off with no `make package` run evidenced in the phase claiming it."
---

# Phase 6: Review Follow-ups

## Overview

Two items phase 5's reviews raised and judged not worth holding that phase open
for. They live here rather than as deferred tasks inside a completed phase,
which the artifact rules disallow and which would in any case misrepresent them:
neither is abandoned, both are simply not urgent.

Both are small. Neither affects behaviour a user would notice today — F-16 is a
missing log line behind a self-healing poll, and F-17 is an evidence-trail gap
rather than a functional one.

## 6.1: Log PropertiesChanged emission failures

### Subtasks
- [x] Report a failed emission rather than discarding the result

### Notes
Revision boundary: an emission failure is observable in the daemon's log.

### Trap
The temptation is to propagate the error out of `announce` and fail the calling
method. That would be wrong: the state change already happened, and the polling
fallback means the client converges within ~500ms regardless. A failed
announcement is a diagnostic event, not a failed operation.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 78f5e8b5e4ccfb1d1f401d7807411e84ee85bf28
- Identity recheck: git log --format=%H, 2026-08-11 17:11, matches recorded revision 78f5e8b5e4ccfb1d1f401d7807411e84ee85bf28
- Focused review: `git show 78f5e8b5e4ccfb1d1f401d7807411e84ee85bf28`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo clippy --workspace --all-targets` | . | PASS (exit 0) | no warnings; `announce` now logs each failed emission with the property name |
| `cargo fmt --check` | . | PASS (exit 0) | no formatting diff |
| `cargo test --workspace` | . | PASS (exit 0) | 43 passed across suites; 0 failed |

`Service::announce` iterates the four `*_changed` emissions and writes
`failed to announce <Property> changed: <error>` to stderr for any that fail,
rather than discarding the result. Errors are deliberately not propagated,
per the trap above. Satisfies **AC-34** (**NFR-08**).

## 6.2: Give AC-33 an evidence row

### Subtasks
- [x] Either run `make package` as evidence, or record the split with phase 4

### Notes
Revision boundary: AC-33's checked state is backed by evidence in the artifact
that claims it.

### Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 58c2a5d98f1a55cfa0612363923621bc50183339
- Identity recheck: git log --format=%H, 2026-08-11 17:11, matches recorded revision 58c2a5d98f1a55cfa0612363923621bc50183339
- Focused review: `git show 58c2a5d98f1a55cfa0612363923621bc50183339`; complete task diff reviewed for correctness, scope, tests, maintainability, and task boundary

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `make package` | . | PASS (exit 0) | container build at source revision 78f5e8b: tests run, both arches packaged — `sleep-block-0.1.1-1.28.git78f5e8b.fc44.{x86_64,aarch64}.rpm` in `tmp/rpmbuild/RPMS/` |
| `rpm -qp --qf '%{VERSION}-%{RELEASE}'` | . | PASS (exit 0) | `0.1.1-1.28.git78f5e8b.fc44` — release embeds the source commit |
| `python3 -c "import rpm; rpm.labelCompare(...)"` | . | PASS (exit 0) | labelCompare returned 1: `1.28.git78f5e8b` orders after the previous build's `1.16.git1b83abf`, so the new package supersedes it |

This run is the evidence row review 02's F-17 found missing: `make package`
executed end-to-end at this phase's checkpoint, producing dual-arch RPMs whose
git-derived release supersedes the prior build. Satisfies **AC-35** and backs
**AC-33** (**FR-22**, **NFR-09**).

## Acceptance Criteria
- [x] **AC-34**: A failed property announcement is visible in the daemon's log.
- [x] **AC-35**: AC-33's checked state is backed by an evidence row, or the
  phase-4 split is recorded explicitly.

## Phase Completion Evidence

- Verified: 2026-08-11
- Repository: ~/Development/Code/sleep-block
- VCS: git
- Revision / checkpoint: 0c242b46e151bf776736fee8620b84ed8a2ae5ca
- Identity recheck: git rev-parse HEAD, 2026-08-11 17:31, matches recorded revision 0c242b46e151bf776736fee8620b84ed8a2ae5ca
- Final aligned review: Plans/SleepBlock/reviews/04-sleepblock-code-review-2acf3a50..78f5e8b5.md; frozen: 2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca

| Command | Working directory | Result | Observable evidence |
|---|---|---|---|
| `cargo test --workspace` | . | PASS (exit 0) | 43 passed; 0 failed |
| `cargo clippy --workspace --all-targets` | . | PASS (exit 0) | no warnings |
| `cargo fmt --check` | . | PASS (exit 0) | no formatting diff |
| `make package` | . | PASS (exit 0) | dual-arch RPMs at release `1.28.git78f5e8b` superseding `1.16.git1b83abf` |

### Completed task identities

- `6.1`: `78f5e8b5e4ccfb1d1f401d7807411e84ee85bf28`
- `6.2`: `58c2a5d98f1a55cfa0612363923621bc50183339`
