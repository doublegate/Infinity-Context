# Sprint 4 — Savings Ledger and Analytics

**Phase:** Phase 10 — Prevent and Prove  
**Release:** v1.8.0  
**Sprint Goal:** Record every saving in one ledger and report totals and missed savings to the
user.  
**Estimated Duration:** 1 week

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** how rtk and context-mode report savings (`gain`-style
  commands), to keep output familiar without copying their figures.
- **Installed agent behaviour:** where each installed agent writes session logs and their formats,
  for `icx discover`.
- **Project evidence:** the Phase 8 `icx stats` storage and schema; the stub, rewrite and template
  code paths from Sprints 1 to 3; the tokenizer recorded with token counts.

### Research notes

_None recorded yet._

## Tickets

### T-10-4-001 — One savings ledger

**Description:** Record one ledger entry per saving event (stub, rewrite, redirect, template) with
bytes and tokens before and after, the tokenizer, the agent and the source, in the storage shared
with `icx stats`.

**Research:** Confirm the Phase 8 storage can be extended without a format break, or plan the
migration.

**Acceptance Criteria:**

- [ ] Every saving path writes exactly one entry (test per path).
- [ ] Entries carry no tool output, only sizes, hashes and metadata.
- [ ] Ledger totals reconcile with the raw traces in a test.

**Dependencies:** T-83-003, T-10-1-001, T-10-2-001, T-10-3-002  
**Reference:** plan Phase 10 Sprint 4  
**Estimated Complexity:** M

---

### T-10-4-002 — `icx gain`

**Description:** Add `icx gain` to report savings totals per period, agent and source, with JSON
export.

**Research:** None beyond the gate.

**Acceptance Criteria:**

- [ ] Period totals and breakdowns match the ledger (test).
- [ ] `--json` emits a versioned schema.
- [ ] Output states the tokenizer used and that counts are estimates where they are.

**Dependencies:** T-10-4-001  
**Reference:** plan Phase 10 Sprint 4  
**Estimated Complexity:** S

---

### T-10-4-003 — `icx discover`

**Description:** Add `icx discover` to scan agent session logs for bulky outputs that `icx` did not
save (no filter, below thresholds, agent unsupported) and report them as missed savings with the
suggested setting or filter.

**Research:** Hypothesis: the session logs show recurring missed savings that a setting or filter
would capture. Measurement: run on the owner's own logs and count actionable findings.

**Acceptance Criteria:**

- [ ] Reads the log formats of each supported agent (fixture tests).
- [ ] Reports never include log content, only command names, sizes and counts.
- [ ] Runs read-only on the logs.

**Dependencies:** T-10-4-001  
**Reference:** plan Phase 10 Sprint 4  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 5 is done and its tickets are adjusted.
