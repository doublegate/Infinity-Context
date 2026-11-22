# Sprint 3 — Observability

**Phase:** Phase 8 — Prompt-Cache Proxy and Observability  
**Release:** v1.6.0  
**Sprint Goal:** Make every hook observable with one trace and measured token counts, measure bus
timing per hook as the `icx-shm` baseline, and generate the published numbers from a dataset
schema.  
**Estimated Duration:** 2 weeks

## Research gate

Re-check before any ticket starts:

- **External docs and versions:** OpenTelemetry GenAI semantic conventions and the current
  `tracing` / `tracing-subscriber` / OTLP exporter crate versions (Context7); whether a local
  tokenizer with a recorded name and hash is available offline for token counts.
- **Installed agent behaviour:** the hook events each installed agent fires per turn and their
  timing budgets, so traces and bus timing are attributed to the right event.
- **Project evidence:** issue #32 (`icx-shm` design) and what baseline it asks for;
  `docs/numbers.md` and how its tables are produced today; `crates/icx-bus` publish/poll paths;
  the PostToolUse latency outliers in the field logs.

### Research notes

_None recorded yet._

## Tickets

### T-83-001 — One trace per hook with spans and before/after token counts

**Description:** Emit one trace per hook invocation, with spans for parse, redaction, ingest,
recall, encode and reply, and record token counts of the hook input and of what `icx` returned.
Traces are local by default; export is opt-in.

**Research:** Confirm the GenAI semantic-convention attribute names and pick a tokenizer whose name
and hash are recorded with each count. Measure tracing overhead on the hook path.

**Acceptance Criteria:**

- [ ] Each hook produces exactly one root span with child spans per stage.
- [ ] Before/after token counts carry the tokenizer name and hash.
- [ ] Spans contain no tool output or secrets, only sizes, hashes and timings.
- [ ] Tracing can be disabled; hook latency with and without tracing is measured and published in
  `docs/numbers.md`.

**Dependencies:** None  
**Reference:** `docs/claude/11-enhancements-and-optimizations.md` §F; module 91 (Observability)  
**Estimated Complexity:** L

---

### T-83-002 — Bus publish and poll timing per hook (the `icx-shm` baseline)

**Description:** Measure the time each hook spends publishing to and polling the telepathy bus,
per agent and per hook event, on the current `/dev/shm` seqlock implementation. This is the
measured baseline that issue #32 requires before the `icx-shm` redesign is judged.

**Research:** Read #32 for the exact metrics it needs (latency distribution, contention, message
loss). Confirm the timing source is monotonic and that measurement does not change the
publish/poll path.

**Acceptance Criteria:**

- [ ] Publish and poll durations are recorded per hook event as spans under T-83-001.
- [ ] A reproducible benchmark replays a multi-agent session and reports the distribution.
- [ ] Results, host description and run manifest are measured and published in
  `docs/numbers.md`, and linked from #32.

**Dependencies:** T-83-001  
**Reference:** issue #32; `docs/wire-protocol-and-shm-bus.md`  
**Estimated Complexity:** M

---

### T-83-003 — `icx stats` command

**Description:** Add `icx stats` to summarise local traces and counters: hooks run, bytes and
tokens before and after, stubs issued, dereferences, provider cache counters (T-81-003) and bus
timing, over a chosen period, with JSON output. Its storage is shared with the Phase 10 savings
ledger (T-10-4-001).

**Research:** Confirm the storage layout so Phase 10 can extend it without a migration break.

**Acceptance Criteria:**

- [ ] `icx stats --since <period>` prints a table and `--json` emits a versioned schema.
- [ ] Counts reconcile with the raw traces in a test.
- [ ] Missing data is reported as missing, not as zero.

**Dependencies:** T-83-001, T-81-003  
**Reference:** plan Phase 8 Sprint 3  
**Estimated Complexity:** M

---

### T-83-004 — Optional local read-only dashboard

**Description:** Optionally serve the `icx stats` data as a local, read-only page bound to
loopback. This ticket may be deferred without blocking the release.

**Research:** Decide whether a static HTML export is sufficient before adding a server; check the
threat-model rules for local listeners.

**Acceptance Criteria:**

- [ ] The dashboard is off by default, binds to loopback only and is read-only.
- [ ] It reads only the `icx stats` JSON schema.
- [ ] The threat model is updated for the new listener, or the static export is chosen instead.

**Dependencies:** T-83-003  
**Reference:** plan Phase 8 Sprint 3; `docs/security-and-threat-model.md` §4.4  
**Estimated Complexity:** S

---

### T-83-005 — Generated numbers table and benchmark dataset schema

**Description:** Define a versioned benchmark results schema (seed, compiler target, hardware,
iterations, distribution, corpus hash, measurement scope) and generate the `docs/numbers.md`
tables from it, so published values are never copied by hand.

**Research:** Inventory every table in `docs/numbers.md` and every document that copies a value
from it.

**Acceptance Criteria:**

- [ ] `icx benchmark` writes results in the schema.
- [ ] A generator produces the `docs/numbers.md` tables, and CI fails when the committed tables
  differ from the generated ones.
- [ ] Copied values elsewhere link to the generated row instead of repeating the number.

**Dependencies:** None  
**Reference:** `docs/codex/09-context-budget-and-enhancements.md:27-29`  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] Line coverage is `>= 95%` (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Affected docs and `CHANGELOG.md` `[Unreleased]` are updated.
- [ ] Upstream re-scan for Sprint 4 is done and its tickets are adjusted.
