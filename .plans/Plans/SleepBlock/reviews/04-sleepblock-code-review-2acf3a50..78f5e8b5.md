---
title: "Code Review: SleepBlock — Phase 6 (phase gate)"
type: review
status: resolved
created: 2026-08-11
updated: 2026-08-11
tags: [review]
related: [Plans/SleepBlock]
review_of: "Plans/SleepBlock/06-Review-Followups.md"
rev: "2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca"
review_scope: phase
frozen: true
verdict: Aligned
reviewed_planning_revision: "0c242b46e151bf776736fee8620b84ed8a2ae5ca"
review_mode: independent
lane_results:
  - lane: review_plan_drift
    result: PASS/Aligned
    reviewed_identity: "2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca"
    evidence: "Traced every changed line in the six-file range to task 6.1, task 6.2's evidence-only requirement, gate bookkeeping, or the clean-worktree housekeeping the gate depends on; re-ran cargo fmt --check and cargo test --workspace to spot-check evidence rows rather than trust them; verified the Trap is honoured at both announce call sites (toggle line 51, set_keep_screen_awake line 67); no missing work, no creep, no drift."
  - lane: review_quality
    result: PASS/Aligned
    reviewed_identity: "2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca"
    evidence: "Re-ran cargo fmt --check, cargo clippy --workspace --all-targets, and cargo test --workspace: 43 tests pass, 0 clippy warnings; judged the announce doc comment as earning its place by stating the log-don't-propagate invariant; the round-two finding against the .gitignore comment's unverified causal claim was confirmed fixed in commit 0c242b4; no Critical, Major, or Minor findings."
  - lane: review_spec_compliance
    result: PASS/Aligned
    reviewed_identity: "2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca"
    evidence: "Mapped AC-34/NFR-08 to sleep-blockd.rs:134-136 with all four properties named on failure; verified the design's NFR-08 row exactly — announce called from toggle and set_keep_screen_awake, set_keep_running_in_tray exempted via zbus's setter signal; confirmed no test seam exists (SignalEmitter is a concrete zbus type, not a trait), consistent with AC-34's (inspection) classification; re-ran fmt and clippy clean; no violations, no cross-document inconsistencies."
  - lane: review_blind_spots
    result: PASS/Aligned
    reviewed_identity: "2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca"
    evidence: "Cold read of the full diff plus sleep-blockd.rs and state.rs; found no functional defect in the announce change; two Minor latent notes against .gitignore classification (.gitmodules at line 11 via git check-ignore -v, and a repo-root .gitconfig question), both recorded as F-29/F-30 with dispositions; hidden-risk level Low with no code action required."
findings:
  - id: F-25
    severity: minor
    title: "Four sequential PropertiesChanged signals could be one batched emission"
    status: answered
  - id: F-26
    severity: question
    title: "The new failure-log line has no automated test"
    status: answered
  - id: F-27
    severity: question
    title: "Where does the daemon's stderr actually land in deployment?"
    status: answered
  - id: F-28
    severity: minor
    title: "The .gitignore comment blamed container builds without evidence"
    status: fixed
  - id: F-29
    severity: minor
    title: ".gitmodules is ignored under a comment that only justifies per-user state"
    status: answered
  - id: F-30
    severity: question
    title: "Is ignoring a repo-root .gitconfig intentional?"
    status: answered
followups: []
---

# Code Review: SleepBlock — Phase 6 (phase gate)

Phase-gate review of the frozen range
`2acf3a504436835b241ff97ef2a4ad3393d282cd..0c242b46e151bf776736fee8620b84ed8a2ae5ca`.
The range holds one code change (task 6.1's emission-failure logging in
`sleep-blockd.rs`), the planning-artifact evidence for tasks 6.1 and 6.2,
`.gitignore` housekeeping the completion gate's clean-worktree requirement
depends on, and one in-range correction (F-28).

Three four-lane passes ran against this phase as the endpoint moved:
`..78f5e8b5` (code only), `..e8d770af` (after evidence and housekeeping), and
the final `..0c242b46` recorded in `lane_results` above. Each pass was four
fresh-context agents. The endpoint moved twice for non-material reasons — the
gate's clean-worktree rule required ignoring local droppings, and the
second-pass quality lane caught a factually wrong comment in that very
housekeeping (F-28), whose fix became the final endpoint.

## Overall Verdict

**Aligned.** On the final range all four lanes returned clean: drift Strong
with every changed line traced to a task or the gate's own preconditions;
quality Strong with zero findings at any severity and the verification gates
re-run rather than trusted; spec-compliance Strong with AC-34 and AC-35 mapped
to code and evidence; blind-spots Low with two latent `.gitignore`
classification notes and no code action required. No material change resulted
from this review, so the reviewed state stands.

