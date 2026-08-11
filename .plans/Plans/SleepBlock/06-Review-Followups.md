---
title: "Review Follow-ups"
type: phase
plan: "SleepBlock"
phase: 6
status: planned
created: 2026-08-11
updated: 2026-08-11
deliverable: "The two follow-ups phase 5's reviews raised but did not block on"
tasks:
  - id: "6.1"
    title: "Log PropertiesChanged emission failures"
    status: planned
    verification: "A failed property emission leaves a line in the daemon's log rather than only a slightly late indicator. Serves NFR-08."
    justifies: "Carries review 02 finding F-16. `Service::announce` discards all four emission results, so a stuck-looking indicator has no trail to grep — the polling fallback hides the failure rather than reporting it."
  - id: "6.2"
    title: "Give AC-33 an evidence row"
    status: planned
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
- [ ] Report a failed emission rather than discarding the result

### Notes
Revision boundary: an emission failure is observable in the daemon's log.

### Trap
The temptation is to propagate the error out of `announce` and fail the calling
method. That would be wrong: the state change already happened, and the polling
fallback means the client converges within ~500ms regardless. A failed
announcement is a diagnostic event, not a failed operation.

### Completion Evidence

Pending — not complete.

## 6.2: Give AC-33 an evidence row

### Subtasks
- [ ] Either run `make package` as evidence, or record the split with phase 4

### Notes
Revision boundary: AC-33's checked state is backed by evidence in the artifact
that claims it.

### Completion Evidence

Pending — not complete.

## Acceptance Criteria
- [ ] **AC-34**: A failed property announcement is visible in the daemon's log.
- [ ] **AC-35**: AC-33's checked state is backed by an evidence row, or the
  phase-4 split is recorded explicitly.

## Phase Completion Evidence

Pending — not complete.
