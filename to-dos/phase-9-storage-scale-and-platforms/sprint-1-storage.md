# Sprint 1 — Storage

**Phase:** Phase 9 — Storage, Scale and Platforms  
**Release:** v1.7.0  
**Sprint Goal:** Make saves incremental, compress the CAS, index cold grep, reduce lock
contention between parallel agents, and re-redact a stored workspace (`icx rescan --apply`), each
change backed by a measurement.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** current `zstd` crate version and dictionary API; file-locking
  options on Linux, macOS and Windows (`flock`, `fcntl`, `LockFileEx`) and the crate in use
  (Context7).
- **Installed agent behaviour:** how many hooks run concurrently per agent and across agents in a
  normal session (Claude Code parallel tool calls, Codex, agy, Muse, Hermes).
- **Project evidence:** the save path and what derived state is rewritten on each save
  (`CHANGELOG.md:601-603`); the field PostToolUse latency outliers of about 8 s; the
  `grep_cold` implementation; the Phase 7 Sprint 1 trigram index design.

### Research notes

_None recorded yet._

## Tickets

### T-91-001 — Incremental saves of derived state

**Description:** Persist only the derived state (indexes, graph, occurrence tables) that changed
since the last save, instead of rewriting it whole.

**Research:** Hypothesis: save time is dominated by rewriting unchanged derived state.
Measurement: profile a save on a field-sized workspace before changing code; publish the before
and after save times. A rejected hypothesis closes the ticket with the profile.

**Acceptance Criteria:**

- [ ] A crash at any point during a save leaves a readable workspace (fault-injection test).
- [ ] Loading an incrementally saved workspace equals loading a full save (equivalence test).
- [ ] Save time before and after is measured and published in `docs/numbers.md`.

**Dependencies:** None  
**Reference:** `CHANGELOG.md:601-603`; plan Phase 9 Sprint 1  
**Estimated Complexity:** L

---

### T-91-002 — zstd compression in the CAS

**Description:** Compress CAS blobs with zstd, optionally with a trained dictionary for small
tool outputs, while keeping BLAKE3 addresses over the uncompressed bytes.

**Research:** Hypothesis: zstd reduces on-disk size of the field corpus with read latency still
within the hook budget; a dictionary helps small blobs. Measurement: ratio and read/write latency
with no compression, zstd, and zstd with dictionary on the field corpus.

**Acceptance Criteria:**

- [ ] Content addresses are unchanged; integrity verification still checks the uncompressed bytes.
- [ ] Existing uncompressed stores remain readable.
- [ ] Ratio and latency for each option are measured and published in `docs/numbers.md`.
- [ ] The default is chosen from the measurement and recorded in an ADR.

**Dependencies:** None  
**Reference:** plan Phase 9 Sprint 1; `docs/virtual-memory-and-dag-engine.md`  
**Estimated Complexity:** M

---

### T-91-003 — Line/trigram index for `grep_cold`

**Description:** Back `grep_cold` with a line and trigram index so a cold grep reads candidate
blobs only, falling back to a full scan when the pattern has no usable trigrams.

**Research:** Reuse the Phase 7 Sprint 1 trigram index if it fits. Hypothesis: the index cuts cold
grep time on large stores. Measurement: grep time against store size with and without the index.

**Acceptance Criteria:**

- [ ] Results with the index equal results from a full scan (property test over random patterns).
- [ ] Patterns without usable trigrams fall back to a scan.
- [ ] Grep time and index size are measured and published in `docs/numbers.md`.

**Dependencies:** None (coordinates with T-71-001)  
**Reference:** plan Phase 9 Sprint 1; plan Phase 7 Sprint 1  
**Estimated Complexity:** M

---

### T-91-004 — Lock-contention reduction for parallel agents

**Description:** Reduce the time hooks wait on the workspace lock when several agents or parallel
tool calls run at once: shorter critical sections, append-only ingest outside the lock, and lock
waits recorded in the hook trace.

**Research:** Hypothesis: the field PostToolUse outliers of about 8 s are lock waits. Measurement:
lock-wait spans (Phase 8 trace) on a replayed parallel session. If the outliers have another cause,
record it and retarget the ticket.

**Acceptance Criteria:**

- [ ] Lock wait is a span in the hook trace.
- [ ] A concurrent-agent stress test shows no lost or duplicated artifacts.
- [ ] Lock wait distribution before and after is measured and published in `docs/numbers.md`.

**Dependencies:** T-83-001  
**Reference:** field finding (8 s PostToolUse outliers); plan Phase 9 Sprint 1  
**Estimated Complexity:** L

### T-91-005 — `icx rescan --apply`: re-redact a stored workspace

**Description:** Extend the v1.5.0 report-only `icx rescan` (T-74-001) with an apply step that
re-redacts stored content under the current redactor. It writes new CAS objects and remaps every
reference rather than editing objects in place, keeps old handles resolvable, and removes the old
bytes from the database file. Deferred from v1.5.0 by owner decision (2026-11-25); the full
reference map is in #102.

**Research:** Re-check the reference map recorded in #102 against the store after T-91-001 and
T-91-002, which change how derived state and CAS bytes are saved. Hypothesis: a bottom-up DAG
remap that reuses stored ordinals reproduces every non-redacted digest unchanged. Measurement: a
remap over a field-sized workspace with no redaction changes leaves the DAG, epoch roots and seal
identical.

**Acceptance Criteria:**

- [ ] Dry-run stays the default; `--apply` needs explicit confirmation and writes nothing
  without it (tested).
- [ ] The Merkle-DAG, sealed epochs, HDC index, graph, lexical index, occurrence records and
  subagent steps are rebuilt or remapped, and derived text is re-sanitized, not only id-mapped.
- [ ] Old `#icx:` handles resolve through an old-to-new alias table (or a tombstone naming the
  re-redaction).
- [ ] Re-redaction is atomic per workspace: the rebuild runs outside the workspace lock, and the
  swap commits under it only if the generation is unchanged. A snapshot is kept, and its
  path-bound seal is documented.
- [ ] `verify_all_merkle_integrity_with_cas` passes afterwards. So do the new checks: epoch copies
  match the DAG; graph provenance, HDC events, occurrences and subagent steps name stored
  artifacts; and no artifact changes under `redact` again.
- [ ] After `wal_checkpoint(TRUNCATE)` and `VACUUM`, no re-redacted secret remains in the
  database or WAL bytes (tested by scanning the files); legacy v1.0.x JSON files are reported.

**Dependencies:** T-74-001 (v1.5.0 report), T-91-001, T-91-002
**Reference:** #102; T-74-001 research (2026-11-25)
**Estimated Complexity:** XL

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 2 is done and its tickets are adjusted.