## Findings

### F-25 — Minor: four sequential signals could be one batched emission

`announce` awaits four `*_changed` calls sequentially; the underlying
`PropertiesChanged` signal accepts a map and could carry all four in one
emission. Pre-existing shape. The new per-property log lines make it newly
observable: one transient bus hiccup can produce up to four near-identical
lines. Raised in the first pass; the third-pass blind-spot lane independently
re-validated it against the zbus macro source and reached the same
no-action-required conclusion.

### F-26 — Question: no automated test for the failure-log line

Raised independently by the quality and spec lanes in every pass. Forcing a
generated `*_changed` emission to fail requires a bus-level fault injected
mid-call; the third-pass spec lane additionally confirmed no cheaper seam
exists because `SignalEmitter` is a concrete zbus type, not a trait.

### F-27 — Question: where does the daemon's stderr land?

No systemd unit or log redirection exists in the repo; the blind-spot lane
asked whether "logged" means "visible in practice."

### F-28 — Minor: the .gitignore comment asserted an unverified mechanism

The second-pass quality lane proved the comment wrong from the repo's own
files: the `Containerfile` sets `HOME=/tmp` and the Makefile mounts the project
only at `/src`, so container builds cannot be the source of the dotfile
droppings the comment blamed them for.

### F-29 — Minor: `.gitmodules` ignored under a per-user-state rationale

`git submodule add` writes `.gitmodules` at the repo root; ignoring it could
let a future submodule reference go uncommitted silently. Latent — the repo
uses no submodules.

### F-30 — Question: is ignoring a repo-root `.gitconfig` intentional?

A repo-root `.gitconfig` is more plausibly project-scoped than personal, so
grouping it with shell rc files could swallow a deliberately shared config.

## Resolution Log

### F-25 — answered (2026-08-11)

Accepted as an accurate description of a pre-existing shape. The emission cost
is four local-bus round trips on a human-interaction-rate path (button and
tray clicks), so batching would optimise something unmeasurable. Recorded for
whoever next touches `announce`; no task raised, per both lanes' own
no-action-required recommendation.

### F-26 — answered (2026-08-11)

AC-34 is deliberately classified *(inspection)* in the spec, exactly because
this failure path needs a bus-level fault injected mid-call and zbus exposes
no mockable seam. All lanes that raised it reached that conclusion themselves
from the spec text. No task raised.

### F-27 — answered (2026-08-11)

There is no unit file: the daemon is spawned by the GUI (or a terminal), and
its stderr goes wherever the parent's does — under a desktop session launch
that is the user journal on systemd systems. Before this change the failure
produced nothing anywhere; a line on stderr is strictly more observable in
every deployment, which is what NFR-08 asks. No task raised.

### F-28 — fixed (2026-08-11)

Fixed in commit `0c242b46e151bf776736fee8620b84ed8a2ae5ca`, inside the
reviewed range: the comment now states what the entries are without asserting
a mechanism. The third-pass quality lane confirmed the fix and returned zero
findings. Comment-only; no behaviour touched.

### F-29 — answered (2026-08-11)

Intentional, with a cause the lane could not see: an untracked local artifact
named `.gitmodules` (a `/dev/null` device-file mask, like `.idea` and
`.vscode`) sits in this worktree, and the completion gate requires
untracked-clean status — ignoring it is what makes the gate satisfiable on
this machine. The repo uses no submodules; if one is ever added, `git
submodule add` stages `.gitmodules` explicitly, which bypasses gitignore. No
task raised.

### F-30 — answered (2026-08-11)

Same cause as F-29: a local `.gitconfig` artifact exists in this worktree and
the entry keeps the gate's clean-status requirement satisfiable. The project
deliberately carries no repo-root git config; nothing shared is being
swallowed. No task raised.

## Orchestrator Observations

The endpoint moved twice, and both moves were the gate's own mechanics rather
than code problems: the clean-worktree rule forced the `.gitignore` commit,
and reviewing that commit surfaced F-28 inside it. The second-pass quality
lane's F-28 catch — disproving a comment from the repo's own build files — is
the round that earned its cost. The third pass returned zero code findings
across all four lanes; its two residual notes (F-29, F-30) are dispositioned
above rather than fixed, because fixing non-material nits after a frozen
endpoint restarts the gate indefinitely. All findings are terminal with the
reviewed state materially unchanged, so this review stands as the phase's
final aligned review.
